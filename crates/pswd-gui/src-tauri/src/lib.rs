//! PSWD Master : le backend Rust de l'interface graphique. Les commandes appelées par
//! l'interface s'appuient sur `pswd-app` ; les secrets ne quittent Rust que sur demande
//! explicite de l'utilisateur (SPEC §9).

#![forbid(unsafe_code)]
#![cfg_attr(coverage_nightly, feature(coverage_attribute))]

mod admin;
#[cfg(target_os = "android")]
mod android_clipboard;
mod commands;
mod editing;
mod entries;
mod error;
mod files;
mod server;
mod settings;
mod sharing;
mod state;
mod sync;
#[cfg(test)]
mod testing;

use std::error::Error;

use pswd_app::Vaults;
use pswd_infra::Locations;
use tauri::Manager;

use crate::state::{AppState, Clipboard};

/// Démarre l'application : sur le bureau par `main`, sur mobile par le code natif généré.
// Exclu de la couverture : ouvre une vraie fenêtre ; les commandes sont testées avec le moteur
// factice de Tauri.
#[cfg_attr(mobile, tauri::mobile_entry_point)]
#[cfg_attr(coverage_nightly, coverage(off))]
pub fn run() {
    let builder = tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_fs::init());
    #[cfg(target_os = "android")]
    let builder = builder.plugin(android_clipboard::plugin());
    let result = builder
        .setup(|app| {
            let locations = locations(app.handle())?;
            let events = commands::Events(app.handle().clone());
            let clipboard = clipboard(app.handle());
            app.manage(AppState::new(Vaults::new(locations), clipboard, events));
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::device_kind,
            commands::list_vaults,
            commands::create_vault,
            commands::unlock_vault,
            commands::lock_vault,
            commands::current_vault,
            commands::app_hidden,
            commands::app_shown,
            commands::list_entries,
            commands::entry_detail,
            commands::list_tags,
            commands::reveal_password,
            commands::copy_password,
            commands::add_entry,
            commands::update_entry,
            commands::delete_entry,
            commands::entry_history,
            commands::change_password,
            commands::recover_vault,
            commands::delete_vault,
            commands::import_entries,
            commands::backup_file_name,
            commands::backup_to_file,
            commands::backup_file_contents,
            commands::restore_from_file,
            commands::get_settings,
            commands::save_settings,
            commands::server_status,
            commands::connect_server,
            commands::sync_server,
            commands::add_device_key,
            commands::disconnect_server,
            commands::list_accounts,
            commands::vault_members,
            commands::invite_member,
            commands::list_invitations,
            commands::accept_invitation,
            commands::delete_invitation,
            commands::leave_vault,
            commands::sync_vault,
            commands::account_status,
            commands::delete_vault_everywhere,
            commands::admin_accounts,
            commands::add_member,
            commands::delete_account,
            commands::set_admin,
            commands::revoke_device,
            commands::pending_keys,
            commands::cancel_key,
            commands::server_vaults,
            commands::delete_server_vault,
            commands::attach_account,
            commands::remove_member,
            commands::vault_versions,
            commands::restore_version,
            commands::set_public_address,
            commands::server_backup_file_name,
            commands::backup_server_to_file
        ])
        .run(tauri::generate_context!());
    if let Err(error) = result {
        eprintln!("PSWD Master : {error}");
        std::process::exit(1);
    }
}

/// Sur le bureau, les dossiers de la CLI : les deux programmes voient les mêmes coffres.
#[cfg(desktop)]
#[cfg_attr(coverage_nightly, coverage(off))]
fn locations(_: &tauri::AppHandle) -> Result<Locations, Box<dyn Error>> {
    Ok(Locations::system()?)
}

/// Sur mobile, le dossier privé de l'application.
#[cfg(mobile)]
#[cfg_attr(coverage_nightly, coverage(off))]
fn locations(app: &tauri::AppHandle) -> Result<Locations, Box<dyn Error>> {
    let dir = app.path().app_data_dir()?;
    Ok(Locations::new(&dir.join("config"), &dir.join("data")))
}

/// Sur le bureau, le presse-papiers du système, ouvert à la première copie.
#[cfg(desktop)]
#[cfg_attr(coverage_nightly, coverage(off))]
fn clipboard(_: &tauri::AppHandle) -> Clipboard {
    Box::new(pswd_infra::LazyClipboard::default())
}

/// Sur Android, le plugin natif, enregistré avant le démarrage de l'application.
#[cfg(target_os = "android")]
#[cfg_attr(coverage_nightly, coverage(off))]
fn clipboard(app: &tauri::AppHandle) -> Clipboard {
    Box::new(
        app.state::<android_clipboard::AndroidClipboard<tauri::Wry>>()
            .inner()
            .clone(),
    )
}

/// Sur iOS, pas encore de presse-papiers : il doit marquer le contenu comme sensible.
#[cfg(target_os = "ios")]
#[cfg_attr(coverage_nightly, coverage(off))]
fn clipboard(_: &tauri::AppHandle) -> Clipboard {
    Box::new(UnavailableClipboard)
}

// EXTENSION(mobile): plugin natif Swift qui pose les marqueurs « sensible » (SPEC §12).
#[cfg(target_os = "ios")]
struct UnavailableClipboard;

#[cfg(target_os = "ios")]
#[cfg_attr(coverage_nightly, coverage(off))]
impl pswd_core::ports::SecureClipboard for UnavailableClipboard {
    fn copy_secret(
        &mut self,
        _: &pswd_core::Secret,
        _: std::time::Duration,
    ) -> Result<(), pswd_core::ports::PortError> {
        Err(pswd_core::ports::PortError::new(
            "copie pas encore disponible sur mobile",
        ))
    }
}
