//! Sauvegarde dans un fichier `.pswd` sur disque externe (`backup`) et restauration comme
//! nouveau coffre (`restore`).

use std::path::Path;

use pswd_app::backup_file_name;
use pswd_infra::{read_backup, write_backup};

use crate::app::{App, CliError};
use crate::terminal::Terminal;

impl<T: Terminal, C> App<T, C> {
    /// Tous les coffres, ou seulement `name`.
    pub(crate) fn backup(
        &mut self,
        destination: &Path,
        name: Option<&str>,
    ) -> Result<(), CliError> {
        let now = (self.clock)();
        let only = name.as_ref().map(std::slice::from_ref);
        let (backup, count) = self.vaults.backup_to_file(only, now)?;
        let path = write_backup(destination, &backup_file_name(now), &backup)?;
        self.terminal.print(&format!(
            "{count} coffre(s) sauvegardé(s) dans {}.",
            path.display()
        ));
        Ok(())
    }

    /// Le coffre est restauré sous son nom s'il est libre, sinon sous un nom daté : rien n'est
    /// jamais écrasé ni fusionné.
    pub(crate) fn restore(&mut self, backup: &Path, name: Option<&str>) -> Result<(), CliError> {
        let backup = read_backup(backup)?;
        let names = self.vaults.backup_file_contents(&backup)?;
        let name = match name {
            Some(name) => name.to_owned(),
            None => self.choose_vault(&names)?,
        };
        let target = self.vaults.restore_from_file(&backup, &name)?;

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
    use pswd_core::{BackupFile, BackupFileError, EntryContent, EntryKind, Timestamp};
    use pswd_infra::ConfigError;

    use crate::cli::Command;
    use crate::testing::TestEnv;

    use super::*;

    /// La sauvegarde que crée `backup` avec l'horloge des tests (1 s après 1970).
    const FILE_NAME: &str = "pswd-sauvegarde_1970-01-01_00-00-01.pswd";

    fn usb_key(env: &TestEnv) -> PathBuf {
        let usb = env.dir.path().join("usb");
        fs::create_dir_all(&usb).unwrap();
        usb
    }

    fn backup_command(destination: &Path, vault: Option<&str>) -> Command {
        Command::Backup {
            destination: destination.to_owned(),
            vault: vault.map(str::to_owned),
        }
    }

    /// Sauvegarde les coffres de l'appareil `a` sur la clé ; renvoie le fichier écrit.
    fn backup(env: &TestEnv, vault: Option<&str>) -> PathBuf {
        let usb = usb_key(env);
        env.app("a", &[]).run(backup_command(&usb, vault)).unwrap();
        usb.join(FILE_NAME)
    }

    fn contents(file: &Path) -> Vec<String> {
        let data = fs::read(file).unwrap();
        let backup = BackupFile::parse(&data).unwrap();
        backup.names().map(str::to_owned).collect()
    }

    fn restore(backup: &Path, name: Option<&str>) -> Command {
        Command::Restore {
            backup: backup.to_owned(),
            name: name.map(str::to_owned),
        }
    }

    /// Coffres `perso` (mot de passe « Maître-2026! ») et `travail` sur l'appareil `a`.
    fn env_with_two_vaults() -> TestEnv {
        let env = TestEnv::with_vault();
        let mut app = env.app("a", &["Autre-mot-2026!", "Autre-mot-2026!"]);
        app.run(Command::Init {
            name: "travail".to_owned(),
        })
        .unwrap();
        env
    }

    #[test]
    fn backup_writes_every_vault_into_a_new_file() {
        let env = env_with_two_vaults();
        let usb = usb_key(&env);
        let mut app = env.app("a", &[]);
        app.run(backup_command(&usb, None)).unwrap();
        let file = usb.join(FILE_NAME);
        assert_eq!(
            app.terminal.output(),
            format!("2 coffre(s) sauvegardé(s) dans {}.", file.display())
        );
        assert_eq!(contents(&file), ["perso", "travail"]);
        let data = fs::read(&file).unwrap();
        let local = env.config("a").vault("perso").unwrap().vault_file();
        assert_eq!(
            BackupFile::parse(&data).unwrap().vault("perso"),
            Some(fs::read(local).unwrap().as_slice())
        );
    }

    #[test]
    fn backup_can_be_limited_to_one_vault() {
        let env = env_with_two_vaults();
        assert_eq!(contents(&backup(&env, Some("travail"))), ["travail"]);
    }

    #[test]
    fn backup_requires_a_vault() {
        let env = TestEnv::new();
        let mut app = env.app("a", &[]);
        let usb = usb_key(&env);
        assert_eq!(
            app.run(backup_command(&usb, None)),
            Err(CliError::App(AppError::NothingToBackUp))
        );
        assert_eq!(
            app.run(backup_command(&usb, Some("perso"))),
            Err(CliError::App(AppError::Config(ConfigError::UnknownVault(
                "perso".to_owned()
            ))))
        );
        assert!(!usb.join(FILE_NAME).exists());
    }

    #[test]
    fn backup_to_a_missing_folder_is_refused() {
        let env = TestEnv::with_vault();
        let mut app = env.app("a", &[]);
        let error = app
            .run(backup_command(&env.dir.path().join("débranchée"), None))
            .unwrap_err();
        assert!(
            error
                .to_string()
                .starts_with("dossier de destination introuvable"),
            "{error}"
        );
    }

    #[test]
    fn backup_never_replaces_an_existing_file() {
        let env = TestEnv::with_vault();
        let file = backup(&env, None);
        let before = fs::read(&file).unwrap();
        let error = env
            .app("a", &[])
            .run(backup_command(file.parent().unwrap(), None))
            .unwrap_err();
        assert!(error.to_string().contains("existe déjà"), "{error}");
        assert_eq!(fs::read(&file).unwrap(), before);
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
        let file = backup(&env, None);
        let mut app = env.app("b", &[]);
        app.run(restore(&file, Some("perso"))).unwrap();
        assert!(
            app.terminal
                .output()
                .starts_with("Coffre « perso » restauré.\n"),
        );
        let vault = env.local_vault("b", "Maître-2026!");
        assert_eq!(vault.entries().count(), 1);
    }

    #[test]
    fn restore_never_replaces_an_existing_vault() {
        let env = TestEnv::with_vault();
        let file = backup(&env, None);
        env.edit_vault("a", |vault| {
            vault.add(
                EntryContent::new(EntryKind::Note, "après".to_owned()),
                Timestamp::from_millis(1),
            );
        });
        let mut app = env.app("a", &[]);
        app.run(restore(&file, Some("perso"))).unwrap();
        assert!(
            app.terminal.output().starts_with(
                "Coffre « perso » restauré sous le nom « perso-sauvegarde-1970-01-01 » : \
                 ce nom était déjà pris.\n"
            ),
            "{}",
            app.terminal.output()
        );
        assert_eq!(env.local_vault("a", "Maître-2026!").entries().count(), 1);
        let config = env.config("a");
        let restored = config.vault("perso-sauvegarde-1970-01-01").unwrap();
        let data = fs::read(&file).unwrap();
        assert_eq!(
            BackupFile::parse(&data).unwrap().vault("perso"),
            Some(fs::read(restored.vault_file()).unwrap().as_slice())
        );

        assert_eq!(
            app.run(restore(&file, Some("perso"))),
            Err(CliError::App(AppError::Config(ConfigError::VaultExists(
                "perso-sauvegarde-1970-01-01".to_owned()
            ))))
        );
    }

    #[test]
    fn restore_asks_which_vault_when_none_is_given() {
        let env = env_with_two_vaults();
        let file = backup(&env, None);
        let mut app = env.app("b", &[" travail "]);
        app.run(restore(&file, None)).unwrap();
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
    fn unknown_vaults_and_unusable_files_are_reported() {
        let env = TestEnv::with_vault();
        let file = backup(&env, None);
        let mut app = env.app("b", &[]);
        assert_eq!(
            app.run(restore(&file, Some("travail"))),
            Err(CliError::App(AppError::NotInBackup("travail".to_owned())))
        );
        let other = env.dir.path().join("autre.pswd");
        fs::write(&other, BackupFile::new(Timestamp::from_millis(0)).encode()).unwrap();
        assert_eq!(
            app.run(restore(&other, None)),
            Err(CliError::App(AppError::EmptyBackup))
        );
        fs::write(&other, b"pas une sauvegarde").unwrap();
        assert_eq!(
            app.run(restore(&other, None)),
            Err(CliError::App(AppError::BackupFile(
                BackupFileError::Malformed
            )))
        );
        let error = app
            .run(restore(&env.dir.path().join("absente.pswd"), None))
            .unwrap_err();
        assert!(error.to_string().contains("lecture de"), "{error}");
        assert!(env.config("b").vaults.is_empty());
    }

    #[test]
    fn local_write_failure_leaves_nothing_behind() {
        let env = TestEnv::with_vault();
        let file = backup(&env, None);
        let vaults = env.vaults_dir("b");
        fs::create_dir_all(vaults.parent().unwrap()).unwrap();
        fs::write(&vaults, b"").unwrap();
        let mut app = env.app("b", &[]);
        assert!(matches!(
            app.run(restore(&file, Some("perso"))),
            Err(CliError::App(AppError::Port(_)))
        ));
        assert!(env.config("b").vaults.is_empty());
    }
}
