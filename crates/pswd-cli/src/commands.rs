//! Commandes ponctuelles sur un coffre existant : `passwd`, `recover`, `import`.

use std::fs;
use std::path::Path;

use pswd_app::{AppError, VaultSession, add_entries};
use pswd_core::ports::SecureClipboard;
use pswd_core::{Credentials, RecoveryKey, import_csv};
use pswd_infra::Config;

use crate::app::{App, CliError};
use crate::terminal::Terminal;

/// Ce que l'utilisateur fournit pour déverrouiller le coffre.
#[derive(Clone, Copy)]
pub(crate) enum Unlock {
    Password,
    RecoveryKey,
}

/// Coffre déverrouillé, avec son nom et la configuration.
pub(crate) struct Unlocked {
    pub name: String,
    pub session: VaultSession,
    pub config: Config,
}

impl<T: Terminal, C: SecureClipboard> App<T, C> {
    pub(crate) fn unlock(&mut self, name: &str, unlock: Unlock) -> Result<Unlocked, CliError> {
        let config = self.vaults.config()?;
        config.vault(name)?;
        let session = match unlock {
            Unlock::Password => {
                let password = self.terminal.prompt_secret("Mot de passe maître : ")?;
                self.vaults.open(name, Credentials::Password(&password))?
            }
            Unlock::RecoveryKey => {
                let text = self.terminal.prompt_secret("Clé de récupération : ")?;
                let key = RecoveryKey::parse(text.expose()).map_err(CliError::RecoveryKey)?;
                self.vaults.open(name, Credentials::RecoveryKey(&key))?
            }
        };
        Ok(Unlocked {
            name: name.to_owned(),
            session,
            config,
        })
    }

    /// `passwd` (ancien mot de passe) et `recover` (clé de récupération).
    pub(crate) fn change_password(&mut self, name: &str, unlock: Unlock) -> Result<(), CliError> {
        let mut unlocked = self.unlock(name, unlock)?;
        let password = self.new_master_password()?;
        unlocked
            .session
            .change_password(&password, self.kdf, (self.clock)())?;
        self.terminal.print(&format!(
            "Mot de passe maître de « {} » changé.",
            unlocked.name
        ));
        Ok(())
    }

    /// Tout le fichier est validé avant d'ajouter quoi que ce soit.
    pub(crate) fn import(&mut self, name: &str, file: &Path) -> Result<(), CliError> {
        let data = fs::read(file).map_err(|error| {
            AppError::File(format!(
                "lecture de {} impossible : {error}",
                file.display()
            ))
        })?;
        let contents = import_csv(&data).map_err(AppError::Csv)?;
        let mut unlocked = self.unlock(name, Unlock::Password)?;
        let count = add_entries(&mut unlocked.session, contents, (self.clock)())?;
        self.terminal.print(&format!(
            "{count} entrée(s) importée(s) dans « {} ».",
            unlocked.name
        ));
        self.terminal.print(&format!(
            "⚠ {} contient vos mots de passe en clair : supprimez-le.",
            file.display()
        ));
        Ok(())
    }

    pub(crate) fn confirm(&mut self, question: &str) -> Result<bool, CliError> {
        let answer = self.terminal.prompt(question)?;
        Ok(matches!(answer.trim().to_lowercase().as_str(), "o" | "oui"))
    }
}

#[cfg(test)]
mod tests {
    use pswd_app::AppError;
    use pswd_core::{FileError, RecoveryKeyError, SessionError, Vault};

    use crate::cli::Command;
    use crate::testing::TestEnv;

    use super::*;

    fn titles(vault: &Vault) -> Vec<String> {
        let mut titles: Vec<String> = vault
            .entries()
            .filter(|entry| !entry.is_deleted())
            .map(|entry| entry.content().title.clone())
            .collect();
        titles.sort_unstable();
        titles
    }

    const CSV: &str = "title,kind,username,password,urls,notes,tags\r\n\
        GitHub,login,arthur,hunter2,https://github.com,,dev\r\n\
        Wi-Fi,note,,,,ABCD,maison\r\n";

    #[test]
    fn passwd_replaces_the_master_password() {
        let env = TestEnv::with_vault();
        let mut app = env.app("a", &["Maître-2026!", "Nouveau-2026!", "Nouveau-2026!"]);
        app.run(Command::Passwd {
            name: "perso".to_owned(),
        })
        .unwrap();
        assert_eq!(
            app.terminal.output(),
            "Mot de passe maître de « perso » changé."
        );
        assert!(env.local_vault_result("a", "Maître-2026!").is_err());
        env.local_vault("a", "Nouveau-2026!");
    }

