//! `pswd open` : le shell interactif d'un coffre déverrouillé.

use std::time::Duration;

use clap::Parser;
use pswd_core::ports::{SecretStore, SecureClipboard};
use pswd_core::{Entry, EntryKind, PasswordOptions, Session, Vault, generate_password};
use pswd_infra::{FileStore, GitRemote};
use rand::SeedableRng;
use rand::rngs::{StdRng, SysRng};

use crate::app::{App, CliError, describe};
use crate::commands::{Unlock, Unlocked};
use crate::terminal::Terminal;

pub(crate) type VaultSession = Session<FileStore, GitRemote>;

/// Nombre de caractères d'identifiant affichés : assez pour désigner une entrée.
const SHORT_ID_LEN: usize = 8;
const MASK: &str = "••••••••";

#[derive(Debug, Parser)]
#[command(no_binary_name = true, name = "", disable_version_flag = true)]
struct ShellLine {
    #[command(subcommand)]
    command: ShellCommand,
}

/// Après une commande : continuer ou fermer le coffre.
enum Flow {
    Continue,
    Quit,
}

#[derive(Debug, clap::Subcommand)]
enum ShellCommand {
    /// Liste les entrées.
    List {
        /// Seulement les entrées portant ce tag.
        #[arg(long)]
        tag: Option<String>,
    },
    /// Cherche dans les titres, identifiants, URLs et tags.
    Search {
        text: String,
        /// Seulement les entrées portant ce tag.
        #[arg(long)]
        tag: Option<String>,
    },
    /// Affiche une entrée (mot de passe masqué).
    Show {
        /// Titre ou début de l'identifiant.
        entry: String,
        /// Affiche le mot de passe en clair.
        #[arg(long)]
        show: bool,
    },
    /// Copie le mot de passe d'une entrée dans le presse-papiers.
    Copy {
        /// Titre ou début de l'identifiant.
        entry: String,
    },
    /// Génère un mot de passe.
    Gen {
        #[arg(long, default_value_t = PasswordOptions::default().length)]
        length: usize,
        /// Sans symboles.
        #[arg(long)]
        no_symbols: bool,
    },
    /// Ajoute une entrée.
    Add {
        /// Génère le mot de passe.
        #[arg(long)]
        generate: bool,
    },
    /// Modifie une entrée (réponse vide : inchangé, « - » : effacer).
    Edit {
        /// Titre ou début de l'identifiant.
        entry: String,
        /// Génère un nouveau mot de passe.
        #[arg(long)]
        generate: bool,
    },
    /// Supprime une entrée.
    Rm {
        /// Titre ou début de l'identifiant.
        entry: String,
    },
    /// Versions précédentes d'une entrée.
    History {
        /// Titre ou début de l'identifiant.
        entry: String,
    },
    /// Liste les tags.
    Tags,
    /// Synchronise avec le dépôt distant.
    Sync,
    /// Ferme le coffre.
    #[command(alias = "exit")]
    Quit,
}

impl<T: Terminal, K: SecretStore, C: SecureClipboard> App<T, K, C> {
    pub(crate) fn open(&mut self, name: Option<&str>) -> Result<(), CliError> {
        let Unlocked {
            name,
            mut session,
            config,
        } = self.unlock(name, Unlock::Password)?;
        self.terminal.print(&format!(
            "Coffre « {name} » ouvert. Tapez help pour l'aide."
        ));
        self.terminal.print(&describe(session.status()));
        self.shell(&name, &mut session, config.clipboard_clear())?;
        if session.has_pending_changes() {
            self.terminal.print(&describe(session.sync()?));
        }
        Ok(())
    }

    fn shell(
        &mut self,
        name: &str,
        session: &mut VaultSession,
        clipboard_ttl: Duration,
    ) -> Result<(), CliError> {
        loop {
            let line = match self.terminal.prompt(&format!("{name}> ")) {
                Ok(line) => line,
                Err(CliError::InputClosed) => return Ok(()),
                Err(error) => return Err(error),
            };
            let Some(words) = shlex::split(&line) else {
                self.terminal.print("erreur : guillemets non fermés");
                continue;
            };
            if words.is_empty() {
                continue;
            }
            let command = match ShellLine::try_parse_from(words) {
                Ok(line) => line.command,
                Err(error) => {
                    self.terminal.print(error.render().to_string().trim_end());
                    continue;
                }
            };
            match self.execute(command, session, clipboard_ttl) {
                Ok(Flow::Continue) => {}
                Ok(Flow::Quit) => return Ok(()),
                Err(error) => self.terminal.print(&format!("erreur : {error}")),
            }
        }
    }

