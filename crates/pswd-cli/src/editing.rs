//! Commandes du shell qui modifient le coffre : `add`, `edit`, `rm`, plus `history` et `tags`.

use std::collections::BTreeMap;

use pswd_core::ports::SecureClipboard;
use pswd_core::{EntryContent, EntryKind, PasswordOptions, Secret, Tags, Vault, generate_password};

use crate::app::{App, CliError};
use crate::shell::{VaultSession, find_entry, system_rng};
use crate::terminal::Terminal;
use crate::utc::format_timestamp;

/// Réponse qui efface un champ lors d'un `edit`.
const CLEAR: &str = "-";

impl<T: Terminal, C: SecureClipboard> App<T, C> {
    pub(crate) fn add_entry(
        &mut self,
        session: &mut VaultSession,
        generate: bool,
    ) -> Result<(), CliError> {
        let kind = parse_kind(&self.terminal.prompt("Type (login ou note) [login] : ")?)?;
        let title = self.terminal.prompt("Titre : ")?;
        if title.is_empty() {
            return Err(CliError::EmptyTitle);
        }
        let mut content = EntryContent::new(kind, title);
        if kind == EntryKind::Login {
            content.username = non_empty(self.terminal.prompt("Identifiant : ")?);
            content.password = if generate {
                Some(generated_password()?)
            } else {
                let password = self
                    .terminal
                    .prompt_secret("Mot de passe (vide si aucun) : ")?;
                (!password.expose().is_empty()).then_some(password)
            };
            content.urls = words(&self.terminal.prompt("URLs (séparées par des espaces) : ")?);
        }
        content.notes = non_empty(self.terminal.prompt("Notes : ")?);
        content.tags = Tags::new(words(
            &self.terminal.prompt("Tags (séparés par des espaces) : ")?,
        ));
        let title = content.title.clone();
        let now = (self.clock)();
        session.edit(|vault| vault.add(content, now))?;
        self.terminal.print(&format!("Entrée « {title} » ajoutée."));
        if generate {
            self.print_generated(&title);
        }
        Ok(())
    }

    /// Chaque question montre la valeur actuelle : vide pour la garder, `-` pour l'effacer.
    pub(crate) fn edit_entry(
        &mut self,
        session: &mut VaultSession,
        reference: &str,
        generate: bool,
    ) -> Result<(), CliError> {
        let entry = find_entry(session.vault(), reference)?;
        let id = entry.id();
        let mut content = entry.content().clone();
        let title = self
            .terminal
            .prompt(&format!("Titre [{}] : ", content.title))?;
        if !title.is_empty() {
            content.title = title;
        }
        if content.kind == EntryKind::Login {
            content.username = self.edit_optional("Identifiant", content.username)?;
            content.password = if generate {
                Some(generated_password()?)
            } else {
                let answer = self
                    .terminal
                    .prompt_secret("Mot de passe (vide : inchangé, - : effacer) : ")?;
                match answer.expose() {
                    "" => content.password,
                    CLEAR => None,
                    _ => Some(answer),
                }
            };
            content.urls = self.edit_list("URLs", content.urls)?;
        }
        content.notes = self.edit_optional("Notes", content.notes)?;
        content.tags = Tags::new(self.edit_list("Tags", content.tags.as_slice().to_vec())?);
        let title = content.title.clone();
        let now = (self.clock)();
        let updated = session.edit(|vault| vault.update(id, content, now))?;
        updated.map_err(CliError::Vault)?;
        self.terminal
            .print(&format!("Entrée « {title} » modifiée."));
        if generate {
            self.print_generated(&title);
        }
        Ok(())
    }

    pub(crate) fn remove_entry(
        &mut self,
        session: &mut VaultSession,
        reference: &str,
    ) -> Result<(), CliError> {
        let entry = find_entry(session.vault(), reference)?;
        let (id, title) = (entry.id(), entry.content().title.clone());
        if !self.confirm(&format!("Supprimer « {title} » ? (o/N) : "))? {
            self.terminal.print("Suppression annulée.");
            return Ok(());
        }
        let now = (self.clock)();
        let deleted = session.edit(|vault| vault.delete(id, now))?;
        deleted.map_err(CliError::Vault)?;
        self.terminal
            .print(&format!("Entrée « {title} » supprimée."));
        Ok(())
    }

    pub(crate) fn show_history(&mut self, vault: &Vault, reference: &str) -> Result<(), CliError> {
        let entry = find_entry(vault, reference)?;
        let current = entry.content();
        if entry.history().is_empty() {
            self.terminal.print("Aucune version précédente.");
            return Ok(());
        }
        self.terminal.print(&format!(
            "Versions précédentes de « {} », des plus récentes aux plus anciennes :",
            current.title
        ));
        for snapshot in entry.history() {
            let content = &snapshot.content;
            let mut line = format!(
                "  {}  {}",
                format_timestamp(snapshot.updated_at),
                content.title
            );
            if let Some(username) = &content.username {
                line.push_str(" — ");
                line.push_str(username);
            }
            if content.password != current.password {
                line.push_str("  (mot de passe différent)");
            }
            self.terminal.print(&line);
        }
        Ok(())
    }

