use std::{fs, path::PathBuf};

use serde::{Deserialize, Serialize};

use crate::{
    audio::DEFAULT_MAX_RECORDING_SECONDS,
    error::{AppError, AppResult},
};

const SERVICE_NAME: &str = "talk-to-me-goose";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ProviderKind {
    OpenAi,
    Gemini,
}

impl Default for ProviderKind {
    fn default() -> Self {
        Self::OpenAi
    }
}

impl ProviderKind {
    pub fn label(self) -> &'static str {
        match self {
            Self::OpenAi => "OpenAI",
            Self::Gemini => "Gemini",
        }
    }

    pub fn keyring_user(self) -> &'static str {
        match self {
            Self::OpenAi => "openai",
            Self::Gemini => "gemini",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct AppConfig {
    pub provider: ProviderKind,
    pub model: String,
    pub shortcut: String,
    pub max_recording_seconds: u64,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            provider: ProviderKind::OpenAi,
            model: "gpt-4o-mini-transcribe".to_string(),
            shortcut: "Command+Shift+Space".to_string(),
            max_recording_seconds: DEFAULT_MAX_RECORDING_SECONDS,
        }
    }
}

impl AppConfig {
    pub fn validate(&self) -> AppResult<()> {
        if self.model.trim().is_empty() {
            return Err(AppError::Config("model must not be empty".to_string()));
        }
        if self.shortcut.trim().is_empty() {
            return Err(AppError::Config("shortcut must not be empty".to_string()));
        }
        if !(1..=3600).contains(&self.max_recording_seconds) {
            return Err(AppError::Config(
                "max_recording_seconds must be between 1 and 3600".to_string(),
            ));
        }
        Ok(())
    }

    pub fn path() -> AppResult<PathBuf> {
        let base = dirs::config_dir()
            .ok_or_else(|| AppError::Config("could not determine config directory".to_string()))?;
        Ok(base.join("talk-to-me-goose").join("config.toml"))
    }

    pub fn load() -> AppResult<Self> {
        let path = Self::path()?;
        if !path.exists() {
            return Ok(Self::default());
        }
        let content = fs::read_to_string(path)?;
        let config: Self = toml::from_str(&content)
            .map_err(|error| AppError::Config(format!("could not parse config: {error}")))?;
        config.validate()?;
        Ok(config)
    }

    pub fn save(&self) -> AppResult<()> {
        self.validate()?;
        let path = Self::path()?;
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let content = toml::to_string_pretty(self)
            .map_err(|error| AppError::Config(format!("could not serialize config: {error}")))?;
        fs::write(path, content)?;
        Ok(())
    }

    pub fn api_key(&self) -> AppResult<Option<String>> {
        let entry = keyring::Entry::new(SERVICE_NAME, self.provider.keyring_user())
            .map_err(|error| AppError::Credentials(error.to_string()))?;
        match entry.get_password() {
            Ok(value) if !value.trim().is_empty() => Ok(Some(value)),
            Ok(_) => Ok(None),
            Err(_) => Ok(None),
        }
    }

    pub fn set_api_key(&self, value: &str) -> AppResult<()> {
        if value.trim().is_empty() {
            return Err(AppError::Credentials(
                "API key must not be empty".to_string(),
            ));
        }
        let entry = keyring::Entry::new(SERVICE_NAME, self.provider.keyring_user())
            .map_err(|error| AppError::Credentials(error.to_string()))?;
        entry
            .set_password(value.trim())
            .map_err(|error| AppError::Credentials(error.to_string()))?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::{AppConfig, ProviderKind};

    #[test]
    fn default_config_is_valid() {
        let config = AppConfig::default();
        assert_eq!(config.provider, ProviderKind::OpenAi);
        assert!(config.validate().is_ok());
    }

    #[test]
    fn rejects_invalid_recording_limit() {
        let mut config = AppConfig::default();
        config.max_recording_seconds = 0;
        assert!(config.validate().is_err());
    }
}
