//! Sauvegarde sur disque externe (`backup`) et restauration comme nouveau coffre (`restore`).

use std::path::Path;

use pswd_core::ports::LocalStore;
use pswd_core::{SessionError, VaultFile};
use pswd_infra::{
    ConfigFile, FileStore, VaultConfig, backup_vault_file, backup_vault_names, create_backup,
};

use crate::app::{App, CliError};
use crate::setup::removing_on_failure;
use crate::terminal::Terminal;
use crate::utc::backup_stamp;

impl<T: Terminal, C> App<T, C> {
    /// Tous les coffres, ou seulement `name`.
    pub(crate) fn backup(
        &mut self,
        destination: &Path,
        name: Option<&str>,
    ) -> Result<(), CliError> {
        let config = ConfigFile::new(self.locations.config_file()).load()?;
        let vaults: Vec<(&str, &VaultConfig)> = match name {
            Some(name) => vec![(name, config.vault(name)?)],
            None => config
                .vaults
                .iter()
                .map(|(name, vault)| (name.as_str(), vault))
                .collect(),
        };
        if vaults.is_empty() {
            return Err(CliError::NothingToBackUp);
        }
        let backup = create_backup(destination, &backup_stamp((self.clock)()), &vaults)?;
        self.terminal.print(&format!(
            "{} coffre(s) sauvegardé(s) dans {}.",
            vaults.len(),
            backup.display()
        ));
        Ok(())
    }

    /// Le coffre est restauré sous son nom s'il est libre, sinon sous un nom daté : rien n'est
    /// jamais écrasé ni fusionné.
    pub(crate) fn restore(&mut self, backup: &Path, name: Option<&str>) -> Result<(), CliError> {
        let names = backup_vault_names(backup)?;
        if names.is_empty() {
            return Err(CliError::EmptyBackup);
        }
        let name = match name {
            Some(name) => name.to_owned(),
            None => self.choose_vault(&names)?,
        };
        if !names.contains(&name) {
            return Err(CliError::NotInBackup(name));
        }
        let source = FileStore::new(backup_vault_file(backup, &name));
        let file = source.load()?.ok_or(CliError::NotInBackup(name.clone()))?;
        VaultFile::check_format(&file).map_err(CliError::InvalidBackup)?;

        let mut config_file = ConfigFile::new(self.locations.config_file());
        let mut config = config_file.load()?;
        let target = if config.ensure_available(&name).is_ok() {
            name.clone()
        } else {
            format!("{name}-sauvegarde-{}", backup_date(backup))
        };
        config.ensure_available(&target)?;
        let vault = VaultConfig {
            path: self.locations.vault_dir(&target),
        };
        removing_on_failure(&vault.path, || {
            let mut local = FileStore::new(vault.vault_file());
            if local.load()?.is_some() {
                return Err(CliError::Session(SessionError::LocalVaultExists));
            }
            Ok(local.save_atomic(&file)?)
        })?;
        config.add_vault(&target, vault)?;
        config_file.save(&config)?;

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

/// Date d'un dossier de sauvegarde `AAAA-MM-JJ_HH-MM-SS` : la partie avant `_`.
fn backup_date(backup: &Path) -> String {
    let folder = backup
        .file_name()
        .map(|name| name.to_string_lossy())
        .unwrap_or_default();
    folder.split('_').next().unwrap_or_default().to_owned()
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::PathBuf;

    use pswd_core::{EntryContent, EntryKind, FileError, Timestamp};
    use pswd_infra::ConfigError;

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
        assert_eq!(app.run(command), Err(CliError::NothingToBackUp));
        let command = Command::Backup {
            destination: usb_key(&env),
            vault: Some("perso".to_owned()),
        };
        assert_eq!(
            app.run(command),
            Err(CliError::Config(ConfigError::UnknownVault(
                "perso".to_owned()
            )))
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
            Err(CliError::Config(ConfigError::VaultExists(
                "perso-sauvegarde-1970-01-01".to_owned()
            )))
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
            Err(CliError::NotInBackup("travail".to_owned()))
        );
        fs::remove_dir_all(folder.join("vaults").join("perso")).unwrap();
        assert_eq!(app.run(restore(&folder, None)), Err(CliError::EmptyBackup));
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
            Err(CliError::InvalidBackup(FileError::Malformed))
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
            Err(CliError::Session(SessionError::LocalVaultExists))
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
            Err(CliError::Port(_))
        ));
        assert!(env.config("b").vaults.is_empty());
    }

    #[test]
    fn backup_date_is_the_folder_prefix() {
        assert_eq!(
            backup_date(Path::new("E:/pswd-sauvegardes/2026-10-23_14-30-00")),
            "2026-10-23"
        );
        assert_eq!(backup_date(Path::new("E:/copie")), "copie");
        assert_eq!(backup_date(Path::new("/")), "");
    }
}