    pub(crate) fn list_tags(&mut self, vault: &Vault) {
        let mut counts: BTreeMap<&str, usize> = BTreeMap::new();
        for entry in vault.entries().filter(|entry| !entry.is_deleted()) {
            for tag in entry.content().tags.as_slice() {
                *counts.entry(tag).or_default() += 1;
            }
        }
        if counts.is_empty() {
            self.terminal.print("Aucun tag.");
        }
        for (tag, count) in counts {
            self.terminal.print(&format!("{tag} ({count})"));
        }
    }

    fn edit_optional(
        &mut self,
        label: &str,
        current: Option<String>,
    ) -> Result<Option<String>, CliError> {
        let shown = current.as_deref().unwrap_or("");
        let answer = self.terminal.prompt(&format!("{label} [{shown}] : "))?;
        Ok(match answer.as_str() {
            "" => current,
            CLEAR => None,
            _ => Some(answer),
        })
    }

    fn edit_list(&mut self, label: &str, current: Vec<String>) -> Result<Vec<String>, CliError> {
        let answer = self
            .terminal
            .prompt(&format!("{label} [{}] : ", current.join(" ")))?;
        Ok(match answer.as_str() {
            "" => current,
            CLEAR => Vec::new(),
            _ => words(&answer),
        })
    }

    fn print_generated(&mut self, title: &str) {
        self.terminal.print(&format!(
            "Mot de passe généré : « copy {title} » pour le copier."
        ));
    }
}

fn parse_kind(answer: &str) -> Result<EntryKind, CliError> {
    match answer.trim().to_lowercase().as_str() {
        "" | "login" => Ok(EntryKind::Login),
        "note" => Ok(EntryKind::Note),
        other => Err(CliError::UnknownKind(other.to_owned())),
    }
}

fn non_empty(answer: String) -> Option<String> {
    (!answer.is_empty()).then_some(answer)
}

fn words(answer: &str) -> Vec<String> {
    answer.split_whitespace().map(str::to_owned).collect()
}

fn generated_password() -> Result<Secret, CliError> {
    generate_password(PasswordOptions::default(), &mut system_rng()?).map_err(CliError::Generator)
}

#[cfg(test)]
mod tests {
    use pswd_core::{Entry, Timestamp};

    use crate::cli::Command;
    use crate::testing::TestEnv;

    use super::*;

    /// Ouvre `perso` sur `a`, exécute `commands` (réponses comprises), renvoie ce qui a été
    /// affiché après l'ouverture.
    fn session_output(env: &TestEnv, commands: &[&str]) -> String {
        let mut answers = vec!["maître"];
        answers.extend_from_slice(commands);
        let mut app = env.app("a", &answers);
        app.run(Command::Open {
            name: "perso".to_owned(),
        })
        .unwrap();
        let output = app.terminal.output();
        let after_open = output.split_once("Tapez help pour l'aide.").unwrap().1;
        after_open.trim_start_matches('\n').to_owned()
    }

    fn entries(env: &TestEnv) -> Vec<Entry> {
        let mut entries = Vec::new();
        env.edit_vault("a", |vault| entries = vault.entries().cloned().collect());
        entries
    }

