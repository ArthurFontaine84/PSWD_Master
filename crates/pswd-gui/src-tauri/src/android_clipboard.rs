//! Presse-papiers d'Android : le plugin natif `SecureClipboardPlugin` (Kotlin, dans `gen/android`)
//! marque la copie comme sensible et l'efface après le délai (SPEC §9, §12).
//!
//! Exclu de la couverture : compilé pour Android seulement, testé à la main sur un appareil.

use std::time::Duration;

use pswd_core::Secret;
use pswd_core::ports::{PortError, SecureClipboard};
use serde::Serialize;
use serde::de::IgnoredAny;
use tauri::plugin::{Builder, PluginHandle, TauriPlugin};
use tauri::{Manager, Runtime};

/// Paquet Kotlin de l'application, où se trouve la classe du plugin.
const PLUGIN_PACKAGE: &str = "fr.arthurfontaine.pswdmaster";

pub struct AndroidClipboard<R: Runtime>(PluginHandle<R>);

// Écrit à la main : `#[derive(Clone)]` exigerait `R: Clone`, que le moteur ne fournit pas.
impl<R: Runtime> Clone for AndroidClipboard<R> {
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct CopySecretArgs<'a> {
    value: &'a str,
    clear_after_ms: u64,
}

/// Enregistre le plugin natif ; son accès est ensuite rangé dans l'état de l'application.
pub fn plugin<R: Runtime>() -> TauriPlugin<R> {
    Builder::new("secure-clipboard")
        .setup(|app, api| {
            let handle = api.register_android_plugin(PLUGIN_PACKAGE, "SecureClipboardPlugin")?;
            app.manage(AndroidClipboard(handle));
            Ok(())
        })
        .build()
}

impl<R: Runtime> SecureClipboard for AndroidClipboard<R> {
    fn copy_secret(&mut self, value: &Secret, ttl: Duration) -> Result<(), PortError> {
        let args = CopySecretArgs {
            value: value.expose(),
            clear_after_ms: u64::try_from(ttl.as_millis()).unwrap_or(u64::MAX),
        };
        self.0
            .run_mobile_plugin::<IgnoredAny>("copySecret", args)
            .map(drop)
            .map_err(|error| PortError::new(format!("copie impossible : {error}")))
    }

    /// Rien à faire : Android arrête l'application sans prévenir, et l'effacement au lancement
    /// suivant couvre ce cas (SPEC §12).
    fn clear_now(&mut self) {}
}
