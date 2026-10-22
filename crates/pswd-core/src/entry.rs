use uuid::Uuid;

use crate::{Secret, Tags, Timestamp};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EntryKind {
    Login,
    Note,
}

/// Contenu modifiable d'une entrée.
#[derive(Debug, Clone, PartialEq, Eq)]
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
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entry {
    id: Uuid,
    content: EntryContent,
    created_at: Timestamp,
    updated_at: Timestamp,
}

impl Entry {
    #[must_use]
    pub fn new(content: EntryContent, now: Timestamp) -> Self {
        Self {
            id: Uuid::new_v4(),
            content,
            created_at: now,
            updated_at: now,
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
}

#[cfg(test)]
mod tests {
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
    fn new_entry_keeps_its_content() {
        let entry = Entry::new(github(), Timestamp::from_millis(1_000));
        assert_eq!(entry.content(), &github());
    }

    #[test]
    fn new_entry_is_created_and_updated_now() {
        let now = Timestamp::from_millis(1_000);
        let entry = Entry::new(github(), now);
        assert_eq!(entry.created_at(), now);
        assert_eq!(entry.updated_at(), now);
    }

    #[test]
    fn new_entry_gets_a_random_v4_id() {
        let first = Entry::new(github(), Timestamp::from_millis(1_000));
        let second = Entry::new(github(), Timestamp::from_millis(1_000));
        assert_eq!(first.id().get_version_num(), 4);
        assert_ne!(first.id(), second.id());
    }

    #[test]
    fn debug_hides_the_password() {
        let entry = Entry::new(github(), Timestamp::from_millis(1_000));
        assert!(!format!("{entry:?}").contains("hunter2"));
    }
}
