//! Sauvegarde sur disque externe (`backup`) et restauration comme nouveau coffre (`restore`).

use std::path::Path;

use crate::app::{App, CliError};
use crate::terminal::Terminal;

impl<T: Terminal, C> App<T, C> {
    /// Tous les coffres, ou seulement `name`.
    pub(crate) fn backup(
        &mut self,
        destination: &Path,
        name: Option<&str>,
    ) -> Result<(), CliError> {
        let (backup, count) = self.vaults.backup(destination, name, (self.clock)())?;
        self.terminal.print(&format!(
            "{count} coffre(s) sauvegardé(s) dans {}.",
            backup.display()
        ));
        Ok(())
    }

    /// Le coffre est restauré sous son nom s'il est libre, sinon sous un nom daté : rien n'est
    /// jamais écrasé ni fusionné.
    pub(crate) fn restore(&mut self, backup: &Path, name: Option<&str>) -> Result<(), CliError> {
        let names = self.vaults.backup_contents(backup)?;
        let name = match name {
            Some(name) => name.to_owned(),
            None => self.choose_vault(&names)?,
        };
        let target = self.vaults.restore_from_backup(backup, &name)?;

        if target == name {
            self.terminal.print(&format!("Coffre « {name} » restauré."));
        } else {
            self.terminal.print(&format!(
                "Coffre « {name} » restauré sous le nom « {target} » : ce nom était déjà pris."
            ));
        }
        self.terminal
            .print("Il s'ouvre avec le mot de passe maître en vigueur à la date de la sauvegarde.");
        Ok(())
    }

    fn choose_vault(&mut self, names: &[String]) -> Result<String, CliError> {
        self.terminal.print("Coffres de cette sauvegarde :");
        for name in names {
            self.terminal.print(&format!("  {name}"));
        }
        Ok(self
            .terminal
            .prompt("Coffre à restaurer : ")?
            .trim()
            .to_owned())
    }
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::PathBuf;

    use pswd_app::AppError;
    use pswd_core::{EntryContent, EntryKind, FileError, SessionError, Timestamp};
    use pswd_infra::{ConfigError, backup_vault_file, backup_vault_names};

    use crate::cli::Command;
    use crate::testing::TestEnv;

    use super::*;

    /// La sauvegarde que crée `backup` dans `usb` avec l'horloge des tests (1 s après 1970).
    const STAMP: &str = "1970-01-01_00-00-01";

    fn usb_key(env: &TestEnv) -> PathBuf {
        let usb = env.dir.path().join("usb");
        fs::create_dir_all(&usb).unwrap();
        usb
    }

    fn backup(env: &TestEnv, vault: Option<&str>) -> PathBuf {
        let usb = usb_key(env);
        let mut app = env.app("a", &[]);
        app.run(Command::Backup {
            destination: usb.clone(),
            vault: vault.map(str::to_owned),
        })
        .unwrap();
        usb.join("pswd-sauvegardes").join(STAMP)
    }

    fn restore(backup: &Path, name: Option<&str>) -> Command {
        Command::Restore {
            backup: backup.to_owned(),
            name: name.map(str::to_owned),
        }
    }

    /// Coffres `perso` (mot de passe « maître ») et `travail` sur l'appareil `a`.
    fn env_with_two_vaults() -> TestEnv {
        let env = TestEnv::with_vault();
        let mut app = env.app("a", &["autre", "autre"]);
        app.run(Command::Init {
            name: "travail".to_owned(),
        })
        .unwrap();
        env
    }

    #[test]
    fn backup_copies_every_vault() {
        let env = env_with_two_vaults();
        let usb = usb_key(&env);
        let mut app = env.app("a", &[]);
        app.run(Command::Backup {
            destination: usb.clone(),
            vault: None,
        })
        .unwrap();
        let folder = usb.join("pswd-sauvegardes").join(STAMP);
        assert_eq!(
            app.terminal.output(),
            format!("2 coffre(s) sauvegardé(s) dans {}.", folder.display())
        );
        assert_eq!(backup_vault_names(&folder).unwrap(), ["perso", "travail"]);
        let local = env.config("a").vault("perso").unwrap().vault_file();
        assert_eq!(
            fs::read(backup_vault_file(&folder, "perso")).unwrap(),
            fs::read(local).unwrap()
        );
    }

    #[test]
    fn backup_can_be_limited_to_one_vault() {
        let env = env_with_two_vaults();
        let folder = backup(&env, Some("travail"));
        assert_eq!(backup_vault_names(&folder).unwrap(), ["travail"]);
    }

    #[test]
    fn backup_requires_a_vault() {
        let env = TestEnv::new();
        let mut app = env.app("a", &[]);
        let command = Command::Backup {
            destination: usb_key(&env),
            vault: None,
        };
        assert_eq!(
            app.run(command),
            Err(CliError::App(AppError::NothingToBackUp))
        );
        let command = Command::Backup {
            destination: usb_key(&env),
            vault: Some("perso".to_owned()),
        };
        assert_eq!(
            app.run(command),
            Err(CliError::App(AppError::Config(ConfigError::UnknownVault(
                "perso".to_owned()
            ))))
        );
    }

    #[test]
    fn backup_to_a_missing_folder_is_refused() {
        let env = TestEnv::with_vault();
        let mut app = env.app("a", &[]);
        let error = app
            .run(Command::Backup {
                destination: env.dir.path().join("débranchée"),
                vault: None,
            })
            .unwrap_err();
        assert!(
            error
                .to_string()
                .starts_with("dossier de destination introuvable"),
            "{error}"
        );
    }

