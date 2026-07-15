use tauri::{
    menu::{Menu, MenuItem},
    tray::TrayIconBuilder,
    AppHandle, Manager,
};

use crate::{
    settings_store, shortcut,
    state::{AppState, TrayMenuItems},
    window,
};

/// Tray menu strings aren't worth pulling in the frontend's full i18n
/// dictionary for — just the three labels here, in the two languages the
/// app supports. Defaults to English for anything other than "es", same as
/// `settings_store::default_ui_language`.
fn labels(ui_language: &str) -> (&'static str, &'static str, &'static str) {
    if ui_language == "es" {
        ("Traducir ahora", "Configuración…", "Salir")
    } else {
        ("Translate now", "Settings…", "Quit")
    }
}

pub fn setup(app: &AppHandle) -> tauri::Result<()> {
    let ui_language = settings_store::load(app)
        .map(|s| s.ui_language)
        .unwrap_or_else(|_| "en".to_string());
    let (translate_label, settings_label, quit_label) = labels(&ui_language);

    let translate_item =
        MenuItem::with_id(app, "translate", translate_label, true, None::<&str>)?;
    let settings_item = MenuItem::with_id(app, "settings", settings_label, true, None::<&str>)?;
    let quit_item = MenuItem::with_id(app, "quit", quit_label, true, None::<&str>)?;
    let menu = Menu::with_items(app, &[&translate_item, &settings_item, &quit_item])?;

    if let Some(state) = app.try_state::<AppState>() {
        *state.tray_items.lock().unwrap() = Some(TrayMenuItems {
            translate: translate_item,
            settings: settings_item,
            quit: quit_item,
        });
    }

    TrayIconBuilder::new()
        .icon(app.default_window_icon().unwrap().clone())
        .menu(&menu)
        .show_menu_on_left_click(true)
        .on_menu_event(|app, event| match event.id.as_ref() {
            "translate" => shortcut::trigger_translate(app),
            "settings" => window::show_settings(app),
            "quit" => app.exit(0),
            _ => {}
        })
        .build(app)?;

    Ok(())
}

/// Relabels the existing tray menu items in place — called from
/// `save_ui_language` so switching the app language in Settings updates the
/// tray immediately, not just on next launch.
pub fn update_language(app: &AppHandle, ui_language: &str) {
    let (translate_label, settings_label, quit_label) = labels(ui_language);
    if let Some(state) = app.try_state::<AppState>() {
        if let Some(items) = state.tray_items.lock().unwrap().as_ref() {
            let _ = items.translate.set_text(translate_label);
            let _ = items.settings.set_text(settings_label);
            let _ = items.quit.set_text(quit_label);
        }
    }
}
