use std::sync::Arc;

use keyring_core::{CredentialStore, Entry, Error};
use pswd_core::Secret;
use pswd_core::ports::{PortError, SecretStore};

const SERVICE: &str = "pswd";

/// Jetons d'accès rangés dans le trousseau : service `pswd`, compte = nom du coffre.
pub struct KeyringStore {
    store: Arc<CredentialStore>,
}

impl KeyringStore {
    /// Trousseau du système : Gestionnaire d'identifiants (Windows), Trousseau (macOS),
    /// Secret Service (Linux).
    ///
    /// # Errors
    ///
    /// Si le trousseau du système est inaccessible.
    // Exclue de la couverture : dépend du trousseau réel de la machine, absent des environnements
    // de test (CI sans session graphique) ; la logique est testée avec `with_store`.
    #[cfg_attr(coverage_nightly, coverage(off))]
    pub fn system() -> Result<Self, PortError> {
        #[cfg(target_os = "windows")]
        let store = windows_native_keyring_store::Store::new();
        #[cfg(target_os = "macos")]
        let store = apple_native_keyring_store::keychain::Store::new();
        #[cfg(all(unix, not(target_os = "macos")))]
        let store = zbus_secret_service_keyring_store::Store::new();
        let store: Arc<CredentialStore> = store.map_err(|error| keyring_error(&error))?;
        Ok(Self::with_store(store))
    }

    #[must_use]
    pub fn with_store(store: Arc<CredentialStore>) -> Self {
        Self { store }
    }

    fn entry(&self, vault_name: &str) -> Result<Entry, Error> {
        self.store.build(SERVICE, vault_name, None)
    }
}

impl SecretStore for KeyringStore {
    fn token(&self, vault_name: &str) -> Result<Option<Secret>, PortError> {
        match self
            .entry(vault_name)
            .and_then(|entry| entry.get_password())
        {
            Ok(token) => Ok(Some(Secret::new(token))),
            Err(Error::NoEntry) => Ok(None),
            Err(error) => Err(keyring_error(&error)),
        }
    }

    fn set_token(&mut self, vault_name: &str, token: &Secret) -> Result<(), PortError> {
        self.entry(vault_name)
            .and_then(|entry| entry.set_password(token.expose()))
            .map_err(|error| keyring_error(&error))
    }
}

/// Le message de `keyring-core` ne contient jamais le secret (seulement son `Debug`).
fn keyring_error(error: &Error) -> PortError {
    PortError::new(format!("trousseau : {error}"))
}

#[cfg(test)]
mod tests {
    use keyring_core::mock;

    use super::*;

    fn store() -> KeyringStore {
        KeyringStore::with_store(mock::Store::new().unwrap())
    }

    fn secret(value: &str) -> Secret {
        Secret::new(value.to_owned())
    }

    #[test]
    fn unknown_vault_has_no_token() {
        assert_eq!(store().token("perso"), Ok(None));
    }

    #[test]
    fn token_is_saved_per_vault() {
        let mut store = store();
        store.set_token("perso", &secret("jeton-perso")).unwrap();
        store
            .set_token("travail", &secret("jeton-travail"))
            .unwrap();
        assert_eq!(store.token("perso"), Ok(Some(secret("jeton-perso"))));
        assert_eq!(store.token("travail"), Ok(Some(secret("jeton-travail"))));
    }

    #[test]
    fn saving_again_replaces_the_token() {
        let mut store = store();
        store.set_token("perso", &secret("ancien")).unwrap();
        store.set_token("perso", &secret("nouveau")).unwrap();
        assert_eq!(store.token("perso"), Ok(Some(secret("nouveau"))));
    }

    #[test]
    fn keyring_failures_are_reported_without_the_token() {
        let mut store = store();
        store.set_token("perso", &secret("jeton-secret")).unwrap();
        let entry = store.entry("perso").unwrap();
        let credential: &mock::Cred = entry.as_any().downcast_ref().unwrap();
        credential.set_error(Error::BadEncoding(b"jeton-secret".to_vec()));
        let error = store.token("perso").unwrap_err().to_string();
        assert!(error.starts_with("trousseau : "), "{error}");
        assert!(!error.contains("jeton-secret"), "{error}");

        credential.set_error(Error::NoStorageAccess(Box::new(Error::NoEntry)));
        let error = store.set_token("perso", &secret("autre")).unwrap_err();
        assert!(error.to_string().starts_with("trousseau : "), "{error}");
    }
}
