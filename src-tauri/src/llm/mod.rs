mod anthropic;
mod gemini;
mod lmstudio;
mod openai;

pub use anthropic::AnthropicProvider;
pub use gemini::GeminiProvider;
pub use lmstudio::LmStudioProvider;
pub use openai::OpenAiProvider;

/// System prompt is what actually determines translation quality — this is
/// the product's real value-add over a literal/dumb translator. Kept as a
/// function (not a const) so language direction can become configurable
/// later without touching the provider implementations.
pub fn system_prompt(source_lang: &str, target_lang: &str) -> String {
    format!(
        "You are a professional bilingual translator helping a {source_lang}-speaking \
         professional communicate with {target_lang}-speaking colleagues. Translate the \
         given {source_lang} text into natural, fluent, professional {target_lang} exactly \
         as a native {target_lang}-speaking coworker would write it in a workplace context \
         (Slack, email, docs, chat). Do not translate word-for-word — prioritize natural \
         phrasing, correct idioms, and a professional-but-conversational register. The input \
         may be a short fragment or informal phrase rather than a full sentence; translate \
         accordingly without forcing unnatural formality. Preserve code snippets, proper \
         nouns, URLs, and technical terms unchanged. Output ONLY the translated text — no \
         quotes, no explanation, no preamble."
    )
}

pub trait TranslationProvider {
    async fn translate(&self, text: &str, source_lang: &str, target_lang: &str) -> Result<String, String>;
}

/// Which provider is active plus whatever credentials it needs. `base_url`
/// is only meaningful for `lmstudio`; `api_key` is optional there (LM
/// Studio doesn't validate it) but required for the three hosted providers.
#[derive(Debug, Clone)]
pub struct ProviderConfig {
    pub provider: String,
    pub api_key: Option<String>,
    pub base_url: Option<String>,
}

pub async fn translate(
    cfg: &ProviderConfig,
    text: &str,
    source_lang: &str,
    target_lang: &str,
) -> Result<String, String> {
    match cfg.provider.as_str() {
        "anthropic" => {
            let key = cfg
                .api_key
                .clone()
                .ok_or_else(|| "Falta la API key de Anthropic. Abre Configuración.".to_string())?;
            AnthropicProvider::new(key)
                .translate(text, source_lang, target_lang)
                .await
        }
        "openai" => {
            let key = cfg
                .api_key
                .clone()
                .ok_or_else(|| "Falta la API key de OpenAI. Abre Configuración.".to_string())?;
            OpenAiProvider::new(key)
                .translate(text, source_lang, target_lang)
                .await
        }
        "gemini" => {
            let key = cfg
                .api_key
                .clone()
                .ok_or_else(|| "Falta la API key de Gemini. Abre Configuración.".to_string())?;
            GeminiProvider::new(key)
                .translate(text, source_lang, target_lang)
                .await
        }
        "lmstudio" => {
            let url = cfg
                .base_url
                .clone()
                .filter(|u| !u.trim().is_empty())
                .ok_or_else(|| "Falta la URL del servidor de LM Studio. Abre Configuración.".to_string())?;
            LmStudioProvider::new(url, cfg.api_key.clone())
                .translate(text, source_lang, target_lang)
                .await
        }
        other => Err(format!("Proveedor desconocido: {other}")),
    }
}
