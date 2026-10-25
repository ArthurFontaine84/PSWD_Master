//! Commandes appelables depuis l'interface (déclarées aussi dans `build.rs`).

// Tauri impose de recevoir l'état (`State`) et les arguments par valeur.
#![allow(clippy::needless_pass_by_value)]

use pswd_core::Secret;
use serde::Serialize;
use tauri::{AppHandle, Emitter, Runtime, State};

use crate::editing::{EntryInput, HistoryItem};
use crate::entries::{EntryDetail, EntrySummary, TagCount};
use crate::error::UiError;
use crate::files;
use crate::settings::Settings;
use crate::state::{AppState, VaultInfo};

/// Événement envoyé à l'interface quand le coffre se verrouille par inactivité.
pub const VAULT_LOCKED: &str = "vault-locked";

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
) -> Result<CreatedVault, UiError> {
    let password = Secret::new(password);
    let (vault, recovery_key) = state.create(&name, &password)?;
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

#[tauri::command]
pub fn delete_vault(
    state: State<'_, AppState>,
    name: String,
    confirmation: String,
) -> Result<(), UiError> {
    state.delete_vault(&name, &confirmation)
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

/// Tous les coffres dans le fichier `.pswd` choisi, écrit par Rust ; renvoie leur nombre.
#[tauri::command]
pub fn backup_to_file<R: Runtime>(
    app: AppHandle<R>,
    state: State<'_, AppState>,
    path: String,
) -> Result<usize, UiError> {
    let (data, count) = state.backup_file()?;
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

/// Prévient l'interface quand le coffre se verrouille par inactivité.
pub fn lock_notifier<R: Runtime>(app: AppHandle<R>) -> impl Fn() + Send + Sync + 'static {
    move || {
        // Au mieux : si la fenêtre est déjà fermée, il n'y a plus personne à prévenir.
        let _ = app.emit(VAULT_LOCKED, ());
    }
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::sync::mpsc;
    use std::time::Duration;

    use pswd_app::Vaults;
    use pswd_core::crypto::KdfParams;
    use pswd_core::ports::{PortError, SecureClipboard};
    use pswd_core::{EntryContent, EntryKind, Tags, Timestamp};
    use pswd_infra::Locations;
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
                .invoke_handler(tauri::generate_handler![
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
                    not_allowed
                ])
                .build(tauri::generate_context!())
                .unwrap();
            app.manage(AppState::with_settings(
                vaults,
                Box::new(FakeClipboard),
                kdf,
                || Timestamp::from_millis(1_000),
                auto_lock,
                true,
                lock_notifier(app.handle().clone()),
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
                json!({ "name": name, "password": password }),
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
        app.create("travail", "maître").unwrap();
        app.create("perso", "maître").unwrap();
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
        let created = app.create("perso", "maître").unwrap();
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
        app.create("perso", "maître").unwrap();
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
                json!({ "name": "perso", "password": "maître" })
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
        app.create("perso", "maître").unwrap();
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
        app.create("perso", "maître").unwrap();
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

    /// Presse-papiers qui accepte tout, sans rien garder.
    struct FakeClipboard;

    impl SecureClipboard for FakeClipboard {
        fn copy_secret(&mut self, _: &Secret, _: Duration) -> Result<(), PortError> {
            Ok(())
        }
    }

    #[test]
    fn existing_entries_are_returned_without_their_password() {
        let app = TestApp::new(None);
        app.create("perso", "maître").unwrap();
        let state = app.app.state::<AppState>();
        state.lock();
        let (mut session, _) = state
            .vaults
            .create(
                "travail",
                &Secret::new("maître".to_owned()),
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
            json!({ "name": "travail", "password": "maître" }),
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
        app.create("perso", "maître").unwrap();
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
        app.create("perso", "maître").unwrap();
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
        let key = app.create("perso", "maître").unwrap()["recoveryKey"].clone();
        assert_eq!(
            app.call(
                "change_password",
                json!({ "current": "maître", "password": "nouveau" })
            ),
            Ok(Value::Null)
        );
        app.call("lock_vault", json!({})).unwrap();
        assert_eq!(
            app.call(
                "recover_vault",
                json!({ "name": "perso", "recoveryKey": key, "password": "encore" })
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
            app.call("backup_to_file", json!({ "path": file })),
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
