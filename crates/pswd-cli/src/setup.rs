//! Création (`init`) et suppression (`delete`) d'un coffre.

use std::fs;
use std::path::Path;

use pswd_core::{Secret, Session};
use pswd_infra::{ConfigFile, FileStore, VaultConfig};

use crate::app::{App, CliError};
use crate::terminal::Terminal;

impl<T: Terminal, C> App<T, C> {
    pub(crate) fn init(&mut self, name: &str) -> Result<(), CliError> {
        let mut config_file = ConfigFile::new(self.locations.config_file());
        let mut config = config_file.load()?;
        config.ensure_available(name)?;
        let password = self.new_master_password()?;
        let vault = VaultConfig {
            path: self.locations.vault_dir(name),
        };
        let (_, recovery_key) = removing_on_failure(&vault.path, || {
            let local = FileStore::new(vault.vault_file());
            Ok(Session::create(local, &password, self.kdf, (self.clock)())?)
        })?;
        config.add_vault(name, vault)?;
        config_file.save(&config)?;

        let recovery_key = recovery_key.encode();
        self.terminal.print(&format!("Coffre « {name} » créé."));
        self.terminal.print(&format!(
            "\nClé de récupération : notez-la et rangez-la en lieu sûr, elle ne sera plus \
             affichée.\nElle permet de choisir un nouveau mot de passe maître en cas d'oubli \
             (pswd recover {name}).\n"
        ));
        self.terminal
            .print(&format!("    {}\n", recovery_key.expose()));
        Ok(())
    }

    /// Le nom doit être retapé. Les fichiers sont supprimés avant la configuration : un échec
    /// ne change rien.
    pub(crate) fn delete(&mut self, name: &str) -> Result<(), CliError> {
        let mut config_file = ConfigFile::new(self.locations.config_file());
        let mut config = config_file.load()?;
        let vault = config.vault(name)?.clone();
        self.terminal.print(&format!(
            "⚠ Le coffre « {name} » et toutes ses entrées seront supprimés de cette machine, \
             sans retour possible. Une sauvegarde (pswd backup) permet de le restaurer plus tard."
        ));
        let answer = self
            .terminal
            .prompt("Pour confirmer, retapez le nom du coffre : ")?;
        if answer.trim() != name {
            self.terminal.print("Suppression annulée.");
            return Ok(());
        }
        FileStore::new(vault.vault_file()).remove()?;
        // Le dossier n'est retiré que s'il est vide : un fichier inconnu n'est jamais effacé.
        let _ = fs::remove_dir(&vault.path);
        config.remove_vault(name)?;
        config_file.save(&config)?;

        self.terminal.print(&format!("Coffre « {name} » supprimé."));
        Ok(())
    }

    pub(crate) fn new_master_password(&mut self) -> Result<Secret, CliError> {
        let password = self
            .terminal
            .prompt_secret("Nouveau mot de passe maître : ")?;
        if password.expose().is_empty() {
            return Err(CliError::EmptyPassword);
        }
        let confirmation = self.terminal.prompt_secret("Confirmation : ")?;
        if confirmation != password {
            return Err(CliError::PasswordMismatch);
        }
        Ok(password)
    }
}

/// Supprime le dossier du coffre si `create` échoue alors qu'il n'existait pas avant : un
/// essai raté ne laisse rien derrière lui.
pub(crate) fn removing_on_failure<R>(
    dir: &Path,
    create: impl FnOnce() -> Result<R, CliError>,
) -> Result<R, CliError> {
    let existed = dir.exists();
    let result = create();
    if result.is_err() && !existed {
        // Au mieux : l'erreur de création est plus utile que celle du nettoyage.
        let _ = fs::remove_dir_all(dir);
    }
    result
}

#[cfg(test)]
mod tests {
    use pswd_core::SessionError;
    use pswd_core::ports::PortError;
    use pswd_infra::ConfigError;

    use crate::cli::Command;
    use crate::testing::TestEnv;

    use super::*;

    fn init(name: &str) -> Command {
        Command::Init {
            name: name.to_owned(),
        }
    }

    #[test]
    fn init_creates_and_registers_the_vault() {
        let env = TestEnv::new();
        let mut app = env.app("a", &["maître", "maître"]);
        app.run(init("perso")).unwrap();

        let config = env.config("a");
        assert_eq!(config.vaults.keys().collect::<Vec<_>>(), ["perso"]);
        assert_eq!(
            config.vault("perso").unwrap().path,
            env.vault_dir("a", "perso")
        );
        assert!(env.local_vault("a", "maître").entries().next().is_none());

        let output = app.terminal.output();
        assert!(output.starts_with("Coffre « perso » créé.\n"), "{output}");
        let key_line = output
            .lines()
            .find(|line| line.starts_with("    "))
            .unwrap()
            .trim();
        assert_eq!(key_line.split('-').count(), 13, "{key_line}");
    }

