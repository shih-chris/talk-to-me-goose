use async_trait::async_trait;
use base64::{Engine as _, engine::general_purpose::STANDARD};
use reqwest::{Client, StatusCode, multipart};
use serde::Deserialize;

use crate::{
    audio::AudioData,
    config::{AppConfig, ProviderKind},
    error::{AppError, AppResult},
};

#[async_trait]
pub trait Transcriber: Send + Sync {
    async fn transcribe(&self, audio: &AudioData, model: &str, api_key: &str) -> AppResult<String>;
}

pub struct OpenAiTranscriber {
    client: Client,
    endpoint: String,
}

impl OpenAiTranscriber {
    pub fn new(client: Client) -> Self {
        Self {
            client,
            endpoint: "https://api.openai.com/v1/audio/transcriptions".to_string(),
        }
    }
}

#[derive(Debug, Deserialize)]
struct OpenAiResponse {
    text: String,
}

#[async_trait]
impl Transcriber for OpenAiTranscriber {
    async fn transcribe(&self, audio: &AudioData, model: &str, api_key: &str) -> AppResult<String> {
        let wav = audio.to_wav()?;
        let file = multipart::Part::bytes(wav)
            .file_name("dictation.wav")
            .mime_str("audio/wav")
            .map_err(|error| AppError::Provider(format!("invalid audio MIME type: {error}")))?;
        let form = multipart::Form::new()
            .part("file", file)
            .text("model", model.to_string())
            .text("response_format", "json");
        let response = self
            .client
            .post(&self.endpoint)
            .bearer_auth(api_key)
            .multipart(form)
            .send()
            .await?;
        let status = response.status();
        if !status.is_success() {
            return Err(provider_response_error("OpenAI", status, response).await);
        }
        parse_openai_response(&response.text().await.map_err(AppError::Http)?)
    }
}

pub struct GeminiTranscriber {
    client: Client,
    endpoint: String,
}

impl GeminiTranscriber {
    pub fn new(client: Client, model: &str) -> Self {
        Self {
            client,
            endpoint: format!(
                "https://generativelanguage.googleapis.com/v1beta/models/{model}:generateContent"
            ),
        }
    }
}

#[derive(Debug, serde::Serialize)]
struct GeminiRequest {
    contents: Vec<GeminiContent>,
}

#[derive(Debug, serde::Serialize)]
struct GeminiContent {
    parts: Vec<GeminiPart>,
}

#[derive(Debug, serde::Serialize)]
#[serde(rename_all = "snake_case")]
enum GeminiPart {
    Text(String),
    InlineData { mime_type: String, data: String },
}

#[derive(Debug, Deserialize)]
struct GeminiResponse {
    candidates: Option<Vec<GeminiCandidate>>,
}

#[derive(Debug, Deserialize)]
struct GeminiCandidate {
    content: Option<GeminiResponseContent>,
}

#[derive(Debug, Deserialize)]
struct GeminiResponseContent {
    parts: Option<Vec<GeminiResponsePart>>,
}

#[derive(Debug, Deserialize)]
struct GeminiResponsePart {
    text: Option<String>,
}

#[async_trait]
impl Transcriber for GeminiTranscriber {
    async fn transcribe(
        &self,
        audio: &AudioData,
        _model: &str,
        api_key: &str,
    ) -> AppResult<String> {
        let request = GeminiRequest {
            contents: vec![GeminiContent {
                parts: vec![
                    GeminiPart::Text(
                        "Transcribe the attached audio exactly. Return only the spoken text."
                            .to_string(),
                    ),
                    GeminiPart::InlineData {
                        mime_type: "audio/wav".to_string(),
                        data: STANDARD.encode(audio.to_wav()?),
                    },
                ],
            }],
        };
        let response = self
            .client
            .post(&self.endpoint)
            .query(&[("key", api_key)])
            .json(&request)
            .send()
            .await?;
        let status = response.status();
        if !status.is_success() {
            return Err(provider_response_error("Gemini", status, response).await);
        }
        parse_gemini_response(&response.text().await.map_err(AppError::Http)?)
    }
}

fn parse_openai_response(body: &str) -> AppResult<String> {
    let body: OpenAiResponse = serde_json::from_str(body)
        .map_err(|_| response_diagnostic("OpenAI", "invalid_response"))?;
    normalize_transcript(body.text).map_err(|_| response_diagnostic("OpenAI", "empty_transcript"))
}

fn parse_gemini_response(body: &str) -> AppResult<String> {
    let body: GeminiResponse = serde_json::from_str(body)
        .map_err(|_| response_diagnostic("Gemini", "invalid_response"))?;
    let text = body
        .candidates
        .and_then(|candidates| candidates.into_iter().next())
        .and_then(|candidate| candidate.content)
        .and_then(|content| content.parts)
        .and_then(|parts| parts.into_iter().find_map(|part| part.text))
        .ok_or_else(|| response_diagnostic("Gemini", "missing_transcript"))?;
    normalize_transcript(text).map_err(|_| response_diagnostic("Gemini", "empty_transcript"))
}

