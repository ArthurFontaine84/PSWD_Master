//! Commandes appelables depuis l'interface (déclarées aussi dans `build.rs`).

// Tauri impose de recevoir l'état (`State`) et les arguments par valeur.
#![allow(clippy::needless_pass_by_value)]

use pswd_core::Secret;
use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager, Runtime, State};
use uuid::Uuid;

use crate::admin::{AccountStatus, AdminAccount, AdminVault, KeptVersion, Key, Restored};
use crate::editing::{EntryInput, HistoryItem};
use crate::entries::{EntryDetail, EntrySummary, TagCount};
use crate::error::UiError;
use crate::files;
use crate::server::{Disconnection, Exchange, ServerStatus};
use crate::settings::Settings;
use crate::sharing::{Invitation, Member, Members};
use crate::state::{AppState, Notifier, VaultInfo};
use crate::sync::SyncReport;

/// Événement envoyé à l'interface quand le coffre se verrouille par inactivité.
pub const VAULT_LOCKED: &str = "vault-locked";

/// Événement envoyé à l'interface à la fin d'une synchro automatique du coffre ouvert.
pub const VAULT_SYNCED: &str = "vault-synced";

/// Coffre créé et ouvert, avec sa clé de récupération : affichée une seule fois, à la demande
/// explicite de l'utilisateur qui vient de créer le coffre.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CreatedVault {
    vault: VaultInfo,
    recovery_key: String,
}

/// Le type d'appareil : sur mobile, une sauvegarde ne peut pas être un dossier (SPEC §12).
const DEVICE_KIND: &str = if cfg!(mobile) { "mobile" } else { "desktop" };

#[tauri::command]
pub fn device_kind() -> &'static str {
    DEVICE_KIND
}

/// Noms des coffres de cet appareil, triés.
#[tauri::command]
pub fn list_vaults(state: State<'_, AppState>) -> Result<Vec<String>, UiError> {
    Ok(state.vaults.config()?.vaults.into_keys().collect())
}

#[tauri::command]
pub fn create_vault(
    state: State<'_, AppState>,
    name: String,
    password: String,
    shared: bool,
) -> Result<CreatedVault, UiError> {
    let password = Secret::new(password);
    let (vault, recovery_key) = state.create(&name, &password, shared)?;
    Ok(CreatedVault {
        vault,
        recovery_key: recovery_key.expose().to_owned(),
    })
}

#[tauri::command]
pub fn unlock_vault(
    state: State<'_, AppState>,
    name: String,
    password: String,
) -> Result<VaultInfo, UiError> {
    let password = Secret::new(password);
    state.unlock(&name, &password)
}

#[tauri::command]
pub fn lock_vault(state: State<'_, AppState>) {
    state.lock();
}

/// Le coffre ouvert, s'il y en a un ; chaque appel compte comme une activité.
#[tauri::command]
pub fn current_vault(state: State<'_, AppState>) -> Option<VaultInfo> {
    state.current()
}

/// L'application passe en arrière-plan : sur mobile, le verrouillage approche.
#[tauri::command]
pub fn app_hidden(state: State<'_, AppState>) {
    state.hide();
}

/// L'application revient au premier plan : le coffre encore ouvert, s'il y en a un.
#[tauri::command]
pub fn app_shown(state: State<'_, AppState>) -> Option<VaultInfo> {
    state.show()
}

/// Entrées du coffre ouvert qui correspondent à `text` et à `tag`, sans mot de passe.
#[tauri::command]
pub fn list_entries(
    state: State<'_, AppState>,
    text: String,
    tag: Option<String>,
) -> Result<Vec<EntrySummary>, UiError> {
    state.entries(&text, tag.as_deref())
}

#[tauri::command]
pub fn entry_detail(state: State<'_, AppState>, id: String) -> Result<EntryDetail, UiError> {
    state.entry(&id)
}

#[tauri::command]
pub fn list_tags(state: State<'_, AppState>) -> Result<Vec<TagCount>, UiError> {
    state.tags()
}

/// Le mot de passe d'une entrée, sur demande explicite de l'utilisateur (« Afficher »).
#[tauri::command]
pub fn reveal_password(state: State<'_, AppState>, id: String) -> Result<String, UiError> {
    Ok(state.reveal_password(&id)?.expose().to_owned())
}

/// Copie le mot de passe sans le transmettre à l'interface ; renvoie le délai d'effacement en
/// secondes.
#[tauri::command]
pub fn copy_password(state: State<'_, AppState>, id: String) -> Result<u64, UiError> {
    Ok(state.copy_password(&id)?.as_secs())
}

#[tauri::command]
pub fn add_entry(state: State<'_, AppState>, entry: EntryInput) -> Result<EntryDetail, UiError> {
    state.add_entry(entry)
}

#[tauri::command]
pub fn update_entry(
    state: State<'_, AppState>,
    id: String,
    entry: EntryInput,
) -> Result<EntryDetail, UiError> {
    state.update_entry(&id, entry)
}

#[tauri::command]
pub fn delete_entry(state: State<'_, AppState>, id: String) -> Result<(), UiError> {
    state.delete_entry(&id)
}

/// Versions précédentes d'une entrée, sans mot de passe.
#[tauri::command]
pub fn entry_history(state: State<'_, AppState>, id: String) -> Result<Vec<HistoryItem>, UiError> {
    state.history(&id)
}

/// Le mot de passe actuel est redemandé avant d'accepter le nouveau.
#[tauri::command]
pub fn change_password(
    state: State<'_, AppState>,
    current: String,
    password: String,
) -> Result<(), UiError> {
    state.change_password(&Secret::new(current), &Secret::new(password))
}

