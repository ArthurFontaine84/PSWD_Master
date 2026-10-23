//! Doublures communes aux tests des commandes.

use std::collections::VecDeque;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use pswd_core::crypto::KdfParams;
use pswd_core::ports::{PortError, SecureClipboard};
use pswd_core::{Credentials, Secret, Session, Timestamp, Vault};
use pswd_infra::{Config, ConfigFile, FileStore, GitRemote, KeyringStore, Locations};
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

    fn next_answer(&mut self) -> Result<String, CliError> {
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

pub type TestApp = App<ScriptedTerminal, KeyringStore, FakeClipboard>;

/// Un dépôt distant nu et des « appareils », chacun avec ses propres dossiers.
pub struct TestEnv {
    pub dir: TempDir,
    pub url: String,
    keyring: Arc<keyring_core::mock::Store>,
    clipboard: FakeClipboard,
}

impl TestEnv {
    pub fn new() -> Self {
        let dir = TempDir::new().unwrap();
        let remote = dir.path().join("remote.git");
        git2::Repository::init_bare(&remote).unwrap();
        let url = remote.to_str().unwrap().to_owned();
        let keyring = keyring_core::mock::Store::new().unwrap();
        Self {
            dir,
            url,
            keyring,
            clipboard: FakeClipboard::default(),
        }
    }

    /// Coffre `perso` créé sur l'appareil `a`, sans jeton, mot de passe « maître ».
    pub fn with_vault() -> Self {
        let env = Self::new();
        let mut app = env.app("a", &[&env.url, "", "maître", "maître"]);
        app.run(Command::Init {
            name: "perso".to_owned(),
        })
        .unwrap();
        env
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
            keyring: KeyringStore::with_store(self.keyring.clone()),
            clipboard: self.clipboard.clone(),
            locations: self.locations(device),
            kdf: KdfParams::new(8 * 1024, 1, 1).unwrap(),
            clock: || Timestamp::from_millis(1_000),
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

    /// Modifie le coffre `perso` depuis l'appareil `device`, puis le publie.
    pub fn edit_vault(&self, device: &str, change: impl FnOnce(&mut Vault)) {
        let vault = self.config(device).vault(Some("perso")).unwrap().1.clone();
        let remote = GitRemote::open(&vault.repo_dir(), &vault.url, None).unwrap();
        let password = Secret::new("maître".to_owned());
        let mut session = Session::open(
            FileStore::new(vault.vault_file()),
            remote,
            Credentials::Password(&password),
        )
        .unwrap();
        session.edit(change).unwrap();
    }

    /// Le dépôt distant devient injoignable.
    pub fn take_remote_offline(&self) {
        std::fs::rename(&self.url, self.dir.path().join("hors-ligne.git")).unwrap();
    }

    pub fn clipboard_copies(&self) -> Vec<(String, Duration)> {
        self.clipboard.0.lock().unwrap().copies.clone()
    }

    pub fn break_clipboard(&self) {
        self.clipboard.0.lock().unwrap().broken = true;
    }

    pub fn remote_has_vault(&self) -> bool {
        let repo = git2::Repository::open_bare(&self.url).unwrap();
        let tree = repo
            .find_reference("refs/heads/main")
            .unwrap()
            .peel_to_tree()
            .unwrap();
        tree.get_name("vault.enc").is_some()
    }
}
