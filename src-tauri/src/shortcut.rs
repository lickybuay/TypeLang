use tauri::{AppHandle, Manager};

use crate::{focus, state::AppState, window};

/// Shared entry point for starting a translation: capture whatever app
/// currently has focus, stash it in app state, then show the popup. Called
/// both from the global shortcut handler and the tray's "Traducir ahora"
/// menu item.
pub fn trigger_translate(app: &AppHandle) {
    match focus::capture() {
        Ok(captured) => {
            let state = app.state::<AppState>();
            if let Ok(mut guard) = state.captured_focus.lock() {
                *guard = Some(captured);
            }
            if let Err(e) = window::show_popup(app) {
                eprintln!("[shortcut] failed to show popup: {e}");
            }
        }
        Err(e) => eprintln!("[shortcut] failed to capture focus: {e}"),
    }
}

/// Same capture-then-show pattern as `trigger_translate`, but opens the
/// snippets picker instead — its own global shortcut (see
/// `settings_store::AppSettings::snippet_shortcut`), independent of the
/// translate one.
pub fn trigger_snippets(app: &AppHandle) {
    match focus::capture() {
        Ok(captured) => {
            let state = app.state::<AppState>();
            if let Ok(mut guard) = state.captured_focus.lock() {
                *guard = Some(captured);
            }
            if let Err(e) = window::show_snippets(app) {
                eprintln!("[shortcut] failed to show snippets: {e}");
            }
        }
        Err(e) => eprintln!("[shortcut] failed to capture focus: {e}"),
    }
}
