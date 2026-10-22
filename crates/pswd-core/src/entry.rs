use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{Secret, Tags, Timestamp};

pub(crate) const HISTORY_LIMIT: usize = 10;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum EntryKind {
    Login,
    Note,
}

/// Contenu modifiable d'une entrée.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EntryContent {
    pub kind: EntryKind,
    pub title: String,
    pub username: Option<String>,
    pub password: Option<Secret>,
    pub urls: Vec<String>,
    pub notes: Option<String>,
    pub tags: Tags,
}

impl EntryContent {
    #[must_use]
    pub fn new(kind: EntryKind, title: String) -> Self {
        Self {
            kind,
            title,
            username: None,
            password: None,
            urls: Vec::new(),
            notes: None,
            tags: Tags::default(),
        }
    }

    /// Ordre total sur le contenu, utilisé pour départager la fusion de façon déterministe.
    pub(crate) fn sort_key(&self) -> impl Ord + '_ {
        (
            self.kind,
            &self.title,
            &self.username,
            self.password.as_ref().map(Secret::expose),
            &self.urls,
            &self.notes,
            self.tags.as_slice(),
        )
    }
}

/// Version précédente d'une entrée.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EntrySnapshot {
    pub content: EntryContent,
    pub updated_at: Timestamp,
}

impl EntrySnapshot {
    pub(crate) fn sort_key(&self) -> impl Ord + '_ {
        (self.updated_at, self.content.sort_key())
    }
}

/// Les modifications passent par [`crate::Vault`], qui garantit que l'entrée n'est pas supprimée.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Entry {
    pub(crate) id: Uuid,
    pub(crate) content: EntryContent,
    pub(crate) created_at: Timestamp,
    pub(crate) updated_at: Timestamp,
    pub(crate) deleted: bool,
    /// Date de la dernière suppression : les versions antérieures ne sont plus jamais conservées.
    pub(crate) deleted_at: Option<Timestamp>,
    pub(crate) history: Vec<EntrySnapshot>,
}

impl Entry {
    pub(crate) fn new(content: EntryContent, now: Timestamp) -> Self {
        Self {
            id: Uuid::new_v4(),
            content,
            created_at: now,
            updated_at: now,
            deleted: false,
            deleted_at: None,
            history: Vec::new(),
        }
    }

    #[must_use]
    pub fn id(&self) -> Uuid {
        self.id
    }

    #[must_use]
    pub fn content(&self) -> &EntryContent {
        &self.content
    }

    #[must_use]
    pub fn created_at(&self) -> Timestamp {
        self.created_at
    }

    #[must_use]
    pub fn updated_at(&self) -> Timestamp {
        self.updated_at
    }

    #[must_use]
    pub fn is_deleted(&self) -> bool {
        self.deleted
    }

    /// Versions précédentes, les plus récentes d'abord.
    #[must_use]
    pub fn history(&self) -> &[EntrySnapshot] {
        &self.history
    }

    /// Sans effet si le contenu est inchangé.
    pub(crate) fn update(&mut self, content: EntryContent, now: Timestamp) {
        if self.content == content {
            return;
        }
        let previous = EntrySnapshot {
            content: std::mem::replace(&mut self.content, content),
            updated_at: self.updated_at,
        };
        self.history.insert(0, previous);
        self.history.truncate(HISTORY_LIMIT);
        self.updated_at = now;
    }

    pub(crate) fn delete(&mut self, now: Timestamp) {
        self.content = EntryContent::new(self.content.kind, String::new());
        self.history.clear();
        self.deleted = true;
        self.deleted_at = self.deleted_at.max(Some(now));
        self.updated_at = now;
    }
}

#[cfg(test)]
mod tests {
    use std::cmp::Ordering;

    use super::*;

    fn github() -> EntryContent {
        EntryContent {
            username: Some("arthur".to_owned()),
            password: Some(Secret::new("hunter2".to_owned())),
            urls: vec!["https://github.com".to_owned()],
            tags: Tags::new(["dev"]),
            ..EntryContent::new(EntryKind::Login, "GitHub".to_owned())
        }
    }

    fn titled(title: &str) -> EntryContent {
        EntryContent::new(EntryKind::Note, title.to_owned())
    }

    fn at(millis: i64) -> Timestamp {
        Timestamp::from_millis(millis)
    }

    #[test]
    fn new_content_has_only_kind_and_title() {
        let content = EntryContent::new(EntryKind::Note, "Wi-Fi".to_owned());
        assert_eq!(content.kind, EntryKind::Note);
        assert_eq!(content.title, "Wi-Fi");
        assert_eq!(content.username, None);
        assert_eq!(content.password, None);
        assert_eq!(content.urls, Vec::<String>::new());
        assert_eq!(content.notes, None);
        assert_eq!(content.tags, Tags::default());
    }

