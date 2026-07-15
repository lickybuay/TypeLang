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
}

fn default_provider() -> String {
    "anthropic".to_string()
}

fn default_ui_language() -> String {
    "en".to_string()
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
