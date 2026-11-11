pub const SAMPLE_RATE: u32 = 48_000;
pub const CHANNELS: usize = 2;
pub const FRAME_SAMPLES: usize = 960;
pub const PCM_SAMPLES: usize = FRAME_SAMPLES * CHANNELS;
pub const TCP_PORT: u16 = 47321;
pub const UDP_PORT: u16 = 47322;
pub const MAGIC: &[u8; 4] = b"LAU1";

pub fn encode_packet(seq: u32, timestamp_ms: u32, opus_payload: &[u8], out: &mut Vec<u8>) {
    out.clear();
    out.extend_from_slice(MAGIC);
    out.extend_from_slice(&seq.to_le_bytes());
    out.extend_from_slice(&timestamp_ms.to_le_bytes());
    out.extend_from_slice(&(opus_payload.len() as u16).to_le_bytes());
    out.extend_from_slice(opus_payload);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn opus_roundtrip_keeps_a_tone() {
        let mut pcm = [0i16; PCM_SAMPLES];
        let mut phase = 0.0f32;
        let step = 440.0 * std::f32::consts::TAU / SAMPLE_RATE as f32;
        for frame in 0..FRAME_SAMPLES {
            let sample = (phase.sin() * 0.2 * i16::MAX as f32) as i16;
            pcm[frame * 2] = sample;
            pcm[frame * 2 + 1] = sample;
            phase += step;
        }

        let mut encoder =
            opus::Encoder::new(SAMPLE_RATE, opus::Channels::Stereo, opus::Application::LowDelay)
                .unwrap();
        encoder.set_bitrate(opus::Bitrate::Bits(128_000)).unwrap();
        let mut packet = [0u8; 4000];
        let len = encoder.encode(&pcm, &mut packet).unwrap();

        let mut framed = Vec::new();
        encode_packet(7, 20, &packet[..len], &mut framed);
        assert_eq!(&framed[0..4], b"LAU1");
        assert_eq!(u32::from_le_bytes(framed[4..8].try_into().unwrap()), 7);

        let mut decoder = opus::Decoder::new(SAMPLE_RATE, opus::Channels::Stereo).unwrap();
        let mut decoded = [0i16; PCM_SAMPLES];
        let n = decoder.decode(&packet[..len], &mut decoded, false).unwrap();
        assert_eq!(n, FRAME_SAMPLES);
        let energy: f64 = decoded.iter().map(|s| (*s as f64) * (*s as f64)).sum();
        let rms = (energy / decoded.len() as f64).sqrt();
        assert!(rms > 1000.0, "rms {rms} was too low for a decoded tone");
    }
}
