use crate::{
    audio::AudioData,
    error::{AppError, AppResult},
    platform::{AudioRecorder, Capability, CredentialStore, HotkeyBackend, TextInserter},
};

pub struct UnsupportedHotkey;

impl HotkeyBackend for UnsupportedHotkey {
    fn register(&mut self, _shortcut: &str) -> AppResult<()> {
        Err(AppError::Capability(
            "global shortcuts are unavailable on this desktop environment".to_string(),
        ))
    }

    fn triggered(&mut self) -> bool {
        false
    }

    fn capability(&self) -> Capability {
        Capability::unavailable("global shortcuts are unavailable; configure a desktop shortcut")
    }
}

pub struct UnsupportedAudio;

impl AudioRecorder for UnsupportedAudio {
    fn start(&mut self, _max_seconds: u64) -> AppResult<()> {
        Err(AppError::Capability(
            "microphone capture is unavailable".to_string(),
        ))
    }

    fn stop(&mut self) -> AppResult<AudioData> {
        Err(AppError::Capability(
            "microphone capture is unavailable".to_string(),
        ))
    }

    fn is_recording(&self) -> bool {
        false
    }

    fn capability(&self) -> Capability {
        Capability::unavailable("microphone capture is not implemented for this platform")
    }
}

pub struct ClipboardInserter;

impl TextInserter for ClipboardInserter {
    fn insert(&self, _text: &str) -> AppResult<()> {
        Err(AppError::Capability(
            "automatic text insertion is unavailable; copy the pending transcript".to_string(),
        ))
    }

    fn copy(&self, text: &str) -> AppResult<()> {
        let mut clipboard =
            arboard::Clipboard::new().map_err(|error| AppError::Clipboard(error.to_string()))?;
        clipboard
            .set_text(text.to_string())
            .map_err(|error| AppError::Clipboard(error.to_string()))
    }

    fn capability(&self) -> Capability {
        Capability::unavailable("automatic insertion is unavailable; copy transcripts manually")
    }
}

pub struct UnsupportedCredentials;

impl CredentialStore for UnsupportedCredentials {
    fn capability(&self) -> Capability {
        Capability::unavailable("platform credential storage is unavailable")
    }
}