/// Mot de passe oublié : la clé de récupération fixe un nouveau mot de passe et ouvre le coffre.
#[tauri::command]
pub fn recover_vault(
    state: State<'_, AppState>,
    name: String,
    recovery_key: String,
    password: String,
) -> Result<VaultInfo, UiError> {
    state.recover(&name, &Secret::new(recovery_key), &Secret::new(password))
}

/// Attend la fin d'une synchro en cours : hors du fil de l'interface.
#[tauri::command]
pub async fn delete_vault<R: Runtime>(
    app: AppHandle<R>,
    name: String,
    confirmation: String,
) -> Result<(), UiError> {
    in_background(app, move |state| state.delete_vault(&name, &confirmation)).await
}

/// Le fichier est lu par Rust : son contenu en clair ne passe jamais par l'interface.
#[tauri::command]
pub fn import_entries<R: Runtime>(
    app: AppHandle<R>,
    state: State<'_, AppState>,
    path: String,
) -> Result<usize, UiError> {
    state.import_entries(&files::read(&app, &path)?)
}

/// Nom proposé dans la fenêtre d'enregistrement d'une sauvegarde `.pswd`.
#[tauri::command]
pub fn backup_file_name(state: State<'_, AppState>) -> String {
    state.backup_file_name()
}

/// Les coffres choisis dans le fichier `.pswd` choisi, écrit par Rust ; renvoie leur nombre.
#[tauri::command]
pub fn backup_to_file<R: Runtime>(
    app: AppHandle<R>,
    state: State<'_, AppState>,
    path: String,
    names: Vec<String>,
) -> Result<usize, UiError> {
    let (data, count) = state.backup_file(&names)?;
    files::write(&app, &path, &data)?;
    Ok(count)
}

#[tauri::command]
pub fn backup_file_contents<R: Runtime>(
    app: AppHandle<R>,
    state: State<'_, AppState>,
    path: String,
) -> Result<Vec<String>, UiError> {
    state.backup_file_contents(&files::read(&app, &path)?)
}

#[tauri::command]
pub fn restore_from_file<R: Runtime>(
    app: AppHandle<R>,
    state: State<'_, AppState>,
    path: String,
    name: String,
) -> Result<String, UiError> {
    state.restore_from_file(&files::read(&app, &path)?, &name)
}

#[tauri::command]
pub fn get_settings(state: State<'_, AppState>) -> Result<Settings, UiError> {
    state.settings()
}

#[tauri::command]
pub fn save_settings(state: State<'_, AppState>, settings: Settings) -> Result<Settings, UiError> {
    state.save_settings(settings)
}

/// Le serveur de l'appareil et l'état de chaque coffre, demandé au serveur.
#[tauri::command]
pub async fn server_status<R: Runtime>(app: AppHandle<R>) -> Result<ServerStatus, UiError> {
    in_background(app, AppState::server_status).await
}

/// `account_name` reste vide, sauf avec une clé administrateur.
#[tauri::command]
pub async fn connect_server<R: Runtime>(
    app: AppHandle<R>,
    address: String,
    key: String,
    device_name: String,
    account_name: String,
) -> Result<(), UiError> {
    let key = Secret::new(key);
    in_background(app, move |state| {
        state.connect(&address, &key, &device_name, &account_name)
    })
    .await
}

/// Échange avec le serveur, sans mot de passe : téléchargement, publication, mise à jour.
#[tauri::command]
pub async fn sync_server<R: Runtime>(app: AppHandle<R>) -> Result<Exchange, UiError> {
    in_background(app, AppState::exchange).await
}

/// Échange automatique de la liste des coffres, muet ; `null` si un échange est déjà en cours.
#[tauri::command]
pub async fn refresh_server<R: Runtime>(
    app: AppHandle<R>,
) -> Result<Option<ServerStatus>, UiError> {
    in_background(app, AppState::refresh).await
}

/// Clé de connexion d'un autre appareil, sur demande explicite de l'utilisateur.
#[tauri::command]
pub async fn add_device_key<R: Runtime>(app: AppHandle<R>) -> Result<String, UiError> {
    let key = in_background(app, AppState::new_device_key).await?;
    Ok(key.expose().to_owned())
}

#[tauri::command]
pub async fn disconnect_server<R: Runtime>(
    app: AppHandle<R>,
    keep_local: bool,
) -> Result<Disconnection, UiError> {
    in_background(app, move |state| state.disconnect(keep_local)).await
}

/// Comptes du serveur, pour choisir qui inviter dans un coffre commun.
#[tauri::command]
pub async fn list_accounts<R: Runtime>(app: AppHandle<R>) -> Result<Vec<Member>, UiError> {
    in_background(app, AppState::accounts).await
}

/// Membres du coffre commun `vault` et invitations en attente.
#[tauri::command]
pub async fn vault_members<R: Runtime>(
    app: AppHandle<R>,
    vault: String,
) -> Result<Members, UiError> {
    in_background(app, move |state| state.members(&vault)).await
}

#[tauri::command]
pub async fn invite_member<R: Runtime>(
    app: AppHandle<R>,
    vault: String,
    account: Uuid,
) -> Result<(), UiError> {
    in_background(app, move |state| state.invite(&vault, account)).await
}

/// Invitations reçues par le compte de l'appareil.
#[tauri::command]
pub async fn list_invitations<R: Runtime>(app: AppHandle<R>) -> Result<Vec<Invitation>, UiError> {
    in_background(app, AppState::invitations).await
}

