//! Serveur : `connect`, `disconnect`, `add-device`, `status`, `sync`, et la synchro d'un coffre
//! ouvert.

use pswd_app::{AppError, KeptReason, SyncOutcome, Update, VaultSession, VaultState};
use pswd_core::{ConnectionKey, SyncStatus};

use crate::app::{App, CliError};
use crate::terminal::Terminal;

impl<T: Terminal, C> App<T, C> {
    pub(crate) fn connect(&mut self, address: &str) -> Result<(), CliError> {
        if self.vaults.server()?.is_some() {
            return Err(AppError::AlreadyConnected.into());
        }
        let key = self.terminal.prompt_secret("Clé de connexion : ")?;
        let key = ConnectionKey::parse(key.expose()).map_err(CliError::ConnectionKey)?;
        let device_name = self.terminal.prompt("Nom de cet appareil : ")?;
        let account_name = self
            .terminal
            .prompt("Nom du compte (clé administrateur seulement, sinon laisser vide) : ")?;
        let account_name = Some(account_name.trim()).filter(|name| !name.is_empty());
        self.vaults
            .connect(address, &key, device_name.trim(), account_name)?;
        self.terminal.print("Appareil connecté au serveur.");
        if let Err(error) = self.exchange() {
            self.terminal.print(&format!(
                "⚠ Échange avec le serveur incomplet : {error}. Réessayer avec pswd sync."
            ));
        }
        Ok(())
    }

    /// `pswd sync` : échange avec le serveur, sans mot de passe.
    pub(crate) fn sync_vaults(&mut self) -> Result<(), CliError> {
        if !self.exchange()? {
            self.terminal.print("Coffres à jour.");
        }
        Ok(())
    }

    /// Renvoie `true` si l'échange a changé quelque chose ou rencontré un problème.
    fn exchange(&mut self) -> Result<bool, CliError> {
        let exchange = self.vaults.exchange()?;
        let mut lines = Vec::new();
        for (server_name, result) in exchange.downloaded {
            lines.push(match result {
                Ok(name) if name == server_name => format!("Coffre « {name} » téléchargé."),
                Ok(name) => format!(
                    "Coffre « {server_name} » téléchargé sous le nom « {name} » (nom déjà pris \
                     sur cet appareil)."
                ),
                Err(error) => {
                    format!("⚠ Coffre « {server_name} » du serveur non téléchargé : {error}")
                }
            });
        }
        for (name, result) in exchange.published {
            lines.push(match result {
                Ok(()) => format!("Coffre « {name} » publié."),
                Err(error) => format!("⚠ Coffre « {name} » non publié : {error}"),
            });
        }
        for (name, result) in exchange.updated {
            lines.push(match result {
                Ok(Update::Sent) => format!("Coffre « {name} » envoyé au serveur."),
                Ok(Update::Received) => format!("Coffre « {name} » mis à jour depuis le serveur."),
                Ok(Update::NeedsUnlock) => format!(
                    "⚠ Coffre « {name} » modifié ici et sur le serveur : l'ouvrir pour terminer \
                     la synchro."
                ),
                Err(error) => format!("⚠ Coffre « {name} » non synchronisé : {error}"),
            });
        }
        for name in exchange.removed {
            lines.push(format!(
                "Coffre « {name} » supprimé du serveur : gardé sur cet appareil. Pour le \
                 republier, l'ouvrir puis taper sync."
            ));
        }
        for (name, result) in exchange.left {
            lines.push(match result {
                Ok(()) => format!(
                    "Coffre « {name} » supprimé de cet appareil : ce compte n'en est plus membre."
                ),
                Err(error) => format!("⚠ Coffre « {name} » quitté mais pas supprimé : {error}"),
            });
        }
        for line in &lines {
            self.terminal.print(line);
        }
        Ok(!lines.is_empty())
    }

