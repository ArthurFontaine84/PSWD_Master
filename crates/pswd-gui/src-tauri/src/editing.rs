//! Ce que l'interface envoie pour créer ou modifier une entrée, et l'historique qu'elle reçoit.

use pswd_app::generate;
use pswd_core::{Entry, EntryContent, EntryKind, PasswordOptions, Secret, Tags};
use serde::{Deserialize, Serialize};

use crate::error::UiError;

/// Le formulaire d'une entrée. Une note n'a ni identifiant, ni mot de passe, ni URL : ces champs
/// sont ignorés. Un mot de passe exige un mot de passe, une note exige un texte.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EntryInput {
    kind: InputKind,
    title: String,
    username: String,
    password: PasswordChoice,
    urls: Vec<String>,
    notes: String,
    tags: Vec<String>,
}

#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "lowercase")]
enum InputKind {
    Login,
    Note,
}

/// Ce que devient le mot de passe. `Generate` le tire en Rust : il ne passe jamais par
/// l'interface, sauf si l'utilisateur demande ensuite à le voir.
#[derive(Debug, Deserialize)]
#[serde(tag = "mode", rename_all = "camelCase")]
pub enum PasswordChoice {
    Keep,
    Set { value: String },
    Generate { length: usize, symbols: bool },
}

impl EntryInput {
    /// Le contenu à enregistrer ; `current` est le mot de passe gardé par `Keep`.
    pub fn into_content(self, current: Option<Secret>) -> Result<EntryContent, UiError> {
        let title = self.title.trim();
        if title.is_empty() {
            return Err(UiError::code("emptyTitle"));
        }
        let kind = match self.kind {
            InputKind::Login => EntryKind::Login,
            InputKind::Note => EntryKind::Note,
        };
        let mut content = EntryContent::new(kind, title.to_owned());
        if kind == EntryKind::Login {
            content.username = non_empty(&self.username);
            content.password = match self.password {
                PasswordChoice::Keep => current,
                PasswordChoice::Set { value } => (!value.is_empty()).then(|| Secret::new(value)),
                PasswordChoice::Generate { length, symbols } => {
                    Some(generate(PasswordOptions { length, symbols })?)
                }
            };
            if content.password.is_none() {
                return Err(UiError::code("emptyPassword"));
            }
            content.urls = self.urls.iter().filter_map(|url| non_empty(url)).collect();
        }
        content.notes = non_empty(&self.notes);
        if kind == EntryKind::Note && content.notes.is_none() {
            return Err(UiError::code("emptyNote"));
        }
        content.tags = Tags::new(self.tags);
        Ok(content)
    }
}

/// Champ vide (espaces compris) : absent.
fn non_empty(value: &str) -> Option<String> {
    let value = value.trim();
    (!value.is_empty()).then(|| value.to_owned())
}

/// Une version précédente d'une entrée, sans mot de passe.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HistoryItem {
    updated_at: i64,
    title: String,
    username: Option<String>,
    password_changed: bool,
}

