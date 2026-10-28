use serde::{Deserialize, Serialize};

use super::{system_prompt, user_message, PromptOptions, TranslationProvider, TRUNCATED};

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
    // "length" means the reply was cut at a token limit.
    #[serde(default)]
    finish_reason: Option<String>,
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
        opts: &PromptOptions,
    ) -> Result<String, String> {
        let body = ChatRequest {
            model: MODEL,
            temperature: 0.3,
            messages: vec![
                ChatMessage {
                    role: "system",
                    content: system_prompt(source_lang, target_lang, tone, opts),
                },
                ChatMessage {
                    role: "user",
                    content: user_message(text, opts).into_owned(),
                },
            ],
        };

        let client = super::http_client()?;
        let response = client
            .post(API_URL)
            .header("Authorization", format!("Bearer {}", self.api_key))
            .header("content-type", "application/json")
            .json(&body)
            .send()
            .await
            .map_err(|e| format!("network error calling OpenAI: {}", e.without_url()))?;

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
            .map_err(|e| format!("failed to parse OpenAI response: {}", e.without_url()))?;

        let choice = parsed
            .choices
            .into_iter()
            .next()
            .ok_or_else(|| "OpenAI response had no choices".to_string())?;
        if choice.finish_reason.as_deref() == Some("length") {
            return Err(TRUNCATED.to_string());
        }
        Ok(choice.message.content)
    }
}
