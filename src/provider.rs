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
        .map_err(|error| AppError::Provider(format!("invalid OpenAI response: {error}")))?;
    normalize_transcript(body.text)
}

fn parse_gemini_response(body: &str) -> AppResult<String> {
    let body: GeminiResponse = serde_json::from_str(body)
        .map_err(|error| AppError::Provider(format!("invalid Gemini response: {error}")))?;
    let text = body
        .candidates
        .and_then(|candidates| candidates.into_iter().next())
        .and_then(|candidate| candidate.content)
        .and_then(|content| content.parts)
        .and_then(|parts| parts.into_iter().find_map(|part| part.text))
        .ok_or_else(|| AppError::Provider("Gemini response contained no transcript".to_string()))?;
    normalize_transcript(text)
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
    provider: &str,
    status: StatusCode,
    response: reqwest::Response,
) -> AppError {
    let body = response.text().await.unwrap_or_default();
    AppError::Provider(format!(
        "{provider} request failed: {}",
        provider_error_detail(status, &body)
    ))
}

fn provider_error_detail(status: StatusCode, body: &str) -> String {
    if body.trim().is_empty() {
        status.to_string()
    } else {
        format!("{status}: {}", truncate(body, 300))
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

fn truncate(value: &str, max_chars: usize) -> String {
    value.chars().take(max_chars).collect()
}

#[cfg(test)]
mod tests {
    use std::{io::Write, net::TcpListener, thread, time::Duration};

    use reqwest::{Client, StatusCode};

    use crate::{audio::AudioData, error::AppError};

    use super::{
        OpenAiTranscriber, Transcriber, normalize_transcript, parse_gemini_response,
        parse_openai_response, provider_error_detail,
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
    fn preserves_auth_error_context_without_secrets() {
        let detail = provider_error_detail(StatusCode::UNAUTHORIZED, r#"{"error":"invalid key"}"#);
        assert!(detail.contains("401"));
        assert!(detail.contains("invalid key"));
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
