use tauri::{
    menu::{CheckMenuItem, Menu, MenuItem, Submenu},
    tray::TrayIconBuilder,
    AppHandle, Manager,
};

use crate::{
    settings_store, shortcut,
    state::{AppState, TrayMenuItems, TrayToneItems},
    window,
};

/// Tray menu strings aren't worth pulling in the frontend's full i18n
/// dictionary for — just the labels here, in the two languages the app
/// supports. Defaults to English for anything other than "es", same as
/// `settings_store::default_ui_language`.
fn labels(ui_language: &str) -> (&'static str, &'static str, &'static str, &'static str) {
    if ui_language == "es" {
        ("Traducir ahora", "Configuración…", "Salir", "Tono")
    } else {
        ("Translate now", "Settings…", "Quit", "Tone")
    }
}

fn tone_labels(ui_language: &str) -> (&'static str, &'static str) {
    if ui_language == "es" {
        ("Profesional", "Casual")
    } else {
        ("Professional", "Casual")
    }
}

pub fn setup(app: &AppHandle) -> tauri::Result<()> {
    let settings = settings_store::load(app).unwrap_or_default();
    let (translate_label, settings_label, quit_label, tone_menu_label) =
        labels(&settings.ui_language);
    let (professional_label, casual_label) = tone_labels(&settings.ui_language);
    let is_casual = settings.tone == "casual";

    let translate_item =
        MenuItem::with_id(app, "translate", translate_label, true, None::<&str>)?;
    let settings_item = MenuItem::with_id(app, "settings", settings_label, true, None::<&str>)?;

    let professional_item = CheckMenuItem::with_id(
        app,
        "tone-professional",
        professional_label,
        true,
        !is_casual,
        None::<&str>,
    )?;
    let casual_item = CheckMenuItem::with_id(
        app,
        "tone-casual",
        casual_label,
        true,
        is_casual,
        None::<&str>,
    )?;
    let tone_submenu = Submenu::with_items(
        app,
        tone_menu_label,
        true,
        &[&professional_item, &casual_item],
    )?;

    let quit_item = MenuItem::with_id(app, "quit", quit_label, true, None::<&str>)?;
    let menu = Menu::with_items(
        app,
        &[&translate_item, &tone_submenu, &settings_item, &quit_item],
    )?;

    if let Some(state) = app.try_state::<AppState>() {
        *state.tray_items.lock().unwrap() = Some(TrayMenuItems {
            translate: translate_item,
            settings: settings_item,
            quit: quit_item,
        });
        *state.tray_tone_items.lock().unwrap() = Some(TrayToneItems {
            professional: professional_item,
            casual: casual_item,
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
            "tone-professional" => set_tone(app, "professional"),
            "tone-casual" => set_tone(app, "casual"),
            _ => {}
        })
        .build(app)?;

    Ok(())
}

/// Persists the tone picked from the tray submenu itself, then relabels/
/// rechecks the tray to match — same persisted setting `save_tone` (the
/// Settings-page command) writes, so either entry point stays in sync.
fn set_tone(app: &AppHandle, tone: &str) {
    if let Ok(mut settings) = settings_store::load(app) {
        settings.tone = tone.to_string();
        let _ = settings_store::save(app, &settings);
    }
    update_tone(app, tone);
}

/// Relabels the existing tray menu items in place — called from
/// `save_ui_language` so switching the app language in Settings updates the
/// tray immediately, not just on next launch.
pub fn update_language(app: &AppHandle, ui_language: &str) {
    let (translate_label, settings_label, quit_label, _) = labels(ui_language);
    if let Some(state) = app.try_state::<AppState>() {
        if let Some(items) = state.tray_items.lock().unwrap().as_ref() {
            let _ = items.translate.set_text(translate_label);
            let _ = items.settings.set_text(settings_label);
            let _ = items.quit.set_text(quit_label);
        }
    }
    let (professional_label, casual_label) = tone_labels(ui_language);
    if let Some(state) = app.try_state::<AppState>() {
        if let Some(items) = state.tray_tone_items.lock().unwrap().as_ref() {
            let _ = items.professional.set_text(professional_label);
            let _ = items.casual.set_text(casual_label);
        }
    }
}

/// Checks the tray's tone submenu item matching `tone` and unchecks the
/// other — called both from the tray's own click handler and from
/// `save_tone` (the Settings-page command), so the checkmark stays correct
/// no matter which entry point changed it.
pub fn update_tone(app: &AppHandle, tone: &str) {
    if let Some(state) = app.try_state::<AppState>() {
        if let Some(items) = state.tray_tone_items.lock().unwrap().as_ref() {
            let is_casual = tone == "casual";
            let _ = items.professional.set_checked(!is_casual);
            let _ = items.casual.set_checked(is_casual);
        }
    }
}
