use tauri::{AppHandle, Emitter, Manager, WebviewUrl, WebviewWindowBuilder, WindowEvent};

pub const POPUP_LABEL: &str = "popup";
pub const SETTINGS_LABEL: &str = "main";

/// Shows the floating translate popup, creating it on first use. Reused on
/// subsequent shortcut presses instead of rebuilt, and reset via a JS event
/// so the input is always empty when it appears.
pub fn show_popup(app: &AppHandle) -> tauri::Result<()> {
    if let Some(window) = app.get_webview_window(POPUP_LABEL) {
        window.show()?;
        window.set_focus()?;
        window.emit("popup-reset", ())?;
        return Ok(());
    }

    let window = WebviewWindowBuilder::new(app, POPUP_LABEL, WebviewUrl::App("index.html#/popup".into()))
        .title("TypeLang")
        .inner_size(480.0, 206.0)
        .decorations(false)
        .transparent(true)
        .shadow(false)
        .always_on_top(true)
        .skip_taskbar(true)
        .resizable(false)
        .center()
        .visible(true)
        .build()?;

    // Hiding (not closing) on blur is what makes "Escape/blur to dismiss"
    // work without losing the webview state between invocations.
    let hide_on_blur = window.clone();
    window.on_window_event(move |event| {
        if let WindowEvent::Focused(false) = event {
            let _ = hide_on_blur.hide();
        }
    });

    window.set_focus()?;
    Ok(())
}

pub fn hide_popup(app: &AppHandle) {
    if let Some(window) = app.get_webview_window(POPUP_LABEL) {
        let _ = window.hide();
    }
}

pub fn show_settings(app: &AppHandle) {
    if let Some(window) = app.get_webview_window(SETTINGS_LABEL) {
        let _ = window.show();
        let _ = window.set_focus();
    }
}