    pub(crate) fn disconnect(&mut self) -> Result<(), CliError> {
        if self.vaults.server()?.is_none() {
            return Err(AppError::NotConnected.into());
        }
        let answer = self
            .terminal
            .prompt("Garder une copie des coffres sur cet appareil ? [O/n] ")?;
        let keep_local = !matches!(answer.trim().to_lowercase().as_str(), "n" | "non");
        let report = self.vaults.disconnect(keep_local)?;
        self.terminal.print("Appareil déconnecté du serveur.");
        if let Some(error) = report.revoke_error {
            self.terminal.print(&format!(
                "⚠ Jeton de l'appareil non révoqué ({error}) : le révoquer depuis le panneau \
                 d'administration."
            ));
        }
        for name in report.removed {
            self.terminal.print(&format!(
                "Coffre « {name} » retiré de cet appareil ; il reste sur le serveur."
            ));
        }
        for (name, reason) in report.kept {
            let reason = match reason {
                KeptReason::Unpublished => {
                    "des modifications ne sont pas encore sur le serveur".to_owned()
                }
                KeptReason::Error(error) => error.to_string(),
            };
            self.terminal
                .print(&format!("Coffre « {name} » gardé : {reason}."));
        }
        Ok(())
    }

    pub(crate) fn add_device(&mut self) -> Result<(), CliError> {
        let server = self.vaults.server()?.ok_or(AppError::NotConnected)?;
        let key = self.vaults.new_device_key()?;
        self.terminal.print(&format!(
            "Clé de connexion d'un nouvel appareil (une seule utilisation, valable 7 jours) :\n\n    \
             {}\n\nSur le nouvel appareil : pswd connect {}",
            key.expose(),
            server.local_address
        ));
        Ok(())
    }

    pub(crate) fn status(&mut self) -> Result<(), CliError> {
        let names: Vec<String> = self.vaults.config()?.vaults.into_keys().collect();
        let states = if let Some(server) = self.vaults.server()? {
            self.terminal
                .print(&format!("Serveur : {}", server.local_address));
            self.vaults
                .vault_states()
                .inspect_err(|error| self.terminal.print(&format!("⚠ {error}")))
                .ok()
        } else {
            self.terminal
                .print("Aucun serveur (pswd connect <adresse> pour s'y connecter).");
            None
        };
        if names.is_empty() {
            self.terminal.print("Aucun coffre.");
        }
        let lines: Vec<String> = states.map_or_else(
            || names.iter().map(|name| format!("  {name}")).collect(),
            |states| {
                states
                    .into_iter()
                    .map(|(name, state)| format!("  {name} : {}", state_label(state)))
                    .collect()
            },
        );
        for line in &lines {
            self.terminal.print(line);
        }
        Ok(())
    }

    /// Synchro automatique : rien n'est affiché si tout va bien. Renvoie `false` si le coffre a
    /// été supprimé de l'appareil.
    pub(crate) fn sync_quietly(&mut self, name: &str, session: &mut VaultSession) -> bool {
        match self.vaults.sync(name, session) {
            Ok(SyncOutcome::Done(SyncStatus::Synced) | SyncOutcome::NotConnected) => true,
            Ok(outcome) => {
                self.terminal
                    .print(&format!("⚠ Synchro : {}.", describe(&outcome)));
                outcome != SyncOutcome::Left
            }
            Err(error) => {
                self.terminal.print(&format!("⚠ Synchro : {error}."));
                true
            }
        }
    }

    /// Commande `sync` du shell : un coffre supprimé du serveur y est republié.
    pub(crate) fn sync_open_vault(
        &mut self,
        name: &str,
        session: &mut VaultSession,
    ) -> Result<(), CliError> {
        let outcome = self.vaults.republish(name, session)?;
        self.terminal
            .print(&format!("Synchro : {}.", describe(&outcome)));
        Ok(())
    }
}

