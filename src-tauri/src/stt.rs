//! Client for the local OpenAI-compatible speech-to-text server (whisper.cpp's
//! `whisper-server`), used by the dictation button in the popup.

use std::time::Duration;

use reqwest::multipart::{Form, Part};
use serde::Deserialize;

pub const DEFAULT_BASE_URL: &str = "http://127.0.0.1:8178/v1";

const REQUEST_TIMEOUT: Duration = Duration::from_secs(60);

#[derive(Deserialize)]
struct TranscriptionResponse {
    text: String,
}

/// Same `/v1` normalization as the local LLM provider (see `llm::lmstudio`);
/// an empty setting falls back to the default server.
fn normalize_base_url(base_url: &str) -> String {
    let trimmed = base_url.trim().trim_end_matches('/');
    if trimmed.is_empty() {
        DEFAULT_BASE_URL.to_string()
    } else if trimmed.ends_with("/v1") {
        trimmed.to_string()
    } else {
        format!("{trimmed}/v1")
    }
}

/// ISO 639-1 code Whisper expects for the languages in TRANSLATION_LANGUAGES.
fn language_code(source_lang: &str) -> Option<&'static str> {
    match source_lang {
        "Spanish" => Some("es"),
        "English" => Some("en"),
        "French" => Some("fr"),
        "German" => Some("de"),
        "Italian" => Some("it"),
        "Portuguese" => Some("pt"),
        _ => None,
    }
}

/// Whisper prints non-speech markers like "[BLANK_AUDIO]" or "(silence)" for
/// audio with nothing to transcribe; those must not end up in the textarea.
fn is_non_speech_marker(text: &str) -> bool {
    (text.starts_with('[') && text.ends_with(']')) || (text.starts_with('(') && text.ends_with(')'))
}

/// Sends `wav` for transcription and returns the trimmed text (empty when the
/// audio held no speech). `vocabulary` biases Whisper toward those spellings.
pub async fn transcribe(
    base_url: &str,
    wav: Vec<u8>,
    source_lang: &str,
    vocabulary: &[String],
) -> Result<String, String> {
    let base_url = normalize_base_url(base_url);
    let url = format!("{base_url}/audio/transcriptions");

    let file = Part::bytes(wav)
        .file_name("audio.wav")
        .mime_str("audio/wav")
        .map_err(|e| e.to_string())?;
    let mut form = Form::new()
        .part("file", file)
        .text("response_format", "json");
    if let Some(code) = language_code(source_lang) {
        form = form.text("language", code);
    }
    if !vocabulary.is_empty() {
        form = form.text("prompt", vocabulary.join(", "));
    }

    let client = reqwest::Client::builder()
        .timeout(REQUEST_TIMEOUT)
        .build()
        .map_err(|e| e.to_string())?;
    let response = client
        .post(&url)
        .multipart(form)
        .send()
        .await
        .map_err(|e| {
            if e.is_timeout() {
                format!("El servicio de dictado ({base_url}) tardó demasiado en responder")
            } else {
                format!("No se pudo conectar al servicio de dictado ({base_url}). ¿Está levantado?")
            }
        })?;

    if !response.status().is_success() {
        let status = response.status();
        let raw = response.text().await.unwrap_or_default();
        return Err(format!("Error del servicio de dictado ({status}): {raw}"));
    }

    let raw = response
        .text()
        .await
        .map_err(|e| format!("failed to read dictation response: {e}"))?;
    let parsed: TranscriptionResponse = serde_json::from_str(&raw).map_err(|e| {
        let snippet: String = raw.chars().take(500).collect();
        format!("failed to parse dictation response: {e}\nraw body: {snippet}")
    })?;

    let text = parsed.text.trim();
    if is_non_speech_marker(text) {
        return Ok(String::new());
    }
    Ok(text.to_string())
}
