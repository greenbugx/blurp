use crate::contract::{
    ContractError, DraftBatch, DraftRequest, SYSTEM_PROMPT, parse_draft_response,
};
use serde::{Deserialize, Serialize};
use std::env;
use std::time::Duration;
use thiserror::Error;

pub const DEFAULT_BASE_URL: &str = "http://localhost:11434";
pub const DEFAULT_MODEL: &str = "gemma4:e4b";
pub const BASE_URL_ENV: &str = "BLURP_OLLAMA_URL";
pub const MODEL_ENV: &str = "BLURP_MODEL";

const REQUEST_TIMEOUT: Duration = Duration::from_secs(300);
const CONNECT_TIMEOUT: Duration = Duration::from_secs(5);

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OllamaConfig {
    pub base_url: String,
    pub model: String,
}

impl OllamaConfig {
    pub fn resolve(base_url: Option<String>, model: Option<String>) -> Self {
        Self {
            base_url: base_url
                .or_else(|| env::var(BASE_URL_ENV).ok())
                .unwrap_or_else(|| DEFAULT_BASE_URL.to_string())
                .trim_end_matches('/')
                .to_string(),
            model: model
                .or_else(|| env::var(MODEL_ENV).ok())
                .unwrap_or_else(|| DEFAULT_MODEL.to_string())
                .trim()
                .to_string(),
        }
    }
}

impl Default for OllamaConfig {
    fn default() -> Self {
        Self::resolve(None, None)
    }
}

#[derive(Debug, Error)]
pub enum OllamaError {
    #[error("Ollama is not reachable at {url}. Start it with `ollama serve` and try again.")]
    NotRunning { url: String },
    #[error("model '{model}' is not pulled yet. Run `ollama pull {model}` and try again.")]
    ModelNotFound { model: String },
    #[error("Ollama did not reply in time. Try a shorter dump or a faster model.")]
    TimedOut,
    #[error("Ollama model error: {detail}")]
    ModelError { detail: String },
    #[error("Ollama returned HTTP {status}: {detail}")]
    Http { status: u16, detail: String },
    #[error("Ollama reply did not include message content.")]
    InvalidResponse,
    #[error("Ollama request failed: {0}")]
    Request(#[from] reqwest::Error),
    #[error(transparent)]
    Contract(#[from] ContractError),
}

pub struct OllamaClient {
    config: OllamaConfig,
    http: reqwest::Client,
}

impl OllamaClient {
    pub fn new(config: OllamaConfig) -> Result<Self, OllamaError> {
        let http = reqwest::Client::builder()
            .timeout(REQUEST_TIMEOUT)
            .connect_timeout(CONNECT_TIMEOUT)
            .build()
            .map_err(OllamaError::Request)?;

        Ok(Self { config, http })
    }

    pub async fn draft(&self, request: &DraftRequest) -> Result<DraftBatch, OllamaError> {
        let chat = ChatRequest {
            model: &self.config.model,
            messages: vec![
                ChatMessage {
                    role: "system",
                    content: SYSTEM_PROMPT.to_string(),
                },
                ChatMessage {
                    role: "user",
                    content: request.user_message(),
                },
            ],
            stream: false,
            format: "json",
        };
        let url = format!("{}/api/chat", self.config.base_url);

        let response = match self.http.post(url).json(&chat).send().await {
            Ok(response) => response,
            Err(error) if error.is_timeout() => return Err(OllamaError::TimedOut),
            Err(error) if error.is_connect() => {
                return Err(OllamaError::NotRunning {
                    url: self.config.base_url.clone(),
                });
            }
            Err(error) => return Err(OllamaError::Request(error)),
        };

        let status = response.status();
        if !status.is_success() {
            let detail = error_detail(response).await;
            return Err(match status.as_u16() {
                404 => OllamaError::ModelNotFound {
                    model: self.config.model.clone(),
                },
                code if code >= 500 => OllamaError::ModelError { detail },
                code => OllamaError::Http {
                    status: code,
                    detail,
                },
            });
        }

        let reply: ChatResponse = response.json().await.map_err(|error| {
            if error.is_decode() {
                OllamaError::InvalidResponse
            } else {
                OllamaError::Request(error)
            }
        })?;
        let content = reply
            .message
            .and_then(|message| message.content)
            .filter(|content| !content.trim().is_empty())
            .ok_or(OllamaError::InvalidResponse)?;

        parse_draft_response(request, &content).map_err(OllamaError::Contract)
    }
}

async fn error_detail(response: reqwest::Response) -> String {
    let text = response.text().await.unwrap_or_default();
    serde_json::from_str::<serde_json::Value>(&text)
        .ok()
        .and_then(|value| {
            value
                .get("error")
                .and_then(|error| error.as_str().map(str::to_string))
        })
        .unwrap_or(text)
}

#[derive(Serialize)]
struct ChatRequest<'a> {
    model: &'a str,
    messages: Vec<ChatMessage>,
    stream: bool,
    format: &'static str,
}

#[derive(Serialize)]
struct ChatMessage {
    role: &'static str,
    content: String,
}

#[derive(Deserialize)]
struct ChatResponse {
    message: Option<ChatReplyMessage>,
}

#[derive(Deserialize)]
struct ChatReplyMessage {
    content: Option<String>,
}

#[cfg(test)]
pub(crate) mod harness {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::sync::oneshot;