fn state_label(state: VaultState) -> &'static str {
    match state {
        VaultState::Local => "local, publié au prochain pswd sync",
        VaultState::Synced => "à jour",
        VaultState::Outdated => "mis à jour au prochain pswd sync",
        VaultState::NeedsUnlock => "modifié ici et sur le serveur : l'ouvrir pour le synchroniser",
        VaultState::Removed => "supprimé du serveur : l'ouvrir puis taper sync pour le republier",
    }
}

fn describe(outcome: &SyncOutcome) -> String {
    match outcome {
        SyncOutcome::NotConnected => {
            "cet appareil n'est connecté à aucun serveur (pswd connect)".to_owned()
        }
        SyncOutcome::Removed | SyncOutcome::Done(SyncStatus::Removed) => {
            "coffre supprimé du serveur : gardé sur cet appareil ; taper sync pour le republier"
                .to_owned()
        }
        SyncOutcome::Left => {
            "ce compte n'est plus membre du coffre : il a été supprimé de cet appareil".to_owned()
        }
        SyncOutcome::Done(SyncStatus::Synced) => "coffre à jour".to_owned(),
        SyncOutcome::Done(SyncStatus::Offline(error)) => {
            format!("hors ligne ({error}) ; les modifications seront publiées plus tard")
        }
        SyncOutcome::Done(SyncStatus::Conflict) => {
            "le coffre a changé sur le serveur à chaque essai ; réessayer avec sync".to_owned()
        }
        SyncOutcome::Done(SyncStatus::Rejected(error)) => {
            format!("copie du serveur refusée ({error}), elle n'est pas remplacée")
        }
    }
}

#[cfg(test)]
mod tests {
    use std::fs;

    use pswd_core::ports::PortError;
    use pswd_core::{ConnectionKeyError, EntryContent, EntryKind, FileError, Timestamp};

    use crate::cli::Command;
    use crate::testing::TestEnv;

    use super::*;

    const PASSWORD: &str = "Maître-2026!";
    const ADD_NOTE: [&str; 5] = ["add", "note", "Wi-Fi", "", ""];

    fn open() -> Command {
        Command::Open {
            name: "perso".to_owned(),
        }
    }

    fn init() -> Command {
        Command::Init {
            name: "perso".to_owned(),
        }
    }

    /// Lance `command` sur l'appareil et renvoie ce qui a été affiché.
    fn output(env: &TestEnv, device: &str, answers: &[&str], command: Command) -> String {
        let mut app = env.app(device, answers);
        app.run(command).unwrap();
        app.terminal.output()
    }

    fn status(env: &TestEnv, device: &str) -> String {
        output(env, device, &[], Command::Status)
    }

    fn titles(env: &TestEnv, device: &str) -> Vec<String> {
        env.local_vault(device, PASSWORD)
            .entries()
            .map(|entry| entry.content().title.clone())
            .collect()
    }

    #[test]
    fn connection_publishes_the_local_vaults() {
        let env = TestEnv::with_vault();
        let (_, shown) = env.connect_admin("a");
        assert_eq!(
            shown,
            "Appareil connecté au serveur.\nCoffre « perso » publié."
        );
        assert!(status(&env, "a").ends_with("\n  perso : à jour"));
    }

    #[test]
    fn another_device_downloads_the_vaults_of_the_account() {
        let env = TestEnv::with_vault();
        env.connect_admin("a");
        assert_eq!(
            env.connect_same_account("a", "b"),
            "Appareil connecté au serveur.\nCoffre « perso » téléchargé."
        );
        env.local_vault("b", PASSWORD);
    }

    #[test]
    fn a_taken_name_is_reported_at_download() {
        let env = TestEnv::with_vault();
        env.connect_admin("a");
        output(&env, "b", &[PASSWORD, PASSWORD], init());
        let shown = env.connect_same_account("a", "b");
        let lines: Vec<&str> = shown.lines().collect();
        assert_eq!(lines.len(), 3, "{shown}");
        assert!(
            lines[1].starts_with("Coffre « perso » téléchargé sous le nom « perso-"),
            "{shown}"
        );
        assert_eq!(lines[2], "Coffre « perso » publié.");
    }

