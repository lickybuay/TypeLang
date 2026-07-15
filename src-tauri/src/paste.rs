use enigo::{
    Direction::{Click, Press, Release},
    Enigo, Key, Keyboard, Settings,
};

use crate::accessibility;

/// Swaps the OS clipboard to `text`, simulates a paste keystroke, then
/// restores whatever was on the clipboard before. Caller is responsible for
/// having already reactivated the target app (see `focus::restore`) so the
/// paste lands in the right window.
///
/// macOS-only for the Phase 1 MVP: uses Cmd+V. Windows/Linux paste-simulation
/// (Ctrl+V, different focus APIs) is Phase 3/4 scope per the plan.
pub fn paste_text(text: &str) -> Result<(), String> {
    // enigo's CGEventPost-based key simulation doesn't error without
    // Accessibility trust — the event is silently dropped by the OS, so
    // every call below would still return Ok(()) with nothing actually
    // pasted. Checking up front turns that into a real, visible error.
    if !accessibility::is_trusted() {
        accessibility::request_trust();
        return Err(
            "TypeLang necesita el permiso de Accesibilidad para pegar el texto traducido. \
             Concedelo en Configuración del Sistema → Privacidad y Seguridad → Accesibilidad, \
             y probá de nuevo."
                .to_string(),
        );
    }

    let mut clipboard = arboard::Clipboard::new().map_err(|e| e.to_string())?;
    let original = clipboard.get_text().unwrap_or_default();

    clipboard.set_text(text).map_err(|e| e.to_string())?;

    // Give the just-reactivated app a moment to actually accept focus before
    // we synthesize the keystroke, otherwise the paste can land nowhere.
    std::thread::sleep(std::time::Duration::from_millis(150));

    let mut enigo = Enigo::new(&Settings::default()).map_err(|e| e.to_string())?;
    enigo.key(Key::Meta, Press).map_err(|e| e.to_string())?;
    enigo.key(Key::Unicode('v'), Click).map_err(|e| e.to_string())?;
    enigo.key(Key::Meta, Release).map_err(|e| e.to_string())?;

    // Restore the user's original clipboard shortly after, once the paste
    // has definitely gone through.
    std::thread::sleep(std::time::Duration::from_millis(200));
    clipboard.set_text(original).map_err(|e| e.to_string())?;

    Ok(())
}