/// Accepte l'invitation et télécharge le coffre.
#[tauri::command]
pub async fn accept_invitation<R: Runtime>(
    app: AppHandle<R>,
    id: Uuid,
) -> Result<Exchange, UiError> {
    in_background(app, move |state| state.accept_invitation(id)).await
}

/// Refuse une invitation reçue, ou annule une invitation envoyée.
#[tauri::command]
pub async fn delete_invitation<R: Runtime>(app: AppHandle<R>, id: Uuid) -> Result<(), UiError> {
    in_background(app, move |state| state.delete_invitation(id)).await
}

/// Quitte le coffre commun `vault`, qui est supprimé de cet appareil.
#[tauri::command]
pub async fn leave_vault<R: Runtime>(app: AppHandle<R>, vault: String) -> Result<(), UiError> {
    in_background(app, move |state| state.leave(&vault)).await
}

/// Synchronise le coffre ouvert (bouton « Synchroniser ») et renvoie le résultat ; un coffre
/// supprimé du serveur y est republié.
#[tauri::command]
pub async fn sync_vault<R: Runtime>(app: AppHandle<R>) -> Result<SyncReport, UiError> {
    in_background(app, AppState::sync_now).await
}

/// Vérification périodique du coffre ouvert ; une synchro qu'elle lance arrive par
/// `vault-synced`.
#[tauri::command]
pub async fn check_vault<R: Runtime>(app: AppHandle<R>) -> Result<(), UiError> {
    in_background(app, |state| {
        state.check_open();
        Ok(())
    })
    .await
}

/// Compte de l'appareil sur le serveur et son rôle ; l'adresse publique est retenue au passage.
#[tauri::command]
pub async fn account_status<R: Runtime>(app: AppHandle<R>) -> Result<AccountStatus, UiError> {
    in_background(app, AppState::account_status).await
}

/// Supprime le coffre privé `name` du serveur et de cet appareil ; le nom est retapé.
#[tauri::command]
pub async fn delete_vault_everywhere<R: Runtime>(
    app: AppHandle<R>,
    name: String,
    confirmation: String,
) -> Result<(), UiError> {
    in_background(app, move |state| {
        state.delete_everywhere(&name, &confirmation)
    })
    .await
}

#[tauri::command]
pub async fn admin_accounts<R: Runtime>(app: AppHandle<R>) -> Result<Vec<AdminAccount>, UiError> {
    in_background(app, AppState::admin_accounts).await
}

/// Clé de connexion d'un nouveau membre, sur demande explicite de l'administrateur.
#[tauri::command]
pub async fn add_member<R: Runtime>(app: AppHandle<R>, name: String) -> Result<String, UiError> {
    let key = in_background(app, move |state| state.add_member(&name)).await?;
    Ok(key.expose().to_owned())
}

#[tauri::command]
pub async fn delete_account<R: Runtime>(
    app: AppHandle<R>,
    id: Uuid,
    name: String,
    confirmation: String,
) -> Result<(), UiError> {
    in_background(app, move |state| {
        state.delete_account(id, &name, &confirmation)
    })
    .await
}

#[tauri::command]
pub async fn set_admin<R: Runtime>(
    app: AppHandle<R>,
    id: Uuid,
    is_admin: bool,
) -> Result<(), UiError> {
    in_background(app, move |state| state.set_admin(id, is_admin)).await
}

#[tauri::command]
pub async fn revoke_device<R: Runtime>(app: AppHandle<R>, id: Uuid) -> Result<(), UiError> {
    in_background(app, move |state| state.revoke_device(id)).await
}

#[tauri::command]
pub async fn pending_keys<R: Runtime>(app: AppHandle<R>) -> Result<Vec<Key>, UiError> {
    in_background(app, AppState::pending_keys).await
}

#[tauri::command]
pub async fn cancel_key<R: Runtime>(app: AppHandle<R>, id: Uuid) -> Result<(), UiError> {
    in_background(app, move |state| state.cancel_key(id)).await
}

#[tauri::command]
pub async fn server_vaults<R: Runtime>(app: AppHandle<R>) -> Result<Vec<AdminVault>, UiError> {
    in_background(app, AppState::server_vaults).await
}

#[tauri::command]
pub async fn delete_server_vault<R: Runtime>(
    app: AppHandle<R>,
    id: Uuid,
    name: String,
    confirmation: String,
) -> Result<(), UiError> {
    in_background(app, move |state| {
        state.delete_server_vault(id, &name, &confirmation)
    })
    .await
}

/// Rattache un compte à un coffre qui n'a plus de membre.
#[tauri::command]
pub async fn attach_account<R: Runtime>(
    app: AppHandle<R>,
    vault: Uuid,
    account: Uuid,
) -> Result<(), UiError> {
    in_background(app, move |state| state.attach(vault, account)).await
}

#[tauri::command]
pub async fn remove_member<R: Runtime>(
    app: AppHandle<R>,
    vault: Uuid,
    account: Uuid,
) -> Result<(), UiError> {
    in_background(app, move |state| state.remove_member(vault, account)).await
}

#[tauri::command]
pub async fn vault_versions<R: Runtime>(
    app: AppHandle<R>,
    vault: Uuid,
) -> Result<Vec<KeptVersion>, UiError> {
    in_background(app, move |state| state.versions(vault)).await
}