    #[test]
    fn restore_on_a_new_machine_keeps_the_name() {
        let env = TestEnv::with_vault();
        env.edit_vault("a", |vault| {
            vault.add(
                EntryContent::new(EntryKind::Note, "Wi-Fi".to_owned()),
                Timestamp::from_millis(1),
            );
        });
        let folder = backup(&env, None);
        let mut app = env.app("b", &[]);
        app.run(restore(&folder, Some("perso"))).unwrap();
        assert!(
            app.terminal
                .output()
                .starts_with("Coffre « perso » restauré.\n"),
        );
        let vault = env.local_vault("b", "maître");
        assert_eq!(vault.entries().count(), 1);
    }

    #[test]
    fn restore_never_replaces_an_existing_vault() {
        let env = TestEnv::with_vault();
        let folder = backup(&env, None);
        env.edit_vault("a", |vault| {
            vault.add(
                EntryContent::new(EntryKind::Note, "après".to_owned()),
                Timestamp::from_millis(1),
            );
        });
        let mut app = env.app("a", &[]);
        app.run(restore(&folder, Some("perso"))).unwrap();
        assert!(
            app.terminal.output().starts_with(
                "Coffre « perso » restauré sous le nom « perso-sauvegarde-1970-01-01 » : \
                 ce nom était déjà pris.\n"
            ),
            "{}",
            app.terminal.output()
        );
        assert_eq!(env.local_vault("a", "maître").entries().count(), 1);
        let config = env.config("a");
        let restored = config.vault("perso-sauvegarde-1970-01-01").unwrap();
        let file = fs::read(restored.vault_file()).unwrap();
        assert_eq!(fs::read(backup_vault_file(&folder, "perso")).unwrap(), file);

        assert_eq!(
            app.run(restore(&folder, Some("perso"))),
            Err(CliError::App(AppError::Config(ConfigError::VaultExists(
                "perso-sauvegarde-1970-01-01".to_owned()
            ))))
        );
    }

    #[test]
    fn restore_asks_which_vault_when_none_is_given() {
        let env = env_with_two_vaults();
        let folder = backup(&env, None);
        let mut app = env.app("b", &[" travail "]);
        app.run(restore(&folder, None)).unwrap();
        let output = app.terminal.output();
        assert!(
            output.starts_with(
                "Coffres de cette sauvegarde :\n  perso\n  travail\nCoffre « travail » restauré."
            ),
            "{output}"
        );
        assert!(env.config("b").vault("travail").is_ok());
    }

    #[test]
    fn unknown_or_missing_vaults_are_reported() {
        let env = TestEnv::with_vault();
        let folder = backup(&env, None);
        let mut app = env.app("b", &[]);
        assert_eq!(
            app.run(restore(&folder, Some("travail"))),
            Err(CliError::App(AppError::NotInBackup("travail".to_owned())))
        );
        fs::remove_dir_all(folder.join("vaults").join("perso")).unwrap();
        assert_eq!(
            app.run(restore(&folder, None)),
            Err(CliError::App(AppError::EmptyBackup))
        );
        let error = app.run(restore(env.dir.path(), None)).unwrap_err();
        assert!(
            error.to_string().contains("n'est pas une sauvegarde pswd"),
            "{error}"
        );
        assert!(env.config("b").vaults.is_empty());
    }

    #[test]
    fn invalid_file_is_not_restored() {
        let env = TestEnv::with_vault();
        let folder = backup(&env, None);
        fs::write(backup_vault_file(&folder, "perso"), b"pas un coffre").unwrap();
        let mut app = env.app("b", &[]);
        assert_eq!(
            app.run(restore(&folder, Some("perso"))),
            Err(CliError::App(AppError::InvalidBackup(FileError::Malformed)))
        );
        assert!(env.config("b").vaults.is_empty());
        assert!(!env.vault_dir("b", "perso").exists());
    }

    #[test]
    fn unregistered_vault_file_is_never_replaced() {
        let env = TestEnv::with_vault();
        let folder = backup(&env, None);
        let existing = env.vault_dir("b", "perso");
        fs::create_dir_all(&existing).unwrap();
        fs::write(existing.join("vault.enc"), b"ancien").unwrap();
        let mut app = env.app("b", &[]);
        assert_eq!(
            app.run(restore(&folder, Some("perso"))),
            Err(CliError::App(AppError::Session(
                SessionError::LocalVaultExists
            )))
        );
        assert_eq!(fs::read(existing.join("vault.enc")).unwrap(), b"ancien");
        assert!(env.config("b").vaults.is_empty());
    }

    #[test]
    fn local_write_failure_leaves_nothing_behind() {
        let env = TestEnv::with_vault();
        let folder = backup(&env, None);
        let vaults = env.vault_dir("b", "perso");
        let vaults = vaults.parent().unwrap();
        fs::create_dir_all(vaults.parent().unwrap()).unwrap();
        fs::write(vaults, b"").unwrap();
        let mut app = env.app("b", &[]);
        assert!(matches!(
            app.run(restore(&folder, Some("perso"))),
            Err(CliError::App(AppError::Port(_)))
        ));
        assert!(env.config("b").vaults.is_empty());
    }
}
