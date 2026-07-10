use serde::{Deserialize, Serialize};

use super::{system_prompt, TranslationProvider};

/// LM Studio exposes an OpenAI-compatible chat-completions endpoint on
/// whatever local server the user has running (default
/// `http://localhost:1234/v1`). No real API key is required — LM Studio
/// doesn't validate it — so `api_key` is optional here, unlike the hosted
/// providers.
pub struct LmStudioProvider {
    base_url: String,
    api_key: Option<String>,
}

impl LmStudioProvider {
    pub fn new(base_url: String, api_key: Option<String>) -> Self {
        Self {
            base_url: base_url.trim_end_matches('/').to_string(),
            api_key,
        }
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

impl TranslationProvider for LmStudioProvider {
    async fn translate(
        &self,
        text: &str,
        source_lang: &str,
        target_lang: &str,
    ) -> Result<String, String> {
        let url = format!("{}/chat/completions", self.base_url);

        let body = ChatRequest {
            // LM Studio's OpenAI-compat server serves whichever model is
            // currently loaded regardless of this field in most setups; we
            // don't ask the user to name it for v1.
            model: "local-model",
            temperature: 0.3,
            messages: vec![
                ChatMessage {
                    role: "system",
                    content: system_prompt(source_lang, target_lang),
                },
                ChatMessage {
                    role: "user",
                    content: text.to_string(),
                },
            ],
        };

        let client = reqwest::Client::new();
        let mut request = client
            .post(&url)
            .header("content-type", "application/json")
            .json(&body);

        if let Some(key) = self.api_key.as_deref().filter(|k| !k.trim().is_empty()) {
            request = request.header("Authorization", format!("Bearer {key}"));
        }

        let response = request.send().await.map_err(|e| {
            format!(
                "no se pudo conectar a LM Studio en {}: {e}",
                self.base_url
            )
        })?;

        if !response.status().is_success() {
            let status = response.status();
            let raw = response.text().await.unwrap_or_default();
            return Err(format!("LM Studio error ({status}): {raw}"));
        }

        let parsed: ChatResponse = response
            .json()
            .await
            .map_err(|e| format!("failed to parse LM Studio response: {e}"))?;

        parsed
            .choices
            .into_iter()
            .next()
            .map(|c| c.message.content)
            .ok_or_else(|| "LM Studio response had no choices".to_string())
    }
}