pub fn transcriber_for(config: &AppConfig) -> AppResult<Box<dyn Transcriber>> {
    let client = Client::builder()
        .timeout(std::time::Duration::from_secs(120))
        .build()
        .map_err(|error| AppError::Provider(format!("could not create HTTP client: {error}")))?;
    Ok(match config.provider {
        ProviderKind::OpenAi => Box::new(OpenAiTranscriber::new(client)),
        ProviderKind::Gemini => Box::new(GeminiTranscriber::new(client, &config.model)),
    })
}

async fn provider_response_error(
    provider: &'static str,
    status: StatusCode,
    response: reqwest::Response,
) -> AppError {
    let body = response.text().await.unwrap_or_default();
    AppError::ProviderDiagnostic {
        provider,
        status: status.as_u16(),
        category: "request_rejected",
        code: safe_provider_code(&body),
    }
}

fn response_diagnostic(provider: &'static str, category: &'static str) -> AppError {
    AppError::ProviderDiagnostic {
        provider,
        status: 200,
        category,
        code: "none",
    }
}

// Only fixed, recognized codes may leave the response parser. Never log arbitrary
// provider messages or codes: even those fields can contain sensitive payloads.
fn safe_provider_code(body: &str) -> &'static str {
    let Ok(value) = serde_json::from_str::<serde_json::Value>(body) else {
        return "unknown";
    };
    let code = value.pointer("/error/code").and_then(|value| value.as_str())
        .or_else(|| value.pointer("/error/status").and_then(|value| value.as_str()));
    match code {
        Some("insufficient_quota") => "insufficient_quota",
        Some("invalid_api_key") => "invalid_api_key",
        Some("rate_limit_exceeded") => "rate_limit_exceeded",
        Some("model_not_found") => "model_not_found",
        Some("invalid_value") => "invalid_value",
        Some("invalid_request_error") => "invalid_request_error",
        Some("RESOURCE_EXHAUSTED") => "RESOURCE_EXHAUSTED",
        Some("PERMISSION_DENIED") => "PERMISSION_DENIED",
        Some("UNAUTHENTICATED") => "UNAUTHENTICATED",
        Some("INVALID_ARGUMENT") => "INVALID_ARGUMENT",
        _ => "unknown",
    }
}

fn normalize_transcript(text: String) -> AppResult<String> {
    let text = text.trim().to_string();
    if text.is_empty() {
        return Err(AppError::Provider(
            "provider returned an empty transcript".to_string(),
        ));
    }
    Ok(text)
}

#[cfg(test)]
mod tests {
    use std::{io::Write, net::TcpListener, thread, time::Duration};

    use reqwest::Client;

    use crate::{audio::AudioData, error::AppError};

    use super::{
        OpenAiTranscriber, Transcriber, normalize_transcript, parse_gemini_response,
        parse_openai_response, safe_provider_code,
    };

    #[test]
    fn trims_provider_text() {
        assert_eq!(
            normalize_transcript(" hello ".to_string()).unwrap(),
            "hello"
        );
    }

    #[test]
    fn rejects_empty_provider_text() {
        assert!(normalize_transcript("  ".to_string()).is_err());
    }

    #[test]
    fn parses_openai_fixture() {
        let result = parse_openai_response(r#"{"text":"hello from openai"}"#).unwrap();
        assert_eq!(result, "hello from openai");
    }

    #[test]
    fn parses_gemini_fixture() {
        let result = parse_gemini_response(
            r#"{"candidates":[{"content":{"parts":[{"text":"hello from gemini"}]}}]}"#,
        )
        .unwrap();
        assert_eq!(result, "hello from gemini");
    }

    #[test]
    fn rejects_malformed_fixture() {
        assert!(parse_openai_response("not json").is_err());
        assert!(parse_gemini_response(r#"{"candidates":[]}"#).is_err());
    }

    #[test]
    fn preserves_only_allowlisted_error_codes() {
        assert_eq!(safe_provider_code(r#"{"error":{"code":"insufficient_quota","message":"secret"}}"#), "insufficient_quota");
        for body in [r#"{"error":{"code":"secret","message":"private transcript"}}"#, "secret", "{}"] {
            assert_eq!(safe_provider_code(body), "unknown");
        }
    }

    #[test]
    fn distinguishes_response_failures_without_payloads() {
        for (body, category) in [("private transcript", "invalid_response"), (r#"{"text":" "}"#, "empty_transcript")] {
            let error = parse_openai_response(body).unwrap_err().to_string();
            assert!(error.contains(category));
            assert!(!error.contains("private transcript"));
        }
        assert!(parse_gemini_response("{}").unwrap_err().to_string().contains("missing_transcript"));
    }

    #[test]
    fn times_out_slow_provider_response() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        thread::spawn(move || {
            let Ok((mut stream, _)) = listener.accept() else {
                return;
            };
            thread::sleep(Duration::from_millis(100));
            let _ = stream.write_all(b"HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: 15\r\n\r\n{\"text\":\"late\"}");
        });

        let runtime = tokio::runtime::Runtime::new().unwrap();
        let result = runtime.block_on(async {
            let client = Client::builder()
                .timeout(Duration::from_millis(10))
                .build()
                .unwrap();
            let transcriber = OpenAiTranscriber {
                client,
                endpoint: format!("http://{address}"),
            };
            transcriber
                .transcribe(&AudioData::new(vec![0.0], 16_000), "test", "secret")
                .await
        });
        assert!(matches!(result, Err(AppError::Http(_))));
    }
}