    pub async fn serve_once(response: String) -> (String, oneshot::Receiver<String>) {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let (sender, receiver) = oneshot::channel();

        tokio::spawn(async move {
            let (mut socket, _) = listener.accept().await.unwrap();
            let mut received = Vec::new();
            let mut chunk = [0u8; 4096];
            loop {
                let read = socket.read(&mut chunk).await.unwrap_or(0);
                if read == 0 {
                    break;
                }
                received.extend_from_slice(&chunk[..read]);
                let text = String::from_utf8_lossy(&received);
                if let Some(headers_end) = text.find("\r\n\r\n") {
                    let length = content_length(&text[..headers_end]).unwrap_or(0);
                    if received.len() >= headers_end + 4 + length {
                        break;
                    }
                }
            }
            let _ = sender.send(String::from_utf8_lossy(&received).into_owned());
            let _ = socket.write_all(response.as_bytes()).await;
            let _ = socket.shutdown().await;
        });

        (format!("http://{addr}"), receiver)
    }

    fn content_length(headers: &str) -> Option<usize> {
        headers.lines().find_map(|line| {
            let (name, value) = line.split_once(':')?;
            if name.trim().eq_ignore_ascii_case("content-length") {
                value.trim().parse().ok()
            } else {
                None
            }
        })
    }

