use crate::{audio::AudioData, error::AppResult};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CapabilityState {
    Available,
    Unavailable,
}

#[derive(Debug, Clone)]
pub struct Capability {
    pub state: CapabilityState,
    pub detail: String,
}

impl Capability {
    pub fn available(detail: impl Into<String>) -> Self {
        Self {
            state: CapabilityState::Available,
            detail: detail.into(),
        }
    }

    pub fn unavailable(detail: impl Into<String>) -> Self {
        Self {
            state: CapabilityState::Unavailable,
            detail: detail.into(),
        }
    }

    pub fn is_available(&self) -> bool {
        self.state == CapabilityState::Available
    }
}

pub trait HotkeyBackend {
    fn register(&mut self, shortcut: &str) -> AppResult<()>;
    fn triggered(&mut self) -> bool;
    fn capability(&self) -> Capability;
}

pub trait AudioRecorder {
    fn start(&mut self, max_seconds: u64) -> AppResult<()>;
    fn stop(&mut self) -> AppResult<AudioData>;
    fn is_recording(&self) -> bool;
    fn capability(&self) -> Capability;
}

pub trait TextInserter {
    fn insert(&self, text: &str) -> AppResult<()>;
    fn copy(&self, text: &str) -> AppResult<()>;
    fn capability(&self) -> Capability;
}

pub trait CredentialStore {
    fn capability(&self) -> Capability;
}

#[cfg(target_os = "macos")]
pub mod macos;

#[cfg(not(target_os = "macos"))]
pub mod unsupported;
