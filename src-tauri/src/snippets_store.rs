use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager};

/// A saved, reusable piece of text — usually a past translation the user
/// wants to fire off again without retyping (a standard client reply, a
/// recurring status update). `label` is what shows in the picker; `text`
/// is what actually gets pasted.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Snippet {
    pub id: String,
    pub label: String,
    pub text: String,
}

fn snippets_path(app: &AppHandle) -> Result<std::path::PathBuf, String> {
    let dir = app.path().app_config_dir().map_err(|e| e.to_string())?;
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    Ok(dir.join("snippets.json"))
}

pub fn load(app: &AppHandle) -> Result<Vec<Snippet>, String> {
    let path = snippets_path(app)?;
    if !path.exists() {
        return Ok(Vec::new());
    }
    let raw = std::fs::read_to_string(&path).map_err(|e| e.to_string())?;
    // A blank/corrupt file shouldn't brick the picker — treat it as empty
    // rather than surfacing a parse error every time the popup opens.
    Ok(serde_json::from_str(&raw).unwrap_or_default())
}

fn save(app: &AppHandle, snippets: &[Snippet]) -> Result<(), String> {
    let path = snippets_path(app)?;
    let raw = serde_json::to_string_pretty(snippets).map_err(|e| e.to_string())?;
    std::fs::write(&path, raw).map_err(|e| e.to_string())
}

// Timestamp-based, not random — good enough for a local, single-user list
// and avoids pulling in a uuid dependency for one field.
fn new_id() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    format!("s{nanos}")
}

pub fn add(app: &AppHandle, label: String, text: String) -> Result<Snippet, String> {
    let mut snippets = load(app)?;
    let snippet = Snippet {
        id: new_id(),
        label,
        text,
    };
    snippets.push(snippet.clone());
    save(app, &snippets)?;
    Ok(snippet)
}

pub fn update(app: &AppHandle, id: &str, label: String, text: String) -> Result<(), String> {
    let mut snippets = load(app)?;
    if let Some(s) = snippets.iter_mut().find(|s| s.id == id) {
        s.label = label;
        s.text = text;
    }
    save(app, &snippets)
}

pub fn remove(app: &AppHandle, id: &str) -> Result<(), String> {
    let mut snippets = load(app)?;
    snippets.retain(|s| s.id != id);
    save(app, &snippets)
}

/// Applies a new order from the Settings list (drag-to-reorder territory,
/// even though Phase 1 doesn't wire up drag yet) — any id not present in
/// `ordered_ids` keeps its relative order at the end rather than vanishing.
pub fn reorder(app: &AppHandle, ordered_ids: &[String]) -> Result<(), String> {
    let mut snippets = load(app)?;
    let mut reordered = Vec::with_capacity(snippets.len());
    for id in ordered_ids {
        if let Some(pos) = snippets.iter().position(|s| &s.id == id) {
            reordered.push(snippets.remove(pos));
        }
    }
    reordered.extend(snippets);
    save(app, &reordered)
}