    #[test]
    fn failed_download_is_reported() {
        let env = TestEnv::with_vault();
        env.connect_admin("a");
        let vaults_dir = env.vault_dir("b", "perso").parent().unwrap().to_owned();
        fs::create_dir_all(vaults_dir.parent().unwrap()).unwrap();
        fs::write(&vaults_dir, b"").unwrap();
        let shown = env.connect_same_account("a", "b");
        assert!(
            shown.contains("⚠ Coffre « perso » du serveur non téléchargé : "),
            "{shown}"
        );
    }

    #[test]
    fn failed_publication_is_reported() {
        let env = TestEnv::with_vault();
        let file = env.config("a").vault("perso").unwrap().vault_file();
        fs::write(file, b"abime").unwrap();
        let (_, shown) = env.connect_admin("a");
        assert!(
            shown.ends_with("⚠ Coffre « perso » non publié : réponse inattendue du serveur (400)"),
            "{shown}"
        );
    }

    #[test]
    fn connection_survives_an_incomplete_exchange() {
        let env = TestEnv::new();
        let config = env.dir.path().join("a").join("config").join("config.toml");
        fs::create_dir_all(config.parent().unwrap()).unwrap();
        fs::write(&config, "couleur = 1").unwrap();
        let (_, shown) = env.connect_admin("a");
        assert!(
            shown.starts_with(
                "Appareil connecté au serveur.\n⚠ Échange avec le serveur incomplet : \
                 configuration invalide"
            ),
            "{shown}"
        );
        assert!(shown.ends_with("Réessayer avec pswd sync."), "{shown}");
        assert!(env.vaults("a").server().unwrap().is_some());
    }

    #[test]
    fn connection_requires_a_valid_key_and_a_disconnected_device() {
        let env = TestEnv::new();
        let mut app = env.app("a", &["pas une clé"]);
        assert_eq!(
            app.run(Command::Connect {
                address: "127.0.0.1".to_owned()
            }),
            Err(CliError::ConnectionKey(ConnectionKeyError::Malformed))
        );
        let (server, _) = env.connect_admin("a");
        let mut app = env.app("a", &[]);
        assert_eq!(
            app.run(Command::Connect {
                address: server.address
            }),
            Err(CliError::App(AppError::AlreadyConnected))
        );
    }

    #[test]
    fn sync_downloads_new_vaults_and_publishes_local_ones() {
        let env = TestEnv::new();
        env.connect_admin("a");
        env.connect_same_account("a", "b");
        assert_eq!(output(&env, "a", &[], Command::Sync), "Coffres à jour.");
        output(&env, "a", &[PASSWORD, PASSWORD], init());
        assert_eq!(
            output(&env, "a", &[], Command::Sync),
            "Coffre « perso » publié."
        );
        assert_eq!(
            output(&env, "b", &[], Command::Sync),
            "Coffre « perso » téléchargé."
        );
    }

    #[test]
    fn server_commands_require_a_connection() {
        let env = TestEnv::new();
        for command in [Command::Sync, Command::Disconnect, Command::AddDevice] {
            let mut app = env.app("a", &[]);
            assert_eq!(app.run(command), Err(CliError::App(AppError::NotConnected)));
        }
    }

    fn shared_vault(env: &TestEnv, device: &str) {
        let params = pswd_core::crypto::KdfParams::new(8 * 1024, 1, 1).unwrap();
        let password = pswd_core::Secret::new(PASSWORD.to_owned());
        env.vaults(device)
            .create_shared("famille", &password, params, Timestamp::from_millis(0))
            .unwrap();
    }

    fn open_famille() -> Command {
        Command::Open {
            name: "famille".to_owned(),
        }
    }

