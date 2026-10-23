//! Doublures communes aux tests des commandes.

use std::collections::VecDeque;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use pswd_core::crypto::KdfParams;
use pswd_core::ports::{PortError, SecureClipboard};
use pswd_core::{Credentials, FileError, Secret, Session, Timestamp, Vault, VaultFile};
use pswd_infra::{Config, ConfigFile, FileStore, Locations};
use tempfile::TempDir;

use crate::app::{App, CliError};
use crate::cli::Command;
use crate::terminal::Terminal;

/// Réponse qui simule une saisie impossible.
pub const BROKEN_INPUT: &str = "<saisie impossible>";

/// Terminal qui répond aux questions dans l'ordre prévu et garde ce qui est affiché.
pub struct ScriptedTerminal {
    answers: VecDeque<String>,
    output: Vec<String>,
}

impl ScriptedTerminal {
    pub fn output(&self) -> String {
        self.output.join("\n")
    }

    /// `<pause:N>` attend N millisecondes avant de passer à la réponse suivante.
    fn next_answer(&mut self) -> Result<String, CliError> {
        while let Some(pause) = self
            .answers
            .front()
            .and_then(|answer| answer.strip_prefix("<pause:"))
            .and_then(|rest| rest.strip_suffix('>'))
            .and_then(|millis| millis.parse().ok())
        {
            self.answers.pop_front();
            std::thread::sleep(Duration::from_millis(pause));
        }
        match self.answers.pop_front() {
            Some(answer) if answer == BROKEN_INPUT => {
                Err(CliError::Input("clavier débranché".to_owned()))
            }
            Some(answer) => Ok(answer),
            None => Err(CliError::InputClosed),
        }
    }
}

impl Terminal for ScriptedTerminal {
    fn prompt(&mut self, _: &str) -> Result<String, CliError> {
        self.next_answer()
    }

    fn prompt_secret(&mut self, _: &str) -> Result<Secret, CliError> {
        self.next_answer().map(Secret::new)
    }

    fn print(&mut self, text: &str) {
        self.output.push(text.to_owned());
    }
}

#[derive(Default)]
struct ClipboardData {
    copies: Vec<(String, Duration)>,
    broken: bool,
}

/// Presse-papiers qui note les copies, partagé avec le test.
#[derive(Clone, Default)]
pub struct FakeClipboard(Arc<Mutex<ClipboardData>>);

impl SecureClipboard for FakeClipboard {
    fn copy_secret(&mut self, value: &Secret, ttl: Duration) -> Result<(), PortError> {
        let mut data = self.0.lock().unwrap();
        if data.broken {
            return Err(PortError::new("presse-papiers indisponible"));
        }
        data.copies.push((value.expose().to_owned(), ttl));
        Ok(())
    }
}

pub type TestApp = App<ScriptedTerminal, FakeClipboard>;

/// Des « appareils », chacun avec ses propres dossiers.
pub struct TestEnv {
    pub dir: TempDir,
    clipboard: FakeClipboard,
}

impl TestEnv {
    pub fn new() -> Self {
        Self {
            dir: TempDir::new().unwrap(),
            clipboard: FakeClipboard::default(),
        }
    }

    /// Coffre `perso` créé sur l'appareil `a`, mot de passe « maître ».
    pub fn with_vault() -> Self {
        Self::with_vault_and_recovery_key().0
    }

    /// Comme [`TestEnv::with_vault`], avec la clé de récupération affichée.
    pub fn with_vault_and_recovery_key() -> (Self, String) {
        let env = Self::new();
        let mut app = env.app("a", &["maître", "maître"]);
        app.run(Command::Init {
            name: "perso".to_owned(),
        })
        .unwrap();
        let output = app.terminal.output();
        let key = output
            .lines()
            .find(|line| line.starts_with("    "))
            .unwrap()
            .trim()
            .to_owned();
        (env, key)
    }

    /// Le coffre `perso` tel qu'il est enregistré sur l'appareil.
    pub fn local_vault(&self, device: &str, password: &str) -> Vault {
        self.local_vault_result(device, password).unwrap()
    }

    pub fn local_vault_result(&self, device: &str, password: &str) -> Result<Vault, FileError> {
        let vault = self.config(device).vault("perso").unwrap().clone();
        let file = std::fs::read(vault.vault_file()).unwrap();
        VaultFile::unlock_with_password(&file, &Secret::new(password.to_owned()))
            .map(|(_, vault)| vault)
    }

    fn locations(&self, device: &str) -> Locations {
        let root = self.dir.path().join(device);
        Locations::new(&root.join("config"), &root.join("data"))
    }

    pub fn app(&self, device: &str, answers: &[&str]) -> TestApp {
        App {
            terminal: ScriptedTerminal {
                answers: answers.iter().map(|&answer| answer.to_owned()).collect(),
                output: Vec::new(),
            },
            clipboard: self.clipboard.clone(),
            locations: self.locations(device),
            kdf: KdfParams::new(8 * 1024, 1, 1).unwrap(),
            clock: || Timestamp::from_millis(1_000),
            auto_lock: None,
        }
    }

    pub fn config(&self, device: &str) -> Config {
        ConfigFile::new(self.locations(device).config_file())
            .load()
            .unwrap()
    }

    pub fn vault_dir(&self, device: &str, name: &str) -> PathBuf {
        self.locations(device).vault_dir(name)
    }

    /// Modifie le coffre `perso` de l'appareil `device`.
    pub fn edit_vault(&self, device: &str, change: impl FnOnce(&mut Vault)) {
        self.session(device).edit(change).unwrap();
    }

    /// Session ouverte sur le coffre `perso` de l'appareil, mot de passe « maître ».
    pub fn session(&self, device: &str) -> Session<FileStore> {
        let vault = self.config(device).vault("perso").unwrap().clone();
        let password = Secret::new("maître".to_owned());
        Session::open(
            FileStore::new(vault.vault_file()),
            Credentials::Password(&password),
        )
        .unwrap()
    }

    pub fn clipboard_copies(&self) -> Vec<(String, Duration)> {
        self.clipboard.0.lock().unwrap().copies.clone()
    }

    pub fn break_clipboard(&self) {
        self.clipboard.0.lock().unwrap().broken = true;
    }
}
