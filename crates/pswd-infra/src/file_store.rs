use std::fs::{self, File};
use std::io::{self, Write};
use std::path::{Path, PathBuf};

use pswd_core::ports::{LocalStore, PortError};

/// Copie locale du coffre : un fichier `vault.enc` sur le disque.
#[derive(Debug, Clone)]
pub struct FileStore {
    path: PathBuf,
}

impl FileStore {
    #[must_use]
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self { path: path.into() }
    }

    #[must_use]
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Supprime le fichier, et un éventuel fichier temporaire laissé par une écriture
    /// interrompue. Un fichier déjà absent n'est pas une erreur.
    ///
    /// # Errors
    ///
    /// Si un fichier existe mais ne peut pas être supprimé.
    pub fn remove(&self) -> Result<(), PortError> {
        for path in [self.temporary_path(), self.path.clone()] {
            match fs::remove_file(&path) {
                Ok(()) => {}
                Err(error) if error.kind() == io::ErrorKind::NotFound => {}
                Err(error) => return Err(io_error("suppression", &path, &error)),
            }
        }
        Ok(())
    }

    fn temporary_path(&self) -> PathBuf {
        let mut name = self.path.as_os_str().to_owned();
        name.push(".tmp");
        PathBuf::from(name)
    }

    fn write_then_replace(&self, file: &[u8]) -> io::Result<()> {
        self.path.parent().map_or(Ok(()), fs::create_dir_all)?;
        let temporary = self.temporary_path();
        let mut output = File::create(&temporary)?;
        output.write_all(file)?;
        // Les octets sont sur le disque avant le renommage : une coupure de courant ne laisse
        // jamais un `vault.enc` incomplet.
        output.sync_all()?;
        drop(output);
        fs::rename(&temporary, &self.path)
    }
}

impl LocalStore for FileStore {
    fn load(&self) -> Result<Option<Vec<u8>>, PortError> {
        match fs::read(&self.path) {
            Ok(file) => Ok(Some(file)),
            Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(None),
            Err(error) => Err(io_error("lecture", &self.path, &error)),
        }
    }

    fn save_atomic(&mut self, file: &[u8]) -> Result<(), PortError> {
        self.write_then_replace(file)
            .map_err(|error| io_error("écriture", &self.path, &error))
    }
}

pub(crate) fn io_error(action: &str, path: &Path, error: &io::Error) -> PortError {
    PortError::new(format!(
        "{action} de {} impossible : {error}",
        path.display()
    ))
}

#[cfg(test)]
mod tests {
    use tempfile::TempDir;

    use super::*;

    fn store_in(dir: &TempDir) -> FileStore {
        FileStore::new(dir.path().join("vault.enc"))
    }

    #[test]
    fn missing_file_means_no_vault() {
        let dir = TempDir::new().unwrap();
        assert_eq!(store_in(&dir).load(), Ok(None));
    }

    #[test]
    fn saved_file_is_loaded_back_and_replaced() {
        let dir = TempDir::new().unwrap();
        let mut store = store_in(&dir);
        store.save_atomic(b"first").unwrap();
        assert_eq!(store.load(), Ok(Some(b"first".to_vec())));
        store.save_atomic(b"second").unwrap();
        assert_eq!(store.load(), Ok(Some(b"second".to_vec())));
    }

    #[test]
    fn save_creates_missing_folders_and_leaves_no_temporary_file() {
        let dir = TempDir::new().unwrap();
        let mut store = FileStore::new(dir.path().join("vaults").join("perso").join("vault.enc"));
        store.save_atomic(b"data").unwrap();
        assert_eq!(fs::read(store.path()).unwrap(), b"data");
        let names: Vec<_> = fs::read_dir(store.path().parent().unwrap())
            .unwrap()
            .map(|entry| entry.unwrap().file_name())
            .collect();
        assert_eq!(names, ["vault.enc"]);
    }

    #[test]
    fn failed_save_keeps_the_previous_file() {
        let dir = TempDir::new().unwrap();
        let mut store = store_in(&dir);
        store.save_atomic(b"previous").unwrap();
        // Un dossier à la place du fichier temporaire empêche de l'écrire.
        fs::create_dir(store.temporary_path()).unwrap();
        let error = store.save_atomic(b"next").unwrap_err();
        assert!(error.to_string().starts_with("écriture de "), "{error}");
        assert_eq!(store.load(), Ok(Some(b"previous".to_vec())));
    }

    #[test]
    fn unreadable_path_is_an_error_naming_the_file() {
        let dir = TempDir::new().unwrap();
        let store = FileStore::new(dir.path());
        let error = store.load().unwrap_err().to_string();
        assert!(error.starts_with("lecture de "), "{error}");
        assert!(error.contains(&dir.path().display().to_string()), "{error}");
    }

    #[test]
    fn remove_deletes_the_file_and_a_leftover_temporary_file() {
        let dir = TempDir::new().unwrap();
        let mut store = store_in(&dir);
        store.save_atomic(b"data").unwrap();
        fs::write(store.temporary_path(), b"interrompu").unwrap();
        fs::write(dir.path().join("autre.txt"), b"").unwrap();
        store.remove().unwrap();
        assert_eq!(store.load(), Ok(None));
        assert!(!store.temporary_path().exists());
        assert!(dir.path().join("autre.txt").exists());
        assert_eq!(store.remove(), Ok(()));
    }

    #[test]
    fn file_that_cannot_be_removed_is_an_error() {
        let dir = TempDir::new().unwrap();
        let store = store_in(&dir);
        fs::create_dir(store.path()).unwrap();
        let error = store.remove().unwrap_err();
        assert!(error.to_string().starts_with("suppression de "), "{error}");
        assert!(store.path().exists());
    }

    #[test]
    fn folder_that_cannot_be_created_is_an_error() {
        let dir = TempDir::new().unwrap();
        let blocker = dir.path().join("blocker");
        fs::write(&blocker, b"").unwrap();
        let mut store = FileStore::new(blocker.join("vault.enc"));
        assert!(store.save_atomic(b"data").is_err());
    }
}
