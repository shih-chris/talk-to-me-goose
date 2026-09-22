use thiserror::Error;

#[derive(Debug, Error)]
pub enum AppError {
    #[error("configuration error: {0}")]
    Config(String),
    #[error("audio error: {0}")]
    Audio(String),
    #[error("provider error: {0}")]
    Provider(String),
    #[error("{provider} transcription failed: {category} (HTTP {status}, code {code})")]
    ProviderDiagnostic {
        provider: &'static str,
        status: u16,
        category: &'static str,
        code: &'static str,
    },
    #[error("platform capability unavailable: {0}")]
    Capability(String),
    #[error("clipboard error: {0}")]
    Clipboard(String),
    #[error("credential store error: {0}")]
    Credentials(String),
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("HTTP error: {0}")]
    Http(#[from] reqwest::Error),
}

pub type AppResult<T> = Result<T, AppError>;
