//! Sauvegardes sur disque externe : copie brute des fichiers chiffrés, un dossier daté par
//! sauvegarde.

use std::fs::{self, File};
use std::io::{self, Write};
use std::path::{Path, PathBuf};

use pswd_core::ports::PortError;

use crate::VaultConfig;
use crate::file_store::io_error;

/// Dossier créé à la racine de la destination.
pub const BACKUPS_DIR: &str = "pswd-sauvegardes";
const VAULTS_DIR: &str = "vaults";
const VAULT_FILE: &str = "vault.enc";

/// Copie chaque coffre dans `<destination>/pswd-sauvegardes/<stamp>/vaults/<nom>/vault.enc` et
/// renvoie le dossier de la sauvegarde. Une sauvegarde existante n'est jamais modifiée ; en cas
/// d'échec, la sauvegarde incomplète est retirée.
///
/// # Errors
///
/// Si la destination n'est pas un dossier existant, si la sauvegarde `stamp` existe déjà, ou si
/// une copie échoue.
pub fn create_backup(
    destination: &Path,
    stamp: &str,
    vaults: &[(&str, &VaultConfig)],
) -> Result<PathBuf, PortError> {
    // Sans cette vérification, une clé USB débranchée donnerait une sauvegarde sur le disque
    // de la machine (sous Linux, le point de montage est un dossier ordinaire).
    if !destination.is_dir() {
        return Err(PortError::new(format!(
            "dossier de destination introuvable : {}",
            destination.display()
        )));
    }
    let backups = destination.join(BACKUPS_DIR);
    fs::create_dir_all(&backups).map_err(|error| io_error("création", &backups, &error))?;
    let backup = backups.join(stamp);
    if let Err(error) = fs::create_dir(&backup) {
        return Err(if error.kind() == io::ErrorKind::AlreadyExists {
            PortError::new(format!("la sauvegarde {} existe déjà", backup.display()))
        } else {
            io_error("création", &backup, &error)
        });
    }
    if let Err(error) = copy_vaults(&backup, vaults) {
        // Au mieux : l'erreur de copie est plus utile que celle du nettoyage.
        let _ = fs::remove_dir_all(&backup);
        return Err(error);
    }
    Ok(backup)
}

fn copy_vaults(backup: &Path, vaults: &[(&str, &VaultConfig)]) -> Result<(), PortError> {
    for (name, vault) in vaults {
        let source = vault.vault_file();
        let file = fs::read(&source).map_err(|error| io_error("lecture", &source, &error))?;
        let target = backup_vault_file(backup, name);
        write_new(&target, &file).map_err(|error| io_error("écriture", &target, &error))?;
    }
    Ok(())
}

/// Le fichier est sur le disque au retour : la clé peut être retirée aussitôt.
fn write_new(path: &Path, file: &[u8]) -> io::Result<()> {
    path.parent().map_or(Ok(()), fs::create_dir_all)?;
    let mut output = File::create_new(path)?;
    output.write_all(file)?;
    output.sync_all()
}

/// Coffres contenus dans une sauvegarde, triés par nom. Une entrée illisible est ignorée.
///
/// # Errors
///
/// Si `backup` n'est pas un dossier de sauvegarde lisible.
pub fn backup_vault_names(backup: &Path) -> Result<Vec<String>, PortError> {
    let entries = fs::read_dir(backup.join(VAULTS_DIR)).map_err(|_| {
        PortError::new(format!(
            "{} n'est pas une sauvegarde pswd (choisir un dossier daté)",
            backup.display()
        ))
    })?;
    let mut names = Vec::new();
    for entry in entries.flatten() {
        if entry.path().join(VAULT_FILE).is_file() {
            names.push(entry.file_name().to_string_lossy().into_owned());
        }
    }
    names.sort();
    Ok(names)
}

/// Fichier `vault.enc` du coffre `name` dans une sauvegarde.
#[must_use]
pub fn backup_vault_file(backup: &Path, name: &str) -> PathBuf {
    backup.join(VAULTS_DIR).join(name).join(VAULT_FILE)
}

#[cfg(test)]
mod tests {
    use tempfile::TempDir;

    use super::*;

    const STAMP: &str = "2026-10-23_14-30-00";

    /// Un coffre `name` sur la machine, dont le fichier contient `content`.
    fn local_vault(dir: &TempDir, name: &str, content: &[u8]) -> VaultConfig {
        let vault = VaultConfig {
            path: dir.path().join("machine").join(name),
        };
        fs::create_dir_all(&vault.path).unwrap();
        fs::write(vault.vault_file(), content).unwrap();
        vault
    }

    /// Dossier qui tient lieu de clé USB.
    fn usb_key(dir: &TempDir) -> PathBuf {
        let usb = dir.path().join("usb");
        fs::create_dir(&usb).unwrap();
        usb
    }

