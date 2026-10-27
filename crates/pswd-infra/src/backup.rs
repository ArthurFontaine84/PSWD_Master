//! Sauvegardes sur disque externe : un fichier `.pswd` par sauvegarde, jamais remplacé.

use std::fs::{self, File};
use std::io::{self, Write};
use std::path::{Path, PathBuf};

use pswd_core::ports::PortError;

use crate::file_store::io_error;

/// Écrit la sauvegarde `backup` dans un nouveau fichier `file_name` de `destination` et renvoie
/// son chemin. Le fichier est sur le disque au retour : la clé peut être retirée aussitôt.
///
/// # Errors
///
/// Si la destination n'est pas un dossier existant, si le fichier existe déjà (il n'est jamais
/// remplacé), ou si l'écriture échoue (le fichier incomplet est alors retiré).
pub fn write_backup(
    destination: &Path,
    file_name: &str,
    backup: &[u8],
) -> Result<PathBuf, PortError> {
    // Sans cette vérification, une clé USB débranchée donnerait une sauvegarde sur le disque
    // de la machine (sous Linux, le point de montage est un dossier ordinaire).
    if !destination.is_dir() {
        return Err(PortError::new(format!(
            "dossier de destination introuvable : {}",
            destination.display()
        )));
    }
    let path = destination.join(file_name);
    let mut output = File::create_new(&path).map_err(|error| {
        if error.kind() == io::ErrorKind::AlreadyExists {
            PortError::new(format!("la sauvegarde {} existe déjà", path.display()))
        } else {
            io_error("création", &path, &error)
        }
    })?;
    let written = output.write_all(backup).and_then(|()| output.sync_all());
    // Fermé avant d'être retiré : Windows refuse de supprimer un fichier ouvert.
    drop(output);
    written.map_err(discard_partial(&path))?;
    Ok(path)
}

/// En cas d'échec d'écriture : retire le fichier de sauvegarde à moitié écrit, et renvoie
/// l'erreur d'écriture.
// Exclu de la couverture : une écriture qui échoue après la création du fichier (disque plein,
// clé retirée pendant la copie) ne se provoque pas dans un test.
#[cfg_attr(coverage_nightly, coverage(off))]
fn discard_partial(path: &Path) -> impl FnOnce(io::Error) -> PortError + '_ {
    move |error| {
        // Au mieux : l'erreur d'écriture est plus utile que celle du nettoyage.
        let _ = fs::remove_file(path);
        io_error("écriture", path, &error)
    }
}

/// Contenu d'un fichier de sauvegarde.
///
/// # Errors
///
/// Si le fichier est introuvable ou illisible.
pub fn read_backup(path: &Path) -> Result<Vec<u8>, PortError> {
    fs::read(path).map_err(|error| io_error("lecture", path, &error))
}

#[cfg(test)]
mod tests {
    use tempfile::TempDir;

    use super::*;

    const NAME: &str = "pswd-sauvegarde_2026-10-23_14-30-00.pswd";

    /// Dossier qui tient lieu de clé USB.
    fn usb_key(dir: &TempDir) -> PathBuf {
        let usb = dir.path().join("usb");
        fs::create_dir(&usb).unwrap();
        usb
    }

    #[test]
    fn backup_is_written_then_read_back() {
        let dir = TempDir::new().unwrap();
        let usb = usb_key(&dir);
        let path = write_backup(&usb, NAME, b"sauvegarde").unwrap();
        assert_eq!(path, usb.join(NAME));
        assert_eq!(read_backup(&path).unwrap(), b"sauvegarde");
    }

    #[test]
    fn missing_destination_is_refused() {
        let dir = TempDir::new().unwrap();
        let missing = dir.path().join("débranchée");
        let error = write_backup(&missing, NAME, b"x").unwrap_err();
        assert_eq!(
            error.to_string(),
            format!("dossier de destination introuvable : {}", missing.display())
        );
        assert!(!missing.exists());
    }

    #[test]
    fn existing_backup_is_never_replaced() {
        let dir = TempDir::new().unwrap();
        let usb = usb_key(&dir);
        write_backup(&usb, NAME, b"premiere").unwrap();
        let error = write_backup(&usb, NAME, b"seconde").unwrap_err();
        assert_eq!(
            error.to_string(),
            format!("la sauvegarde {} existe déjà", usb.join(NAME).display())
        );
        assert_eq!(fs::read(usb.join(NAME)).unwrap(), b"premiere");
    }

    #[test]
    fn file_that_cannot_be_created_is_reported() {
        let dir = TempDir::new().unwrap();
        let usb = usb_key(&dir);
        let error = write_backup(&usb, "absent/sauvegarde.pswd", b"x").unwrap_err();
        assert!(error.to_string().starts_with("création de "), "{error}");
    }

    #[test]
    fn missing_backup_cannot_be_read() {
        let dir = TempDir::new().unwrap();
        let error = read_backup(&dir.path().join(NAME)).unwrap_err();
        assert!(error.to_string().starts_with("lecture de "), "{error}");
    }
}
