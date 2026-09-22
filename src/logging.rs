use std::{
    env,
    fs::{self, File, OpenOptions},
    io::{self, LineWriter, Write},
    path::PathBuf,
    sync::{Arc, Mutex},
};

use tracing_subscriber::{EnvFilter, fmt::MakeWriter};

use talk_to_me_goose::error::AppError;

#[derive(Debug, Clone)]
pub struct LoggingState {
    pub path: Option<PathBuf>,
    pub fallback: bool,
    pub error: Option<String>,
}

#[derive(Clone)]
struct LineWriterFactory(Arc<Mutex<LineWriter<File>>>);

impl LineWriterFactory {
    fn new(file: File) -> Self {
        Self(Arc::new(Mutex::new(LineWriter::new(file))))
    }
}

impl Write for LineWriterFactory {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        self.0
            .lock()
            .map_err(|_| io::Error::other("log writer lock poisoned"))?
            .write(bytes)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.0
            .lock()
            .map_err(|_| io::Error::other("log writer lock poisoned"))?
            .flush()
    }
}

impl<'a> MakeWriter<'a> for LineWriterFactory {
    type Writer = Self;

    fn make_writer(&'a self) -> Self::Writer {
        self.clone()
    }
}

pub fn log_path() -> Option<PathBuf> {
    dirs::data_local_dir().map(|directory| directory.join("talk-to-me-goose").join("app.log"))
}

pub fn init() -> LoggingState {
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));
    let path = log_path();
    let mut file_error = None;

    if let Some(path) = &path {
        if let Some(directory) = path.parent()
            && let Err(error) = fs::create_dir_all(directory)
        {
            file_error = Some(format!("could not create log directory: {error}"));
        }

        if file_error.is_none() {
            match OpenOptions::new().create(true).append(true).open(path) {
                Ok(file) => {
                    let writer = LineWriterFactory::new(file);
                    match tracing_subscriber::fmt()
                        .with_env_filter(filter.clone())
                        .with_ansi(false)
                        .with_target(false)
                        .with_writer(writer)
                        .compact()
                        .try_init()
                    {
                        Ok(()) => {
                            return LoggingState {
                                path: Some(path.clone()),
                                fallback: false,
                                error: None,
                            };
                        }
                        Err(error) => file_error = Some(error.to_string()),
                    }
                }
                Err(error) => file_error = Some(format!("could not open log file: {error}")),
            }
        }
    } else {
        file_error = Some("could not determine the platform log directory".to_string());
    }

    let fallback_error = tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_ansi(false)
        .with_target(false)
        .with_writer(io::stderr)
        .compact()
        .try_init()
        .err()
        .map(|error| error.to_string());
    let state = fallback_state(path, file_error.or(fallback_error));
    if let Some(error) = &state.error {
        eprintln!("Talk To Me Goose logging fallback: {error}");
    }
    state
}

pub fn safe_error(error: &AppError) -> String {
    match error {
        AppError::Provider(_) => "provider error".to_string(),
        AppError::Http(error) => {
            let category = if error.is_timeout() { "timeout" }
                else if error.is_connect() { "connection" }
                else if error.is_body() { "response_body" }
                else if error.is_decode() { "decode" }
                else { "request" };
            format!("HTTP request error: {category}")
        }
        AppError::Credentials(_) => "credential store error".to_string(),
        _ => sanitize(&error.to_string()),
    }
}

pub fn sanitize(value: &str) -> String {
    let mut value = value.replace(['\n', '\r'], " ");
    for marker in ["Bearer ", "api_key=", "api_key:", "\"api_key\":", "key="] {
        value = redact_after_marker(&value, marker);
    }
    value.chars().take(300).collect()
}

fn redact_after_marker(value: &str, marker: &str) -> String {
    let mut output = String::with_capacity(value.len());
    let mut remaining = value;
    while let Some(index) = remaining.find(marker) {
        let (before, after_marker) = remaining.split_at(index + marker.len());
        output.push_str(&remaining[..index]);
        output.push_str(before.split_at(index).1);
        let end = after_marker
            .find(|character: char| {
                character.is_whitespace() || matches!(character, ',' | '}' | '"')
            })
            .unwrap_or(after_marker.len());
        output.push_str("[REDACTED]");
        remaining = &after_marker[end..];
    }
    output.push_str(remaining);
    output
}

pub fn filter_description() -> String {
    filter_description_from(env::var("RUST_LOG").ok().as_deref())
}

fn filter_description_from(value: Option<&str>) -> String {
    value
        .filter(|value| !value.trim().is_empty())
        .unwrap_or("info")
        .to_string()
}

fn fallback_state(path: Option<PathBuf>, error: Option<String>) -> LoggingState {
    LoggingState {
        path,
        fallback: true,
        error,
    }
}

#[cfg(test)]
mod tests {
    use talk_to_me_goose::error::AppError;

    use super::{fallback_state, filter_description_from, log_path, safe_error, sanitize};

    #[test]
    fn resolves_a_stable_application_log_path() {
        let path = log_path().expect("platform data directory should exist");
        assert_eq!(
            path.file_name().and_then(|name| name.to_str()),
            Some("app.log")
        );
        assert_eq!(
            path.parent()
                .and_then(|path| path.file_name().and_then(|name| name.to_str())),
            Some("talk-to-me-goose")
        );
    }

    #[test]
    fn defaults_to_info_filter() {
        assert_eq!(filter_description_from(None), "info");
        assert_eq!(filter_description_from(Some("debug")), "debug");
    }

    #[test]
    fn fallback_state_preserves_the_log_failure() {
        let state = fallback_state(None, Some("permission denied".to_string()));
        assert!(state.fallback);
        assert_eq!(state.error.as_deref(), Some("permission denied"));
    }

    #[test]
    fn redacts_known_secret_markers() {
        let safe = sanitize("request failed api_key=secret-value Bearer token-value");
        assert!(!safe.contains("secret-value"));
        assert!(!safe.contains("token-value"));
        assert!(safe.contains("[REDACTED]"));
    }

    #[test]
    fn preserves_structured_provider_diagnostics() {
        let error = AppError::ProviderDiagnostic {
            provider: "OpenAI",
            status: 429,
            category: "request_rejected",
            code: "insufficient_quota",
        };
        let safe = safe_error(&error);
        assert!(safe.contains("429"));
        assert!(safe.contains("insufficient_quota"));
    }

    #[test]
    fn does_not_log_provider_error_payloads() {
        let safe = safe_error(&AppError::Provider(
            "provider response contained transcript or secret payload".to_string(),
        ));
        assert_eq!(safe, "provider error");
    }
}
