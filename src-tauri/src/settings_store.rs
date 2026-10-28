use std::path::Path;
use std::sync::Mutex;

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
    // its own global shortcut, and what is pasted: the translation
    // ("translate"), the transcript cleaned up in its own language
    // ("improve") or as spoken ("original") — see `parse_dictation_output`.
    #[serde(default = "default_dictation_shortcut")]
    pub dictation_shortcut: String,
    #[serde(default = "default_dictation_output")]
    pub dictation_output: String,
    // What "keep the language" means when `dictation_output` is not
    // "translate": "improve" or "original". Chosen in Settings only; the pill
    // toggles between "translate" and this. Old files without it get it from
    // `dictation_output` — see `fix_direct_output`.
    #[serde(default = "default_direct_output")]
    pub direct_output: String,
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

fn default_direct_output() -> String {
    "improve".to_string()
}

// Empty until the user sets one. Expected: a foreground script that starts
// the local STT/LLM servers and keeps running until killed, which is what
// lets services.rs stop it by signalling its process group.
fn default_autostart_command() -> String {
    String::new()
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
            direct_output: default_direct_output(),
        }
    }
}

fn settings_path(app: &AppHandle) -> Result<std::path::PathBuf, String> {
    let dir = app.path().app_config_dir().map_err(|e| e.to_string())?;
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    Ok(dir.join("settings.json"))
}

/// Canonical form of a `dictation_output` value ("translate", "improve" or
/// "original"), or `None` if it is not one. Callers that read a stored value
/// fall back to "translate".
pub fn parse_dictation_output(raw: &str) -> Option<&'static str> {
    ["translate", "improve", "original"].into_iter().find(|o| *o == raw)
}

/// Canonical form of a `direct_output` value: "improve" or "original".
pub fn parse_direct_output(raw: &str) -> Option<&'static str> {
    ["improve", "original"].into_iter().find(|o| *o == raw)
}

/// The pill chip's toggle: from "translate" (or an unknown stored value,
/// which counts as it) to the user's `direct` choice, from anything else back
/// to "translate".
pub fn toggled_dictation_output(current: &str, direct: &str) -> &'static str {
    match parse_dictation_output(current) {
        Some("improve") | Some("original") => "translate",
        _ => parse_direct_output(direct).unwrap_or("improve"),
    }
}

/// Settings' "when not translating" choice. While the dictation is in the
/// direct mode, the effective output follows it right away.
pub fn set_direct_output(settings: &mut AppSettings, direct: &str) -> Result<(), String> {
    let direct = parse_direct_output(direct)
        .ok_or_else(|| format!("Valor de salida inválido: {direct}"))?;
    settings.direct_output = direct.to_string();
    if matches!(parse_dictation_output(&settings.dictation_output), Some("improve" | "original")) {
        settings.dictation_output = direct.to_string();
    }
    Ok(())
}

/// Files written before `direct_output` existed (or with a bad value) take it
/// from `dictation_output`, so a user who had "original" keeps it as the direct
/// choice; anything else defaults to "improve".
fn fix_direct_output(settings: &mut AppSettings, stored: bool) {
    if stored && parse_direct_output(&settings.direct_output).is_some() {
        return;
    }
    settings.direct_output = parse_direct_output(&settings.dictation_output)
        .unwrap_or("improve")
        .to_string();
}

fn read_from(path: &Path) -> Result<AppSettings, String> {
    if !path.exists() {
        return Ok(AppSettings::default());
    }
    let raw = std::fs::read_to_string(path).map_err(|e| e.to_string())?;
    let value: serde_json::Value = serde_json::from_str(&raw).map_err(|e| e.to_string())?;
    let stored = value.get("direct_output").is_some();
    let mut settings: AppSettings = serde_json::from_value(value).map_err(|e| e.to_string())?;
    fix_direct_output(&mut settings, stored);
    Ok(settings)
}