    #[test]
    fn init_checks_the_name_before_asking_anything() {
        let env = TestEnv::with_vault();
        let mut app = env.app("a", &[]);
        assert_eq!(
            app.run(init("perso")),
            Err(CliError::Config(ConfigError::VaultExists(
                "perso".to_owned()
            )))
        );
        assert_eq!(
            app.run(init("../ailleurs")),
            Err(CliError::Config(ConfigError::InvalidVaultName(
                "../ailleurs".to_owned()
            )))
        );
    }

    #[test]
    fn master_password_must_be_confirmed_and_not_empty() {
        let env = TestEnv::new();
        let mut app = env.app("a", &["maître", "maitre"]);
        assert_eq!(app.run(init("perso")), Err(CliError::PasswordMismatch));
        let mut app = env.app("a", &[""]);
        assert_eq!(app.run(init("perso")), Err(CliError::EmptyPassword));
        assert!(env.config("a").vaults.is_empty());
        assert!(!env.vault_dir("a", "perso").exists());
    }

    #[test]
    fn unregistered_vault_file_is_never_replaced() {
        let env = TestEnv::new();
        let existing = env.vault_dir("a", "perso");
        fs::create_dir_all(&existing).unwrap();
        fs::write(existing.join("vault.enc"), b"ancien").unwrap();
        let mut app = env.app("a", &["maître", "maître"]);
        assert_eq!(
            app.run(init("perso")),
            Err(CliError::Session(SessionError::LocalVaultExists))
        );
        assert_eq!(fs::read(existing.join("vault.enc")).unwrap(), b"ancien");
        assert!(env.config("a").vaults.is_empty());
    }

    #[test]
    fn local_folder_that_cannot_be_created_is_reported() {
        let env = TestEnv::new();
        let data = env.dir.path().join("a").join("data");
        fs::create_dir_all(&data).unwrap();
        fs::write(data.join("vaults"), b"").unwrap();
        let mut app = env.app("a", &["maître", "maître"]);
        let error = app.run(init("perso")).unwrap_err();
        assert!(
            matches!(
                error,
                CliError::Session(SessionError::Local(PortError { .. }))
            ),
            "{error}"
        );
        assert!(!env.vault_dir("a", "perso").exists());
    }

    fn delete(name: &str) -> Command {
        Command::Delete {
            name: name.to_owned(),
        }
    }

    /// Coffres `perso` et `travail` sur l'appareil `a`.
    fn env_with_two_vaults() -> TestEnv {
        let env = TestEnv::with_vault();
        env.app("a", &["autre", "autre"])
            .run(init("travail"))
            .unwrap();
        env
    }

    #[test]
    fn delete_removes_the_vault_once_its_name_is_retyped() {
        let env = env_with_two_vaults();
        let mut app = env.app("a", &[" perso "]);
        app.run(delete("perso")).unwrap();
        let output = app.terminal.output();
        assert!(output.starts_with("⚠ Le coffre « perso »"), "{output}");
        assert!(output.ends_with("Coffre « perso » supprimé."), "{output}");
        assert!(!env.vault_dir("a", "perso").exists());
        let config = env.config("a");
        assert_eq!(config.vaults.keys().collect::<Vec<_>>(), ["travail"]);
    }

    #[test]
    fn delete_is_cancelled_unless_the_exact_name_is_retyped() {
        let env = TestEnv::with_vault();
        for answer in ["", "o", "Perso"] {
            let mut app = env.app("a", &[answer]);
            app.run(delete("perso")).unwrap();
            assert!(
                app.terminal.output().ends_with("Suppression annulée."),
                "{answer:?}"
            );
            env.local_vault("a", "maître");
        }
    }

    #[test]
    fn deleting_an_unknown_vault_asks_nothing() {
        let env = TestEnv::with_vault();
        let mut app = env.app("a", &[]);
        assert_eq!(
            app.run(delete("travail")),
            Err(CliError::Config(ConfigError::UnknownVault(
                "travail".to_owned()
            )))
        );
    }

    #[test]
    fn deleted_name_can_be_used_again() {
        let env = TestEnv::with_vault();
        env.app("a", &["perso"]).run(delete("perso")).unwrap();
        let config = env.config("a");
        assert!(config.vaults.is_empty());

        env.app("a", &["neuf", "neuf"]).run(init("perso")).unwrap();
        env.local_vault("a", "neuf");
    }

    #[test]
    fn unknown_files_in_the_vault_folder_are_kept() {
        let env = TestEnv::with_vault();
        let folder = env.vault_dir("a", "perso");
        fs::write(folder.join("notes.txt"), b"a garder").unwrap();
        env.app("a", &["perso"]).run(delete("perso")).unwrap();
        assert_eq!(fs::read(folder.join("notes.txt")).unwrap(), b"a garder");
        assert!(!folder.join("vault.enc").exists());
    }

    #[test]
    fn failed_removal_changes_nothing() {
        let env = TestEnv::with_vault();
        let file = env.vault_dir("a", "perso").join("vault.enc");
        fs::remove_file(&file).unwrap();
        fs::create_dir(&file).unwrap();
        let mut app = env.app("a", &["perso"]);
        let error = app.run(delete("perso")).unwrap_err();
        assert!(error.to_string().starts_with("suppression de "), "{error}");
        assert!(env.config("a").vault("perso").is_ok());
    }
}
