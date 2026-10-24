use std::env;
use std::path::Path;

// Seules les commandes déclarées ici sont appelables depuis l'interface, et seulement si une
// capacité (`capabilities/`) les autorise.
const COMMANDS: &[&str] = &[
    "device_kind",
    "list_vaults",
    "create_vault",
    "unlock_vault",
    "lock_vault",
    "current_vault",
    "app_hidden",
    "app_shown",
    "list_entries",
    "entry_detail",
    "list_tags",
    "reveal_password",
    "copy_password",
    "add_entry",
    "update_entry",
    "delete_entry",
    "entry_history",
    "change_password",
    "recover_vault",
    "delete_vault",
    "import_entries",
    "backup_vaults",
    "backup_contents",
    "restore_vault",
    "backup_file_name",
    "backup_to_file",
    "backup_file_contents",
    "restore_from_file",
    "get_settings",
    "save_settings",
];

fn main() {
    let manifest = tauri_build::AppManifest::new().commands(COMMANDS);
    let windows = tauri_build::WindowsAttributes::new_without_app_manifest();
    let attributes = tauri_build::Attributes::new()
        .app_manifest(manifest)
        .windows_attributes(windows);
    if let Err(error) = tauri_build::try_build(attributes) {
        eprintln!("{error:#}");
        std::process::exit(1);
    }
    embed_windows_manifest();
}

/// `tauri-build` ne lie son manifeste Windows (contrôles communs v6) qu'au binaire de
/// l'application ; sans lui, les exécutables de test ne démarrent pas
/// (`STATUS_ENTRYPOINT_NOT_FOUND`). On le lie donc à tous les exécutables.
fn embed_windows_manifest() {
    let target_os = env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();
    let target_env = env::var("CARGO_CFG_TARGET_ENV").unwrap_or_default();
    if target_os != "windows" || target_env != "msvc" {
        return;
    }
    let dir = env::var("CARGO_MANIFEST_DIR").unwrap_or_default();
    let manifest = Path::new(&dir).join("windows-app-manifest.xml");
    println!("cargo:rerun-if-changed={}", manifest.display());
    println!("cargo:rustc-link-arg=/MANIFEST:EMBED");
    println!("cargo:rustc-link-arg=/MANIFESTINPUT:{}", manifest.display());
}
