use serde::{Deserialize, Serialize};

use super::{system_prompt, TranslationProvider};

const API_URL: &str = "https://api.openai.com/v1/chat/completions";
const MODEL: &str = "gpt-4o-mini";

pub struct OpenAiProvider {
    api_key: String,
}

impl OpenAiProvider {
    pub fn new(api_key: String) -> Self {
        Self { api_key }
    }
}

#[derive(Serialize)]
struct ChatMessage {
    role: &'static str,
    content: String,
}

#[derive(Serialize)]
struct ChatRequest {
    model: &'static str,
    temperature: f32,
    messages: Vec<ChatMessage>,
}

#[derive(Deserialize)]
struct ChoiceMessage {
    content: String,
}

#[derive(Deserialize)]
struct Choice {
    message: ChoiceMessage,
}

#[derive(Deserialize)]
struct ChatResponse {
    choices: Vec<Choice>,
}

#[derive(Deserialize)]
struct ErrorBody {
    error: ErrorDetail,
}

#[derive(Deserialize)]
struct ErrorDetail {
    message: String,
}

impl TranslationProvider for OpenAiProvider {
    async fn translate(
        &self,
        text: &str,
        source_lang: &str,
        target_lang: &str,
        tone: &str,
    ) -> Result<String, String> {
        let body = ChatRequest {
            model: MODEL,
            temperature: 0.3,
            messages: vec![
                ChatMessage {
                    role: "system",
                    content: system_prompt(source_lang, target_lang, tone),
                },
                ChatMessage {
                    role: "user",
                    content: text.to_string(),
                },
            ],
        };

        let client = reqwest::Client::new();
        let response = client
            .post(API_URL)
            .header("Authorization", format!("Bearer {}", self.api_key))
            .header("content-type", "application/json")
            .json(&body)
            .send()
            .await
            .map_err(|e| format!("network error calling OpenAI: {e}"))?;

        if !response.status().is_success() {
            let status = response.status();
            let raw = response.text().await.unwrap_or_default();
            let message = serde_json::from_str::<ErrorBody>(&raw)
                .map(|b| b.error.message)
                .unwrap_or(raw);
            return Err(format!("OpenAI API error ({status}): {message}"));
        }

        let parsed: ChatResponse = response
            .json()
            .await
            .map_err(|e| format!("failed to parse OpenAI response: {e}"))?;

        parsed
            .choices
            .into_iter()
            .next()
            .map(|c| c.message.content)
            .ok_or_else(|| "OpenAI response had no choices".to_string())
    }
}