/// Writes next to the target and renames over it, so a crash or a concurrent
/// reader never sees a half-written `settings.json`.
fn write_atomic(path: &Path, settings: &AppSettings) -> Result<(), String> {
    let raw = serde_json::to_string_pretty(settings).map_err(|e| e.to_string())?;
    let tmp = path.with_extension("json.tmp");
    std::fs::write(&tmp, raw).map_err(|e| e.to_string())?;
    std::fs::rename(&tmp, path).map_err(|e| {
        let _ = std::fs::remove_file(&tmp);
        e.to_string()
    })
}

pub fn load(app: &AppHandle) -> Result<AppSettings, String> {
    read_from(&settings_path(app)?)
}

/// Serializes every read-modify-write of `settings.json`: commands run on
/// different threads, and two unserialized saves would drop one of the edits.
static WRITE_LOCK: Mutex<()> = Mutex::new(());

fn update_at<F>(path: &Path, change: F) -> Result<AppSettings, String>
where
    F: FnOnce(&mut AppSettings) -> Result<(), String>,
{
    // The guarded data is `()`: a panic in an earlier closure leaves nothing
    // inconsistent, so a poisoned lock is safe to reuse.
    let _guard = WRITE_LOCK.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
    let mut settings = read_from(path)?;
    change(&mut settings)?;
    write_atomic(path, &settings)?;
    Ok(settings)
}

/// Loads the settings, applies `change` and persists the result, all under
/// one lock. If `change` fails nothing is written. Returns what was saved.
pub fn update<F>(app: &AppHandle, change: F) -> Result<AppSettings, String>
where
    F: FnOnce(&mut AppSettings) -> Result<(), String>,
{
    update_at(&settings_path(app)?, change)
}

#[cfg(test)]
mod tests {
    use super::*;

    const DEFAULT_COMMAND: &str = "";

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

