use keyring::Entry;

const SERVICE: &str = "com.sergio.typelang";

/// Each provider gets its own keychain entry (e.g. "anthropic-api-key",
/// "openai-api-key") so switching providers in settings never clobbers a
/// key the user already entered for another one.
fn entry_for(provider: &str) -> Result<Entry, String> {
    Entry::new(SERVICE, &format!("{provider}-api-key")).map_err(|e| e.to_string())
}

pub fn get_api_key(provider: &str) -> Result<Option<String>, String> {
    match entry_for(provider)?.get_password() {
        Ok(key) => Ok(Some(key)),
        Err(keyring::Error::NoEntry) => Ok(None),
        Err(e) => Err(e.to_string()),
    }
}

pub fn set_api_key(provider: &str, key: &str) -> Result<(), String> {
    entry_for(provider)?.set_password(key).map_err(|e| e.to_string())
}

pub fn clear_api_key(provider: &str) -> Result<(), String> {
    match entry_for(provider)?.delete_credential() {
        Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
        Err(e) => Err(e.to_string()),
    }
}
