use serde::{Deserialize, Serialize};

use super::{system_prompt, TranslationProvider};

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

#[derive(Deserialize)]
struct RespContent {
    parts: Vec<RespPart>,
}

#[derive(Deserialize)]
struct Candidate {
    content: RespContent,
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
    ) -> Result<String, String> {
        let url = format!(
            "https://generativelanguage.googleapis.com/v1beta/models/{MODEL}:generateContent?key={}",
            self.api_key
        );

        let body = GenerateRequest {
            system_instruction: SystemInstruction {
                parts: vec![Part {
                    text: system_prompt(source_lang, target_lang),
                }],
            },
            contents: vec![Content {
                role: "user",
                parts: vec![Part {
                    text: text.to_string(),
                }],
            }],
            generation_config: GenerationConfig { temperature: 0.3 },
        };

        let client = reqwest::Client::new();
        let response = client
            .post(&url)
            .header("content-type", "application/json")
            .json(&body)
            .send()
            .await
            .map_err(|e| format!("network error calling Gemini: {e}"))?;

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
            .map_err(|e| format!("failed to parse Gemini response: {e}"))?;

        parsed
            .candidates
            .into_iter()
            .next()
            .and_then(|c| c.content.parts.into_iter().find_map(|p| p.text))
            .ok_or_else(|| "Gemini response had no text content".to_string())
    }
}