    #[test]
    fn passwd_requires_the_current_password() {
        let env = TestEnv::with_vault();
        let mut app = env.app("a", &["autre"]);
        assert_eq!(
            app.run(Command::Passwd {
                name: "perso".to_owned()
            }),
            Err(CliError::App(AppError::Session(SessionError::File(
                FileError::WrongPassword
            ))))
        );
    }

    #[test]
    fn passwd_requires_a_confirmed_new_password() {
        let env = TestEnv::with_vault();
        let mut app = env.app("a", &["Maître-2026!", "Nouveau-2026!", "Nouvau-2026!"]);
        assert_eq!(
            app.run(Command::Passwd {
                name: "perso".to_owned()
            }),
            Err(CliError::PasswordMismatch)
        );
        env.local_vault("a", "Maître-2026!");
    }

    #[test]
    fn recover_sets_a_new_password_with_the_recovery_key() {
        let (env, recovery_key) = TestEnv::with_vault_and_recovery_key();
        let typed = recovery_key.to_lowercase().replace('-', " ");
        let mut app = env.app("a", &[&typed, "Retrouvé-2026!", "Retrouvé-2026!"]);
        app.run(Command::Recover {
            name: "perso".to_owned(),
        })
        .unwrap();
        env.local_vault("a", "Retrouvé-2026!");
    }

    #[test]
    fn recover_rejects_an_invalid_or_wrong_key() {
        let env = TestEnv::with_vault();
        let mut app = env.app("a", &["pas une clé"]);
        assert_eq!(
            app.run(Command::Recover {
                name: "perso".to_owned()
            }),
            Err(CliError::RecoveryKey(RecoveryKeyError::InvalidCharacter))
        );
        let other_key = RecoveryKey::generate().unwrap().encode();
        let mut app = env.app("a", &[other_key.expose()]);
        assert_eq!(
            app.run(Command::Recover {
                name: "perso".to_owned()
            }),
            Err(CliError::App(AppError::Session(SessionError::File(
                FileError::WrongRecoveryKey
            ))))
        );
    }

    #[test]
    fn import_adds_every_entry_and_reminds_to_delete_the_file() {
        let env = TestEnv::with_vault();
        let file = env.dir.path().join("import.csv");
        fs::write(&file, CSV).unwrap();
        let mut app = env.app("a", &["Maître-2026!"]);
        app.run(Command::Import {
            name: "perso".to_owned(),
            file: file.clone(),
        })
        .unwrap();
        let output = app.terminal.output();
        assert!(
            output.starts_with("2 entrée(s) importée(s) dans « perso ».\n"),
            "{output}"
        );
        assert!(output.ends_with("contient vos mots de passe en clair : supprimez-le."));
        assert_eq!(
            titles(&env.local_vault("a", "Maître-2026!")),
            ["GitHub", "Wi-Fi"]
        );
    }

    #[test]
    fn invalid_or_missing_csv_imports_nothing() {
        let env = TestEnv::with_vault();
        let file = env.dir.path().join("import.csv");
        fs::write(&file, format!("{CSV}incomplète,login\r\n")).unwrap();
        let mut app = env.app("a", &["Maître-2026!"]);
        assert_eq!(
            app.run(Command::Import {
                name: "perso".to_owned(),
                file: file.clone(),
            }),
            Err(CliError::App(AppError::Csv(
                pswd_core::CsvError::InvalidLine {
                    line: 4,
                    problem: pswd_core::LineProblem::FieldCount
                }
            )))
        );
        let missing = env.dir.path().join("absent.csv");
        let error = app
            .run(Command::Import {
                name: "perso".to_owned(),
                file: missing,
            })
            .unwrap_err();
        assert!(error.to_string().starts_with("lecture de "), "{error}");
        assert_eq!(
            titles(&env.local_vault("a", "Maître-2026!")),
            Vec::<String>::new()
        );
    }

    #[test]
    fn import_reports_a_local_write_failure() {
        let env = TestEnv::with_vault();
        let file = env.dir.path().join("import.csv");
        fs::write(&file, CSV).unwrap();
        let vault = env.config("a").vault("perso").unwrap().clone();
        let mut temporary = vault.vault_file().into_os_string();
        temporary.push(".tmp");
        fs::create_dir(&temporary).unwrap();
        let mut app = env.app("a", &["Maître-2026!"]);
        assert!(matches!(
            app.run(Command::Import {
                name: "perso".to_owned(),
                file
            }),
            Err(CliError::App(AppError::Session(SessionError::Local(_))))
        ));
        assert_eq!(
            titles(&env.local_vault("a", "Maître-2026!")),
            Vec::<String>::new()
        );
    }
}