    #[test]
    fn a_vault_left_on_another_device_is_deleted() {
        let env = TestEnv::new();
        env.connect_admin("a");
        shared_vault(&env, "a");
        output(&env, "a", &[], Command::Sync);
        env.connect_same_account("a", "b");
        env.vaults("a").leave("famille").unwrap();

        assert_eq!(
            output(&env, "b", &[], Command::Sync),
            "Coffre « famille » supprimé de cet appareil : ce compte n'en est plus membre."
        );
        assert!(status(&env, "b").ends_with("\nAucun coffre."));
    }

    #[test]
    fn opening_a_vault_left_elsewhere_deletes_it() {
        let env = TestEnv::new();
        env.connect_admin("a");
        shared_vault(&env, "a");
        output(&env, "a", &[], Command::Sync);
        env.connect_same_account("a", "b");
        env.vaults("a").leave("famille").unwrap();

        let shown = output(&env, "b", &[PASSWORD], open_famille());
        assert!(
            shown.ends_with(
                "⚠ Synchro : ce compte n'est plus membre du coffre : il a été supprimé de cet \
                 appareil."
            ),
            "{shown}"
        );
        assert!(env.config("b").vaults.is_empty());
    }

    #[test]
    fn a_vault_left_but_not_deleted_is_reported() {
        let env = TestEnv::new();
        env.connect_admin("a");
        shared_vault(&env, "a");
        output(&env, "a", &[], Command::Sync);
        env.connect_same_account("a", "b");
        env.vaults("a").leave("famille").unwrap();
        let file = env.config("b").vault("famille").unwrap().vault_file();
        let mut temporary = file.into_os_string();
        temporary.push(".tmp");
        fs::create_dir(&temporary).unwrap();
        let shown = output(&env, "b", &[], Command::Sync);
        assert!(
            shown.starts_with("⚠ Coffre « famille » quitté mais pas supprimé : "),
            "{shown}"
        );
    }

    #[test]
    fn a_vault_deleted_from_the_server_is_kept_until_republished() {
        let env = TestEnv::with_vault();
        env.connect_admin("a");
        env.connect_same_account("a", "b");
        env.vaults("a").delete_everywhere("perso").unwrap();

        assert_eq!(
            output(&env, "b", &[], Command::Sync),
            "Coffre « perso » supprimé du serveur : gardé sur cet appareil. Pour le republier, \
             l'ouvrir puis taper sync."
        );
        assert!(status(&env, "b").ends_with(
            "  perso : supprimé du serveur : l'ouvrir puis taper sync pour le republier"
        ));
        let shown = output(&env, "b", &[PASSWORD, "sync"], open());
        assert!(shown.ends_with("Synchro : coffre à jour."), "{shown}");
        assert!(status(&env, "b").ends_with("  perso : à jour"));
    }

    #[test]
    fn sync_updates_vaults_without_their_password() {
        let env = TestEnv::with_vault();
        env.connect_admin("a");
        env.connect_same_account("a", "b");
        let note = |title: &str| {
            let title = title.to_owned();
            move |vault: &mut pswd_core::Vault| {
                vault.add(
                    EntryContent::new(EntryKind::Note, title),
                    Timestamp::from_millis(1),
                );
            }
        };
        env.edit_vault("b", note("Banque"));
        assert!(status(&env, "b").ends_with("  perso : mis à jour au prochain pswd sync"));
        assert_eq!(
            output(&env, "b", &[], Command::Sync),
            "Coffre « perso » envoyé au serveur."
        );
        assert_eq!(
            output(&env, "a", &[], Command::Sync),
            "Coffre « perso » mis à jour depuis le serveur."
        );
        assert_eq!(titles(&env, "a"), ["Banque"]);

        env.edit_vault("a", note("Wi-Fi"));
        env.edit_vault("b", note("Cantine"));
        output(&env, "a", &[], Command::Sync);
        assert!(
            status(&env, "b").ends_with(
                "  perso : modifié ici et sur le serveur : l'ouvrir pour le synchroniser"
            )
        );
        assert_eq!(
            output(&env, "b", &[], Command::Sync),
            "⚠ Coffre « perso » modifié ici et sur le serveur : l'ouvrir pour terminer la synchro."
        );

        fs::remove_file(env.config("b").vault("perso").unwrap().vault_file()).unwrap();
        let shown = output(&env, "b", &[], Command::Sync);
        assert!(
            shown.starts_with("⚠ Coffre « perso » non synchronisé : "),
            "{shown}"
        );
    }

