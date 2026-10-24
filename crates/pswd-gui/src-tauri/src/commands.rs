//! Commandes appelables depuis l'interface (déclarées aussi dans `build.rs`).

// Tauri impose de recevoir l'état (`State`) par valeur.
#![allow(clippy::needless_pass_by_value)]

use tauri::State;

use crate::error::UiError;
use crate::state::AppState;

/// Noms des coffres de cet appareil, triés.
#[tauri::command]
pub fn list_vaults(state: State<'_, AppState>) -> Result<Vec<String>, UiError> {
    Ok(state.vaults.config()?.vaults.into_keys().collect())
}

#[cfg(test)]
mod tests {
    use std::fs;

    use pswd_app::Vaults;
    use pswd_core::crypto::KdfParams;
    use pswd_core::{Secret, Timestamp};
    use pswd_infra::Locations;
    use serde_json::{Value, json};
    use tauri::WebviewUrl;
    use tauri::http::HeaderMap;
    use tauri::ipc::{CallbackFn, InvokeBody};
    use tauri::test::{INVOKE_KEY, get_ipc_response, mock_builder};
    use tauri::webview::InvokeRequest;
    use tempfile::TempDir;

    use super::*;

    /// Origine des pages de l'application, selon la plateforme.
    #[cfg(any(windows, target_os = "android"))]
    const ORIGIN: &str = "http://tauri.localhost";
    #[cfg(not(any(windows, target_os = "android")))]
    const ORIGIN: &str = "tauri://localhost";

    fn vaults_in(dir: &TempDir) -> Vaults {
        Vaults::new(Locations::new(
            &dir.path().join("config"),
            &dir.path().join("data"),
        ))
    }

    /// Appelle `command` comme le ferait l'interface : la vraie configuration de l'application
    /// (capacités comprises) avec le moteur factice de Tauri.
    fn invoke(vaults: Vaults, command: &str) -> Result<Value, Value> {
        let app = mock_builder()
            .manage(AppState::new(vaults))
            .invoke_handler(tauri::generate_handler![list_vaults, not_allowed])
            .build(tauri::generate_context!())
            .unwrap();
        let webview = tauri::WebviewWindowBuilder::new(&app, "main", WebviewUrl::default())
            .build()
            .unwrap();
        let request = InvokeRequest {
            cmd: command.to_owned(),
            callback: CallbackFn(0),
            error: CallbackFn(1),
            url: ORIGIN.parse().unwrap(),
            body: InvokeBody::default(),
            headers: HeaderMap::default(),
            invoke_key: INVOKE_KEY.to_owned(),
        };
        get_ipc_response(&webview, request).map(|body| body.deserialize().unwrap())
    }

    #[test]
    fn vault_names_are_listed_in_order() {
        let dir = TempDir::new().unwrap();
        let vaults = vaults_in(&dir);
        let params = KdfParams::new(8 * 1024, 1, 1).unwrap();
        for name in ["travail", "perso"] {
            let password = Secret::new("maître".to_owned());
            vaults
                .create(name, &password, params, Timestamp::from_millis(0))
                .unwrap();
        }
        assert_eq!(
            invoke(vaults, "list_vaults"),
            Ok(json!(["perso", "travail"]))
        );
    }

    #[test]
    fn a_new_device_has_no_vault() {
        let dir = TempDir::new().unwrap();
        assert_eq!(invoke(vaults_in(&dir), "list_vaults"), Ok(json!([])));
    }

    #[test]
    fn errors_reach_the_interface_as_codes() {
        let dir = TempDir::new().unwrap();
        fs::create_dir_all(dir.path().join("config")).unwrap();
        fs::write(dir.path().join("config").join("config.toml"), "couleur = 1").unwrap();
        let error = invoke(vaults_in(&dir), "list_vaults").unwrap_err();
        assert_eq!(error["code"], "configInvalid");
        assert!(error["detail"].is_string(), "{error}");
    }

    /// Commande bien enregistrée, mais ni déclarée dans `build.rs` ni autorisée par une capacité.
    #[tauri::command]
    fn not_allowed() -> &'static str {
        "secret"
    }

    #[test]
    fn commands_outside_the_capabilities_are_refused() {
        assert_eq!(not_allowed(), "secret");
        let dir = TempDir::new().unwrap();
        let refusal = invoke(vaults_in(&dir), "not_allowed").unwrap_err();
        assert!(
            refusal
                .as_str()
                .is_some_and(|message| message.contains("not allowed")),
            "{refusal}"
        );
        assert!(invoke(vaults_in(&dir), "plugin:fs|read_file").is_err());
    }
}