    fn temp_settings_path(name: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("typelang-settings-{}-{name}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        dir.join("settings.json")
    }

    #[test]
    fn parse_dictation_output_accepts_the_three_modes() {
        assert_eq!(parse_dictation_output("translate"), Some("translate"));
        assert_eq!(parse_dictation_output("improve"), Some("improve"));
        assert_eq!(parse_dictation_output("original"), Some("original"));
    }

    #[test]
    fn parse_dictation_output_rejects_anything_else() {
        for raw in ["", "Translate", "improved", "raw", " original"] {
            assert_eq!(parse_dictation_output(raw), None, "{raw:?}");
        }
    }

    #[test]
    fn parse_direct_output_accepts_only_improve_and_original() {
        assert_eq!(parse_direct_output("improve"), Some("improve"));
        assert_eq!(parse_direct_output("original"), Some("original"));
        for raw in ["translate", "", "Improve"] {
            assert_eq!(parse_direct_output(raw), None, "{raw:?}");
        }
    }

    #[test]
    fn toggle_goes_between_translate_and_the_direct_choice() {
        assert_eq!(toggled_dictation_output("translate", "improve"), "improve");
        assert_eq!(toggled_dictation_output("translate", "original"), "original");
        assert_eq!(toggled_dictation_output("improve", "improve"), "translate");
        assert_eq!(toggled_dictation_output("original", "original"), "translate");
        // Never lands on a third state: the toggle is two-way.
        assert_eq!(toggled_dictation_output("improve", "original"), "translate");
        // An unknown stored output counts as "translate"; a bad direct choice
        // falls back to "improve".
        assert_eq!(toggled_dictation_output("garbage", "original"), "original");
        assert_eq!(toggled_dictation_output("translate", "garbage"), "improve");
    }

    #[test]
    fn set_direct_output_validates_and_follows_only_in_direct_mode() {
        let mut s = AppSettings::default();
        assert!(set_direct_output(&mut s, "translate").is_err());
        assert!(set_direct_output(&mut s, "x").is_err());
        assert_eq!(s.direct_output, "improve");

        // Translating: the effective output stays.
        set_direct_output(&mut s, "original").unwrap();
        assert_eq!(s.direct_output, "original");
        assert_eq!(s.dictation_output, "translate");

        // Direct mode: the effective output follows.
        s.dictation_output = "original".to_string();
        set_direct_output(&mut s, "improve").unwrap();
        assert_eq!(s.direct_output, "improve");
        assert_eq!(s.dictation_output, "improve");
    }

    fn load_json(raw: &str) -> AppSettings {
        use std::sync::atomic::{AtomicUsize, Ordering};
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let path = temp_settings_path(&format!("json-{}", NEXT.fetch_add(1, Ordering::Relaxed)));
        std::fs::write(&path, raw).unwrap();
        read_from(&path).unwrap()
    }

    #[test]
    fn old_settings_without_direct_output_derive_it() {
        assert_eq!(load_json(r#"{"dictation_output":"original"}"#).direct_output, "original");
        assert_eq!(load_json(r#"{"dictation_output":"improve"}"#).direct_output, "improve");
        assert_eq!(load_json(r#"{"dictation_output":"translate"}"#).direct_output, "improve");
        assert_eq!(load_json("{}").direct_output, "improve");
        // The effective output itself is untouched.
        assert_eq!(load_json(r#"{"dictation_output":"original"}"#).dictation_output, "original");
    }

    #[test]
    fn stored_direct_output_wins_and_a_bad_one_is_derived() {
        let s = load_json(r#"{"dictation_output":"translate","direct_output":"original"}"#);
        assert_eq!(s.direct_output, "original");
        let s = load_json(r#"{"dictation_output":"original","direct_output":"nope"}"#);
        assert_eq!(s.direct_output, "original");
    }

    #[test]
    fn update_persists_and_round_trips() {
        let path = temp_settings_path("round-trip");
        let _ = std::fs::remove_file(&path);

        let saved = update_at(&path, |s| {
            s.dictation_output = "improve".to_string();
            s.vocabulary = "Welcomy".to_string();
            Ok(())
        })
        .unwrap();
        assert_eq!(saved.dictation_output, "improve");

        let loaded = read_from(&path).unwrap();
        assert_eq!(loaded.dictation_output, "improve");
        assert_eq!(loaded.vocabulary, "Welcomy");
        assert_eq!(loaded.provider, AppSettings::default().provider);
        assert!(!path.with_extension("json.tmp").exists(), "temp file left behind");
    }

    #[test]
    fn update_that_fails_writes_nothing() {
        let path = temp_settings_path("failed");
        let _ = std::fs::remove_file(&path);

        let result = update_at(&path, |s| {
            s.tone = "casual".to_string();
            Err("nope".to_string())
        });
        assert_eq!(result.unwrap_err(), "nope");
        assert!(!path.exists());
    }

    #[test]
    fn concurrent_updates_do_not_lose_edits() {
        let path = temp_settings_path("concurrent");
        let _ = std::fs::remove_file(&path);

        let a = path.clone();
        let b = path.clone();
        let first = std::thread::spawn(move || {
            for i in 0..25 {
                update_at(&a, |s| {
                    s.vocabulary = format!("v{i}");
                    Ok(())
                })
                .unwrap();
            }
        });
        let second = std::thread::spawn(move || {
            for i in 0..25 {
                update_at(&b, |s| {
                    s.local_model = format!("m{i}");
                    Ok(())
                })
                .unwrap();
            }
        });
        first.join().unwrap();
        second.join().unwrap();

        let loaded = read_from(&path).unwrap();
        assert_eq!(loaded.vocabulary, "v24");
        assert_eq!(loaded.local_model, "m24");
    }

    #[test]
    fn stored_output_values_still_load() {
        for output in ["translate", "improve", "original"] {
            let raw = format!(r#"{{"dictation_output":"{output}"}}"#);
            let s: AppSettings = serde_json::from_str(&raw).unwrap();
            assert_eq!(s.dictation_output, output);
        }
    }
}
