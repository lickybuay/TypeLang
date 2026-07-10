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
        let trimmed = base_url.trim_end_matches('/').to_string();
        // LM Studio's OpenAI-compatible API lives under `/v1` — if the user
        // configured just `http://localhost:1234`, add it rather than
        // failing with "Unexpected endpoint or method".
        let normalized = if trimmed.ends_with("/v1") {
            trimmed
        } else {
            format!("{trimmed}/v1")
        };
        Self {
            base_url: normalized,
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
    // Some backends/models return `null` here (e.g. a tool-call-only turn)
    // instead of an empty string — Option avoids a hard parse failure on
    // that shape.
    content: Option<String>,
}

#[derive(Deserialize)]
struct Choice {
    message: ChoiceMessage,
}

#[derive(Deserialize)]
struct ChatResponse {
    #[serde(default)]
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

        // Read the raw body first so a parse failure can show the actual
        // JSON LM Studio sent back instead of just reqwest/serde's opaque
        // "error decoding response body" — the shape varies a fair bit
        // across backends/model chat templates.
        let raw = response
            .text()
            .await
            .map_err(|e| format!("failed to read LM Studio response: {e}"))?;

        let parsed: ChatResponse = serde_json::from_str(&raw).map_err(|e| {
            let snippet: String = raw.chars().take(500).collect();
            format!("failed to parse LM Studio response: {e}\nraw body: {snippet}")
        })?;

        parsed
            .choices
            .into_iter()
            .next()
            .and_then(|c| c.message.content)
            .filter(|s| !s.trim().is_empty())
            .ok_or_else(|| format!("LM Studio response had no text content\nraw body: {raw}"))
    }
}
