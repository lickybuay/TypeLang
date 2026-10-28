mod accessibility;
mod audio;
mod commands;
mod dictation_hotkey;
mod focus;
mod keychain;
mod llm;
mod paste;
mod services;
mod settings_store;
mod shortcut;
mod snippets_store;
mod state;
mod stt;
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
                // Three independent global shortcuts share this one handler —
                // reload settings and compare `shortcut` against each
                // registered combo to tell them apart. A JSON read per
                // keypress is trivial next to a human's reaction time, and
                // keeps this in sync with whichever one Settings/tray just
                // changed without any extra bookkeeping. Dictation is
                // hold-to-talk, so it also reacts to the release.
                .with_handler(|app, shortcut, event| {
                    let Ok(settings) = settings_store::load(app) else {
                        return;
                    };
                    if Shortcut::from_str(&settings.dictation_shortcut).ok().as_ref()
                        == Some(shortcut)
                    {
                        match event.state() {
                            ShortcutState::Pressed => dictation_hotkey::on_pressed(app),
                            ShortcutState::Released => dictation_hotkey::on_released(app),
                        }
                        return;
                    }
                    if event.state() != ShortcutState::Pressed {
                        return;
                    }
                    if Shortcut::from_str(&settings.shortcut).ok().as_ref() == Some(shortcut) {
                        crate::shortcut::trigger_translate(app);
                    } else if Shortcut::from_str(&settings.snippet_shortcut).ok().as_ref()
                        == Some(shortcut)
                    {
                        crate::shortcut::trigger_snippets(app);
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
            commands::settings::save_stt_base_url,
            commands::settings::save_vocabulary,
            commands::settings::save_ui_language,
            commands::settings::save_tone,
            commands::settings::save_translation_langs,
            commands::settings::save_shortcut,
            commands::settings::save_snippet_shortcut,
            commands::settings::save_dictation_shortcut,
            commands::settings::save_dictation_output,
            commands::settings::save_autostart,
            commands::settings::save_api_key,
            commands::settings::clear_api_key,
            commands::snippets::get_snippets,
            commands::snippets::save_snippet,
            commands::snippets::update_snippet,
            commands::snippets::delete_snippet,
            commands::snippets::reorder_snippets,
            commands::snippets::paste_snippet,
            commands::snippets::cancel_snippets,
            commands::dictation::start_recording,
            commands::dictation::stop_and_transcribe,
            commands::dictation::cancel_recording,
        ])
        .setup(|app| {
            #[cfg(target_os = "macos")]
            app.set_activation_policy(tauri::ActivationPolicy::Accessory);

            let handle = app.handle();

            let settings = settings_store::load(handle)?;
            let hotkey = Shortcut::from_str(&settings.shortcut).map_err(|e| e.to_string())?;
            app.global_shortcut().register(hotkey)?;

            let snippet_hotkey =
                Shortcut::from_str(&settings.snippet_shortcut).map_err(|e| e.to_string())?;
            app.global_shortcut().register(snippet_hotkey)?;

            // Best-effort: a taken or invalid dictation combo must not stop
            // the app from starting; it can be changed in Settings.
            match Shortcut::from_str(&settings.dictation_shortcut) {
                Ok(hotkey) => {
                    if let Err(e) = app.global_shortcut().register(hotkey) {
                        eprintln!("[app] could not register the dictation shortcut: {e}");
                    }
                }
                Err(e) => eprintln!("[app] invalid dictation shortcut: {e}"),
            }

            tray::setup(handle)?;

            services::start_if_configured(handle);

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

            println!(
                "[app] Shortcuts registered — translate: {}, snippets: {}, dictation: {}. Ready.",
                settings.shortcut, settings.snippet_shortcut, settings.dictation_shortcut
            );
            Ok(())
        })
        .build(tauri::generate_context!())
        .expect("error while building tauri application")
        .run(|app, event| {
            // Every quit path (tray Quit calls `app.exit(0)`) ends in Exit.
            if let tauri::RunEvent::Exit = event {
                services::stop(app);
            }
        });
}
