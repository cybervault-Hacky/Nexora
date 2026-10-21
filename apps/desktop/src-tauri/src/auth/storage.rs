use super::AuthError;
use std::sync::Mutex;

#[cfg(target_os = "windows")]
const KEYRING_SERVICE: &str = "com.nexora.desktop";
#[cfg(target_os = "windows")]
const KEYRING_USER: &str = "session_token";

// In-memory fallback for non-Windows platforms or when the OS keyring is unavailable.
static MEMORY_STORE: Mutex<Option<String>> = Mutex::new(None);

/// Writes (or clears) the in-memory copy of the token.
///
/// A poisoned lock used to be swallowed with `if let Ok`, which reported
/// success while storing nothing — the next read would then claim the user had
/// never signed in. Turning it into an error keeps logout idempotent but stops
/// a save from lying about having happened.
fn remember(token: Option<&str>) -> Result<(), AuthError> {
    let mut store = MEMORY_STORE
        .lock()
        .map_err(|_| AuthError::storage("The in-memory session store is unavailable."))?;

    *store = token.map(str::to_string);

    Ok(())
}

/// Saves the opaque session token in secure OS credential storage.
///
/// On Windows, this utilizes the Windows Credential Manager (DPAPI-backed) via the
/// `keyring` crate. On other platforms or if the OS credential store is unreachable,
/// it safely retains the token in process-isolated memory.
pub fn save_session_token(token: &str) -> Result<(), AuthError> {
    #[cfg(target_os = "windows")]
    {
        match keyring::Entry::new(KEYRING_SERVICE, KEYRING_USER) {
            Ok(entry) => {
                if let Err(e) = entry.set_password(token) {
                    // Fall back to memory cache on storage failure
                    if let Ok(mut mem) = MEMORY_STORE.lock() {
                        *mem = Some(token.to_string());
                    }
                    return Err(AuthError::storage(format!(
                        "Failed to save credential to Windows Credential Manager: {e}"
                    )));
                }
            }
            Err(e) => {
                if let Ok(mut mem) = MEMORY_STORE.lock() {
                    *mem = Some(token.to_string());
                }
                return Err(AuthError::storage(format!(
                    "Failed to access Windows Credential Manager entry: {e}"
                )));
            }
        }
    }

    remember(Some(token))
}

/// Retrieves the session token from secure storage.
pub fn get_session_token() -> Result<Option<String>, AuthError> {
    #[cfg(target_os = "windows")]
    {
        if let Ok(entry) = keyring::Entry::new(KEYRING_SERVICE, KEYRING_USER) {
            match entry.get_password() {
                Ok(secret) => return Ok(Some(secret)),
                Err(keyring::Error::NoEntry) => {
                    // No entry in Windows Credential Manager; check memory fallback
                }
                Err(e) => {
                    // Non-fatal error; fall back to memory store
                    let _ = e;
                }
            }
        }
    }

    let store = MEMORY_STORE
        .lock()
        .map_err(|_| AuthError::storage("The in-memory session store is unavailable."))?;

    Ok(store.clone())
}

/// Deletes the session token upon logout or session revocation.
pub fn delete_session_token() -> Result<(), AuthError> {
    #[cfg(target_os = "windows")]
    {
        if let Ok(entry) = keyring::Entry::new(KEYRING_SERVICE, KEYRING_USER) {
            // keyring 3.x renamed `delete_password` to `delete_credential`. A
            // missing entry is not an error: logout must stay idempotent.
            let _ = entry.delete_credential();
        }
    }

    remember(None)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn storage_memory_round_trip() {
        let test_token = "0123456789abcdef0123456789abcdef";
        save_session_token(test_token).unwrap();
        let loaded = get_session_token().unwrap();
        assert_eq!(loaded.as_deref(), Some(test_token));

        delete_session_token().unwrap();
        let cleared = get_session_token().unwrap();
        assert_eq!(cleared, None);
    }
}