/// Versions précédentes, des plus récentes aux plus anciennes, comparées au mot de passe actuel.
pub fn history(entry: &Entry) -> Vec<HistoryItem> {
    let current = &entry.content().password;
    entry
        .history()
        .iter()
        .map(|snapshot| HistoryItem {
            updated_at: snapshot.updated_at.as_millis(),
            title: snapshot.content.title.clone(),
            username: snapshot.content.username.clone(),
            password_changed: &snapshot.content.password != current,
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use pswd_core::{Timestamp, Vault};
    use serde_json::{Value, json};

    use super::*;

    fn input(value: Value) -> EntryInput {
        serde_json::from_value(value).unwrap()
    }

    fn login(password: &Value) -> EntryInput {
        input(json!({
            "kind": "login",
            "title": "  GitHub ",
            "username": " arthur ",
            "password": password,
            "urls": ["https://github.com", "  "],
            "notes": "",
            "tags": ["Dev", " perso "],
        }))
    }

    fn current() -> Secret {
        Secret::new("ancien".to_owned())
    }

    #[test]
    fn a_login_is_cleaned_before_saving() {
        let content = login(&json!({ "mode": "set", "value": "hunter2" }))
            .into_content(None)
            .unwrap();
        assert_eq!(content.kind, EntryKind::Login);
        assert_eq!(content.title, "GitHub");
        assert_eq!(content.username.as_deref(), Some("arthur"));
        assert_eq!(content.password, Some(Secret::new("hunter2".to_owned())));
        assert_eq!(content.urls, ["https://github.com"]);
        assert_eq!(content.notes, None);
        assert_eq!(content.tags.as_slice(), ["dev", "perso"]);
    }

    #[test]
    fn each_password_choice_is_applied() {
        let keep = login(&json!({ "mode": "keep" })).into_content(Some(current()));
        assert_eq!(keep.unwrap().password, Some(current()));
        let generated = login(&json!({ "mode": "generate", "length": 20, "symbols": false }))
            .into_content(Some(current()))
            .unwrap()
            .password
            .unwrap();
        assert_eq!(generated.expose().len(), 20);
        assert!(generated.expose().chars().all(char::is_alphanumeric));
        assert_eq!(
            login(&json!({ "mode": "generate", "length": 3, "symbols": true }))
                .into_content(None)
                .err(),
            Some(UiError::code("invalidLength"))
        );
    }

    #[test]
    fn a_note_keeps_only_its_text_and_tags() {
        let content = input(json!({
            "kind": "note",
            "title": "Wi-Fi",
            "username": "ignoré",
            "password": { "mode": "set", "value": "ignoré" },
            "urls": ["https://ignoree.example"],
            "notes": " Box : ABCD ",
            "tags": ["maison"],
        }))
        .into_content(Some(current()))
        .unwrap();
        assert_eq!(content.kind, EntryKind::Note);
        assert_eq!(content.username, None);
        assert_eq!(content.password, None);
        assert_eq!(content.urls, Vec::<String>::new());
        assert_eq!(content.notes.as_deref(), Some("Box : ABCD"));
        assert_eq!(content.tags.as_slice(), ["maison"]);
    }

    #[test]
    fn a_login_needs_a_password() {
        let empty = Some(UiError::code("emptyPassword"));
        let blank = login(&json!({ "mode": "set", "value": "" })).into_content(Some(current()));
        assert_eq!(blank.err(), empty);
        let kept = login(&json!({ "mode": "keep" })).into_content(None);
        assert_eq!(kept.err(), empty);
        let cleared = serde_json::from_value::<PasswordChoice>(json!({ "mode": "clear" }));
        assert!(cleared.is_err());
    }

    #[test]
    fn a_note_needs_a_text() {
        let note = input(json!({
            "kind": "note",
            "title": "Wi-Fi",
            "username": "",
            "password": { "mode": "keep" },
            "urls": [],
            "notes": "   ",
            "tags": [],
        }));
        assert_eq!(
            note.into_content(None).err(),
            Some(UiError::code("emptyNote"))
        );
    }

    #[test]
    fn a_title_is_required() {
        let blank = input(json!({
            "kind": "note",
            "title": "   ",
            "username": "",
            "password": { "mode": "keep" },
            "urls": [],
            "notes": "",
            "tags": [],
        }));
        assert_eq!(
            blank.into_content(None).err(),
            Some(UiError::code("emptyTitle"))
        );
    }

    #[test]
    fn history_never_contains_a_password() {
        let mut vault = Vault::new();
        let first = login(&json!({ "mode": "set", "value": "hunter2" }))
            .into_content(None)
            .unwrap();
        let id = vault.add(first.clone(), Timestamp::from_millis(1));
        let renamed = EntryContent {
            title: "GitHub pro".to_owned(),
            ..first.clone()
        };
        vault
            .update(id, renamed.clone(), Timestamp::from_millis(2))
            .unwrap();
        let new_password = EntryContent {
            password: Some(Secret::new("nouveau".to_owned())),
            ..renamed
        };
        vault
            .update(id, new_password, Timestamp::from_millis(3))
            .unwrap();

        let items = history(vault.get(id).unwrap());
        assert_eq!(
            serde_json::to_value(&items).unwrap(),
            json!([
                { "updatedAt": 2, "title": "GitHub pro", "username": "arthur", "passwordChanged": true },
                { "updatedAt": 1, "title": "GitHub", "username": "arthur", "passwordChanged": true },
            ])
        );
        let text = serde_json::to_string(&items).unwrap();
        assert!(!text.contains("hunter2"), "{text}");
        assert!(!text.contains("nouveau"), "{text}");
    }

    #[test]
    fn unchanged_password_is_not_flagged() {
        let mut vault = Vault::new();
        let first = login(&json!({ "mode": "set", "value": "hunter2" }))
            .into_content(None)
            .unwrap();
        let id = vault.add(first.clone(), Timestamp::from_millis(1));
        let renamed = EntryContent {
            title: "GitHub pro".to_owned(),
            ..first
        };
        vault
            .update(id, renamed, Timestamp::from_millis(2))
            .unwrap();
        let items = serde_json::to_value(history(vault.get(id).unwrap())).unwrap();
        assert_eq!(items[0]["passwordChanged"], false);
    }
}
