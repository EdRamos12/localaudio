mod capture;
mod protocol;
mod server;
mod tray;
mod ui;

use crate::capture::{default_source_name, list_sources, open_source, SourceInfo};
use crate::protocol::{encode_packet, PCM_SAMPLES, SAMPLE_RATE};
use anyhow::{Context, Result};
use eframe::egui;
use server::Server;
use std::rc::Rc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

#[derive(Clone, Debug, PartialEq)]
pub enum EngineState {
    Stopped,
    Starting,
    Capturing,
    Error(String),
}

pub struct Engine {
    pub server: Arc<Server>,
    pub running: AtomicBool,
    stop: AtomicBool,
    pub source: Mutex<String>,
    pub passphrase: Arc<Mutex<String>>,
    pub packets: AtomicU64,
    pub rms: AtomicU64,
    state: Mutex<EngineState>,
    sources: Mutex<Vec<SourceInfo>>,
    pub phone_only: AtomicBool,
    previous_sink: Mutex<Option<String>>,
}

impl Engine {
    fn start(name: String, source: String, passphrase: String) -> Result<Arc<Self>> {
        let passphrase = Arc::new(Mutex::new(passphrase));
        let lan = server::lan_ip()?;
        let server = Server::start(name, Arc::clone(&passphrase), lan)?;
        let engine = Arc::new(Self {
            server,
            running: AtomicBool::new(false),
            stop: AtomicBool::new(false),
            source: Mutex::new(source),
            passphrase,
            packets: AtomicU64::new(0),
            rms: AtomicU64::new(0),
            state: Mutex::new(EngineState::Stopped),
            sources: Mutex::new(list_sources()),
            phone_only: AtomicBool::new(false),
            previous_sink: Mutex::new(None),
        });
        let worker = Arc::clone(&engine);
        thread::Builder::new()
            .name("localaudio-capture".into())
            .spawn(move || worker.capture_loop())
            .context("spawn capture")?;
        Ok(engine)
    }

    fn capture_loop(self: &Arc<Self>) {
        let mut encoder = match opus::Encoder::new(
            SAMPLE_RATE,
            opus::Channels::Stereo,
            opus::Application::LowDelay,
        ) {
            Ok(encoder) => encoder,
            Err(err) => {
                self.set_state(EngineState::Error(format!("Opus encoder failed: {err}")));
                return;
            }
        };
        if let Err(err) = encoder.set_bitrate(opus::Bitrate::Bits(128_000)) {
            self.set_state(EngineState::Error(format!("Opus bitrate failed: {err}")));
            return;
        }
        let mut packet = [0u8; 4000];
        let mut framed = Vec::with_capacity(4000);
        let mut pcm = [0i16; PCM_SAMPLES];
        let mut seq = 0u32;
        let started = Instant::now();
        let mut current_name = String::new();
        let mut source: Option<Box<dyn capture::PcmSource>> = None;

        while !self.stop.load(Ordering::Relaxed) {
            if !self.running.load(Ordering::Relaxed) {
                source = None;
                current_name.clear();
                self.rms.store(0, Ordering::Relaxed);
                self.set_state(EngineState::Stopped);
                thread::sleep(Duration::from_millis(50));
                continue;
            }
            let wanted = self.source.lock().unwrap_or_else(|err| err.into_inner()).clone();
            if source.is_none() || wanted != current_name {
                self.set_state(EngineState::Starting);
                match open_source(&wanted) {
                    Ok(opened) => {
                        source = Some(opened);
                        current_name = wanted;
                        self.set_state(EngineState::Capturing);
                    }
                    Err(err) => {
                        self.set_state(EngineState::Error(err.to_string()));
                        thread::sleep(Duration::from_millis(400));
                        continue;
                    }
                }
            }
            let read = source.as_mut().map(|active| active.read_frame(&mut pcm));
            if let Some(Err(err)) = read {
                self.set_state(EngineState::Error(err.to_string()));
                source = None;
                thread::sleep(Duration::from_millis(200));
                continue;
            }
            let energy: f64 = pcm
                .iter()
                .map(|sample| {
                    let value = *sample as f64;
                    value * value
                })
                .sum();
            let rms = (energy / pcm.len() as f64).sqrt();
            self.rms.store(rms as u64, Ordering::Relaxed);
            match encoder.encode(&pcm, &mut packet) {
                Ok(len) => {
                    let timestamp = started.elapsed().as_millis() as u32;
                    encode_packet(seq, timestamp, &packet[..len], &mut framed);
                    seq = seq.wrapping_add(1);
                    self.server.send_audio(&framed);
                    self.packets.fetch_add(1, Ordering::Relaxed);
                }
                Err(err) => self.set_state(EngineState::Error(format!("Opus encode failed: {err}"))),
            }
        }
    }

    fn set_state(&self, state: EngineState) {
        let mut current = self.state.lock().unwrap_or_else(|err| err.into_inner());
        if *current != state {
            *current = state;
        }
    }

    pub fn state(&self) -> EngineState {
        self.state.lock().unwrap_or_else(|err| err.into_inner()).clone()
    }

    pub fn sources(&self) -> Vec<SourceInfo> {
        self.sources.lock().unwrap_or_else(|err| err.into_inner()).clone()
    }