    fn execute(
        &mut self,
        command: ShellCommand,
        session: &mut VaultSession,
        clipboard_ttl: Duration,
    ) -> Result<Flow, CliError> {
        match command {
            ShellCommand::List { tag } => self.list(session.vault(), "", tag.as_deref()),
            ShellCommand::Search { text, tag } => {
                self.list(session.vault(), &text, tag.as_deref());
            }
            ShellCommand::Show { entry, show } => {
                let entry = find_entry(session.vault(), &entry)?;
                self.terminal.print(&describe_entry(entry, show));
            }
            ShellCommand::Copy { entry } => {
                let entry = find_entry(session.vault(), &entry)?;
                let content = entry.content();
                let password = content
                    .password
                    .as_ref()
                    .ok_or_else(|| CliError::NoPassword(content.title.clone()))?;
                self.clipboard.copy_secret(password, clipboard_ttl)?;
                self.terminal.print(&format!(
                    "Mot de passe de « {} » copié ; effacé du presse-papiers dans {} s.",
                    content.title,
                    clipboard_ttl.as_secs()
                ));
            }
            ShellCommand::Gen { length, no_symbols } => {
                let options = PasswordOptions {
                    length,
                    symbols: !no_symbols,
                };
                let password = generate_password(options, &mut system_rng()?)?;
                self.terminal.print(password.expose());
            }
            ShellCommand::Add { generate } => self.add_entry(session, generate)?,
            ShellCommand::Edit { entry, generate } => self.edit_entry(session, &entry, generate)?,
            ShellCommand::Rm { entry } => self.remove_entry(session, &entry)?,
            ShellCommand::History { entry } => self.show_history(session.vault(), &entry)?,
            ShellCommand::Tags => self.list_tags(session.vault()),
            ShellCommand::Sync => {
                let status = describe(session.sync()?);
                self.terminal.print(&status);
            }
            ShellCommand::Quit => return Ok(Flow::Quit),
        }
        Ok(Flow::Continue)
    }

    /// Entrées non supprimées qui contiennent `text` (sans tenir compte de la casse), triées
    /// par titre.
    fn list(&mut self, vault: &Vault, text: &str, tag: Option<&str>) {
        let text = text.to_lowercase();
        let tag = tag.map(|tag| tag.trim().to_lowercase());
        let mut entries: Vec<&Entry> = vault
            .entries()
            .filter(|entry| !entry.is_deleted())
            .filter(|entry| matches_text(entry, &text))
            .filter(|entry| {
                tag.as_ref()
                    .is_none_or(|tag| entry.content().tags.as_slice().contains(tag))
            })
            .collect();
        if entries.is_empty() {
            self.terminal.print("Aucune entrée.");
            return;
        }
        entries.sort_by_key(|entry| entry.content().title.to_lowercase());
        for entry in entries {
            self.terminal.print(&summary(entry));
        }
    }
}

fn matches_text(entry: &Entry, text: &str) -> bool {
    let content = entry.content();
    std::iter::once(&content.title)
        .chain(&content.username)
        .chain(&content.urls)
        .chain(content.tags.as_slice())
        .any(|field| field.to_lowercase().contains(text))
}

/// Une entrée désignée par son titre (sans tenir compte de la casse), ou à défaut par le début
/// de son identifiant.
pub(crate) fn find_entry<'v>(vault: &'v Vault, reference: &str) -> Result<&'v Entry, CliError> {
    let reference_lower = reference.to_lowercase();
    let live: Vec<&Entry> = vault
        .entries()
        .filter(|entry| !entry.is_deleted())
        .collect();
    let by_title: Vec<&Entry> = live
        .iter()
        .copied()
        .filter(|entry| entry.content().title.to_lowercase() == reference_lower)
        .collect();
    let candidates = if by_title.is_empty() {
        live.into_iter()
            .filter(|entry| entry.id().to_string().starts_with(&reference_lower))
            .collect()
    } else {
        by_title
    };
    match candidates.as_slice() {
        [entry] => Ok(entry),
        [] => Err(CliError::EntryNotFound(reference.to_owned())),
        several => Err(CliError::AmbiguousEntry(
            several
                .iter()
                .map(|entry| format!("{} ({})", entry.content().title, short_id(entry)))
                .collect(),
        )),
    }
}

