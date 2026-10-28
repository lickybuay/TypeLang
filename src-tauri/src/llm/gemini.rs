use serde::{Deserialize, Serialize};

use super::{system_prompt, user_message, PromptOptions, TranslationProvider, TRUNCATED};

const MODEL: &str = "gemini-2.0-flash";

pub struct GeminiProvider {
    api_key: String,
}

impl GeminiProvider {
    pub fn new(api_key: String) -> Self {
        Self { api_key }
    }
}

#[derive(Serialize)]
struct Part {
    text: String,
}

#[derive(Serialize)]
struct Content {
    role: &'static str,
    parts: Vec<Part>,
}

#[derive(Serialize)]
struct SystemInstruction {
    parts: Vec<Part>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct GenerationConfig {
    temperature: f32,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct GenerateRequest {
    system_instruction: SystemInstruction,
    contents: Vec<Content>,
    generation_config: GenerationConfig,
}

#[derive(Deserialize)]
struct RespPart {
    text: Option<String>,
}

// A reply cut at MAX_TOKENS before any text can come without `parts`; default
// them so the finish reason below is what gets reported.
#[derive(Deserialize, Default)]
struct RespContent {
    #[serde(default)]
    parts: Vec<RespPart>,
}

#[derive(Deserialize)]
struct Candidate {
    #[serde(default)]
    content: RespContent,
    // "MAX_TOKENS" means the reply was cut at a token limit.
    #[serde(default, rename = "finishReason")]
    finish_reason: Option<String>,
}

#[derive(Deserialize)]
struct GenerateResponse {
    candidates: Vec<Candidate>,
}

#[derive(Deserialize)]
struct ErrorBody {
    error: ErrorDetail,
}

#[derive(Deserialize)]
struct ErrorDetail {
    message: String,
}

impl TranslationProvider for GeminiProvider {
    async fn translate(
        &self,
        text: &str,
        source_lang: &str,
        target_lang: &str,
        tone: &str,
        opts: &PromptOptions,
    ) -> Result<String, String> {
        // The key goes in a header, not the query string: reqwest errors
        // print the URL, which would carry the key into messages shown to the
        // user.
        let url = format!(
            "https://generativelanguage.googleapis.com/v1beta/models/{MODEL}:generateContent"
        );

        let body = GenerateRequest {
            system_instruction: SystemInstruction {
                parts: vec![Part {
                    text: system_prompt(source_lang, target_lang, tone, opts),
                }],
            },
            contents: vec![Content {
                role: "user",
                parts: vec![Part {
                    text: user_message(text, opts).into_owned(),
                }],
            }],
            generation_config: GenerationConfig { temperature: 0.3 },
        };

        let client = super::http_client()?;
        let response = client
            .post(&url)
            .header("x-goog-api-key", &self.api_key)
            .header("content-type", "application/json")
            .json(&body)
            .send()
            .await
            .map_err(|e| format!("network error calling Gemini: {}", e.without_url()))?;

        if !response.status().is_success() {
            let status = response.status();
            let raw = response.text().await.unwrap_or_default();
            let message = serde_json::from_str::<ErrorBody>(&raw)
                .map(|b| b.error.message)
                .unwrap_or(raw);
            return Err(format!("Gemini API error ({status}): {message}"));
        }

        let parsed: GenerateResponse = response
            .json()
            .await
            .map_err(|e| format!("failed to parse Gemini response: {}", e.without_url()))?;

        let candidate = parsed
            .candidates
            .into_iter()
            .next()
            .ok_or_else(|| "Gemini response had no text content".to_string())?;
        if candidate.finish_reason.as_deref() == Some("MAX_TOKENS") {
            return Err(TRUNCATED.to_string());
        }
        candidate
            .content
            .parts
            .into_iter()
            .find_map(|p| p.text)
            .ok_or_else(|| "Gemini response had no text content".to_string())
    }
}