    #[test]
    fn status_shows_the_server_and_each_vault() {
        let env = TestEnv::new();
        let disconnected = "Aucun serveur (pswd connect <adresse> pour s'y connecter).";
        assert_eq!(status(&env, "a"), format!("{disconnected}\nAucun coffre."));
        let (server, _) = env.connect_admin("a");
        output(&env, "a", &[PASSWORD, PASSWORD], init());
        assert_eq!(
            status(&env, "a"),
            format!(
                "Serveur : {}\n  perso : local, publié au prochain pswd sync",
                server.address
            )
        );
        output(&env, "a", &[], Command::Sync);
        assert_eq!(
            status(&env, "a"),
            format!("Serveur : {}\n  perso : à jour", server.address)
        );
        env.go_offline("a");
        let shown = status(&env, "a");
        assert!(
            shown.ends_with("\n⚠ serveur injoignable\n  perso"),
            "{shown}"
        );
        output(&env, "a", &[""], Command::Disconnect);
        assert_eq!(status(&env, "a"), format!("{disconnected}\n  perso"));
    }

    #[test]
    fn a_device_key_connects_another_device() {
        let env = TestEnv::new();
        let (server, _) = env.connect_admin("a");
        let shown = output(&env, "a", &[], Command::AddDevice);
        assert!(
            shown.ends_with(&format!(
                "Sur le nouvel appareil : pswd connect {}",
                server.address
            )),
            "{shown}"
        );
        let key = shown.lines().nth(2).unwrap().trim();
        let connected = output(
            &env,
            "b",
            &[key, "Portable", ""],
            Command::Connect {
                address: server.address,
            },
        );
        assert_eq!(connected, "Appareil connecté au serveur.");
    }

    #[test]
    fn disconnection_keeps_the_vaults_unless_asked() {
        let env = TestEnv::with_vault();
        env.connect_admin("a");
        assert_eq!(
            output(&env, "a", &["oui"], Command::Disconnect),
            "Appareil déconnecté du serveur."
        );
        env.local_vault("a", PASSWORD);

        let env = TestEnv::with_vault();
        env.connect_admin("a");
        assert_eq!(
            output(&env, "a", &["N"], Command::Disconnect),
            "Appareil déconnecté du serveur.\nCoffre « perso » retiré de cet appareil ; il reste \
             sur le serveur."
        );
        assert!(env.config("a").vaults.is_empty());
    }

    #[test]
    fn disconnection_keeps_unpublished_changes() {
        let env = TestEnv::with_vault();
        env.connect_admin("a");
        env.edit_vault("a", |vault| {
            vault.add(
                EntryContent::new(EntryKind::Note, "Wi-Fi".to_owned()),
                Timestamp::from_millis(1),
            );
        });
        assert_eq!(
            output(&env, "a", &["non"], Command::Disconnect),
            "Appareil déconnecté du serveur.\nCoffre « perso » gardé : des modifications ne sont \
             pas encore sur le serveur."
        );
    }

    #[test]
    fn offline_disconnection_is_reported() {
        let env = TestEnv::with_vault();
        env.connect_admin("a");
        env.go_offline("a");
        assert_eq!(
            output(&env, "a", &["n"], Command::Disconnect),
            "Appareil déconnecté du serveur.\n⚠ Jeton de l'appareil non révoqué (serveur \
             injoignable) : le révoquer depuis le panneau d'administration.\nCoffre « perso » \
             gardé : serveur injoignable."
        );
    }

