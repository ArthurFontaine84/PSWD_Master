//! Adresses d'une entrée, ouvertes dans le navigateur du système (SPEC §12). Seul le backend
//! appelle le plugin `opener`, et seulement pour une adresse web : l'interface n'a aucune de ses
//! permissions.

use tauri::Url;

use crate::error::UiError;

/// Ouvre une adresse dans le navigateur ; renvoie le message d'erreur du système en cas d'échec.
type Open = dyn Fn(&str) -> Result<(), String> + Send + Sync;

/// Le navigateur du système, géré par Tauri à côté de [`crate::state::AppState`].
pub struct Browser(Box<Open>);

impl Browser {
    pub fn new(open: impl Fn(&str) -> Result<(), String> + Send + Sync + 'static) -> Self {
        Self(Box::new(open))
    }

    /// Ouvre `text` s'il s'agit d'une adresse web (voir [`web_address`]).
    pub fn open(&self, text: &str) -> Result<(), UiError> {
        let url = web_address(text)?;
        (self.0)(url.as_str()).map_err(|error| UiError::new("urlNotOpened", error))
    }
}

/// Adresse web à ouvrir : `http` ou `https` seulement, jamais `file:`, `javascript:` ni une
/// autre application. Une adresse sans protocole (`exemple.fr/compte`) est ouverte en `https`.
/// Une adresse qui porte un identifiant (`https://moi@exemple.fr`) est refusée : elle sert à
/// déguiser le vrai site.
fn web_address(text: &str) -> Result<Url, UiError> {
    let text = text.trim();
    let url = if text.contains("://") {
        Url::parse(text)
    } else {
        Url::parse(&format!("https://{text}"))
    }
    .map_err(|_| UiError::code("invalidUrl"))?;
    // `Url` exige déjà un nom d'hôte pour `http` et `https`.
    let web = matches!(url.scheme(), "http" | "https");
    if !web || !url.username().is_empty() || url.password().is_some() {
        return Err(UiError::code("invalidUrl"));
    }
    Ok(url)
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};

    use super::*;

    fn code(error: UiError) -> serde_json::Value {
        serde_json::to_value(error).unwrap()["code"].clone()
    }

    #[test]
    fn web_addresses_are_opened() {
        for (text, opened) in [
            ("https://exemple.fr/compte", "https://exemple.fr/compte"),
            ("http://box.local:8080", "http://box.local:8080/"),
            (" exemple.fr/compte ", "https://exemple.fr/compte"),
            ("exemple.fr:8443", "https://exemple.fr:8443/"),
        ] {
            assert_eq!(web_address(text).unwrap().as_str(), opened, "{text}");
        }
    }

    #[test]
    fn other_addresses_are_refused() {
        for text in [
            "",
            "file:///C:/Windows/system32",
            "javascript://%0Aalert(1)",
            "javascript:alert(1)",
            "mailto:moi@exemple.fr",
            "ftp://exemple.fr",
            "https://moi@exemple.fr",
            "https://moi:secret@exemple.fr",
            "https://:secret@exemple.fr",
            "https://",
            "exemple fr",
        ] {
            assert_eq!(code(web_address(text).unwrap_err()), "invalidUrl", "{text}");
        }
    }

    #[test]
    fn the_browser_receives_the_checked_address() {
        let opened = Arc::new(Mutex::new(Vec::new()));
        let seen = Arc::clone(&opened);
        let browser = Browser::new(move |url| {
            seen.lock().unwrap().push(url.to_owned());
            Ok(())
        });
        browser.open("exemple.fr").unwrap();
        assert_eq!(
            code(browser.open("file:///etc/passwd").unwrap_err()),
            "invalidUrl"
        );
        assert_eq!(*opened.lock().unwrap(), ["https://exemple.fr/"]);

        let broken = Browser::new(|_| Err("aucun navigateur".to_owned()));
        let error = serde_json::to_value(broken.open("exemple.fr").unwrap_err()).unwrap();
        assert_eq!(error["code"], "urlNotOpened");
        assert_eq!(error["detail"], "aucun navigateur");
    }
}