    pub fn http_response(status: u16, reason: &str, body: &str) -> String {
        format!(
            "HTTP/1.1 {status} {reason}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
            body.len()
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::draft::{DraftStatus, Tone};
    use harness::{http_response, serve_once};

    #[test]
    fn config_resolves_explicit_values_then_env_then_defaults() {
        let config = OllamaConfig::resolve(
            Some("http://127.0.0.1:9999/".to_string()),
            Some(" test-model ".to_string()),
        );
        assert_eq!(config.base_url, "http://127.0.0.1:9999");
        assert_eq!(config.model, "test-model");

        let config = OllamaConfig::resolve(None, None);
        assert_eq!(config.base_url, DEFAULT_BASE_URL);
        assert_eq!(config.model, DEFAULT_MODEL);
    }

    #[tokio::test]
    async fn drafts_through_a_mock_ollama_reply() {
        let content = r#"{"drafts":[{"id":"1","title":"Cancel tonight","recipient":"Sam","body":"Hey Sam, I can't make it tonight. Does Thursday work instead?","tone":"casual","missingDetails":[],"status":"draft"}]}"#;
        let reply =
            serde_json::json!({"message": {"role": "assistant", "content": content}, "done": true})
                .to_string();
        let (base_url, received) = serve_once(http_response(200, "OK", &reply)).await;

        let client = OllamaClient::new(OllamaConfig::resolve(
            Some(base_url),
            Some("test-model".to_string()),
        ))
        .unwrap();
        let request = DraftRequest::new("tell sam i cant make it tonight", None).unwrap();

        let batch = client.draft(&request).await.unwrap();

        assert_eq!(batch.dump, request.dump);
        assert_eq!(batch.drafts[0].title, "Cancel tonight");
        assert_eq!(batch.drafts[0].tone, Tone::Casual);
        assert_eq!(batch.drafts[0].status, DraftStatus::Draft);

        let sent = received.await.unwrap();
        let (_, body) = sent.split_once("\r\n\r\n").unwrap();
        let json: serde_json::Value = serde_json::from_str(body).unwrap();
        assert_eq!(json["model"], "test-model");
        assert_eq!(json["stream"], false);
        assert_eq!(json["format"], "json");
        assert_eq!(json["messages"][0]["role"], "system");
        assert_eq!(json["messages"][0]["content"], SYSTEM_PROMPT);
        assert_eq!(json["messages"][1]["role"], "user");
        assert_eq!(json["messages"][1]["content"], request.user_message());
    }

    #[tokio::test]
    async fn a_missing_model_reports_the_pull_command() {
        let reply =
            serde_json::json!({"error": "model \"missing-model\" not found, try pulling it first"})
                .to_string();
        let (base_url, _) = serve_once(http_response(404, "Not Found", &reply)).await;

        let client = OllamaClient::new(OllamaConfig::resolve(
            Some(base_url),
            Some("missing-model".to_string()),
        ))
        .unwrap();
        let request = DraftRequest::new("hello", None).unwrap();

        let error = client.draft(&request).await.unwrap_err();
        assert!(matches!(error, OllamaError::ModelNotFound { .. }));
        assert!(error.to_string().contains("ollama pull missing-model"));
    }

    #[tokio::test]
    async fn a_model_error_surfaces_the_detail() {
        let reply = serde_json::json!({"error": "tokenizer exploded"}).to_string();
        let (base_url, _) = serve_once(http_response(500, "Internal Server Error", &reply)).await;

        let client = OllamaClient::new(OllamaConfig::resolve(Some(base_url), None)).unwrap();
        let request = DraftRequest::new("hello", None).unwrap();

        let error = client.draft(&request).await.unwrap_err();
        assert!(matches!(error, OllamaError::ModelError { .. }));
        assert!(error.to_string().contains("tokenizer exploded"));
    }

    #[tokio::test]
    async fn an_empty_ollama_reply_is_rejected() {
        let (base_url, _) = serve_once(http_response(200, "OK", "{}")).await;

        let client = OllamaClient::new(OllamaConfig::resolve(Some(base_url), None)).unwrap();
        let request = DraftRequest::new("hello", None).unwrap();

        let error = client.draft(&request).await.unwrap_err();
        assert!(matches!(error, OllamaError::InvalidResponse));
    }

    #[tokio::test]
    async fn an_unusable_model_reply_maps_to_the_contract_error() {
        let reply = serde_json::json!({"message": {"role": "assistant", "content": "I cannot help with that."}})
            .to_string();
        let (base_url, _) = serve_once(http_response(200, "OK", &reply)).await;

        let client = OllamaClient::new(OllamaConfig::resolve(Some(base_url), None)).unwrap();
        let request = DraftRequest::new("hello", None).unwrap();

        let error = client.draft(&request).await.unwrap_err();
        assert!(matches!(
            error,
            OllamaError::Contract(ContractError::NoJson)
        ));
        assert!(error.to_string().contains("no JSON object"));
    }

    #[tokio::test]
    async fn a_dead_endpoint_reports_ollama_is_not_running() {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        drop(listener);

        let client =
            OllamaClient::new(OllamaConfig::resolve(Some(format!("http://{addr}")), None)).unwrap();
        let request = DraftRequest::new("hello", None).unwrap();

        let error = client.draft(&request).await.unwrap_err();
        assert!(matches!(error, OllamaError::NotRunning { .. }));
        assert!(error.to_string().contains("ollama serve"));
    }

    #[tokio::test]
    #[ignore]
    async fn drafts_with_the_real_local_ollama() {
        let client = OllamaClient::new(OllamaConfig::default()).unwrap();
        let request = DraftRequest::new(
            "tell anya i cant make it tonight, ask dev whether the invoice was paid, and thank professor lin then ask if a monday submission is okay",
            None,
        )
        .unwrap();

        let batch = client.draft(&request).await.expect("live drafting failed");

        assert!(!batch.drafts.is_empty());
        for draft in &batch.drafts {
            assert!(!draft.title.is_empty());
            assert!(!draft.body.is_empty());
        }
        println!("{}", serde_json::to_string_pretty(&batch).unwrap());
    }
}
