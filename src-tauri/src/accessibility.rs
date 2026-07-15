//! Accessibility (AX) permission check + prompt for macOS.
//!
//! `enigo`'s CGEventPost-based keystroke simulation doesn't error when the
//! process lacks Accessibility trust — the event is posted but the OS just
//! drops it, so `paste::paste_text` was returning `Ok(())` on every call
//! even when nothing actually got typed. Checking trust explicitly (and
//! prompting for it) is what turns that into a real signal instead of a
//! silent no-op.

#[cfg(target_os = "macos")]
mod macos {
    use core_foundation::base::TCFType;
    use core_foundation::boolean::CFBoolean;
    use core_foundation::dictionary::{CFDictionary, CFDictionaryRef};
    use core_foundation::string::CFString;

    #[link(name = "ApplicationServices", kind = "framework")]
    extern "C" {
        fn AXIsProcessTrustedWithOptions(options: CFDictionaryRef) -> u8;
    }

    /// Current trust state, no side effects.
    pub fn is_trusted() -> bool {
        unsafe { AXIsProcessTrustedWithOptions(std::ptr::null()) != 0 }
    }

    /// Same check, but if untrusted, also triggers the system's "TypeLang
    /// would like to control this computer" dialog (with a direct link into
    /// System Settings). macOS only shows this once per app per launch —
    /// safe to call repeatedly.
    pub fn request_trust() -> bool {
        let key = CFString::new("AXTrustedCheckOptionPrompt");
        let value = CFBoolean::true_value();
        let options = CFDictionary::from_CFType_pairs(&[(key, value)]);
        unsafe { AXIsProcessTrustedWithOptions(options.as_concrete_TypeRef()) != 0 }
    }
}

#[cfg(target_os = "macos")]
pub use macos::{is_trusted, request_trust};

#[cfg(not(target_os = "macos"))]
pub fn is_trusted() -> bool {
    true
}
#[cfg(not(target_os = "macos"))]
pub fn request_trust() -> bool {
    true
}
