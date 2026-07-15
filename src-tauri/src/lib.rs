mod accessibility;
mod commands;
mod focus;
mod keychain;
mod llm;
mod paste;
mod settings_store;
mod shortcut;
mod state;
mod tray;
mod window;

use std::str::FromStr;

use tauri::{Manager, WindowEvent};
use tauri_plugin_global_shortcut::{GlobalShortcutExt, Shortcut, ShortcutState};

use state::AppState;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        // Must be the first plugin registered: it's what lets a second
        // launch (e.g. opening the dev build while a release build, or
        // another dev instance, is already running) detect the existing
        // process and hand off to it instead of starting up — otherwise
        // both processes fight over the same global shortcut, and whichever
        // registers second silently wins, leaving the other one dead.
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            window::show_settings(app);
        }))
        .plugin(tauri_plugin_opener::init())
        .plugin(
            tauri_plugin_global_shortcut::Builder::new()
                .with_handler(|app, _shortcut, event| {
                    if event.state() == ShortcutState::Pressed {
                        shortcut::trigger_translate(app);
                    }
                })
                .build(),
        )
        .manage(AppState::default())
        .invoke_handler(tauri::generate_handler![
            commands::translate::translate_and_paste,
            commands::translate::cancel_popup,
            commands::translate::open_settings_from_popup,
            commands::settings::get_settings,
            commands::settings::save_provider,
            commands::settings::save_lmstudio_base_url,
            commands::settings::save_local_model,
            commands::settings::save_ui_language,
            commands::settings::save_translation_langs,
            commands::settings::save_shortcut,
            commands::settings::save_api_key,
            commands::settings::clear_api_key,
        ])
        .setup(|app| {
            #[cfg(target_os = "macos")]
            app.set_activation_policy(tauri::ActivationPolicy::Accessory);

            let handle = app.handle();

            let settings = settings_store::load(handle)?;
            let hotkey = Shortcut::from_str(&settings.shortcut).map_err(|e| e.to_string())?;
            app.global_shortcut().register(hotkey)?;

            tray::setup(handle)?;

            // Fires the system's Accessibility permission dialog on first
            // launch instead of waiting for the first translate attempt to
            // silently fail — macOS only shows this prompt once per app per
            // launch, so calling it here (rather than lazily in paste.rs)
            // means it's already resolved by the time the user tries to
            // paste anything.
            #[cfg(target_os = "macos")]
            accessibility::request_trust();

            // The settings window is the "main" one from tauri.conf.json. It
            // starts hidden (menu-bar-only app); clicking its close button
            // should hide it, not quit the whole app.
            if let Some(main) = app.get_webview_window("main") {
                let main_clone = main.clone();
                main.on_window_event(move |event| {
                    if let WindowEvent::CloseRequested { api, .. } = event {
                        api.prevent_close();
                        let _ = main_clone.hide();
                    }
                });
            }

            println!("[app] Shortcut {} registered. Ready.", settings.shortcut);
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