    pub fn refresh_sources(&self) {
        *self.sources.lock().unwrap_or_else(|err| err.into_inner()) = list_sources();
    }

    #[cfg_attr(not(target_os = "linux"), allow(dead_code))]
    pub fn toggle_phone_only(&self) {
        #[cfg(target_os = "linux")]
        {
            let enable = !self.phone_only.load(Ordering::Relaxed);
            let previous = self
                .previous_sink
                .lock()
                .unwrap_or_else(|err| err.into_inner())
                .clone();
            if enable {
                if let Ok(sink) = capture::default_sink_name() {
                    *self.previous_sink.lock().unwrap_or_else(|err| err.into_inner()) = Some(sink);
                }
            }
            match capture::set_phone_only(enable, previous.as_deref()) {
                Ok(monitor) => {
                    self.refresh_sources();
                    *self.source.lock().unwrap_or_else(|err| err.into_inner()) = monitor;
                    self.phone_only.store(enable, Ordering::Relaxed);
                    if !enable {
                        *self.previous_sink.lock().unwrap_or_else(|err| err.into_inner()) = None;
                    }
                }
                Err(err) => self.set_state(EngineState::Error(err.to_string())),
            }
        }
    }

    pub fn shutdown(&self) {
        if self.phone_only.load(Ordering::Relaxed) {
            self.toggle_phone_only();
        }
        self.stop.store(true, Ordering::Relaxed);
    }
}

struct Args {
    headless: bool,
    source: Option<String>,
    passphrase: String,
    name: String,
}

fn parse_args() -> Args {
    let mut args = Args {
        headless: false,
        source: None,
        passphrase: String::new(),
        name: hostname(),
    };
    let mut argv = std::env::args().skip(1);
    while let Some(arg) = argv.next() {
        match arg.as_str() {
            "--headless" => args.headless = true,
            "--source" => args.source = argv.next(),
            "--passphrase" => args.passphrase = argv.next().unwrap_or_default(),
            "--name" => args.name = argv.next().unwrap_or_else(hostname),
            "--help" | "-h" => {
                println!(
                    "localaudio [--headless] [--source tone|MONITOR] [--passphrase TEXT] [--name TEXT]"
                );
                std::process::exit(0);
            }
            other => eprintln!("ignored argument {other}"),
        }
    }
    args
}

fn hostname() -> String {
    std::env::var("HOSTNAME")
        .ok()
        .filter(|name| !name.is_empty())
        .or_else(|| {
            std::process::Command::new("hostname")
                .output()
                .ok()
                .map(|output| String::from_utf8_lossy(&output.stdout).trim().to_string())
        })
        .filter(|name| !name.is_empty())
        .unwrap_or_else(|| "LocalAudio".to_string())
}

// winit on Wayland only destroys a closed window when its event loop runs again, so the
// window would linger frozen on screen. Run the loop once with an app that bails before painting.
fn flush_closed_window() {
    let _ = eframe::run_native(
        "LocalAudio",
        eframe::NativeOptions {
            viewport: egui::ViewportBuilder::default().with_visible(false),
            ..Default::default()
        },
        Box::new(|_| Err("closed to tray".into())),
    );
}

fn main() -> Result<()> {
    let args = parse_args();
    let source = args.source.clone().unwrap_or_else(default_source_name);
    let engine = match Engine::start(args.name, source, args.passphrase) {
        Ok(engine) => engine,
        Err(err) if !args.headless => return ui::show_startup_error(format!("{err:#}")),
        Err(err) => return Err(err),
    };
    engine.running.store(true, Ordering::Relaxed);
    if args.headless {
        let stop = Arc::clone(&engine);
        ctrlc::set_handler(move || stop.stop.store(true, Ordering::Relaxed))
            .context("ctrl-c handler")?;
        while !engine.stop.load(Ordering::Relaxed) {
            eprintln!(
                "status={:?} listeners={} packets={} rms={}",
                engine.state(),
                engine.server.listener_count(),
                engine.packets.load(Ordering::Relaxed),
                engine.rms.load(Ordering::Relaxed),
            );
            thread::sleep(Duration::from_secs(1));
        }
        return Ok(());
    }

    let tray = tray::Tray::new()
        .map(Rc::new)
        .inspect_err(|err| eprintln!("tray icon unavailable, closing the window will quit: {err:#}"))
        .ok();
    // run_native returns when the window closes; with a tray we keep streaming and reopen on demand.
    let result = loop {
        let ui_engine = Arc::clone(&engine);
        let ui_tray = tray.clone();
        let result = eframe::run_native(
            "LocalAudio",
            eframe::NativeOptions {
                viewport: egui::ViewportBuilder::default()
                    .with_title("LocalAudio")
                    .with_app_id("localaudio")
                    .with_inner_size([440.0, 800.0])
                    .with_min_inner_size([380.0, 520.0])
                    .with_icon(ui::app_icon()),
                ..Default::default()
            },
            Box::new(move |cc| Ok(Box::new(ui::DesktopUi::new(cc, ui_engine, ui_tray)))),
        );
        let Some(tray) = tray.as_ref().filter(|_| result.is_ok()) else {
            break result;
        };
        flush_closed_window();
        if !tray.wait_for_open() {
            break result;
        }
    };
    engine.shutdown();
    result.map_err(|err| anyhow::anyhow!(err.to_string()))
}
