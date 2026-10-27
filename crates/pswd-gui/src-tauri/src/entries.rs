//! Ce que l'interface reçoit d'une entrée : tout sauf le mot de passe, qui n'est envoyé que sur
//! demande explicite (`reveal_password`).

use pswd_core::{Entry, EntryKind};
use serde::Serialize;

/// Une ligne de la liste des entrées.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EntrySummary {
    id: String,
    kind: &'static str,
    title: String,
    username: Option<String>,
    tags: Vec<String>,
    has_password: bool,
}

/// Le détail d'une entrée, sans son mot de passe.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EntryDetail {
    #[serde(flatten)]
    summary: EntrySummary,
    urls: Vec<String>,
    notes: Option<String>,
    /// Millisecondes depuis l'époque Unix (UTC).
    created_at: i64,
    updated_at: i64,
    versions: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct TagCount {
    tag: String,
    count: usize,
}

impl From<&Entry> for EntrySummary {
    fn from(entry: &Entry) -> Self {
        let content = entry.content();
        Self {
            id: entry.id().to_string(),
            kind: match content.kind {
                EntryKind::Login => "login",
                EntryKind::Note => "note",
            },
            title: content.title.clone(),
            username: content.username.clone(),
            tags: content.tags.as_slice().to_vec(),
            has_password: content.password.is_some(),
        }
    }
}

impl From<&Entry> for EntryDetail {
    fn from(entry: &Entry) -> Self {
        let content = entry.content();
        Self {
            summary: EntrySummary::from(entry),
            urls: content.urls.clone(),
            notes: content.notes.clone(),
            created_at: entry.created_at().as_millis(),
            updated_at: entry.updated_at().as_millis(),
            versions: entry.history().len(),
        }
    }
}

impl TagCount {
    pub fn new(tag: &str, count: usize) -> Self {
        Self {
            tag: tag.to_owned(),
            count,
        }
    }
}

#[cfg(test)]
mod tests {
    use pswd_core::{EntryContent, Secret, Tags, Timestamp, Vault};
    use serde_json::json;

    use super::*;

    #[test]
    fn the_interface_never_receives_the_password() {
        let mut vault = Vault::new();
        let content = EntryContent {
            username: Some("arthur".to_owned()),
            password: Some(Secret::new("hunter2".to_owned())),
            urls: vec!["https://github.com".to_owned()],
            notes: Some("compte pro".to_owned()),
            tags: Tags::new(["dev"]),
            ..EntryContent::new(EntryKind::Login, "GitHub".to_owned())
        };
        let id = vault.add(content.clone(), Timestamp::from_millis(1));
        let changed = EntryContent {
            notes: Some("compte perso".to_owned()),
            ..content
        };
        vault
            .update(id, changed, Timestamp::from_millis(2))
            .unwrap();
        let note = vault.add(
            EntryContent::new(EntryKind::Note, "Wi-Fi".to_owned()),
            Timestamp::from_millis(3),
        );

        let detail = serde_json::to_value(EntryDetail::from(vault.get(id).unwrap())).unwrap();
        assert_eq!(
            detail,
            json!({
                "id": id.to_string(),
                "kind": "login",
                "title": "GitHub",
                "username": "arthur",
                "tags": ["dev"],
                "hasPassword": true,
                "urls": ["https://github.com"],
                "notes": "compte perso",
                "createdAt": 1,
                "updatedAt": 2,
                "versions": 1,
            })
        );
        assert!(!detail.to_string().contains("hunter2"));

        let summary = serde_json::to_value(EntrySummary::from(vault.get(note).unwrap())).unwrap();
        assert_eq!(summary["kind"], "note");
        assert_eq!(summary["hasPassword"], false);
        assert_eq!(
            serde_json::to_value(TagCount::new("dev", 2)).unwrap(),
            json!({ "tag": "dev", "count": 2 })
        );
    }
}
