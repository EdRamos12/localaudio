use crate::protocol::SAMPLE_RATE;
use anyhow::{anyhow, Context, Result};
use std::process::Command;

pub trait PcmSource: Send {
    fn read_frame(&mut self, pcm: &mut [i16]) -> Result<()>;
}

pub struct ToneSource {
    phase: f32,
}

impl ToneSource {
    pub fn new() -> Self {
        Self { phase: 0.0 }
    }
}

impl PcmSource for ToneSource {
    fn read_frame(&mut self, pcm: &mut [i16]) -> Result<()> {
        let step = 440.0 * std::f32::consts::TAU / SAMPLE_RATE as f32;
        for frame in 0..(pcm.len() / 2) {
            let sample = (self.phase.sin() * 0.2 * i16::MAX as f32) as i16;
            pcm[frame * 2] = sample;
            pcm[frame * 2 + 1] = sample;
            self.phase += step;
            if self.phase > std::f32::consts::TAU {
                self.phase -= std::f32::consts::TAU;
            }
        }
        let frame = std::time::Duration::from_millis(20);
        std::thread::sleep(frame);
        Ok(())
    }
}

pub fn open_source(name: &str) -> Result<Box<dyn PcmSource>> {
    if name == "tone" {
        return Ok(Box::new(ToneSource::new()));
    }
    #[cfg(target_os = "linux")]
    {
        return Ok(Box::new(linux::PulseMonitor::open(name)?));
    }
    #[cfg(target_os = "windows")]
    {
        let _ = name;
        return Ok(Box::new(windows::WasapiLoopback::open()?));
    }
    #[cfg(not(any(target_os = "linux", target_os = "windows")))]
    {
        let _ = name;
        Err(anyhow!("audio capture is implemented for Linux and Windows"))
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct SourceInfo {
    pub id: String,
    pub label: String,
}

pub fn list_sources() -> Vec<SourceInfo> {
    let mut sources = Vec::new();
    #[cfg(target_os = "linux")]
    sources.extend(linux::list_monitors());
    #[cfg(target_os = "windows")]
    sources.push(SourceInfo {
        id: "default".to_string(),
        label: "Default speakers".to_string(),
    });
    sources.push(SourceInfo {
        id: "tone".to_string(),
        label: "Test tone (440 Hz)".to_string(),
    });
    sources
}

pub fn default_source_name() -> String {
    #[cfg(target_os = "linux")]
    {
        return linux::default_monitor().unwrap_or_else(|_| "tone".to_string());
    }
    #[cfg(target_os = "windows")]
    {
        return "default".to_string();
    }
    #[cfg(not(any(target_os = "linux", target_os = "windows")))]
    "tone".to_string()
}

#[cfg(target_os = "linux")]
pub fn set_phone_only(enable: bool, previous_sink: Option<&str>) -> Result<String> {
    linux::set_phone_only(enable, previous_sink)
}

#[cfg(target_os = "linux")]
pub fn default_sink_name() -> Result<String> {
    linux::default_sink()
}

#[cfg(target_os = "linux")]
mod linux {
    use super::*;
    use crate::protocol::SAMPLE_RATE;
    use libpulse_binding::def::BufferAttr;
    use libpulse_binding::sample::{Format, Spec};
    use libpulse_binding::stream::Direction;
    use libpulse_simple_binding::Simple;

    pub struct PulseMonitor {
        simple: Simple,
    }

    impl PulseMonitor {
        pub fn open(source: &str) -> Result<Self> {
            let spec = Spec {
                format: Format::S16NE,
                channels: 2,
                rate: SAMPLE_RATE,
            };
            if !spec.is_valid() {
                return Err(anyhow!("invalid pulse spec"));
            }
            let frame_bytes = (crate::protocol::PCM_SAMPLES * 2) as u32;
            let attr = BufferAttr {
                maxlength: u32::MAX,
                tlength: u32::MAX,
                prebuf: u32::MAX,
                minreq: u32::MAX,
                fragsize: frame_bytes,
            };
            let simple = Simple::new(
                None,
                "LocalAudio",
                Direction::Record,
                Some(source),
                "capture",
                &spec,
                None,
                Some(&attr),
            )
            .map_err(|err| anyhow!("pulse open {source}: {err}"))?;
            Ok(Self { simple })
        }
    }

    impl PcmSource for PulseMonitor {
        fn read_frame(&mut self, pcm: &mut [i16]) -> Result<()> {
            let mut bytes = vec![0u8; pcm.len() * 2];
            self.simple
                .read(&mut bytes)
                .map_err(|err| anyhow!("pulse read: {err}"))?;
            for (index, sample) in pcm.iter_mut().enumerate() {
                *sample = i16::from_le_bytes([bytes[index * 2], bytes[index * 2 + 1]]);
            }
            Ok(())
        }
    }

    pub fn default_sink() -> Result<String> {
        let output = Command::new("pactl")
            .arg("get-default-sink")
            .output()
            .context("pactl get-default-sink")?;
        if !output.status.success() {
            return Err(anyhow!("pactl get-default-sink failed"));
        }
        let name = String::from_utf8(output.stdout).context("sink name")?;
        Ok(name.trim().to_string())
    }

    pub fn default_monitor() -> Result<String> {
        Ok(format!("{}.monitor", default_sink()?))
    }

    pub fn list_monitors() -> Vec<SourceInfo> {
        let Ok(output) = Command::new("pactl")
            .env("LC_ALL", "C")
            .args(["list", "sources"])
            .output()
        else {
            return Vec::new();
        };
        if !output.status.success() {
            return Vec::new();
        }
        let mut sources = Vec::new();
        let mut name: Option<String> = None;
        for line in String::from_utf8_lossy(&output.stdout).lines() {
            let line = line.trim();
            if let Some(value) = line.strip_prefix("Name: ") {
                name = Some(value.to_string());
            } else if let Some(value) = line.strip_prefix("Description: ") {
                let Some(id) = name.take() else { continue };
                if !id.ends_with(".monitor") {
                    continue;
                }
                let label = value.strip_prefix("Monitor of ").unwrap_or(value).to_string();
                sources.push(SourceInfo { id, label });
            }
        }
        sources
    }

    pub fn set_phone_only(enable: bool, previous_sink: Option<&str>) -> Result<String> {
        if enable {
            if !list_sinks()?.iter().any(|sink| sink == "localaudio-speakers") {
                let status = Command::new("pactl")
                    .args([
                        "load-module",
                        "module-null-sink",
                        "sink_name=localaudio-speakers",
                        "sink_properties=device.description=LocalAudio-Speakers",
                    ])
                    .status()
                    .context("load null sink")?;
                if !status.success() {
                    return Err(anyhow!("could not create LocalAudio-Speakers"));
                }
            }
            let status = Command::new("pactl")
                .args(["set-default-sink", "localaudio-speakers"])
                .status()
                .context("set default sink")?;
            if !status.success() {
                return Err(anyhow!("could not switch the default sink"));
            }
            return Ok("localaudio-speakers.monitor".to_string());
        }
        let sink = previous_sink.unwrap_or("");
        if !sink.is_empty() {
            let status = Command::new("pactl")
                .args(["set-default-sink", sink])
                .status()
                .context("restore default sink")?;
            if !status.success() {
                return Err(anyhow!("could not restore the previous sink"));
            }
            return Ok(format!("{sink}.monitor"));
        }
        default_monitor()
    }

    fn list_sinks() -> Result<Vec<String>> {
        let output = Command::new("pactl")
            .args(["list", "short", "sinks"])
            .output()
            .context("pactl list sinks")?;
        if !output.status.success() {
            return Err(anyhow!("pactl list sinks failed"));
        }
        Ok(String::from_utf8_lossy(&output.stdout)
            .lines()
            .filter_map(|line| line.split('\t').nth(1).map(|name| name.to_string()))
            .collect())
    }

}

#[cfg(target_os = "windows")]
mod windows {
    use super::*;
    use crate::protocol::PCM_SAMPLES;
    use std::collections::VecDeque;
    use std::thread;
    use std::time::Duration;
    use wasapi::*;

    pub struct WasapiLoopback {
        client: AudioClient,
        capture: AudioCaptureClient,
        queue: VecDeque<u8>,
    }

    impl WasapiLoopback {
        pub fn open() -> Result<Self> {
            initialize_mta().map_err(|err| anyhow!("COM init: {err}"))?;
            let enumerator = DeviceEnumerator::new().map_err(|err| anyhow!("{err}"))?;
            let device = enumerator
                .get_default_device(&Direction::Render)
                .map_err(|err| anyhow!("default render device: {err}"))?;
            let mut client = device
                .get_iaudioclient()
                .map_err(|err| anyhow!("audio client: {err}"))?;
            let format = WaveFormat::new(16, 16, &SampleType::Int, SAMPLE_RATE, 2, None);
            let (_default_period, min_period) = client
                .get_device_period()
                .map_err(|err| anyhow!("device period: {err}"))?;
            let mode = StreamMode::PollingShared {
                autoconvert: true,
                buffer_duration_hns: min_period.max(200_000),
            };
            client
                .initialize_client(&format, &Direction::Capture, &mode)
                .map_err(|err| anyhow!("loopback init: {err}"))?;
            let capture = client
                .get_audiocaptureclient()
                .map_err(|err| anyhow!("capture client: {err}"))?;
            client
                .start_stream()
                .map_err(|err| anyhow!("start loopback: {err}"))?;
            Ok(Self {
                client,
                capture,
                queue: VecDeque::with_capacity(PCM_SAMPLES * 8),
            })
        }
    }

    impl PcmSource for WasapiLoopback {
        fn read_frame(&mut self, pcm: &mut [i16]) -> Result<()> {
            let need = pcm.len() * 2;
            while self.queue.len() < need {
                self.capture
                    .read_from_device_to_deque(&mut self.queue)
                    .map_err(|err| anyhow!("wasapi read: {err}"))?;
                if self.queue.len() < need {
                    thread::sleep(Duration::from_millis(2));
                }
            }
            for sample in pcm.iter_mut() {
                let lo = self.queue.pop_front().unwrap();
                let hi = self.queue.pop_front().unwrap();
                *sample = i16::from_le_bytes([lo, hi]);
            }
            Ok(())
        }
    }

    impl Drop for WasapiLoopback {
        fn drop(&mut self) {
            let _ = self.client.stop_stream();
        }
    }
}