fn short_id(entry: &Entry) -> String {
    entry.id().to_string()[..SHORT_ID_LEN].to_owned()
}

fn kind_label(kind: EntryKind) -> &'static str {
    match kind {
        EntryKind::Login => "identifiant",
        EntryKind::Note => "note",
    }
}

/// Une ligne : identifiant court, titre, identifiant de connexion, tags.
fn summary(entry: &Entry) -> String {
    let content = entry.content();
    let mut line = format!("{}  {}", short_id(entry), content.title);
    if let Some(username) = &content.username {
        line.push_str(" — ");
        line.push_str(username);
    }
    for tag in content.tags.as_slice() {
        line.push_str(" #");
        line.push_str(tag);
    }
    line
}

fn describe_entry(entry: &Entry, reveal: bool) -> String {
    let content = entry.content();
    let mut lines = vec![format!("{} ({})", content.title, kind_label(content.kind))];
    if let Some(username) = &content.username {
        lines.push(format!("  Identifiant : {username}"));
    }
    if let Some(password) = &content.password {
        let shown = if reveal { password.expose() } else { MASK };
        lines.push(format!("  Mot de passe : {shown}"));
    }
    for url in &content.urls {
        lines.push(format!("  URL : {url}"));
    }
    if let Some(notes) = &content.notes {
        lines.push(format!("  Notes : {notes}"));
    }
    if !content.tags.as_slice().is_empty() {
        lines.push(format!("  Tags : {}", content.tags.as_slice().join(", ")));
    }
    lines.push(format!(
        "  Versions précédentes : {}",
        entry.history().len()
    ));
    lines.push(format!("  Référence : {}", entry.id()));
    lines.join("\n")
}

/// Générateur cryptographique initialisé par l'aléa du système.
pub(crate) fn system_rng() -> Result<StdRng, CliError> {
    StdRng::try_from_rng(&mut SysRng).map_err(random_unavailable)
}

/// L'aléa du système ne manque que sur une machine défaillante.
// Exclue de la couverture : impossible à provoquer en test.
#[cfg_attr(coverage_nightly, coverage(off))]
fn random_unavailable<E>(_: E) -> CliError {
    CliError::RandomUnavailable
}

#[cfg(test)]
mod tests {
    use pswd_core::{EntryContent, Secret, SessionError, Tags, Timestamp};
    use pswd_infra::ConfigError;

    use crate::cli::Command;
    use crate::testing::{BROKEN_INPUT, TestEnv};

    use super::*;

    fn open() -> Command {
        Command::Open { name: None }
    }

    fn github() -> EntryContent {
        EntryContent {
            username: Some("arthur".to_owned()),
            password: Some(Secret::new("hunter2".to_owned())),
            urls: vec!["https://github.com".to_owned()],
            notes: Some("compte pro".to_owned()),
            tags: Tags::new(["dev", "perso"]),
            ..EntryContent::new(EntryKind::Login, "GitHub".to_owned())
        }
    }

    fn note(title: &str) -> EntryContent {
        EntryContent::new(EntryKind::Note, title.to_owned())
    }

    /// Coffre `perso` sur l'appareil `a`, avec trois entrées vivantes et une supprimée.
    fn env_with_entries() -> TestEnv {
        let env = TestEnv::with_vault();
        env.edit_vault("a", |vault| {
            vault.add(github(), Timestamp::from_millis(1));
            vault.add(note("Code Wi-Fi"), Timestamp::from_millis(1));
            vault.add(
                EntryContent {
                    tags: Tags::new(["perso"]),
                    ..note("banque")
                },
                Timestamp::from_millis(1),
            );
            let deleted = vault.add(note("ancienne"), Timestamp::from_millis(1));
            vault.delete(deleted, Timestamp::from_millis(2)).unwrap();
        });
        env
    }

