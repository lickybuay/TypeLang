use crate::{audio, state::AppState};
use tauri::{
    AppHandle, Emitter, Manager, Monitor, PhysicalPosition, WebviewUrl, WebviewWindow,
    WebviewWindowBuilder, WindowEvent,
};

pub const POPUP_LABEL: &str = "popup";
pub const SETTINGS_LABEL: &str = "main";
pub const SNIPPETS_LABEL: &str = "snippets";
pub const PILL_LABEL: &str = "pill";

// Panel class of the recording pill: it may never become the key or main
// window, which is what keeps a click on it from activating TypeLang.
#[cfg(target_os = "macos")]
tauri_nspanel::tauri_panel! {
    panel!(PillPanel {
        config: {
            can_become_key_window: false,
            can_become_main_window: false,
            is_floating_panel: true
        }
    })
}

// The recording pill; the capsule in Pill.tsx fills the window exactly.
const PILL_WIDTH: f64 = 300.0;
const PILL_HEIGHT: f64 = 52.0;
/// Gap between the pill and the bottom of the monitor's work area (above the
/// Dock).
const PILL_BOTTOM_MARGIN: f64 = 16.0;

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
            // The mic must not stay open behind a hidden window.
            audio::cancel(&hide_on_blur.app_handle().state::<AppState>());
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
    let (Some(monitor), Ok(size)) = (active_monitor(app), window.outer_size()) else {
        return;
    };

    let area = monitor.work_area();
    let x = area.position.x + (area.size.width as i32 - size.width as i32) / 2;
    let y = area.position.y + (area.size.height as i32 - size.height as i32) / 2;
    let _ = window.set_position(PhysicalPosition::new(x, y));
}

/// The monitor under the cursor, else the primary one.
fn active_monitor(app: &AppHandle) -> Option<Monitor> {
    app.cursor_position()
        .ok()
        .and_then(|cursor| app.monitor_from_point(cursor.x, cursor.y).ok().flatten())
        .or_else(|| app.primary_monitor().ok().flatten())
}

pub fn hide_popup(app: &AppHandle) {
    if let Some(window) = app.get_webview_window(POPUP_LABEL) {
        let _ = window.hide();
    }
}

/// Same reuse/reset/reposition pattern as `show_popup` — a second small
/// floating window, this one listing saved snippets instead of taking new
/// text to translate.
pub fn show_snippets(app: &AppHandle) -> tauri::Result<()> {
    if let Some(window) = app.get_webview_window(SNIPPETS_LABEL) {
        center_on_active_monitor(app, &window);
        window.show()?;
        window.set_focus()?;
        window.emit("snippets-reset", ())?;
        return Ok(());
    }

    let window = WebviewWindowBuilder::new(
        app,
        SNIPPETS_LABEL,
        WebviewUrl::App("index.html#/snippets".into()),
    )
    .title("TypeLang")
    .inner_size(320.0, 360.0)
    .decorations(false)
    .transparent(true)
    .shadow(false)
    .always_on_top(true)
    .skip_taskbar(true)
    .resizable(false)
    .visible(true)
    .build()?;

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

pub fn hide_snippets(app: &AppHandle) {
    if let Some(window) = app.get_webview_window(SNIPPETS_LABEL) {
        let _ = window.hide();
    }
}

/// True on the main thread. The pill's NSPanel may only be touched there; off
/// it the pill functions do nothing (and say so once) instead of calling into
/// AppKit.
#[cfg(target_os = "macos")]
fn on_main_thread() -> bool {
    use std::sync::atomic::{AtomicBool, Ordering};
    static WARNED: AtomicBool = AtomicBool::new(false);

    if tauri_nspanel::objc2::MainThreadMarker::new().is_some() {
        return true;
    }
    if !WARNED.swap(true, Ordering::Relaxed) {
        eprintln!("[window] the recording pill was used off the main thread; ignored");
    }
    false
}

/// Options shared by the macOS panel and the plain window: borderless,
/// transparent, always on top and never focused.
fn pill_window_options<'a>(
    builder: WebviewWindowBuilder<'a, tauri::Wry, AppHandle>,
) -> WebviewWindowBuilder<'a, tauri::Wry, AppHandle> {
    builder
        .title("TypeLang")
        .inner_size(PILL_WIDTH, PILL_HEIGHT)
        .decorations(false)
        .transparent(true)
        .shadow(false)
        .always_on_top(true)
        .skip_taskbar(true)
        .resizable(false)
        .focused(false)
        .focusable(false)
        .visible_on_all_workspaces(true)
        .accept_first_mouse(true)
        .visible(false)
}

