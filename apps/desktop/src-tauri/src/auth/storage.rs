use super::AuthError;
use std::sync::Mutex;

#[cfg(target_os = "windows")]
const KEYRING_SERVICE: &str = "com.nexora.desktop";
#[cfg(target_os = "windows")]
const KEYRING_USER: &str = "session_token";

// In-memory fallback for non-Windows platforms or when the OS keyring is unavailable.
static MEMORY_STORE: Mutex<Option<String>> = Mutex::new(None);

/// Writes (or clears) the in-memory copy of the token.
fn remember(token: Option<&str>) -> Result<(), AuthError> {
    let mut store = MEMORY_STORE
        .lock()
        .map_err(|_| AuthError::storage("The in-memory session store is unavailable."))?;

    *store = token.map(str::to_string);
    Ok(())
}

fn memory_token() -> Result<Option<String>, AuthError> {
    let store = MEMORY_STORE
        .lock()
        .map_err(|_| AuthError::storage("The in-memory session store is unavailable."))?;
    Ok(store.clone())
}

/// Saves the opaque session token in secure OS credential storage on Windows,
/// or in process-isolated memory on other platforms. A failed Windows vault
/// write is returned as an error; the volatile fallback is never presented as
/// durable storage or a successful account login.
pub fn save_session_token(token: &str) -> Result<(), AuthError> {
    #[cfg(target_os = "windows")]
    {
        let saved = match keyring::Entry::new(KEYRING_SERVICE, KEYRING_USER) {
            Ok(entry) => entry.set_password(token).is_ok(),
            Err(_) => false,
        };
        if !saved {
            remember(Some(token))?;
            return Err(AuthError::storage(
                "Windows Credential Manager could not persist the account session; the process-only fallback is not a durable signed-in session.",
            ));
        }
    }

    remember(Some(token))
}

/// Retrieves the session token from secure storage. A missing keyring entry is
/// an ordinary signed-out state. A keyring access failure with no volatile
/// fallback is a typed vault error, not a fabricated signed-out state.
pub fn get_session_token() -> Result<Option<String>, AuthError> {
    #[cfg(target_os = "windows")]
    {
        let fallback = memory_token()?;
        let entry = keyring::Entry::new(KEYRING_SERVICE, KEYRING_USER).map_err(|_| {
            AuthError::storage("Windows Credential Manager could not be accessed.")
        });
        return match entry {
            Ok(entry) => match entry.get_password() {
                Ok(secret) => Ok(Some(secret)),
                Err(keyring::Error::NoEntry) => Ok(fallback),
                Err(_) => fallback.map(Some).ok_or_else(|| {
                    AuthError::storage("Windows Credential Manager could not be accessed.")
                }),
            },
            Err(error) => fallback.map(Some).ok_or(error),
        };
    }

    #[cfg(not(target_os = "windows"))]
    {
        memory_token()
    }
}

/// Deletes the session token upon logout or session revocation. Persistent
/// deletion failures are surfaced, while the volatile copy and cached identity
/// are still cleared by the caller.
pub fn delete_session_token() -> Result<(), AuthError> {
    #[cfg(target_os = "windows")]
    let persistent_delete = match keyring::Entry::new(KEYRING_SERVICE, KEYRING_USER) {
        Ok(entry) => match entry.delete_credential() {
            Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
            Err(_) => Err(AuthError::storage(
                "Windows Credential Manager could not remove the saved account session.",
            )),
        },
        Err(_) => Err(AuthError::storage(
            "Windows Credential Manager could not be accessed to remove the saved account session.",
        )),
    };

    #[cfg(not(target_os = "windows"))]
    let persistent_delete: Result<(), AuthError> = Ok(());

    let memory_delete = remember(None);
    match persistent_delete {
        Err(error) => {
            let _ = memory_delete;
            Err(error)
        }
        Ok(()) => memory_delete,
    }
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

    #[test]
    fn vault_failure_messages_never_contain_credentials() {
        let error = AuthError::storage("session token=VAULT_FAILURE_SENTINEL");
        assert!(!error.message.contains("VAULT_FAILURE_SENTINEL"));
    }
}
