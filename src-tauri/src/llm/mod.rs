mod anthropic;
mod gemini;
mod lmstudio;
mod openai;

pub use anthropic::AnthropicProvider;
pub use gemini::GeminiProvider;
pub use lmstudio::LmStudioProvider;
pub use openai::OpenAiProvider;

/// Client for translation requests. The timeouts keep a hung provider from
/// holding a hold-to-talk job open forever.
fn http_client() -> Result<reqwest::Client, String> {
    reqwest::Client::builder()
        .connect_timeout(std::time::Duration::from_secs(5))
        .timeout(std::time::Duration::from_secs(60))
        .build()
        .map_err(|e| e.to_string())
}

/// Languages in TRANSLATION_LANGUAGES (see `src/i18n.ts`) that grammatically
/// mark formality through the second-person pronoun — the mechanism DeepL's
/// own formality feature relies on for these same languages. English has no
/// such distinction, which is exactly why the professional/casual split has
/// to lean on lexical cues (contractions, openers) for it instead.
/// Returns `(casual_pronoun, formal_pronoun)`.
fn tv_distinction(target_lang: &str) -> Option<(&'static str, &'static str)> {
    match target_lang {
        "Spanish" => Some(("\"tú\"", "\"usted\"")),
        "French" => Some(("\"tu\"", "\"vous\"")),
        "German" => Some(("\"du\"", "\"Sie\"")),
        "Italian" => Some(("\"tu\"", "\"Lei\"")),
        // Modern Portuguese (esp. Brazilian) leans on "você" even fairly
        // formally, so the real signal is politeness constructions
        // (conditional "poderia", indirect requests) rather than a second
        // pronoun the way Spanish/French/German/Italian use one.
        "Portuguese" => Some(("informal \"você\"", "polite/indirect phrasing (e.g. \"poderia\")")),
        _ => None,
    }
}

/// One illustrative pair per supported target language, phrased as a
/// request — that's where formality actually surfaces grammatically (a
/// bare statement like "I'll be late" doesn't address the reader, so it
/// doesn't exercise tú/usted, du/Sie, etc. the way a question does).
fn tone_example(target_lang: &str, tone: &str) -> Option<&'static str> {
    let casual = tone == "casual";
    match (target_lang, casual) {
        ("English", true) => Some("\"Can you send me the file when you get a sec?\""),
        ("English", false) => Some("\"Could you send me the file when you get a chance?\""),
        ("Spanish", true) => Some("\"¿Me mandás el archivo cuando puedas?\""),
        ("Spanish", false) => Some("\"¿Podría enviarme el archivo cuando tenga oportunidad?\""),
        ("French", true) => Some("\"Tu peux m'envoyer le fichier quand tu as un moment ?\""),
        ("French", false) => {
            Some("\"Pourriez-vous m'envoyer le fichier lorsque vous aurez un moment ?\"")
        }
        ("German", true) => Some("\"Kannst du mir die Datei schicken, wenn du Zeit hast?\""),
        ("German", false) => {
            Some("\"Könnten Sie mir die Datei schicken, wenn Sie Zeit haben?\"")
        }
        ("Italian", true) => Some("\"Puoi mandarmi il file quando hai un attimo?\""),
        ("Italian", false) => Some("\"Potrebbe inviarmi il file quando ha un momento?\""),
        ("Portuguese", true) => Some("\"Você pode me mandar o arquivo quando der?\""),
        ("Portuguese", false) => {
            Some("\"Poderia me enviar o arquivo quando tiver oportunidade?\"")
        }
        _ => None,
    }
}

