use tauri::{AppHandle, State};
use tokio::sync::oneshot;

use crate::{
    commands::translate::paste_to_target,
    snippets_store::{self, Snippet},
    state::AppState,
    window,
};

#[tauri::command]
pub fn get_snippets(app: AppHandle) -> Result<Vec<Snippet>, String> {
    snippets_store::load(&app)
}

#[tauri::command]
pub fn save_snippet(app: AppHandle, label: String, text: String) -> Result<Snippet, String> {
    let label = label.trim();
    let text = text.trim();
    if label.is_empty() || text.is_empty() {
        return Err("El snippet necesita etiqueta y texto".to_string());
    }
    snippets_store::add(&app, label.to_string(), text.to_string())
}

#[tauri::command]
pub fn update_snippet(
    app: AppHandle,
    id: String,
    label: String,
    text: String,
) -> Result<(), String> {
    let label = label.trim();
    let text = text.trim();
    if label.is_empty() || text.is_empty() {
        return Err("El snippet necesita etiqueta y texto".to_string());
    }
    snippets_store::update(&app, &id, label.to_string(), text.to_string())
}

#[tauri::command]
pub fn delete_snippet(app: AppHandle, id: String) -> Result<(), String> {
    snippets_store::remove(&app, &id)
}

#[tauri::command]
pub fn reorder_snippets(app: AppHandle, ids: Vec<String>) -> Result<(), String> {
    snippets_store::reorder(&app, &ids)
}

/// Same paste-and-restore-focus dance as `translate_and_paste`, minus the
/// LLM call — the snippet's text is already final, so this just needs to
/// get it where the cursor was.
#[tauri::command]
pub async fn paste_snippet(
    app: AppHandle,
    state: State<'_, AppState>,
    text: String,
) -> Result<(), String> {
    let captured = state.captured_focus.lock().map_err(|e| e.to_string())?.clone();

    let (tx, rx) = oneshot::channel();
    let app_for_main = app.clone();
    app.run_on_main_thread(move || {
        let _ = tx.send(paste_to_target(&app_for_main, captured, &text, window::hide_snippets));
    })
    .map_err(|e| e.to_string())?;

    rx.await.map_err(|e| e.to_string())?
}

#[tauri::command]
pub fn cancel_snippets(app: AppHandle) {
    window::hide_snippets(&app);
    window::release_activation(&app);
}
