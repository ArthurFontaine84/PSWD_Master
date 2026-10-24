//! PSWD Master : le backend Rust de l'interface graphique. Les commandes appelées par
//! l'interface s'appuient sur `pswd-app` ; les secrets ne quittent Rust que sur demande
//! explicite de l'utilisateur (SPEC §9).

#![forbid(unsafe_code)]
#![cfg_attr(coverage_nightly, feature(coverage_attribute))]

mod commands;
mod error;
mod state;

use std::error::Error;

use pswd_app::Vaults;
use pswd_infra::Locations;
use tauri::Manager;

use crate::state::AppState;

/// Démarre l'application : sur le bureau par `main`, sur mobile par le code natif généré.
// Exclu de la couverture : ouvre une vraie fenêtre ; les commandes sont testées avec le moteur
// factice de Tauri.
#[cfg_attr(mobile, tauri::mobile_entry_point)]
#[cfg_attr(coverage_nightly, coverage(off))]
pub fn run() {
    let result = tauri::Builder::default()
        .setup(|app| {
            let locations = locations(app.handle())?;
            app.manage(AppState::new(Vaults::new(locations)));
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![commands::list_vaults])
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