    #[test]
    fn sort_key_compares_every_field() {
        let base = github();
        let variants = [
            EntryContent {
                kind: EntryKind::Note,
                ..base.clone()
            },
            EntryContent {
                title: "Gitea".to_owned(),
                ..base.clone()
            },
            EntryContent {
                username: None,
                ..base.clone()
            },
            EntryContent {
                password: Some(Secret::new("other".to_owned())),
                ..base.clone()
            },
            EntryContent {
                urls: Vec::new(),
                ..base.clone()
            },
            EntryContent {
                notes: Some("note".to_owned()),
                ..base.clone()
            },
            EntryContent {
                tags: Tags::default(),
                ..base.clone()
            },
        ];
        for variant in variants {
            assert_ne!(variant.sort_key().cmp(&base.sort_key()), Ordering::Equal);
        }
        assert_eq!(
            base.clone().sort_key().cmp(&base.sort_key()),
            Ordering::Equal
        );
    }

    #[test]
    fn snapshot_sort_key_orders_by_date_first() {
        let older = EntrySnapshot {
            content: titled("z"),
            updated_at: at(1),
        };
        let newer = EntrySnapshot {
            content: titled("a"),
            updated_at: at(2),
        };
        assert!(newer.sort_key() > older.sort_key());
    }

    #[test]
    fn new_entry_keeps_its_content() {
        let entry = Entry::new(github(), at(1_000));
        assert_eq!(entry.content(), &github());
    }

    #[test]
    fn new_entry_is_created_and_updated_now() {
        let entry = Entry::new(github(), at(1_000));
        assert_eq!(entry.created_at(), at(1_000));
        assert_eq!(entry.updated_at(), at(1_000));
    }

    #[test]
    fn new_entry_is_live_without_history() {
        let entry = Entry::new(github(), at(1_000));
        assert!(!entry.is_deleted());
        assert_eq!(entry.deleted_at, None);
        assert_eq!(entry.history(), []);
    }

    #[test]
    fn new_entry_gets_a_random_v4_id() {
        let first = Entry::new(github(), at(1_000));
        let second = Entry::new(github(), at(1_000));
        assert_eq!(first.id().get_version_num(), 4);
        assert_ne!(first.id(), second.id());
    }

    #[test]
    fn debug_hides_the_password() {
        let entry = Entry::new(github(), at(1_000));
        assert!(!format!("{entry:?}").contains("hunter2"));
    }

    #[test]
    fn update_replaces_content_and_keeps_previous_version() {
        let mut entry = Entry::new(titled("v1"), at(1_000));
        entry.update(titled("v2"), at(2_000));
        assert_eq!(entry.content(), &titled("v2"));
        assert_eq!(entry.updated_at(), at(2_000));
        assert_eq!(entry.created_at(), at(1_000));
        assert_eq!(
            entry.history(),
            [EntrySnapshot {
                content: titled("v1"),
                updated_at: at(1_000)
            }]
        );
    }

    #[test]
    fn update_with_same_content_changes_nothing() {
        let mut entry = Entry::new(titled("v1"), at(1_000));
        let before = entry.clone();
        entry.update(titled("v1"), at(2_000));
        assert_eq!(entry, before);
    }

    #[test]
    fn history_keeps_the_ten_most_recent_versions_first() {
        let mut entry = Entry::new(titled("v0"), at(0));
        for version in 1..=12 {
            entry.update(titled(&format!("v{version}")), at(version));
        }
        let titles: Vec<&str> = entry
            .history()
            .iter()
            .map(|snapshot| snapshot.content.title.as_str())
            .collect();
        assert_eq!(
            titles,
            ["v11", "v10", "v9", "v8", "v7", "v6", "v5", "v4", "v3", "v2"]
        );
    }

    #[test]
    fn delete_leaves_an_empty_tombstone() {
        let mut entry = Entry::new(github(), at(1_000));
        entry.update(titled("v2"), at(2_000));
        let id = entry.id();
        entry.delete(at(3_000));
        assert!(entry.is_deleted());
        assert_eq!(entry.id(), id);
        assert_eq!(entry.updated_at(), at(3_000));
        assert_eq!(entry.deleted_at, Some(at(3_000)));
        assert_eq!(
            entry.content(),
            &EntryContent::new(EntryKind::Note, String::new())
        );
        assert_eq!(entry.history(), []);
    }

    #[test]
    fn delete_never_moves_the_deletion_date_back() {
        let mut entry = Entry::new(github(), at(1_000));
        entry.deleted_at = Some(at(5_000));
        entry.delete(at(3_000));
        assert_eq!(entry.deleted_at, Some(at(5_000)));
    }
}
