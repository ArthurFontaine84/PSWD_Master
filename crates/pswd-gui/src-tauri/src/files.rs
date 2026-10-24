//! Fichiers choisis par l'utilisateur dans une fenêtre système : lus et écrits par Rust, leur
//! contenu ne passe jamais par l'interface.

use std::fs::{self, File};
use std::io::Write;
use std::path::Path;

use crate::error::UiError;

pub fn read(path: &Path) -> Result<Vec<u8>, UiError> {
    fs::read(path).map_err(|error| UiError::new("fileUnreadable", error.to_string()))
}

/// Le fichier est sur le disque au retour : une clé USB peut être retirée aussitôt. Un fichier
/// existant est remplacé : la fenêtre système d'enregistrement l'a déjà fait confirmer.
pub fn write(path: &Path, data: &[u8]) -> Result<(), UiError> {
    File::create(path)
        .and_then(|mut file| {
            file.write_all(data)?;
            file.sync_all()
        })
        .map_err(|error| UiError::new("fileUnwritable", error.to_string()))
}

#[cfg(test)]
mod tests {
    use tempfile::TempDir;

    use super::*;

    #[test]
    fn written_file_is_read_back() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("sauvegarde.pswd");
        fs::write(&path, "ancien contenu plus long").unwrap();
        write(&path, b"octets").unwrap();
        assert_eq!(read(&path).unwrap(), b"octets");
    }

    #[test]
    fn missing_folder_or_file_is_reported() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("absent").join("sauvegarde.pswd");
        let code = |error: UiError| serde_json::to_value(error).unwrap()["code"].clone();
        assert_eq!(code(write(&path, b"x").unwrap_err()), "fileUnwritable");
        assert_eq!(code(read(&path).unwrap_err()), "fileUnreadable");
    }
}