    #[test]
    fn every_vault_is_copied_into_a_dated_folder() {
        let dir = TempDir::new().unwrap();
        let perso = local_vault(&dir, "perso", b"coffre perso");
        let travail = local_vault(&dir, "travail", b"coffre travail");
        let usb = usb_key(&dir);
        let backup =
            create_backup(&usb, STAMP, &[("travail", &travail), ("perso", &perso)]).unwrap();
        assert_eq!(backup, usb.join("pswd-sauvegardes").join(STAMP));
        assert_eq!(
            fs::read(backup.join("vaults").join("perso").join("vault.enc")).unwrap(),
            b"coffre perso"
        );
        assert_eq!(
            fs::read(backup_vault_file(&backup, "travail")).unwrap(),
            b"coffre travail"
        );
        assert_eq!(backup_vault_names(&backup).unwrap(), ["perso", "travail"]);
    }

    #[test]
    fn missing_destination_is_refused() {
        let dir = TempDir::new().unwrap();
        let perso = local_vault(&dir, "perso", b"coffre");
        let unplugged = dir.path().join("usb");
        let error = create_backup(&unplugged, STAMP, &[("perso", &perso)]).unwrap_err();
        assert!(
            error
                .to_string()
                .starts_with("dossier de destination introuvable"),
            "{error}"
        );
        assert!(!unplugged.exists());
    }

    #[test]
    fn existing_backup_is_never_modified() {
        let dir = TempDir::new().unwrap();
        let perso = local_vault(&dir, "perso", b"ancien");
        let usb = usb_key(&dir);
        let backup = create_backup(&usb, STAMP, &[("perso", &perso)]).unwrap();
        fs::write(perso.vault_file(), b"nouveau").unwrap();
        let error = create_backup(&usb, STAMP, &[("perso", &perso)]).unwrap_err();
        assert!(error.to_string().ends_with("existe déjà"), "{error}");
        assert_eq!(
            fs::read(backup_vault_file(&backup, "perso")).unwrap(),
            b"ancien"
        );
    }

    #[test]
    fn failed_copy_removes_the_incomplete_backup() {
        let dir = TempDir::new().unwrap();
        let perso = local_vault(&dir, "perso", b"coffre");
        let missing = VaultConfig {
            path: dir.path().join("absent"),
        };
        let usb = usb_key(&dir);
        let error =
            create_backup(&usb, STAMP, &[("perso", &perso), ("absent", &missing)]).unwrap_err();
        assert!(error.to_string().starts_with("lecture de "), "{error}");
        assert!(!usb.join("pswd-sauvegardes").join(STAMP).exists());
    }

    #[test]
    fn copy_never_overwrites_a_file() {
        let dir = TempDir::new().unwrap();
        let perso = local_vault(&dir, "perso", b"coffre");
        let usb = usb_key(&dir);
        let error =
            create_backup(&usb, STAMP, &[("perso", &perso), ("perso", &perso)]).unwrap_err();
        assert!(error.to_string().starts_with("écriture de "), "{error}");
    }

    #[test]
    fn backups_folder_that_cannot_be_created_is_reported() {
        let dir = TempDir::new().unwrap();
        let perso = local_vault(&dir, "perso", b"coffre");
        let usb = usb_key(&dir);
        fs::write(usb.join("pswd-sauvegardes"), b"").unwrap();
        let error = create_backup(&usb, STAMP, &[("perso", &perso)]).unwrap_err();
        assert!(error.to_string().starts_with("création de "), "{error}");
    }

    #[test]
    fn dated_folder_that_cannot_be_created_is_reported() {
        let dir = TempDir::new().unwrap();
        let perso = local_vault(&dir, "perso", b"coffre");
        let usb = usb_key(&dir);
        // Dossier parent absent : `create_dir` échoue pour une autre raison qu'un doublon.
        let error = create_backup(&usb, "absent/2026", &[("perso", &perso)]).unwrap_err();
        assert!(error.to_string().starts_with("création de "), "{error}");
    }

    #[test]
    fn only_folders_holding_a_vault_are_listed() {
        let dir = TempDir::new().unwrap();
        let perso = local_vault(&dir, "perso", b"coffre");
        let usb = usb_key(&dir);
        let backup = create_backup(&usb, STAMP, &[("perso", &perso)]).unwrap();
        fs::create_dir(backup.join("vaults").join("vide")).unwrap();
        fs::write(backup.join("vaults").join("note.txt"), b"").unwrap();
        assert_eq!(backup_vault_names(&backup).unwrap(), ["perso"]);
    }

    #[test]
    fn folder_without_vaults_is_not_a_backup() {
        let dir = TempDir::new().unwrap();
        let usb = usb_key(&dir);
        let error = backup_vault_names(&usb).unwrap_err();
        assert!(
            error
                .to_string()
                .ends_with("n'est pas une sauvegarde pswd (choisir un dossier daté)"),
            "{error}"
        );
    }
}