    /// Ouvre `perso` sur `a` avec le bon mot de passe, exécute `commands`, puis renvoie ce qui
    /// a été affiché après l'ouverture.
    fn session_output(env: &TestEnv, commands: &[&str]) -> String {
        let mut answers = vec!["maître"];
        answers.extend_from_slice(commands);
        let mut app = env.app("a", &answers);
        app.run(open()).unwrap();
        let output = app.terminal.output();
        let after_open = output.split_once("Tapez help pour l'aide.\n").unwrap().1;
        after_open
            .split_once('\n')
            .map_or("", |(_, commands)| commands)
            .to_owned()
    }

    #[test]
    fn open_unlocks_and_closes_at_end_of_input() {
        let env = env_with_entries();
        let mut app = env.app("a", &["maître"]);
        app.run(open()).unwrap();
        assert_eq!(
            app.terminal.output(),
            "Coffre « perso » ouvert. Tapez help pour l'aide.\nSynchronisé avec le dépôt distant."
        );
    }

    #[test]
    fn input_failure_ends_the_shell_with_an_error() {
        let env = env_with_entries();
        let mut app = env.app("a", &["maître", "list", BROKEN_INPUT, "list"]);
        assert_eq!(
            app.run(open()),
            Err(CliError::Input("clavier débranché".to_owned()))
        );
    }

    #[test]
    fn open_reports_wrong_password_or_unknown_vault() {
        let env = TestEnv::with_vault();
        let mut app = env.app("a", &["autre"]);
        assert_eq!(
            app.run(open()),
            Err(CliError::Session(SessionError::File(
                pswd_core::FileError::WrongPassword
            )))
        );
        let mut app = env.app("a", &[]);
        assert_eq!(
            app.run(Command::Open {
                name: Some("travail".to_owned())
            }),
            Err(CliError::Config(ConfigError::UnknownVault(
                "travail".to_owned()
            )))
        );
        let mut app = env.app("b", &[]);
        assert_eq!(
            app.run(open()),
            Err(CliError::Config(ConfigError::NoDefaultVault))
        );
    }

    #[test]
    fn list_shows_live_entries_sorted_by_title() {
        let env = env_with_entries();
        let output = session_output(&env, &["list", "quit"]);
        let titles: Vec<&str> = output
            .lines()
            .map(|line| line.split("  ").nth(1).unwrap())
            .collect();
        assert_eq!(
            titles,
            ["banque #perso", "Code Wi-Fi", "GitHub — arthur #dev #perso"]
        );
    }

    #[test]
    fn list_and_search_filter_by_tag_and_text() {
        let env = env_with_entries();
        let output = session_output(
            &env,
            &[
                "list --tag DEV",
                "search wi-fi",
                "search ARTHUR",
                "search github.com",
                "search perso --tag perso",
                "search introuvable",
            ],
        );
        let lines: Vec<&str> = output.lines().collect();
        assert!(
            lines[0].ends_with("GitHub — arthur #dev #perso"),
            "{output}"
        );
        assert!(lines[1].ends_with("Code Wi-Fi"), "{output}");
        assert!(
            lines[2].ends_with("GitHub — arthur #dev #perso"),
            "{output}"
        );
        assert!(
            lines[3].ends_with("GitHub — arthur #dev #perso"),
            "{output}"
        );
        assert!(lines[4].ends_with("banque #perso"), "{output}");
        assert!(
            lines[5].ends_with("GitHub — arthur #dev #perso"),
            "{output}"
        );
        assert_eq!(lines[6], "Aucune entrée.");
    }

    #[test]
    fn show_masks_the_password_unless_asked() {
        let env = env_with_entries();
        let output = session_output(&env, &["show github", "show GitHub --show"]);
        assert!(output.contains("GitHub (identifiant)"), "{output}");
        assert!(output.contains("  Identifiant : arthur"), "{output}");
        assert!(output.contains("  Mot de passe : ••••••••"), "{output}");
        assert!(output.contains("  Mot de passe : hunter2"), "{output}");
        assert!(output.contains("  URL : https://github.com"), "{output}");
        assert!(output.contains("  Notes : compte pro"), "{output}");
        assert!(output.contains("  Tags : dev, perso"), "{output}");
        assert!(output.contains("  Versions précédentes : 0"), "{output}");
        assert_eq!(output.matches("hunter2").count(), 1, "{output}");
    }