    #[test]
    fn open_vaults_are_synchronized_with_the_server() {
        let env = TestEnv::with_vault();
        env.connect_admin("a");
        env.connect_same_account("a", "b");
        let mut answers = vec![PASSWORD];
        answers.extend(ADD_NOTE);
        answers.extend(["sync", "quit"]);
        let shown = output(&env, "a", &answers, open());
        assert!(shown.ends_with("Synchro : coffre à jour."), "{shown}");
        assert!(!shown.contains('⚠'), "{shown}");

        output(&env, "b", &[PASSWORD], open());
        assert_eq!(titles(&env, "b"), ["Wi-Fi"]);
    }

    #[test]
    fn offline_changes_are_retried_when_closing() {
        let env = TestEnv::with_vault();
        env.connect_admin("a");
        env.go_offline("a");
        let mut answers = vec![PASSWORD];
        answers.extend(ADD_NOTE);
        answers.push("quit");
        let shown = output(&env, "a", &answers, open());
        let warning = "⚠ Synchro : hors ligne (serveur injoignable) ; les modifications seront \
                       publiées plus tard.";
        // À l'ouverture, après l'ajout, puis à la fermeture.
        assert_eq!(shown.matches(warning).count(), 3, "{shown}");
    }

    #[test]
    fn shell_sync_explains_why_nothing_happens_then_publishes() {
        let env = TestEnv::with_vault();
        let shown = output(&env, "a", &[PASSWORD, "sync"], open());
        assert!(
            shown
                .ends_with("Synchro : cet appareil n'est connecté à aucun serveur (pswd connect)."),
            "{shown}"
        );
        env.connect_admin("a");
        output(&env, "a", &[""], Command::Disconnect);
        env.connect_admin("a");
        let shown = output(&env, "a", &[PASSWORD, "sync"], open());
        assert!(shown.ends_with("Synchro : coffre à jour."), "{shown}");
    }

    #[test]
    fn sync_errors_are_shown() {
        let env = TestEnv::new();
        env.connect_admin("a");
        output(&env, "a", &[PASSWORD, PASSWORD], init());
        // Le lien du coffre ne peut pas être noté : la configuration n'est pas inscriptible.
        let blocker = env
            .dir
            .path()
            .join("a")
            .join("config")
            .join("config.toml.tmp");
        fs::create_dir(&blocker).unwrap();
        let shown = output(&env, "a", &[PASSWORD, "sync"], open());
        assert!(shown.contains("\n⚠ Synchro : configuration : "), "{shown}");
        assert!(shown.contains("\nerreur : configuration : "), "{shown}");
    }

    #[test]
    fn a_new_password_reaches_the_other_devices() {
        let env = TestEnv::with_vault();
        env.connect_admin("a");
        env.connect_same_account("a", "b");
        output(
            &env,
            "a",
            &[PASSWORD, "Nouveau-2026!", "Nouveau-2026!"],
            Command::Passwd {
                name: "perso".to_owned(),
            },
        );
        output(&env, "b", &[PASSWORD], open());
        env.local_vault("b", "Nouveau-2026!");
    }

    #[test]
    fn every_outcome_is_described() {
        assert_eq!(
            describe(&SyncOutcome::Done(SyncStatus::Conflict)),
            "le coffre a changé sur le serveur à chaque essai ; réessayer avec sync"
        );
        assert_eq!(
            describe(&SyncOutcome::Done(SyncStatus::Rejected(
                FileError::Corrupted
            ))),
            format!(
                "copie du serveur refusée ({}), elle n'est pas remplacée",
                FileError::Corrupted
            )
        );
        assert_eq!(
            describe(&SyncOutcome::Done(SyncStatus::Offline(PortError::new("x")))),
            "hors ligne (x) ; les modifications seront publiées plus tard"
        );
        for removed in [SyncOutcome::Removed, SyncOutcome::Done(SyncStatus::Removed)] {
            assert_eq!(
                describe(&removed),
                "coffre supprimé du serveur : gardé sur cet appareil ; taper sync pour le \
                 republier"
            );
        }
    }
}
