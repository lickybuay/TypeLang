use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager};

/// Non-secret preferences (provider, LM Studio's local server URL, UI
/// language, translation direction, global shortcut). API keys themselves
/// live in the OS keychain — see `keychain.rs` — never here.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppSettings {
    #[serde(default = "default_provider")]
    pub provider: String,
    #[serde(default)]
    pub lmstudio_base_url: String,
    // Local servers vary in whether they honor this: LM Studio ignores it
    // (serves whatever's loaded in its GUI), but Ollama routes on it and
    // 404s if it doesn't exactly match an `ollama pull`ed tag.
    #[serde(default)]
    pub local_model: String,
    #[serde(default = "default_ui_language")]
    pub ui_language: String,
    #[serde(default = "default_source_lang")]
    pub source_lang: String,
    #[serde(default = "default_target_lang")]
    pub target_lang: String,
    #[serde(default = "default_shortcut")]
    pub shortcut: String,
    // App-wide default; the popup can override it per-message (Tab) without
    // touching this — see llm::system_prompt and commands/translate.rs.
    #[serde(default = "default_tone")]
    pub tone: String,
    // Opens the snippets picker (see window::show_snippets) — a second,
    // independent global shortcut from `shortcut` above.
    #[serde(default = "default_snippet_shortcut")]
    pub snippet_shortcut: String,
    // Optional: launch the user's local AI services (whisper + llama-server)
    // with TypeLang and stop them on quit — see services.rs.
    #[serde(default)]
    pub autostart_enabled: bool,
    #[serde(default = "default_autostart_command")]
    pub autostart_command: String,
    // Dictation: local OpenAI-compatible STT server, plus names/terms the
    // transcriber and the translation prompt should spell exactly. The
    // vocabulary is kept as the raw text from Settings (one term per line or
    // comma-separated) — see `vocabulary_terms`.
    #[serde(default = "default_stt_base_url")]
    pub stt_base_url: String,
    #[serde(default)]
    pub vocabulary: String,
    // Hold-to-talk dictation without the popup (see dictation_hotkey.rs):
    // its own global shortcut, and whether the transcript is translated
    // ("translate") or pasted as spoken ("original").
    #[serde(default = "default_dictation_shortcut")]
    pub dictation_shortcut: String,
    #[serde(default = "default_dictation_output")]
    pub dictation_output: String,
}

impl AppSettings {
    pub fn vocabulary_terms(&self) -> Vec<String> {
        self.vocabulary
            .split(['\n', ','])
            .map(str::trim)
            .filter(|t| !t.is_empty())
            .map(str::to_string)
            .collect()
    }
}

fn default_provider() -> String {
    "anthropic".to_string()
}

fn default_ui_language() -> String {
    "en".to_string()
}

fn default_tone() -> String {
    "professional".to_string()
}

fn default_source_lang() -> String {
    "Spanish".to_string()
}

fn default_target_lang() -> String {
    "English".to_string()
}

// Cmd+Shift+T collides with Chrome's "reopen closed tab" — pick a default
// that's unlikely to already be taken by another app.
fn default_shortcut() -> String {
    "Alt+Shift+T".to_string()
}

// Same letter-swap convention as the translate shortcut above (T -> S),
// easy to remember as "the other one", equally unlikely to collide.
fn default_snippet_shortcut() -> String {
    "Alt+Shift+S".to_string()
}

// Third letter-swap (T -> S -> D, "dictate"), same collision reasoning.
fn default_dictation_shortcut() -> String {
    "Alt+Shift+D".to_string()
}

fn default_dictation_output() -> String {
    "translate".to_string()
}

// Foreground script that starts whisper (:8178) and llama-server (:8091) and
// keeps running until killed, which is what lets services.rs stop it by
// signalling its process group.
fn default_autostart_command() -> String {
    "zsh ~/UTILS/local_llm.sh 4".to_string()
}

fn default_stt_base_url() -> String {
    crate::stt::DEFAULT_BASE_URL.to_string()
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            provider: default_provider(),
            lmstudio_base_url: String::new(),
            local_model: String::new(),
            ui_language: default_ui_language(),
            source_lang: default_source_lang(),
            target_lang: default_target_lang(),
            shortcut: default_shortcut(),
            tone: default_tone(),
            snippet_shortcut: default_snippet_shortcut(),
            autostart_enabled: false,
            autostart_command: default_autostart_command(),
            stt_base_url: default_stt_base_url(),
            vocabulary: String::new(),
            dictation_shortcut: default_dictation_shortcut(),
            dictation_output: default_dictation_output(),
        }
    }
}

fn settings_path(app: &AppHandle) -> Result<std::path::PathBuf, String> {
    let dir = app.path().app_config_dir().map_err(|e| e.to_string())?;
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    Ok(dir.join("settings.json"))
}

pub fn load(app: &AppHandle) -> Result<AppSettings, String> {
    let path = settings_path(app)?;
    if !path.exists() {
        return Ok(AppSettings::default());
    }
    let raw = std::fs::read_to_string(&path).map_err(|e| e.to_string())?;
    serde_json::from_str(&raw).map_err(|e| e.to_string())
}

pub fn save(app: &AppHandle, settings: &AppSettings) -> Result<(), String> {
    let path = settings_path(app)?;
    let raw = serde_json::to_string_pretty(settings).map_err(|e| e.to_string())?;
    std::fs::write(&path, raw).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    const DEFAULT_COMMAND: &str = "zsh ~/UTILS/local_llm.sh 4";

    #[test]
    fn default_has_autostart_off_and_default_command() {
        let s = AppSettings::default();
        assert!(!s.autostart_enabled);
        assert_eq!(s.autostart_command, DEFAULT_COMMAND);
        assert_eq!(s.snippet_shortcut, "Alt+Shift+S");
    }

    #[test]
    fn old_settings_json_gets_new_field_defaults() {
        let old = r#"{"provider":"lmstudio","lmstudio_base_url":"http://localhost:8091","shortcut":"Alt+Shift+T"}"#;
        let s: AppSettings = serde_json::from_str(old).unwrap();
        assert_eq!(s.provider, "lmstudio");
        assert!(!s.autostart_enabled);
        assert_eq!(s.autostart_command, DEFAULT_COMMAND);
        assert_eq!(s.snippet_shortcut, "Alt+Shift+S");
    }

    #[test]
    fn empty_object_deserializes_to_defaults() {
        let s: AppSettings = serde_json::from_str("{}").unwrap();
        assert!(!s.autostart_enabled);
        assert_eq!(s.autostart_command, DEFAULT_COMMAND);
    }

    #[test]
    fn autostart_fields_round_trip() {
        let s = AppSettings {
            autostart_enabled: true,
            autostart_command: "zsh ~/custom.sh 8".to_string(),
            ..AppSettings::default()
        };
        let raw = serde_json::to_string(&s).unwrap();
        let back: AppSettings = serde_json::from_str(&raw).unwrap();
        assert!(back.autostart_enabled);
        assert_eq!(back.autostart_command, "zsh ~/custom.sh 8");
    }
}