/// Crée un nouveau coffre à partir d'une version conservée.
#[tauri::command]
pub async fn restore_version<R: Runtime>(
    app: AppHandle<R>,
    vault: Uuid,
    version: i64,
) -> Result<Restored, UiError> {
    in_background(app, move |state| state.restore_version(vault, version)).await
}

/// Vide, l'adresse publique est effacée.
#[tauri::command]
pub async fn set_public_address<R: Runtime>(
    app: AppHandle<R>,
    address: String,
) -> Result<(), UiError> {
    in_background(app, move |state| state.set_public_address(&address)).await
}

/// Nom proposé dans la fenêtre d'enregistrement de la sauvegarde du serveur.
#[tauri::command]
pub fn server_backup_file_name(state: State<'_, AppState>) -> String {
    state.server_backup_file_name()
}

/// Sauvegarde du serveur, écrite par Rust dans le fichier choisi : elle ne passe pas par
/// l'interface.
#[tauri::command]
pub async fn backup_server_to_file<R: Runtime>(
    app: AppHandle<R>,
    path: String,
) -> Result<(), UiError> {
    let backup = in_background(app.clone(), AppState::backup_server).await?;
    files::write(&app, &path, &backup.content)
}

/// Exécute `task` hors du fil de l'interface : un échange avec le serveur peut durer.
async fn in_background<R: Runtime, T: Send + 'static>(
    app: AppHandle<R>,
    task: impl FnOnce(&AppState) -> Result<T, UiError> + Send + 'static,
) -> Result<T, UiError> {
    tauri::async_runtime::spawn_blocking(move || task(&app.state::<AppState>()))
        .await
        .unwrap_or_else(task_failed)
}

/// Seule une panique de la tâche mène ici, et le code n'en provoque aucune.
// Exclue de la couverture : aucun test ne peut l'atteindre.
#[cfg_attr(coverage_nightly, coverage(off))]
#[allow(clippy::needless_pass_by_value)]
fn task_failed<T>(error: tauri::Error) -> Result<T, UiError> {
    Err(UiError::new("unexpected", error.to_string()))
}

/// Prévient l'interface depuis les autres fils de l'application.
pub struct Events<R: Runtime>(pub AppHandle<R>);

// Au mieux : si la fenêtre est déjà fermée, il n'y a plus personne à prévenir.
impl<R: Runtime> Notifier for Events<R> {
    fn vault_locked(&self) {
        let _ = self.0.emit(VAULT_LOCKED, ());
    }

    fn vault_synced(&self, report: &SyncReport) {
        let _ = self.0.emit(VAULT_SYNCED, report);
    }
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::sync::mpsc;
    use std::time::Duration;

    use pswd_app::Vaults;
    use pswd_core::crypto::KdfParams;
    use pswd_core::{EntryContent, EntryKind, Tags, Timestamp};
    use pswd_infra::Locations;
    use pswd_server::testing::TestServer;
    use serde_json::{Value, json};
    use tauri::http::HeaderMap;
    use tauri::ipc::{CallbackFn, InvokeBody};
    use tauri::test::{INVOKE_KEY, MockRuntime, get_ipc_response, mock_builder};
    use tauri::webview::InvokeRequest;
    use tauri::{App, Listener, Manager, WebviewUrl, WebviewWindow};
    use tempfile::TempDir;

    use super::*;

    /// Origine des pages de l'application, selon la plateforme.
    #[cfg(any(windows, target_os = "android"))]
    const ORIGIN: &str = "http://tauri.localhost";
    #[cfg(not(any(windows, target_os = "android")))]
    const ORIGIN: &str = "tauri://localhost";

    /// La vraie configuration de l'application (capacités comprises) avec le moteur factice de
    /// Tauri : les commandes sont appelées comme le ferait l'interface.
    struct TestApp {
        app: App<MockRuntime>,
        webview: WebviewWindow<MockRuntime>,
        dir: TempDir,
    }

