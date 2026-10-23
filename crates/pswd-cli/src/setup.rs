//! Création d'un coffre (`init`) et rattachement à un coffre existant (`join`).

use std::fs;
use std::path::Path;

use pswd_core::ports::SecretStore;
use pswd_core::{Credentials, Secret, Session};
use pswd_infra::{Config, ConfigFile, FileStore, GitRemote, VaultConfig};

use crate::app::{App, CliError, describe};
use crate::terminal::Terminal;

impl<T: Terminal, K: SecretStore, C> App<T, K, C> {
    pub(crate) fn init(&mut self, name: &str) -> Result<(), CliError> {
        let (mut config_file, mut config) = self.config_for_new_vault(name)?;
        let url = self.terminal.prompt("URL du dépôt distant : ")?;
        let token = self.prompt_token()?;
        let password = self.new_master_password()?;
        let vault = VaultConfig {
            url,
            path: self.locations.vault_dir(name),
        };
        let (session, recovery_key) = removing_on_failure(&vault.path, || {
            let remote = GitRemote::open(&vault.repo_dir(), &vault.url, token.clone())?;
            let local = FileStore::new(vault.vault_file());
            Ok(Session::create(
                local,
                remote,
                &password,
                self.kdf,
                (self.clock)(),
            )?)
        })?;
        self.register(name, vault, token.as_ref(), &mut config, &mut config_file)?;

        let recovery_key = recovery_key.encode();
        self.terminal.print(&format!("Coffre « {name} » créé."));
        self.terminal.print(&describe(session.status()));
        self.terminal.print(
            "\nClé de récupération : notez-la et rangez-la en lieu sûr, elle ne sera plus \
             affichée.\nElle permet de choisir un nouveau mot de passe maître en cas d'oubli \
             (pswd recover).\n",
        );
        self.terminal
            .print(&format!("    {}\n", recovery_key.expose()));
        Ok(())
    }

    pub(crate) fn join(&mut self, name: &str, url: &str) -> Result<(), CliError> {
        let (mut config_file, mut config) = self.config_for_new_vault(name)?;
        let token = self.prompt_token()?;
        let password = self.terminal.prompt_secret("Mot de passe maître : ")?;
        let vault = VaultConfig {
            url: url.to_owned(),
            path: self.locations.vault_dir(name),
        };
        let session = removing_on_failure(&vault.path, || {
            let remote = GitRemote::open(&vault.repo_dir(), &vault.url, token.clone())?;
            let local = FileStore::new(vault.vault_file());
            Ok(Session::join(
                local,
                remote,
                Credentials::Password(&password),
            )?)
        })?;
        self.register(name, vault, token.as_ref(), &mut config, &mut config_file)?;
        let entries = session
            .vault()
            .entries()
            .filter(|entry| !entry.is_deleted())
            .count();
        self.terminal
            .print(&format!("Coffre « {name} » rejoint : {entries} entrée(s)."));
        Ok(())
    }

    /// Vérifie le nom avant toute question.
    fn config_for_new_vault(&self, name: &str) -> Result<(ConfigFile, Config), CliError> {
        let config_file = ConfigFile::new(self.locations.config_file());
        let config = config_file.load()?;
        config.ensure_available(name)?;
        Ok((config_file, config))
    }

    fn prompt_token(&mut self) -> Result<Option<Secret>, CliError> {
        let token = self
            .terminal
            .prompt_secret("Jeton d'accès au dépôt (vide si aucun) : ")?;
        Ok((!token.expose().is_empty()).then_some(token))
    }