/// Creates the recording pill hidden if it does not exist yet. It must never
/// take focus (the target app keeps it) and its buttons must work without
/// activating TypeLang, so on macOS it is a non-activating NSPanel that cannot
/// become key or main, floats at status level on every Space (full-screen
/// ones included), and is shown and hidden through the panel API only.
/// Main thread only (a no-op elsewhere on macOS).
pub fn ensure_pill(app: &AppHandle) -> tauri::Result<()> {
    if app.get_webview_window(PILL_LABEL).is_some() {
        return Ok(());
    }

    #[cfg(target_os = "macos")]
    {
        use tauri_nspanel::{CollectionBehavior, PanelBuilder, PanelLevel, StyleMask};

        if !on_main_thread() {
            return Ok(());
        }
        PanelBuilder::<_, PillPanel>::new(app, PILL_LABEL)
            .url(WebviewUrl::App("index.html#/pill".into()))
            .level(PanelLevel::Status)
            .collection_behavior(
                CollectionBehavior::new()
                    .can_join_all_spaces()
                    .full_screen_auxiliary()
                    .stationary()
                    .ignores_cycle(),
            )
            .add_style_mask(StyleMask::empty().nonactivating_panel())
            .hides_on_deactivate(false)
            .has_shadow(false)
            .transparent(true)
            .with_window(pill_window_options)
            .build()?;
    }
    #[cfg(not(target_os = "macos"))]
    pill_window_options(WebviewWindowBuilder::new(
        app,
        PILL_LABEL,
        WebviewUrl::App("index.html#/pill".into()),
    ))
    .build()?;

    Ok(())
}

/// Moves the pill to the bottom center of the monitor under the cursor and
/// shows it without focusing it. Main thread only.
pub fn show_pill(app: &AppHandle) -> tauri::Result<()> {
    #[cfg(target_os = "macos")]
    if !on_main_thread() {
        return Ok(());
    }
    let Some(window) = app.get_webview_window(PILL_LABEL) else {
        return Ok(());
    };
    place_pill(app, &window);

    #[cfg(target_os = "macos")]
    {
        use tauri_nspanel::ManagerExt;
        // orderFrontRegardless: on screen, never key, never activating.
        if let Ok(panel) = app.get_webview_panel(PILL_LABEL) {
            panel.show();
        }
        Ok(())
    }
    #[cfg(not(target_os = "macos"))]
    window.show()
}

fn place_pill(app: &AppHandle, window: &WebviewWindow) {
    let (Some(monitor), Ok(size)) = (active_monitor(app), window.outer_size()) else {
        return;
    };

    let area = monitor.work_area();
    let margin = (PILL_BOTTOM_MARGIN * monitor.scale_factor()).round() as i32;
    let x = area.position.x + (area.size.width as i32 - size.width as i32) / 2;
    let y = area.position.y + area.size.height as i32 - size.height as i32 - margin;
    let _ = window.set_position(PhysicalPosition::new(x, y));
}

/// Hides the pill (orderOut on macOS). Main thread only.
pub fn hide_pill(app: &AppHandle) {
    #[cfg(target_os = "macos")]
    {
        use tauri_nspanel::ManagerExt;
        if !on_main_thread() {
            return;
        }
        if let Ok(panel) = app.get_webview_panel(PILL_LABEL) {
            panel.hide();
        }
    }
    #[cfg(not(target_os = "macos"))]
    if let Some(window) = app.get_webview_window(PILL_LABEL) {
        let _ = window.hide();
    }
}

/// Whether the pill is on screen. Main thread only: false elsewhere on macOS.
pub fn pill_visible(app: &AppHandle) -> bool {
    #[cfg(target_os = "macos")]
    {
        use tauri_nspanel::ManagerExt;
        if !on_main_thread() {
            return false;
        }
        app.get_webview_panel(PILL_LABEL)
            .is_ok_and(|panel| panel.is_visible())
    }
    #[cfg(not(target_os = "macos"))]
    app.get_webview_window(PILL_LABEL)
        .is_some_and(|w| is_visible(&w))
}

pub fn show_settings(app: &AppHandle) {
    if let Some(window) = app.get_webview_window(SETTINGS_LABEL) {
        let _ = window.show();
        let _ = window.set_focus();
    }
}

fn is_visible(window: &WebviewWindow) -> bool {
    matches!(window.is_visible(), Ok(true))
}

pub fn settings_visible(app: &AppHandle) -> bool {
    app.get_webview_window(SETTINGS_LABEL)
        .is_some_and(|w| is_visible(&w))
}

pub fn settings_focused(app: &AppHandle) -> bool {
    app.get_webview_window(SETTINGS_LABEL)
        .is_some_and(|w| is_visible(&w) && matches!(w.is_focused(), Ok(true)))
}

/// After the popup/snippets window is dismissed with Escape, TypeLang (an
/// accessory app) can stay the active app and swallow the next shortcut.
/// Hiding the app hands activation back to the previous one. Skipped while
/// Settings is open, and never used on blur-hide (another app already took
/// focus there). Also skipped while the recording pill is up: hiding the app
/// would hide the pill with it.
pub fn release_activation(app: &AppHandle) {
    if settings_visible(app) || pill_visible(app) {
        return;
    }
    #[cfg(target_os = "macos")]
    if let Err(e) = app.hide() {
        eprintln!("[window] failed to hide app: {e}");
    }
}
