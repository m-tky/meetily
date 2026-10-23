//! Lossless float32 checkpoint transport; final AAC is encoded only once.
use anyhow::{anyhow, Result};
use std::{
    fs::OpenOptions,
    io::{BufWriter, Write},
    path::Path,
};

pub(crate) fn write_float_checkpoint(path: &Path, samples: &[f32], rate: u32) -> Result<()> {
    if samples.is_empty() || rate == 0 {
        return Err(anyhow!("Empty audio or invalid checkpoint sample rate"));
    }
    let count = u32::try_from(samples.len())?;
    let bytes = count
        .checked_mul(4)
        .ok_or_else(|| anyhow!("Checkpoint exceeds WAV size bound"))?;
    let riff_size = bytes
        .checked_add(50)
        .ok_or_else(|| anyhow!("Checkpoint exceeds RIFF size bound"))?;
    let byte_rate = rate
        .checked_mul(4)
        .ok_or_else(|| anyhow!("Invalid checkpoint byte rate"))?;
    let mut out = BufWriter::new(OpenOptions::new().write(true).create_new(true).open(path)?);
    out.write_all(b"RIFF")?;
    out.write_all(&riff_size.to_le_bytes())?;
    out.write_all(b"WAVEfmt ")?;
    out.write_all(&18u32.to_le_bytes())?; // WAVEFORMATEX, IEEE float, mono
    out.write_all(&3u16.to_le_bytes())?;
    out.write_all(&1u16.to_le_bytes())?;
    out.write_all(&rate.to_le_bytes())?;
    out.write_all(&byte_rate.to_le_bytes())?;
    out.write_all(&4u16.to_le_bytes())?;
    out.write_all(&32u16.to_le_bytes())?;
    out.write_all(&0u16.to_le_bytes())?;
    out.write_all(b"fact")?;
    out.write_all(&4u32.to_le_bytes())?;
    out.write_all(&count.to_le_bytes())?;
    out.write_all(b"data")?;
    out.write_all(&bytes.to_le_bytes())?;
    #[cfg(target_endian = "little")]
    out.write_all(bytemuck::cast_slice(samples))?;
    #[cfg(target_endian = "big")]
    for sample in samples {
        out.write_all(&sample.to_le_bytes())?;
    }
    out.flush()?;
    // Windows FlushFileBuffers requires write access. Sync this writer, not a
    // subsequently reopened read-only handle, before publishing the checkpoint.
    out.get_ref().sync_all()?;
    Ok(())
}

pub(crate) fn checkpoint_index(name: &str) -> Option<usize> {
    let name = name.strip_prefix("audio_chunk_")?;
    name.strip_suffix(".wav")
        .or_else(|| name.strip_suffix(".mp4"))?
        .parse()
        .ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn exact_float_bits_and_no_overwrite() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("audio.wav");
        let samples = [0.0f32, -0.0, 0.12345679, -0.75, f32::from_bits(1)];
        write_float_checkpoint(&path, &samples, 48000).unwrap();
        let bytes = std::fs::read(&path).unwrap();
        assert_eq!(
            &bytes[58..],
            samples
                .iter()
                .flat_map(|s| s.to_le_bytes())
                .collect::<Vec<_>>()
        );
        assert_eq!(
            u32::from_le_bytes(bytes[4..8].try_into().unwrap()) as usize + 8,
            bytes.len()
        );
        assert_eq!(
            u32::from_le_bytes(bytes[46..50].try_into().unwrap()),
            samples.len() as u32
        );
        assert!(write_float_checkpoint(&path, &samples, 48000).is_err());
        assert_eq!(std::fs::read(&path).unwrap(), bytes);
    }
    #[test]
    fn recognizes_both_formats_not_partial_files() {
        assert_eq!(checkpoint_index("audio_chunk_002.wav"), Some(2));
        assert_eq!(checkpoint_index("audio_chunk_002.mp4"), Some(2));
        for name in [
            "audio_chunk_2.wav.tmp",
            "audio_chunk_x.wav",
            ".encoding-2.wav",
            "audio_chunk_2.flac",
        ] {
            assert_eq!(checkpoint_index(name), None);
        }
    }
}
