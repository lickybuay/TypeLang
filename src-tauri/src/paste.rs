use crate::accessibility;

/// Swaps the OS clipboard to `text`, simulates a paste keystroke, then
/// restores whatever was on the clipboard before. Caller is responsible for
/// having already reactivated the target app (see `focus::restore`) so the
/// paste lands in the right window.
///
/// macOS-only for the Phase 1 MVP: uses Cmd+V. Windows/Linux paste-simulation
/// (Ctrl+V, different focus APIs) is Phase 3/4 scope per the plan.
pub fn paste_text(text: &str) -> Result<(), String> {
    // CoreGraphics keystroke posting doesn't error without Accessibility
    // trust — the event is silently dropped by the OS, so every call below
    // would still return Ok(()) with nothing actually pasted. Checking up
    // front turns that into a real, visible error.
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
    // `None` when the clipboard holds no text (e.g. an image): nothing we can
    // put back, so it is left alone afterwards.
    let original = clipboard.get_text().ok();

    clipboard.set_text(text).map_err(|e| e.to_string())?;

    // Give the just-reactivated app a moment to actually accept focus before
    // we synthesize the keystroke, otherwise the paste can land nowhere.
    std::thread::sleep(std::time::Duration::from_millis(150));

    keystroke::wait_for_modifiers_released();
    keystroke::post_cmd_v()?;

    // Restore the user's original clipboard once a slow app has definitely
    // read ours — but only if nobody copied something else in the meantime.
    std::thread::sleep(std::time::Duration::from_millis(600));
    if let Some(original) = original {
        if clipboard.get_text().ok().as_deref() == Some(text) {
            clipboard.set_text(original).map_err(|e| e.to_string())?;
        }
    }

    Ok(())
}

#[cfg(target_os = "macos")]
mod keystroke {
    use std::time::{Duration, Instant};

    use core_graphics::event::{CGEvent, CGEventFlags, CGEventTapLocation, CGKeyCode};
    use core_graphics::event_source::{CGEventSource, CGEventSourceStateID};

    /// kVK_ANSI_V: a physical key position, so it is Cmd+V on any layout.
    const KVK_ANSI_V: CGKeyCode = 9;
    const MODIFIER_WAIT: Duration = Duration::from_millis(800);
    const MODIFIER_POLL: Duration = Duration::from_millis(10);

    #[link(name = "CoreGraphics", kind = "framework")]
    extern "C" {
        // Not exposed by core-graphics 0.23.
        fn CGEventSourceFlagsState(state_id: CGEventSourceStateID) -> u64;
    }

    fn modifiers_down() -> bool {
        let mask = (CGEventFlags::CGEventFlagCommand
            | CGEventFlags::CGEventFlagAlternate
            | CGEventFlags::CGEventFlagShift
            | CGEventFlags::CGEventFlagControl)
            .bits();
        let flags = unsafe { CGEventSourceFlagsState(CGEventSourceStateID::CombinedSessionState) };
        flags & mask != 0
    }

    /// A hotkey the user is still holding (Alt+Shift+...) would mix into the
    /// paste, so wait briefly for the physical release.
    pub fn wait_for_modifiers_released() {
        let deadline = Instant::now() + MODIFIER_WAIT;
        while modifiers_down() && Instant::now() < deadline {
            std::thread::sleep(MODIFIER_POLL);
        }
    }

    /// Posts Cmd+V with the Command flag set on both events, so the app sees
    /// the shortcut even if the session state has not registered Cmd yet.
    pub fn post_cmd_v() -> Result<(), String> {
        let source = CGEventSource::new(CGEventSourceStateID::HIDSystemState)
            .map_err(|_| "No se pudo crear la fuente de eventos del teclado".to_string())?;
        for key_down in [true, false] {
            let event = CGEvent::new_keyboard_event(source.clone(), KVK_ANSI_V, key_down)
                .map_err(|_| "No se pudo crear el evento de teclado".to_string())?;
            event.set_flags(CGEventFlags::CGEventFlagCommand);
            event.post(CGEventTapLocation::HID);
        }
        Ok(())
    }
}

#[cfg(not(target_os = "macos"))]
mod keystroke {
    pub fn wait_for_modifiers_released() {}

    pub fn post_cmd_v() -> Result<(), String> {
        Err("Pegar solo está soportado en macOS".to_string())
    }
}
