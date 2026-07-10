use tauri::{
    menu::{Menu, MenuItem},
    tray::TrayIconBuilder,
    AppHandle,
};

use crate::{shortcut, window};

pub fn setup(app: &AppHandle) -> tauri::Result<()> {
    let translate_item =
        MenuItem::with_id(app, "translate", "Traducir ahora", true, None::<&str>)?;
    let settings_item =
        MenuItem::with_id(app, "settings", "Configuración…", true, None::<&str>)?;
    let quit_item = MenuItem::with_id(app, "quit", "Salir", true, None::<&str>)?;
    let menu = Menu::with_items(app, &[&translate_item, &settings_item, &quit_item])?;

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