    fn live(env: &TestEnv) -> Vec<EntryContent> {
        entries(env)
            .into_iter()
            .filter(|entry| !entry.is_deleted())
            .map(|entry| entry.content().clone())
            .collect()
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

    fn env_with_github() -> TestEnv {
        let env = TestEnv::with_vault();
        env.edit_vault("a", |vault| {
            vault.add(github(), Timestamp::from_millis(1));
        });
        env
    }

    #[test]
    fn add_asks_every_field_of_a_login() {
        let env = TestEnv::with_vault();
        let output = session_output(
            &env,
            &[
                "add",
                "",
                "GitHub",
                "arthur",
                "hunter2",
                "https://github.com",
                "compte pro",
                "Dev perso",
            ],
        );
        assert_eq!(output, "Entrée « GitHub » ajoutée.");
        assert_eq!(live(&env), [github()]);
    }

    #[test]
    fn add_a_note_asks_only_its_fields() {
        let env = TestEnv::with_vault();
        session_output(&env, &["add", "NOTE", "Code Wi-Fi", "ABCD-1234", ""]);
        let expected = EntryContent {
            notes: Some("ABCD-1234".to_owned()),
            ..EntryContent::new(EntryKind::Note, "Code Wi-Fi".to_owned())
        };
        assert_eq!(live(&env), [expected]);
    }

    #[test]
    fn add_with_generate_creates_a_strong_password() {
        let env = TestEnv::with_vault();
        let output = session_output(&env, &["add --generate", "login", "Banque", "", "", "", ""]);
        assert!(
            output.ends_with("Mot de passe généré : « copy Banque » pour le copier."),
            "{output}"
        );
        let password = live(&env)[0].password.clone().unwrap();
        assert_eq!(password.expose().chars().count(), 24);
    }

    #[test]
    fn add_rejects_an_unknown_kind_or_an_empty_title() {
        let env = TestEnv::with_vault();
        let output = session_output(&env, &["add", "carte", "add", "", ""]);
        let lines: Vec<&str> = output.lines().collect();
        assert_eq!(lines[0], "erreur : type inconnu « carte » : login ou note");
        assert_eq!(lines[1], "erreur : le titre ne peut pas être vide");
        assert_eq!(live(&env), Vec::<EntryContent>::new());
    }

    #[test]
    fn edit_keeps_empty_answers_and_clears_dashes() {
        let env = env_with_github();
        let output = session_output(&env, &["edit github", "", "-", "", "-", "", "pro"]);
        assert_eq!(output, "Entrée « GitHub » modifiée.");
        let expected = EntryContent {
            username: None,
            urls: Vec::new(),
            tags: Tags::new(["pro"]),
            ..github()
        };
        assert_eq!(live(&env), [expected]);
    }

    #[test]
    fn edit_replaces_values_and_keeps_the_previous_version() {
        let env = env_with_github();
        session_output(
            &env,
            &[
                "edit github",
                "GitHub perso",
                "moi",
                "nouveau",
                "https://a.example https://b.example",
                "-",
                "-",
            ],
        );
        let entry = entries(&env).remove(0);
        let content = entry.content();
        assert_eq!(content.title, "GitHub perso");
        assert_eq!(content.username.as_deref(), Some("moi"));
        assert_eq!(content.password, Some(Secret::new("nouveau".to_owned())));
        assert_eq!(content.urls, ["https://a.example", "https://b.example"]);
        assert_eq!(content.notes, None);
        assert_eq!(content.tags, Tags::default());
        assert_eq!(entry.history()[0].content, github());
    }

    #[test]
    fn edit_can_remove_or_generate_the_password() {
        let env = env_with_github();
        session_output(&env, &["edit github", "", "", "-", "", "", ""]);
        assert_eq!(live(&env)[0].password, None);
        let output = session_output(&env, &["edit github --generate", "", "", "", "", ""]);
        assert!(
            output.ends_with("« copy GitHub » pour le copier."),
            "{output}"
        );
        assert_eq!(live(&env)[0].password.as_ref().unwrap().expose().len(), 24);
    }

    #[test]
    fn edit_of_a_note_asks_only_its_fields() {
        let env = TestEnv::with_vault();
        env.edit_vault("a", |vault| {
            vault.add(
                EntryContent::new(EntryKind::Note, "Wi-Fi".to_owned()),
                Timestamp::from_millis(1),
            );
        });
        session_output(&env, &["edit wi-fi", "", "ABCD", "maison"]);
        let expected = EntryContent {
            notes: Some("ABCD".to_owned()),
            tags: Tags::new(["maison"]),
            ..EntryContent::new(EntryKind::Note, "Wi-Fi".to_owned())
        };
        assert_eq!(live(&env), [expected]);
    }

    #[test]
    fn rm_asks_for_confirmation() {
        let env = env_with_github();
        let output = session_output(&env, &["rm github", "", "rm github", "Oui"]);
        let lines: Vec<&str> = output.lines().collect();
        assert_eq!(
            lines,
            ["Suppression annulée.", "Entrée « GitHub » supprimée."]
        );
        assert_eq!(live(&env), Vec::<EntryContent>::new());
        assert!(entries(&env)[0].is_deleted());
    }

    #[test]
    fn history_lists_previous_versions() {
        let env = env_with_github();
        let output = session_output(&env, &["history github"]);
        assert_eq!(output, "Aucune version précédente.");
        session_output(
            &env,
            &["edit github", "GitHub pro", "", "nouveau", "", "", ""],
        );
        session_output(&env, &["edit \"github pro\"", "", "", "", "", "-", ""]);
        let output = session_output(&env, &["history \"github pro\""]);
        let lines: Vec<&str> = output.lines().collect();
        assert_eq!(
            lines,
            [
                "Versions précédentes de « GitHub pro », des plus récentes aux plus anciennes :",
                "  1970-01-01 00:00 UTC  GitHub pro — arthur",
                "  1970-01-01 00:00 UTC  GitHub — arthur  (mot de passe différent)",
            ]
        );
    }

    #[test]
    fn history_of_a_note_has_no_username() {
        let env = TestEnv::with_vault();
        env.edit_vault("a", |vault| {
            let id = vault.add(
                EntryContent::new(EntryKind::Note, "Wi-Fi".to_owned()),
                Timestamp::from_millis(1),
            );
            vault
                .update(
                    id,
                    EntryContent::new(EntryKind::Note, "Box".to_owned()),
                    Timestamp::from_millis(2),
                )
                .unwrap();
        });
        let output = session_output(&env, &["history box"]);
        assert!(
            output.ends_with("\n  1970-01-01 00:00 UTC  Wi-Fi"),
            "{output}"
        );
    }

    #[test]
    fn tags_are_counted_on_live_entries() {
        let env = env_with_github();
        let output = session_output(&env, &["tags"]);
        assert_eq!(output, "dev (1)\nperso (1)");
        session_output(&env, &["rm github", "o"]);
        assert_eq!(session_output(&env, &["tags"]), "Aucun tag.");
    }
}