    #[test]
    fn show_of_a_note_has_only_its_fields() {
        let env = env_with_entries();
        let output = session_output(&env, &["show \"code wi-fi\""]);
        assert!(output.starts_with("Code Wi-Fi (note)\n  Versions précédentes : 0\n"));
    }

    #[test]
    fn entry_can_be_designated_by_the_start_of_its_reference() {
        let env = env_with_entries();
        let listing = session_output(&env, &["list"]);
        let github_id = listing
            .lines()
            .find(|line| line.contains("GitHub"))
            .unwrap()
            .split("  ")
            .next()
            .unwrap()
            .to_owned();
        let output = session_output(&env, &[&format!("show {github_id}")]);
        assert!(output.starts_with("GitHub (identifiant)"), "{output}");
    }

    #[test]
    fn unknown_or_ambiguous_entry_is_reported() {
        let env = env_with_entries();
        env.edit_vault("a", |vault| {
            vault.add(note("GitHub"), Timestamp::from_millis(3));
        });
        let output = session_output(&env, &["show gmail", "show github", "show ancienne"]);
        let lines: Vec<&str> = output.lines().collect();
        assert_eq!(lines[0], "erreur : aucune entrée ne correspond à « gmail »");
        assert!(
            lines[1].starts_with("erreur : plusieurs entrées correspondent : GitHub ("),
            "{output}"
        );
        assert_eq!(
            lines[2],
            "erreur : aucune entrée ne correspond à « ancienne »"
        );
    }

    #[test]
    fn copy_puts_the_password_in_the_clipboard() {
        let env = env_with_entries();
        let output = session_output(&env, &["copy github", "copy banque"]);
        assert_eq!(
            env.clipboard_copies(),
            [("hunter2".to_owned(), Duration::from_secs(20))]
        );
        let lines: Vec<&str> = output.lines().collect();
        assert_eq!(
            lines[0],
            "Mot de passe de « GitHub » copié ; effacé du presse-papiers dans 20 s."
        );
        assert_eq!(
            lines[1],
            "erreur : l'entrée « banque » n'a pas de mot de passe"
        );
    }

    #[test]
    fn clipboard_failure_is_reported() {
        let env = env_with_entries();
        env.break_clipboard();
        let output = session_output(&env, &["copy github"]);
        assert_eq!(output, "erreur : presse-papiers indisponible");
    }

    #[test]
    fn gen_prints_a_password_with_the_requested_options() {
        let env = env_with_entries();
        let output = session_output(
            &env,
            &["gen", "gen --length 40 --no-symbols", "gen --length 3"],
        );
        let lines: Vec<&str> = output.lines().collect();
        assert_eq!(lines[0].chars().count(), 24, "{output}");
        assert_eq!(lines[1].len(), 40, "{output}");
        assert!(lines[1].chars().all(char::is_alphanumeric), "{output}");
        assert_eq!(
            lines[2],
            "erreur : la longueur doit être comprise entre 8 et 128"
        );
    }

    #[test]
    fn sync_reports_the_status() {
        let env = env_with_entries();
        let output = session_output(&env, &["sync"]);
        assert_eq!(output, "Synchronisé avec le dépôt distant.");
    }

    #[test]
    fn invalid_lines_are_reported_and_the_shell_continues() {
        let env = env_with_entries();
        let output = session_output(
            &env,
            &["", "show \"code", "danse", "list --tag", "exit", "list"],
        );
        assert!(
            output.starts_with("erreur : guillemets non fermés\n"),
            "{output}"
        );
        assert!(
            output.contains("unrecognized subcommand 'danse'"),
            "{output}"
        );
        assert!(output.contains("--tag <TAG>"), "{output}");
        assert!(
            !output.contains("Code Wi-Fi"),
            "exit doit fermer le coffre : {output}"
        );
    }

    #[test]
    fn help_lists_the_commands() {
        let env = env_with_entries();
        let output = session_output(&env, &["help"]);
        for command in [
            "list", "search", "show", "copy", "gen", "add", "edit", "rm", "history", "tags",
            "sync", "quit",
        ] {
            assert!(output.contains(command), "{command} : {output}");
        }
    }
}