/// System prompt is what actually determines translation quality — this is
/// the product's real value-add over a literal/dumb translator. Kept as a
/// function (not a const) so language direction and tone can become
/// configurable without touching the provider implementations.
///
/// `tone` is "casual" or anything else (treated as "professional", the
/// default) — a request-scoped choice, not a provider credential, which is
/// why it travels alongside `source_lang`/`target_lang` instead of living on
/// `ProviderConfig`. The popup can override it per-message (Tab) without
/// touching the persisted app-wide default.
///
/// `opts` carries what varies per request beyond the languages and tone, so
/// every provider builds the exact same prompt from it.
pub fn system_prompt(
    source_lang: &str,
    target_lang: &str,
    tone: &str,
    opts: &PromptOptions,
) -> String {
    let casual = tone == "casual";

    // Descriptive register words alone ("professional-but-conversational" vs
    // "casual, informal") measured as nearly indistinguishable in testing —
    // smaller local models in particular converged on ~identical output for
    // both. Explicit, opposite instructions about contractions/openers gave
    // a real, consistent difference instead (verified against gemma-4-26b-a4b
    // on several test phrases).
    let (audience, register) = if casual {
        (
            "friends or family",
            "a warm, casual register — like texting a close friend. Use contractions freely \
             (I'm, don't, can't, gonna, wanna), casual openers where natural (Hey, So), and \
             relaxed everyday phrasing",
        )
    } else {
        (
            "colleagues",
            "a professional register appropriate for workplace communication (Slack, email, \
             docs) — clear and polished. Avoid slangy contractions like \"gonna\"/\"wanna\" and \
             casual openers like \"Hey\"; use complete, precise phrasing, but keep it natural, \
             not stiff or overly formal. Everyday contractions (I'm, I'll, don't, haven't) are \
             natural in professional writing — use them",
        )
    };

    // English doesn't grammaticalize formality, so this is a no-op there —
    // but for the five other supported target languages it's the strongest
    // signal available (the same mechanism DeepL's formality toggle uses).
    let pronoun_note = tv_distinction(target_lang)
        .map(|(informal, formal)| {
            let pronoun = if casual { informal } else { formal };
            format!(" In {target_lang} specifically, address the reader as {pronoun}.")
        })
        .unwrap_or_default();

    // A single worked example beats a paragraph of description at getting
    // smaller/local models to actually shift register — few-shot examples
    // measurably outperform instruction-only prompting for this kind of
    // style-transfer task.
    let example = tone_example(target_lang, tone)
        .map(|phrase| format!("\n\nExample of this register in {target_lang}: {phrase}"))
        .unwrap_or_default();

    let dictation = if opts.from_voice { dictation_rules() } else { String::new() };
    let vocabulary = if opts.vocabulary.is_empty() {
        String::new()
    } else {
        format!(
            " Use these exact spellings for names and terms: {}.",
            opts.vocabulary.join(", ")
        )
    };

    format!(
        "You are a bilingual translator helping a {source_lang}-speaking person communicate \
         with {target_lang}-speaking {audience}. Translate the given {source_lang} text into \
         natural, fluent {target_lang} exactly as a native {target_lang} speaker would write \
         it, in {register}.{pronoun_note} Do not translate word-for-word — prioritize natural \
         phrasing and correct idioms. The input may be a short fragment or informal phrase \
         rather than a full sentence; translate accordingly without forcing unnatural \
         formality. Preserve code snippets, proper nouns, URLs, and technical terms unchanged. \
         This includes file names and paths (e.g. \"Archivo.tsx\", \"componente/Boton.tsx\", \
         \"config.json\") — never translate the file name itself or its extension, even if a \
         word inside it looks like an ordinary {source_lang} word, and never change its \
         capitalization either (\"Archivo.tsx\" must stay exactly \"Archivo.tsx\", not \
         \"archivo.tsx\" or \"File.tsx\") — file names are case-sensitive, so copy them through \
         byte-for-byte, casing included.{dictation}{vocabulary}{example} Output ONLY the \
         translated text — no quotes, no explanation, no preamble."
    )
}

/// Extra rules for input that came from the dictation button instead of the
/// keyboard: raw speech-to-text carries disfluencies a typed message doesn't.
fn dictation_rules() -> String {
    " The input is a raw speech-to-text transcript: drop filler words (eh, este, o sea, \
     bueno, tipo), false starts and repetitions; when the speaker corrects themselves \
     (\"no, perdón\", \"digo\"), keep only the corrected version."
        .to_string()
}

/// Per-request prompt additions that aren't part of the language pair or tone.
#[derive(Debug, Clone, Default)]
pub struct PromptOptions {
    /// The text came from speech-to-text, not the keyboard.
    pub from_voice: bool,
    /// Names and terms the translation must spell exactly.
    pub vocabulary: Vec<String>,
}

pub trait TranslationProvider {
    async fn translate(
        &self,
        text: &str,
        source_lang: &str,
        target_lang: &str,
        tone: &str,
        opts: &PromptOptions,
    ) -> Result<String, String>;
}

/// Which provider is active plus whatever credentials it needs. `base_url`
/// and `model` are only meaningful for `lmstudio` (the generic "Local LLM"
/// option); `api_key` is optional there (most local servers don't validate
/// it) but required for the three hosted providers.
#[derive(Debug, Clone)]
pub struct ProviderConfig {
    pub provider: String,
    pub api_key: Option<String>,
    pub base_url: Option<String>,
    pub model: Option<String>,
}

pub async fn translate(
    cfg: &ProviderConfig,
    text: &str,
    source_lang: &str,
    target_lang: &str,
    tone: &str,
    opts: &PromptOptions,
) -> Result<String, String> {
    match cfg.provider.as_str() {
        "anthropic" => {
            let key = cfg
                .api_key
                .clone()
                .ok_or_else(|| "Falta la API key de Anthropic. Abre Configuración.".to_string())?;
            AnthropicProvider::new(key)
                .translate(text, source_lang, target_lang, tone, opts)
                .await
        }
        "openai" => {
            let key = cfg
                .api_key
                .clone()
                .ok_or_else(|| "Falta la API key de OpenAI. Abre Configuración.".to_string())?;
            OpenAiProvider::new(key)
                .translate(text, source_lang, target_lang, tone, opts)
                .await
        }
        "gemini" => {
            let key = cfg
                .api_key
                .clone()
                .ok_or_else(|| "Falta la API key de Gemini. Abre Configuración.".to_string())?;
            GeminiProvider::new(key)
                .translate(text, source_lang, target_lang, tone, opts)
                .await
        }
        "lmstudio" => {
            let url = cfg
                .base_url
                .clone()
                .filter(|u| !u.trim().is_empty())
                .ok_or_else(|| "Falta la URL del servidor local. Abre Configuración.".to_string())?;
            LmStudioProvider::new(url, cfg.api_key.clone(), cfg.model.clone())
                .translate(text, source_lang, target_lang, tone, opts)
                .await
        }
        other => Err(format!("Proveedor desconocido: {other}")),
    }
}
