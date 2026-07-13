use tauri::{
    AppHandle, Emitter, Manager, PhysicalPosition, WebviewUrl, WebviewWindow,
    WebviewWindowBuilder, WindowEvent,
};

pub const POPUP_LABEL: &str = "popup";
pub const SETTINGS_LABEL: &str = "main";

/// Shows the floating translate popup, creating it on first use. Reused on
/// subsequent shortcut presses instead of rebuilt, and reset via a JS event
/// so the input is always empty when it appears.
pub fn show_popup(app: &AppHandle) -> tauri::Result<()> {
    if let Some(window) = app.get_webview_window(POPUP_LABEL) {
        center_on_active_monitor(app, &window);
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

    center_on_active_monitor(app, &window);
    window.set_focus()?;
    Ok(())
}

/// Centers `window` on whichever monitor currently has the cursor. The
/// window is built once and reused across shortcut presses (see
/// `show_popup` above), so without this it stays pinned to whatever monitor
/// it first appeared on — wrong as soon as the user switches screens on a
/// multi-monitor setup. Falls back to the primary monitor if the cursor's
/// monitor can't be resolved, and is a no-op if neither is available.
fn center_on_active_monitor(app: &AppHandle, window: &WebviewWindow) {
    let monitor = app
        .cursor_position()
        .ok()
        .and_then(|cursor| app.monitor_from_point(cursor.x, cursor.y).ok().flatten())
        .or_else(|| app.primary_monitor().ok().flatten());

    let (Some(monitor), Ok(size)) = (monitor, window.outer_size()) else {
        return;
    };

    let area = monitor.work_area();
    let x = area.position.x + (area.size.width as i32 - size.width as i32) / 2;
    let y = area.position.y + (area.size.height as i32 - size.height as i32) / 2;
    let _ = window.set_position(PhysicalPosition::new(x, y));
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