    fn new_master_password(&mut self) -> Result<Secret, CliError> {
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

    fn register(
        &mut self,
        name: &str,
        vault: VaultConfig,
        token: Option<&Secret>,
        config: &mut Config,
        config_file: &mut ConfigFile,
    ) -> Result<(), CliError> {
        if let Some(token) = token {
            self.keyring.set_token(name, token)?;
        }
        config.add_vault(name, vault)?;
        config_file.save(config)?;
        Ok(())
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
    use pswd_core::ports::PortError;
    use pswd_core::{EntryContent, EntryKind, FileError, SessionError, Timestamp};
    use pswd_infra::ConfigError;

    use crate::cli::Command;
    use crate::testing::TestEnv;

    use super::*;

    fn init(name: &str) -> Command {
        Command::Init {
            name: name.to_owned(),
        }
    }

    fn join(name: &str, url: &str) -> Command {
        Command::Join {
            name: name.to_owned(),
            url: url.to_owned(),
        }
    }

    /// `init perso` sur l'appareil `a`, sans jeton, mot de passe « maître ».
    fn init_perso(env: &TestEnv) {
        let mut app = env.app("a", &[&env.url, "", "maître", "maître"]);
        app.run(init("perso")).unwrap();
    }

    /// Sur l'appareil `a` : une entrée gardée et une supprimée, publiées.
    fn add_entries_on_a(env: &TestEnv) {
        let note = |title: &str| EntryContent::new(EntryKind::Note, title.to_owned());
        let at = Timestamp::from_millis;
        env.edit_vault("a", |vault| {
            vault.add(note("gardée"), at(1));
            let deleted = vault.add(note("supprimée"), at(1));
            vault.delete(deleted, at(2)).unwrap();
        });
    }

    #[test]
    fn init_creates_registers_and_publishes_the_vault() {
        let env = TestEnv::new();
        let mut app = env.app("a", &[&env.url, "", "maître", "maître"]);
        app.run(init("perso")).unwrap();

        let config = env.config("a");
        let (name, vault) = config.vault(None).unwrap();
        assert_eq!(name, "perso");
        assert_eq!(vault.url, env.url);
        assert!(vault.vault_file().exists());
        assert!(env.remote_has_vault());

        let output = app.terminal.output();
        assert!(output.contains("Coffre « perso » créé."), "{output}");
        assert!(output.contains("Synchronisé"), "{output}");
        let key_line = output
            .lines()
            .find(|line| line.starts_with("    "))
            .unwrap()
            .trim();
        assert_eq!(key_line.split('-').count(), 13, "{key_line}");
    }

    #[test]
    fn init_saves_the_token_in_the_keyring() {
        let env = TestEnv::new();
        let mut app = env.app("a", &[&env.url, "jeton", "maître", "maître"]);
        app.run(init("perso")).unwrap();
        assert_eq!(
            app.keyring.token("perso"),
            Ok(Some(Secret::new("jeton".to_owned())))
        );
    }

    #[test]
    fn init_checks_the_name_before_asking_anything() {
        let env = TestEnv::new();
        init_perso(&env);
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
        let mut app = env.app("a", &[&env.url, "", "maître", "maitre"]);
        assert_eq!(app.run(init("perso")), Err(CliError::PasswordMismatch));
        let mut app = env.app("a", &[&env.url, "", ""]);
        assert_eq!(app.run(init("perso")), Err(CliError::EmptyPassword));
        assert!(env.config("a").vaults.is_empty());
        assert!(!env.vault_dir("a", "perso").exists());
    }

    #[test]
    fn failed_init_leaves_nothing_behind() {
        let env = TestEnv::new();
        init_perso(&env);
        let mut app = env.app("b", &[&env.url, "", "autre", "autre"]);
        assert_eq!(
            app.run(init("perso")),
            Err(CliError::Session(SessionError::RemoteVaultExists))
        );
        assert!(!env.vault_dir("b", "perso").exists());
        assert!(env.config("b").vaults.is_empty());
    }

    #[test]
    fn failed_init_keeps_a_folder_that_already_existed() {
        let env = TestEnv::new();
        init_perso(&env);
        let existing = env.vault_dir("b", "perso");
        fs::create_dir_all(&existing).unwrap();
        let mut app = env.app("b", &[&env.url, "", "autre", "autre"]);
        assert!(app.run(init("perso")).is_err());
        assert!(existing.exists());
    }

    #[test]
    fn unreachable_remote_is_reported() {
        let env = TestEnv::new();
        let missing = env.dir.path().join("absent.git");
        let missing = missing.to_str().unwrap();
        let mut app = env.app("a", &[missing, "", "maître", "maître"]);
        assert!(matches!(
            app.run(init("perso")),
            Err(CliError::Session(SessionError::Remote(_)))
        ));
        assert!(!env.vault_dir("a", "perso").exists());
    }

    #[test]
    fn local_folder_that_cannot_be_created_is_reported() {
        let env = TestEnv::new();
        let data = env.dir.path().join("a").join("data");
        fs::create_dir_all(&data).unwrap();
        fs::write(data.join("vaults"), b"").unwrap();
        let mut app = env.app("a", &[&env.url, "", "maître", "maître"]);
        let error = app.run(init("perso")).unwrap_err();
        assert!(matches!(error, CliError::Port(PortError { .. })), "{error}");
    }

    #[test]
    fn join_copies_an_existing_vault() {
        let env = TestEnv::new();
        init_perso(&env);
        add_entries_on_a(&env);
        let mut app = env.app("b", &["jeton", "maître"]);
        app.run(join("perso", &env.url)).unwrap();
        let output = app.terminal.output();
        assert!(
            output.contains("Coffre « perso » rejoint : 1 entrée(s)."),
            "{output}"
        );
        let config = env.config("b");
        let (_, vault) = config.vault(Some("perso")).unwrap();
        assert!(vault.vault_file().exists());
        assert_eq!(
            app.keyring.token("perso"),
            Ok(Some(Secret::new("jeton".to_owned())))
        );
    }

    #[test]
    fn join_with_a_wrong_password_leaves_nothing_behind() {
        let env = TestEnv::new();
        init_perso(&env);
        let mut app = env.app("b", &["", "autre"]);
        assert_eq!(
            app.run(join("perso", &env.url)),
            Err(CliError::Session(SessionError::File(
                FileError::WrongPassword
            )))
        );
        assert!(!env.vault_dir("b", "perso").exists());
        assert!(env.config("b").vaults.is_empty());
    }

    #[test]
    fn join_checks_the_name_first() {
        let env = TestEnv::new();
        init_perso(&env);
        let mut app = env.app("a", &[]);
        assert_eq!(
            app.run(join("perso", &env.url)),
            Err(CliError::Config(ConfigError::VaultExists(
                "perso".to_owned()
            )))
        );
    }
}
