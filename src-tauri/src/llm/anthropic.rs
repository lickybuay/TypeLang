use serde::{Deserialize, Serialize};

use super::{system_prompt, TranslationProvider};

const API_URL: &str = "https://api.anthropic.com/v1/messages";
const MODEL: &str = "claude-haiku-4-5-20251001";
const ANTHROPIC_VERSION: &str = "2023-06-01";

pub struct AnthropicProvider {
    api_key: String,
}

impl AnthropicProvider {
    pub fn new(api_key: String) -> Self {
        Self { api_key }
    }
}

#[derive(Serialize)]
struct Message {
    role: &'static str,
    content: String,
}

#[derive(Serialize)]
struct MessagesRequest {
    model: &'static str,
    max_tokens: u32,
    temperature: f32,
    system: String,
    messages: Vec<Message>,
}

#[derive(Deserialize)]
struct ContentBlock {
    text: Option<String>,
}

#[derive(Deserialize)]
struct MessagesResponse {
    content: Vec<ContentBlock>,
}

#[derive(Deserialize)]
struct ErrorBody {
    error: ErrorDetail,
}

#[derive(Deserialize)]
struct ErrorDetail {
    message: String,
}

impl TranslationProvider for AnthropicProvider {
    async fn translate(
        &self,
        text: &str,
        source_lang: &str,
        target_lang: &str,
        tone: &str,
    ) -> Result<String, String> {
        let body = MessagesRequest {
            model: MODEL,
            max_tokens: 1024,
            temperature: 0.3,
            system: system_prompt(source_lang, target_lang, tone),
            messages: vec![Message {
                role: "user",
                content: text.to_string(),
            }],
        };

        let client = reqwest::Client::new();
        let response = client
            .post(API_URL)
            .header("x-api-key", &self.api_key)
            .header("anthropic-version", ANTHROPIC_VERSION)
            .header("content-type", "application/json")
            .json(&body)
            .send()
            .await
            .map_err(|e| format!("network error calling Anthropic: {e}"))?;

        if !response.status().is_success() {
            let status = response.status();
            let raw = response.text().await.unwrap_or_default();
            let message = serde_json::from_str::<ErrorBody>(&raw)
                .map(|b| b.error.message)
                .unwrap_or(raw);
            return Err(format!("Anthropic API error ({status}): {message}"));
        }

        let parsed: MessagesResponse = response
            .json()
            .await
            .map_err(|e| format!("failed to parse Anthropic response: {e}"))?;

        parsed
            .content
            .into_iter()
            .find_map(|block| block.text)
            .ok_or_else(|| "Anthropic response had no text content".to_string())
    }
}
