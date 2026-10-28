use std::time::{Duration, Instant};

use tauri::{AppHandle, Manager};

use crate::{
    focus::{self, FocusedApp},
    state::{AppState, CapturedTarget},
    window,
};

/// How long a previously captured app stays a valid target when the shortcut
/// is pressed while TypeLang itself is frontmost.
const TARGET_MAX_AGE: Duration = Duration::from_secs(10 * 60);

/// Decides the paste target after a fresh capture. TypeLang itself is never an
/// app target (restoring it would send an Apple Event to our own process):
/// with the Settings window focused it becomes `SelfSettings`, otherwise the
/// previous app is kept while it is recent enough, else there is no target.
fn next_target(
    prev: Option<CapturedTarget>,
    captured: FocusedApp,
    settings_focused: bool,
    now: Instant,
) -> Option<CapturedTarget> {
    if !captured.is_self() {
        return Some(CapturedTarget::App { app: captured, at: now });
    }
    if settings_focused {
        return Some(CapturedTarget::SelfSettings);
    }
    match prev {
        Some(CapturedTarget::App { at, .. }) if now.saturating_duration_since(at) >= TARGET_MAX_AGE => None,
        other => other,
    }
}

/// Captures the frontmost app and stores the resulting paste target. Returns
/// false when the capture failed, in which case nothing should be shown.
fn capture_target(app: &AppHandle) -> bool {
    match focus::capture() {
        Ok(captured) => {
            let settings_focused = window::settings_focused(app);
            let state = app.state::<AppState>();
            if let Ok(mut guard) = state.captured_focus.lock() {
                *guard = next_target(guard.take(), captured, settings_focused, Instant::now());
            }
            true
        }
        Err(e) => {
            eprintln!("[shortcut] failed to capture focus: {e}");
            false
        }
    }
}

/// Shared entry point for starting a translation: capture whatever app
/// currently has focus, stash it in app state, then show the popup. Called
/// both from the global shortcut handler and the tray's "Traducir ahora"
/// menu item.
pub fn trigger_translate(app: &AppHandle) {
    if capture_target(app) {
        if let Err(e) = window::show_popup(app) {
            eprintln!("[shortcut] failed to show popup: {e}");
        }
    }
}

/// Same capture-then-show pattern as `trigger_translate`, but opens the
/// snippets picker instead — its own global shortcut (see
/// `settings_store::AppSettings::snippet_shortcut`), independent of the
/// translate one.
pub fn trigger_snippets(app: &AppHandle) {
    if capture_target(app) {
        if let Err(e) = window::show_snippets(app) {
            eprintln!("[shortcut] failed to show snippets: {e}");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{Duration, Instant};

    fn app(name: &str) -> FocusedApp {
        FocusedApp {
            name: name.to_string(),
            bundle_id: None,
            pid: Some(std::process::id() as i32 + 1),
        }
    }

    fn own() -> FocusedApp {
        FocusedApp {
            name: "TypeLang".to_string(),
            bundle_id: None,
            pid: Some(std::process::id() as i32),
        }
    }

    fn prev_app(name: &str, at: Instant) -> Option<CapturedTarget> {
        Some(CapturedTarget::App { app: app(name), at })
    }

    #[test]
    fn other_app_becomes_the_target() {
        let now = Instant::now();
        let next = next_target(prev_app("Old", now), app("Mail"), false, now);
        assert!(matches!(next, Some(CapturedTarget::App { app, at }) if app.name == "Mail" && at == now));
    }

    #[test]
    fn self_with_settings_focused_targets_settings() {
        let now = Instant::now();
        let next = next_target(prev_app("Mail", now), own(), true, now);
        assert!(matches!(next, Some(CapturedTarget::SelfSettings)));
    }

    #[test]
    fn self_keeps_a_fresh_previous_target() {
        let at = Instant::now();
        let now = at + Duration::from_secs(60);
        let next = next_target(prev_app("Mail", at), own(), false, now);
        assert!(matches!(next, Some(CapturedTarget::App { app, at: kept }) if app.name == "Mail" && kept == at));
    }

    #[test]
    fn self_drops_a_stale_previous_target() {
        let at = Instant::now();
        let now = at + TARGET_MAX_AGE + Duration::from_secs(1);
        assert!(next_target(prev_app("Mail", at), own(), false, now).is_none());
    }

    #[test]
    fn self_without_previous_target_has_none() {
        assert!(next_target(None, own(), false, Instant::now()).is_none());
    }
}
