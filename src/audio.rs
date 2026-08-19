use crate::error::{AppError, AppResult};

pub const DEFAULT_MAX_RECORDING_SECONDS: u64 = 300;

#[derive(Debug, Clone)]
pub struct AudioData {
    pub samples: Vec<f32>,
    pub sample_rate: u32,
}

impl AudioData {
    pub fn new(samples: Vec<f32>, sample_rate: u32) -> Self {
        Self {
            samples,
            sample_rate,
        }
    }

    pub fn is_empty(&self) -> bool {
        self.samples.is_empty()
    }

    pub fn duration_seconds(&self) -> f64 {
        if self.sample_rate == 0 {
            return 0.0;
        }
        self.samples.len() as f64 / self.sample_rate as f64
    }

    pub fn to_wav(&self) -> AppResult<Vec<u8>> {
        let sample_bytes = self.samples.len().checked_mul(2).ok_or_else(|| {
            AppError::Audio("recording is too large to encode as WAV".to_string())
        })?;
        let riff_size = 36usize.checked_add(sample_bytes).ok_or_else(|| {
            AppError::Audio("recording is too large to encode as WAV".to_string())
        })?;
        let byte_rate = self
            .sample_rate
            .checked_mul(2)
            .ok_or_else(|| AppError::Audio("sample rate is too large".to_string()))?;
        let mut wav = Vec::with_capacity(44 + sample_bytes);
        wav.extend_from_slice(b"RIFF");
        wav.extend_from_slice(&(riff_size as u32).to_le_bytes());
        wav.extend_from_slice(b"WAVEfmt ");
        wav.extend_from_slice(&16u32.to_le_bytes());
        wav.extend_from_slice(&1u16.to_le_bytes());
        wav.extend_from_slice(&1u16.to_le_bytes());
        wav.extend_from_slice(&self.sample_rate.to_le_bytes());
        wav.extend_from_slice(&byte_rate.to_le_bytes());
        wav.extend_from_slice(&2u16.to_le_bytes());
        wav.extend_from_slice(&16u16.to_le_bytes());
        wav.extend_from_slice(b"data");
        wav.extend_from_slice(&(sample_bytes as u32).to_le_bytes());
        for sample in &self.samples {
            let sample = sample.clamp(-1.0, 1.0);
            let value = (sample * i16::MAX as f32) as i16;
            wav.extend_from_slice(&value.to_le_bytes());
        }
        Ok(wav)
    }
}

#[cfg(test)]
mod tests {
    use super::AudioData;

    #[test]
    fn encodes_mono_audio_as_wav() {
        let audio = AudioData::new(vec![0.0, 0.5, -0.5], 16_000);
        let wav = audio.to_wav().expect("audio should encode");

        assert_eq!(&wav[0..4], b"RIFF");
        assert_eq!(audio.duration_seconds(), 3.0 / 16_000.0);
    }
}
