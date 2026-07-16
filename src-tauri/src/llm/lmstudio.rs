use serde::{Deserialize, Serialize};

use super::{system_prompt, TranslationProvider};

/// Local LLM servers (LM Studio, Ollama, llama.cpp's server, etc.) that
/// speak the OpenAI-compatible `/v1/chat/completions` shape. No real API
/// key is required by most of them, so `api_key` is optional here, unlike
/// the hosted providers.
///
/// The `model` field matters more than it first looks: LM Studio only
/// ignores it when a single model is loaded — with several models loaded
/// at once (e.g. a small one dedicated to this app alongside a bigger one
/// used for something else, both on the same `localhost:1234` server) it's
/// exactly what routes each request to the right one. Ollama is stricter
/// still: it 404s if the value isn't an exact, already-`ollama pull`ed tag.
/// We always send *something* — falling back to a harmless placeholder —
/// since an empty/missing model field is itself invalid on some servers,
/// but leaving it unset is only safe in the single-loaded-model case.
pub struct LmStudioProvider {
    base_url: String,
    api_key: Option<String>,
    model: String,
}

impl LmStudioProvider {
    pub fn new(base_url: String, api_key: Option<String>, model: Option<String>) -> Self {
        let trimmed = base_url.trim_end_matches('/').to_string();
        // The OpenAI-compatible API on these servers lives under `/v1` —
        // if the user configured just `http://localhost:1234`, add it
        // rather than failing with "Unexpected endpoint or method".
        let normalized = if trimmed.ends_with("/v1") {
            trimmed
        } else {
            format!("{trimmed}/v1")
        };
        let model = model
            .filter(|m| !m.trim().is_empty())
            .unwrap_or_else(|| "local-model".to_string());
        Self {
            base_url: normalized,
            api_key,
            model,
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
    model: String,
    temperature: f32,
    max_tokens: u32,
    messages: Vec<ChatMessage>,
}

/// Generous headroom for what's meant to be a short chat/email fragment —
/// high enough that a normal translation never gets near it, but bounded so
/// a misbehaving model can't run away. Reasoning-capable local models (MoE
/// variants especially) can spend a chunk of this "thinking" before the
/// visible answer starts, which is exactly the scenario this is guarding
/// against: without an explicit cap here, we're fully at the mercy of
/// whatever response-length limit happens to be set for the model/preset in
/// LM Studio, and a low one truncates the translation mid-word.
const MAX_TOKENS: u32 = 1024;

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
    // "length" means the server cut generation short to respect a token
    // limit (ours or one configured server-side) — the content is a partial
    // fragment, not a real translation, even if it happens to look
    // plausible. Surfacing this is what catches responses like a lone "v"
    // instead of silently pasting them.
    #[serde(default)]
    finish_reason: Option<String>,
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
        tone: &str,
    ) -> Result<String, String> {
        let url = format!("{}/chat/completions", self.base_url);

        let body = ChatRequest {
            model: self.model.clone(),
            temperature: 0.3,
            max_tokens: MAX_TOKENS,
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

        // Some local servers don't validate this at all (LM Studio, Ollama),
        // but a handful (llama.cpp with `--api-key`, vLLM) enforce it, and
        // an entirely missing header trips up a few client stacks even when
        // the value itself is never checked — so always send *something*.
        let key = self
            .api_key
            .as_deref()
            .filter(|k| !k.trim().is_empty())
            .unwrap_or("not-needed");

        let client = reqwest::Client::new();
        let request = client
            .post(&url)
            .header("content-type", "application/json")
            .header("Authorization", format!("Bearer {key}"))
            .json(&body);

        let response = request.send().await.map_err(|e| {
            format!(
                "no se pudo conectar al LLM local en {}: {e}",
                self.base_url
            )
        })?;

        if !response.status().is_success() {
            let status = response.status();
            let raw = response.text().await.unwrap_or_default();
            let hint = if status.as_u16() == 404 {
                " (si usás Ollama, esto suele significar que el modelo no está descargado — corré `ollama pull <modelo>` primero)"
            } else {
                ""
            };
            return Err(format!("Error del LLM local ({status}){hint}: {raw}"));
        }

        // Read the raw body first so a parse failure can show the actual
        // JSON the server sent back instead of just reqwest/serde's opaque
        // "error decoding response body" — the shape varies a fair bit
        // across backends/model chat templates.
        let raw = response
            .text()
            .await
            .map_err(|e| format!("failed to read local LLM response: {e}"))?;

        let parsed: ChatResponse = serde_json::from_str(&raw).map_err(|e| {
            let snippet: String = raw.chars().take(500).collect();
            format!("failed to parse local LLM response: {e}\nraw body: {snippet}")
        })?;

        let choice = parsed
            .choices
            .into_iter()
            .next()
            .ok_or_else(|| format!("Local LLM response had no text content\nraw body: {raw}"))?;

        let text = choice
            .message
            .content
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .ok_or_else(|| format!("Local LLM response had no text content\nraw body: {raw}"))?;

        if choice.finish_reason.as_deref() == Some("length") {
            return Err(format!(
                "El LLM local cortó la respuesta antes de terminar (\"{text}\") — llegó al límite \
                 de tokens. Si tu modelo tiene modo de razonamiento/\"thinking\", puede estar \
                 gastando el presupuesto en eso antes de escribir la traducción. Subí el límite \
                 de tokens de respuesta para este modelo/preset en tu servidor local y probá de nuevo."
            ));
        }

        Ok(text)
    }
}
