use crate::model::atomic_write;
use std::io::Cursor;
use std::path::{Path, PathBuf};

pub fn mock_wav() -> Vec<u8> {
    let mut cursor = Cursor::new(Vec::new());
    {
        let mut writer = hound::WavWriter::new(
            &mut cursor,
            hound::WavSpec {
                channels: 1,
                sample_rate: 44100,
                bits_per_sample: 16,
                sample_format: hound::SampleFormat::Int,
            },
        )
        .expect("memory WAV");
        for i in 0..11025 {
            let x = (i as f32 * 440.0 * std::f32::consts::TAU / 44100.0).sin();
            writer
                .write_sample((x * 2000.0) as i16)
                .expect("memory sample");
        }
        writer.finalize().expect("memory finalization");
    }
    cursor.into_inner()
}
pub fn samples(data: &[u8]) -> Result<Vec<i16>, String> {
    let reader = hound::WavReader::new(Cursor::new(data)).map_err(|_| "E_AUDIO: invalid WAV")?;
    let s = reader.spec();
    if s.channels != 1
        || s.sample_rate != 44100
        || s.bits_per_sample != 16
        || s.sample_format != hound::SampleFormat::Int
    {
        return Err("E_AUDIO: expected PCM16 mono 44100 Hz WAV".into());
    }
    let result = reader
        .into_samples::<i16>()
        .collect::<Result<Vec<_>, _>>()
        .map_err(|_| "E_AUDIO: truncated WAV")?;
    if result.is_empty() {
        return Err("E_AUDIO: empty WAV".into());
    }
    Ok(result)
}
pub fn write_mix(path: &Path, segments: &[(String, String, PathBuf)]) -> Result<(), String> {
    let part = path.join("dialogue.wav.part");
    let mut srt = String::new();
    let mut offset = 0u64;
    {
        let mut w = hound::WavWriter::create(
            &part,
            hound::WavSpec {
                channels: 1,
                sample_rate: 44100,
                bits_per_sample: 16,
                sample_format: hound::SampleFormat::Int,
            },
        )
        .map_err(|_| "E_MIX")?;
        for (i, (_, text, file)) in segments.iter().enumerate() {
            let pcm = samples(&std::fs::read(file).map_err(|_| "E_MIX_READ")?)?;
            let end = offset + pcm.len() as u64;
            if end > 2_000_000_000 {
                return Err("E_MIX: exceeds RIFF size limit; segment files retained".into());
            }
            srt.push_str(&format!(
                "{}\n{} --> {}\n{}\n\n",
                i + 1,
                timestamp(offset * 1000 / 44100),
                timestamp(end * 1000 / 44100),
                text.replace("\r", "")
            ));
            for x in pcm {
                w.write_sample(x).map_err(|_| "E_MIX")?;
            }
            offset = end;
        }
        w.finalize().map_err(|_| "E_MIX")?;
    }
    std::fs::rename(part, path.join("dialogue.wav")).map_err(|_| "E_MIX_RENAME")?;
    atomic_write(&path.join("dialogue.srt"), srt.as_bytes())
}
fn timestamp(ms: u64) -> String {
    format!(
        "{:02}:{:02}:{:02},{:03}",
        ms / 3600000,
        (ms / 60000) % 60,
        (ms / 1000) % 60,
        ms % 1000
    )
}
