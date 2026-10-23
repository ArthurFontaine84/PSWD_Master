//! Création d'un coffre (`init`).

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
        self.terminal.print(
            "\nClé de récupération : notez-la et rangez-la en lieu sûr, elle ne sera plus \
             affichée.\nElle permet de choisir un nouveau mot de passe maître en cas d'oubli \
             (pswd recover).\n",
        );
        self.terminal
            .print(&format!("    {}\n", recovery_key.expose()));
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
fn removing_on_failure<R>(
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
        let (name, vault) = config.vault(None).unwrap();
        assert_eq!(name, "perso");
        assert_eq!(vault.path, env.vault_dir("a", "perso"));
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
}