    /// Toutes les commandes, comme dans `run`.
    // Exclu de la couverture : le code écrit par `generate_handler!` contient, pour chaque
    // commande `async`, le cas d'un argument absent ou mal formé, que l'interface n'envoie
    // jamais ; il est exclu de la même façon dans `run`.
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn handler() -> impl Fn(tauri::ipc::Invoke<MockRuntime>) -> bool + Send + Sync + 'static {
        tauri::generate_handler![
            device_kind,
            list_vaults,
            create_vault,
            unlock_vault,
            lock_vault,
            current_vault,
            app_hidden,
            app_shown,
            list_entries,
            entry_detail,
            list_tags,
            reveal_password,
            copy_password,
            add_entry,
            update_entry,
            delete_entry,
            entry_history,
            change_password,
            recover_vault,
            delete_vault,
            import_entries,
            backup_file_name,
            backup_to_file,
            backup_file_contents,
            restore_from_file,
            get_settings,
            save_settings,
            server_status,
            connect_server,
            sync_server,
            refresh_server,
            add_device_key,
            disconnect_server,
            list_accounts,
            vault_members,
            invite_member,
            list_invitations,
            accept_invitation,
            delete_invitation,
            leave_vault,
            sync_vault,
            check_vault,
            account_status,
            delete_vault_everywhere,
            admin_accounts,
            add_member,
            delete_account,
            set_admin,
            revoke_device,
            pending_keys,
            cancel_key,
            server_vaults,
            delete_server_vault,
            attach_account,
            remove_member,
            vault_versions,
            restore_version,
            set_public_address,
            server_backup_file_name,
            backup_server_to_file,
            not_allowed
        ]
    }

    impl TestApp {
        fn new(auto_lock: Option<Duration>) -> Self {
            let dir = TempDir::new().unwrap();
            let vaults = Vaults::new(Locations::new(
                &dir.path().join("config"),
                &dir.path().join("data"),
            ));
            let kdf = KdfParams::new(8 * 1024, 1, 1).unwrap();
            let app = mock_builder()
                .plugin(tauri_plugin_fs::init())
                .invoke_handler(handler())
                .build(tauri::generate_context!())
                .unwrap();
            app.manage(AppState::with_settings(
                vaults,
                Box::new(crate::testing::FakeClipboard::default()),
                kdf,
                || Timestamp::from_millis(1_000),
                auto_lock,
                true,
                Events(app.handle().clone()),
            ));
            let webview = tauri::WebviewWindowBuilder::new(&app, "main", WebviewUrl::default())
                .build()
                .unwrap();
            Self { app, webview, dir }
        }

        fn call(&self, command: &str, arguments: Value) -> Result<Value, Value> {
            let request = InvokeRequest {
                cmd: command.to_owned(),
                callback: CallbackFn(0),
                error: CallbackFn(1),
                url: ORIGIN.parse().unwrap(),
                body: InvokeBody::Json(arguments),
                headers: HeaderMap::default(),
                invoke_key: INVOKE_KEY.to_owned(),
            };
            get_ipc_response(&self.webview, request).map(|body| body.deserialize().unwrap())
        }

        fn create(&self, name: &str, password: &str) -> Result<Value, Value> {
            self.call(
                "create_vault",
                json!({ "name": name, "password": password, "shared": false }),
            )
        }

        fn config_file(&self) -> std::path::PathBuf {
            self.dir.path().join("config").join("config.toml")
        }
    }

    #[test]
    fn the_interface_knows_it_runs_on_a_desktop() {
        let app = TestApp::new(None);
        assert_eq!(app.call("device_kind", json!({})), Ok(json!("desktop")));
    }

    #[test]
    fn vault_names_are_listed_in_order() {
        let app = TestApp::new(None);
        assert_eq!(app.call("list_vaults", json!({})), Ok(json!([])));
        app.create("travail", "Maître-2026!").unwrap();
        app.create("perso", "Maître-2026!").unwrap();
        assert_eq!(
            app.call("list_vaults", json!({})),
            Ok(json!(["perso", "travail"]))
        );
    }

    #[test]
    fn errors_reach_the_interface_as_codes() {
        let app = TestApp::new(None);
        fs::create_dir_all(app.config_file().parent().unwrap()).unwrap();
        fs::write(app.config_file(), "couleur = 1").unwrap();
        let error = app.call("list_vaults", json!({})).unwrap_err();
        assert_eq!(error["code"], "configInvalid");
        assert!(error["detail"].is_string(), "{error}");
    }

    #[test]
    fn created_vault_is_open_and_shows_its_recovery_key() {
        let app = TestApp::new(None);
        let created = app.create("perso", "Maître-2026!").unwrap();
        assert_eq!(created["vault"], json!({ "name": "perso", "entries": 0 }));
        let key = created["recoveryKey"].as_str().unwrap();
        assert_eq!(key.split('-').count(), 13, "{key}");
        assert_eq!(
            app.call("current_vault", json!({})),
            Ok(json!({ "name": "perso", "entries": 0 }))
        );
        assert_eq!(
            app.create("perso", "autre"),
            Err(json!({ "code": "vaultExists", "detail": "perso" }))
        );
    }

    #[test]
    fn vault_is_unlocked_with_its_password_and_locked_on_demand() {
        let app = TestApp::new(None);
        app.create("perso", "Maître-2026!").unwrap();
        assert_eq!(app.call("lock_vault", json!({})), Ok(Value::Null));
        assert_eq!(app.call("current_vault", json!({})), Ok(Value::Null));
        assert_eq!(
            app.call(
                "unlock_vault",
                json!({ "name": "perso", "password": "autre" })
            ),
            Err(json!({ "code": "wrongPassword", "detail": null }))
        );
        assert_eq!(
            app.call(
                "unlock_vault",
                json!({ "name": "perso", "password": "Maître-2026!" })
            ),
            Ok(json!({ "name": "perso", "entries": 0 }))
        );
        assert_eq!(
            app.call(
                "unlock_vault",
                json!({ "name": "inconnu", "password": "x" })
            ),
            Err(json!({ "code": "unknownVault", "detail": "inconnu" }))
        );
    }

    #[test]
    fn idle_vault_locks_itself_and_warns_the_interface() {
        let app = TestApp::new(Some(Duration::from_millis(50)));
        let (sender, receiver) = mpsc::channel();
        app.app.listen_any(VAULT_LOCKED, move |_| {
            let _ = sender.send(());
        });
        app.create("perso", "Maître-2026!").unwrap();
        assert_eq!(receiver.recv_timeout(Duration::from_secs(5)), Ok(()));
        assert_eq!(app.call("current_vault", json!({})), Ok(Value::Null));
    }

    #[test]
    fn vault_in_background_locks_itself_and_warns_the_interface() {
        let app = TestApp::new(None);
        let (sender, receiver) = mpsc::channel();
        app.app.listen_any(VAULT_LOCKED, move |_| {
            let _ = sender.send(());
        });
        app.create("perso", "Maître-2026!").unwrap();
        assert_eq!(app.call("app_hidden", json!({})), Ok(Value::Null));
        assert_eq!(
            app.call("app_shown", json!({})),
            Ok(json!({ "name": "perso", "entries": 0 }))
        );
        let mut settings = app.call("get_settings", json!({})).unwrap();
        settings["backgroundLockSeconds"] = json!(0);
        app.call("save_settings", json!({ "settings": settings }))
            .unwrap();
        app.call("app_hidden", json!({})).unwrap();
        assert_eq!(receiver.recv_timeout(Duration::from_secs(5)), Ok(()));
        assert_eq!(app.call("app_shown", json!({})), Ok(Value::Null));
    }

    #[test]
    fn existing_entries_are_returned_without_their_password() {
        let app = TestApp::new(None);
        app.create("perso", "Maître-2026!").unwrap();
        let state = app.app.state::<AppState>();
        state.lock();
        let (mut session, _) = state
            .vaults
            .create(
                "travail",
                &Secret::new("Maître-2026!".to_owned()),
                KdfParams::new(8 * 1024, 1, 1).unwrap(),
                Timestamp::from_millis(0),
            )
            .unwrap();
        let id = session
            .edit(|vault| {
                vault.add(
                    EntryContent {
                        password: Some(Secret::new("hunter2".to_owned())),
                        tags: Tags::new(["dev"]),
                        ..EntryContent::new(EntryKind::Login, "GitHub".to_owned())
                    },
                    Timestamp::from_millis(1),
                )
            })
            .unwrap()
            .to_string();
        drop(session);
        app.call(
            "unlock_vault",
            json!({ "name": "travail", "password": "Maître-2026!" }),
        )
        .unwrap();
        let listed = app
            .call("list_entries", json!({ "text": "", "tag": "dev" }))
            .unwrap();
        assert_eq!(listed[0]["title"], "GitHub");
        assert!(!listed.to_string().contains("hunter2"));
        let detail = app.call("entry_detail", json!({ "id": id })).unwrap();
        assert_eq!(detail["hasPassword"], true);
        assert!(!detail.to_string().contains("hunter2"));
        assert_eq!(
            app.call("list_tags", json!({})),
            Ok(json!([{ "tag": "dev", "count": 1 }]))
        );
        assert_eq!(
            app.call("reveal_password", json!({ "id": id })),
            Ok(json!("hunter2"))
        );
        assert_eq!(
            app.call("copy_password", json!({ "id": id })),
            Ok(json!(20))
        );
    }

    #[test]
    fn entries_are_read_through_their_commands() {
        let app = TestApp::new(None);
        let empty = json!({ "text": "", "tag": null });
        assert_eq!(
            app.call("list_entries", empty.clone()),
            Err(json!({ "code": "vaultLocked", "detail": null }))
        );
        app.create("perso", "Maître-2026!").unwrap();
        assert_eq!(app.call("list_entries", empty), Ok(json!([])));
        assert_eq!(
            app.call("list_entries", json!({ "text": "x", "tag": "dev" })),
            Ok(json!([]))
        );
        assert_eq!(app.call("list_tags", json!({})), Ok(json!([])));
        let unknown = json!({ "id": "inconnue" });
        let not_found = Err(json!({ "code": "entryNotFound", "detail": "inconnue" }));
        assert_eq!(app.call("entry_detail", unknown.clone()), not_found);
        assert_eq!(app.call("reveal_password", unknown.clone()), not_found);
        assert_eq!(app.call("copy_password", unknown), not_found);
    }

    #[test]
    fn entries_are_edited_through_their_commands() {
        let app = TestApp::new(None);
        app.create("perso", "Maître-2026!").unwrap();
        let form = |title: &str, password: Value| {
            json!({
                "kind": "login",
                "title": title,
                "username": "arthur",
                "password": password,
                "urls": ["https://github.com"],
                "notes": "",
                "tags": ["dev"],
            })
        };
        let added = app
            .call(
                "add_entry",
                json!({ "entry": form("GitHub", json!({ "mode": "generate", "length": 24, "symbols": true })) }),
            )
            .unwrap();
        assert_eq!(added["title"], "GitHub");
        assert_eq!(added["hasPassword"], true);
        let id = added["id"].clone();
        let updated = app
            .call(
                "update_entry",
                json!({ "id": id, "entry": form("GitHub pro", json!({ "mode": "keep" })) }),
            )
            .unwrap();
        assert_eq!(updated["versions"], 1);
        let history = app.call("entry_history", json!({ "id": id })).unwrap();
        assert_eq!(history[0]["title"], "GitHub");
        assert_eq!(history[0]["passwordChanged"], false);
        assert_eq!(
            app.call("delete_entry", json!({ "id": id })),
            Ok(Value::Null)
        );
        assert_eq!(
            app.call("delete_entry", json!({ "id": id })),
            Err(json!({ "code": "entryNotFound", "detail": id }))
        );
    }

    #[test]
    fn vaults_are_managed_through_their_commands() {
        let app = TestApp::new(None);
        let key = app.create("perso", "Maître-2026!").unwrap()["recoveryKey"].clone();
        assert_eq!(
            app.call(
                "change_password",
                json!({ "current": "Maître-2026!", "password": "Nouveau-2026!" })
            ),
            Ok(Value::Null)
        );
        app.call("lock_vault", json!({})).unwrap();
        assert_eq!(
            app.call(
                "recover_vault",
                json!({ "name": "perso", "recoveryKey": key, "password": "Encore-mot-2026!" })
            ),
            Ok(json!({ "name": "perso", "entries": 0 }))
        );
        let csv = app.dir.path().join("import.csv");
        fs::write(
            &csv,
            "title,kind,username,password,urls,notes,tags\r\nWi-Fi,note,,,,ABCD,\r\n",
        )
        .unwrap();
        assert_eq!(
            app.call(
                "import_entries",
                json!({ "path": csv.display().to_string() })
            ),
            Ok(json!(1))
        );

        let usb = app.dir.path().join("usb");
        fs::create_dir(&usb).unwrap();
        let name = app.call("backup_file_name", json!({})).unwrap();
        assert_eq!(name, "pswd-sauvegarde_1970-01-01_00-00-01.pswd");
        let file = usb.join(name.as_str().unwrap()).display().to_string();
        assert_eq!(
            app.call(
                "backup_to_file",
                json!({ "path": file, "names": ["perso"] })
            ),
            Ok(json!(1))
        );
        assert_eq!(
            app.call("backup_file_contents", json!({ "path": file })),
            Ok(json!(["perso"]))
        );
        let restore = json!({ "path": file, "name": "perso" });
        assert_eq!(
            app.call("restore_from_file", restore.clone()),
            Ok(json!("perso-sauvegarde-1970-01-01"))
        );
        assert_eq!(
            app.call("restore_from_file", restore),
            Err(json!({ "code": "vaultExists", "detail": "perso-sauvegarde-1970-01-01" }))
        );

        let settings = app.call("get_settings", json!({})).unwrap();
        assert_eq!(settings["theme"], "auto");
        let dark = json!({
            "autoLockMinutes": 5,
            "clipboardClearSeconds": 30,
            "backgroundLockSeconds": 60,
            "theme": "dark",
        });
        assert_eq!(
            app.call("save_settings", json!({ "settings": dark })),
            Ok(dark)
        );

        assert_eq!(
            app.call(
                "delete_vault",
                json!({ "name": "perso", "confirmation": "perso" })
            ),
            Ok(Value::Null)
        );
        assert_eq!(app.call("current_vault", json!({})), Ok(Value::Null));
    }

    #[test]
    fn the_server_is_reached_through_its_commands() {
        let app = TestApp::new(None);
        let (sender, receiver) = mpsc::channel();
        app.app.listen_any(VAULT_SYNCED, move |event| {
            let _ = sender.send(event.payload().to_owned());
        });
        assert_eq!(
            app.call("server_status", json!({})),
            Ok(json!({ "address": null, "trouble": null, "vaults": [] }))
        );
        let server = TestServer::start();
        let connection = json!({
            "address": server.address,
            "key": server.admin_key.expose(),
            "deviceName": "PC",
            "accountName": "Arthur",
        });
        assert_eq!(app.call("connect_server", connection), Ok(Value::Null));

        app.create("perso", "Maître-2026!").unwrap();
        let synced =
            json!({ "vault": "perso", "state": "synced", "detail": null, "changed": false });
        let report = receiver.recv_timeout(Duration::from_secs(30)).unwrap();
        assert_eq!(serde_json::from_str::<Value>(&report).unwrap(), synced);
        assert_eq!(app.call("sync_vault", json!({})), Ok(synced));
        assert_eq!(app.call("check_vault", json!({})), Ok(Value::Null));
        assert_eq!(
            app.call("sync_server", json!({})),
            Ok(json!({
                "downloaded": [],
                "notDownloaded": [],
                "published": [],
                "notPublished": [],
                "sent": [],
                "received": [],
                "needsUnlock": [],
                "notUpdated": [],
                "removed": [],
                "left": [],
                "notLeft": [],
            }))
        );
        let key = app.call("add_device_key", json!({})).unwrap();
        assert_eq!(key.as_str().unwrap().split('-').count(), 13, "{key}");
        assert_eq!(
            app.call("disconnect_server", json!({ "keepLocal": true })),
            Ok(json!({ "revokeError": null, "removed": [], "kept": [] }))
        );
        assert_eq!(
            app.call("add_device_key", json!({})),
            Err(json!({ "code": "notConnected", "detail": null }))
        );
    }

    #[test]
    fn shared_vaults_are_managed_through_their_commands() {
        let app = TestApp::new(None);
        let server = TestServer::start();
        let connection = json!({
            "address": server.address,
            "key": server.admin_key.expose(),
            "deviceName": "PC",
            "accountName": "Arthur",
        });
        app.call("connect_server", connection).unwrap();
        app.call(
            "create_vault",
            json!({ "name": "famille", "password": "Maître-2026!", "shared": true }),
        )
        .unwrap();
        app.call("sync_server", json!({})).unwrap();
        let accounts = app.call("list_accounts", json!({})).unwrap();
        assert_eq!(accounts[0]["name"], "Arthur");
        let famille = json!({ "vault": "famille" });
        let members = app.call("vault_members", famille.clone()).unwrap();
        assert_eq!(members, json!({ "members": accounts, "invitations": [] }));
        assert_eq!(app.call("list_invitations", json!({})), Ok(json!([])));
        let nobody = json!({ "id": Uuid::nil() });
        let unknown = |result: Result<Value, Value>| result.unwrap_err()["code"].clone();
        assert_eq!(
            unknown(app.call(
                "invite_member",
                json!({ "vault": "famille", "account": Uuid::nil() })
            )),
            "serverError"
        );
        assert_eq!(
            unknown(app.call("accept_invitation", nobody.clone())),
            "serverError"
        );
        assert_eq!(
            unknown(app.call("delete_invitation", nobody)),
            "serverError"
        );
        assert_eq!(app.call("leave_vault", famille.clone()), Ok(Value::Null));
        assert_eq!(
            app.call("vault_members", famille),
            Err(json!({ "code": "unknownVault", "detail": "famille" }))
        );
    }

    #[test]
    fn the_server_is_administered_through_its_commands() {
        let app = TestApp::new(None);
        let server = TestServer::start();
        let connection = json!({
            "address": server.address,
            "key": server.admin_key.expose(),
            "deviceName": "PC",
            "accountName": "Arthur",
        });
        app.call("connect_server", connection).unwrap();
        app.create("perso", "Maître-2026!").unwrap();
        app.call("sync_server", json!({})).unwrap();
        let status = app.call("account_status", json!({})).unwrap();
        assert_eq!(status["isAdmin"], true);

        let key = app
            .call("add_member", json!({ "name": "Camille" }))
            .unwrap();
        assert_eq!(key.as_str().unwrap().split('-').count(), 13, "{key}");
        let keys = app.call("pending_keys", json!({})).unwrap();
        assert_eq!(
            app.call("cancel_key", json!({ "id": keys[0]["id"] })),
            Ok(Value::Null)
        );
        let accounts = app.call("admin_accounts", json!({})).unwrap();
        let arthur = accounts[0]["id"].clone();
        let unknown = Uuid::nil();
        let code = |result: Result<Value, Value>| result.unwrap_err()["code"].clone();
        assert_eq!(
            code(app.call("set_admin", json!({ "id": arthur, "isAdmin": false }))),
            "serverError"
        );
        assert_eq!(
            code(app.call(
                "delete_account",
                json!({ "id": unknown, "name": "X", "confirmation": "X" })
            )),
            "serverError"
        );
        assert_eq!(
            code(app.call("revoke_device", json!({ "id": unknown }))),
            "serverError"
        );

        let vaults = app.call("server_vaults", json!({})).unwrap();
        let perso = vaults[0]["id"].clone();
        let versions = app
            .call("vault_versions", json!({ "vault": perso }))
            .unwrap();
        assert_eq!(versions[0]["version"], 1);
        let restored = app
            .call("restore_version", json!({ "vault": perso, "version": 1 }))
            .unwrap();
        assert_eq!(restored["name"], "perso-v1");
        // Un coffre privé garde son propriétaire, et n'a donc pas de place pour un autre.
        let member = json!({ "vault": restored["id"], "account": arthur });
        assert_eq!(
            code(app.call("remove_member", member.clone())),
            "serverError"
        );
        assert_eq!(code(app.call("attach_account", member)), "serverError");
        let deletion =
            json!({ "id": restored["id"], "name": "perso-v1", "confirmation": "perso-v1" });
        assert_eq!(app.call("delete_server_vault", deletion), Ok(Value::Null));
        assert_eq!(
            app.call("set_public_address", json!({ "address": "maison" })),
            Ok(Value::Null)
        );

        let name = app.call("server_backup_file_name", json!({})).unwrap();
        assert_eq!(name, "pswd-server-sauvegarde_1970-01-01_00-00-01.db");
        let file = app.dir.path().join("serveur.db");
        let path = json!({ "path": file.display().to_string() });
        assert_eq!(app.call("backup_server_to_file", path), Ok(Value::Null));
        assert!(fs::read(&file).unwrap().starts_with(b"SQLite format 3\0"));
        let elsewhere = app.dir.path().join("absent").join("serveur.db");
        let path = json!({ "path": elsewhere.display().to_string() });
        assert_eq!(
            code(app.call("backup_server_to_file", path)),
            "fileUnwritable"
        );

        let everywhere = json!({ "name": "perso", "confirmation": "perso" });
        assert_eq!(
            app.call("delete_vault_everywhere", everywhere.clone()),
            Ok(Value::Null)
        );
        assert_eq!(app.call("list_vaults", json!({})), Ok(json!([])));
        app.call("disconnect_server", json!({ "keepLocal": true }))
            .unwrap();
        assert_eq!(
            code(app.call("backup_server_to_file", json!({ "path": "x" }))),
            "notConnected"
        );
        assert_eq!(
            code(app.call("delete_vault_everywhere", everywhere)),
            "notConnected"
        );
    }

    /// Commande bien enregistrée, mais ni déclarée dans `build.rs` ni autorisée par une capacité.
    #[tauri::command]
    fn not_allowed() -> &'static str {
        "secret"
    }

    #[test]
    fn commands_outside_the_capabilities_are_refused() {
        assert_eq!(not_allowed(), "secret");
        let app = TestApp::new(None);
        let refused = |command: &str, arguments: Value| {
            let refusal = app.call(command, arguments).unwrap_err();
            assert!(
                refusal
                    .as_str()
                    .is_some_and(|message| message.contains("not allowed")),
                "{command} : {refusal}"
            );
        };
        refused("not_allowed", json!({}));
        // Bouton retour et fermeture : capacité réservée à Android.
        refused(
            "plugin:app|register_listener",
            json!({ "event": "back-button", "handler": 0 }),
        );
        // Le plugin `fs` sert au backend seul : l'interface ne lit ni n'écrit aucun fichier.
        let file = app.config_file();
        fs::create_dir_all(file.parent().unwrap()).unwrap();
        fs::write(&file, "").unwrap();
        let path = file.display().to_string();
        refused("plugin:fs|read_file", json!({ "path": path }));
        refused("plugin:fs|read_text_file", json!({ "path": path }));
        refused("plugin:fs|write_file", json!({ "path": path }));
        refused("plugin:fs|remove", json!({ "path": path }));
        assert!(file.exists());
    }
}
