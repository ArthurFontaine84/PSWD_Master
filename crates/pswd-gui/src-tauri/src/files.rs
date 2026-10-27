//! Fichiers choisis par l'utilisateur dans une fenêtre système : lus et écrits par Rust, leur
//! contenu ne passe jamais par l'interface. Le plugin `fs` ouvre aussi bien un chemin du bureau
//! qu'une adresse `content://` d'Android ; l'interface n'a accès à aucune de ses commandes.

use std::io::{Read, Write};

use tauri::{AppHandle, Runtime};
use tauri_plugin_fs::{FilePath, FsExt, OpenOptions};

use crate::error::UiError;

pub fn read<R: Runtime>(app: &AppHandle<R>, path: &str) -> Result<Vec<u8>, UiError> {
    let mut options = OpenOptions::new();
    options.read(true);
    let mut data = Vec::new();
    app.fs()
        .open(file_path(path), options)
        .and_then(|mut file| file.read_to_end(&mut data))
        .map_err(|error| UiError::new("fileUnreadable", error.to_string()))?;
    Ok(data)
}

/// Le fichier est sur le disque au retour : une clé USB peut être retirée aussitôt. Un fichier
/// existant est remplacé : la fenêtre système d'enregistrement l'a déjà fait confirmer.
pub fn write<R: Runtime>(app: &AppHandle<R>, path: &str, data: &[u8]) -> Result<(), UiError> {
    let mut options = OpenOptions::new();
    options.write(true).create(true).truncate(true);
    app.fs()
        .open(file_path(path), options)
        .and_then(|mut file| {
            file.write_all(data)?;
            file.sync_all()
        })
        .map_err(|error| UiError::new("fileUnwritable", error.to_string()))
}

/// Une adresse (`content://`, `file://`) ou un chemin ordinaire.
fn file_path(path: &str) -> FilePath {
    let Ok(path) = path.parse();
    path
}

#[cfg(test)]
mod tests {
    use std::fs;

    use tauri::test::{mock_builder, mock_context, noop_assets};
    use tempfile::TempDir;

    use super::*;

    fn app() -> tauri::App<tauri::test::MockRuntime> {
        mock_builder()
            .plugin(tauri_plugin_fs::init())
            .build(mock_context(noop_assets()))
            .unwrap()
    }

    fn code(error: UiError) -> serde_json::Value {
        serde_json::to_value(error).unwrap()["code"].clone()
    }

    #[test]
    fn written_file_replaces_the_old_one_and_is_read_back() {
        let (app, dir) = (app(), TempDir::new().unwrap());
        let path = dir.path().join("sauvegarde.pswd");
        fs::write(&path, "ancien contenu plus long").unwrap();
        let path = path.display().to_string();
        write(app.handle(), &path, b"octets").unwrap();
        assert_eq!(read(app.handle(), &path).unwrap(), b"octets");
    }

    #[test]
    fn file_urls_are_understood() {
        let (app, dir) = (app(), TempDir::new().unwrap());
        let path = dir.path().join("sauvegarde.pswd");
        fs::write(&path, "octets").unwrap();
        let url = tauri::Url::from_file_path(&path).unwrap().to_string();
        assert!(url.starts_with("file://"), "{url}");
        assert_eq!(read(app.handle(), &url).unwrap(), b"octets");
    }

    #[test]
    fn missing_folder_or_file_is_reported() {
        let (app, dir) = (app(), TempDir::new().unwrap());
        let path = dir.path().join("absent").join("sauvegarde.pswd");
        let path = path.display().to_string();
        assert_eq!(
            code(write(app.handle(), &path, b"x").unwrap_err()),
            "fileUnwritable"
        );
        assert_eq!(
            code(read(app.handle(), &path).unwrap_err()),
            "fileUnreadable"
        );
    }
}
