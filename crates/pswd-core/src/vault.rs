use std::collections::BTreeMap;
use std::fmt;

use uuid::Uuid;

use crate::{Entry, EntryContent, Timestamp};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VaultError {
    EntryNotFound(Uuid),
}

impl fmt::Display for VaultError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EntryNotFound(id) => write!(f, "entrée introuvable : {id}"),
        }
    }
}

impl std::error::Error for VaultError {}

/// Coffre déchiffré. Les entrées supprimées restent présentes sous forme de tombstones.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Vault {
    pub(crate) id: Uuid,
    pub(crate) entries: BTreeMap<Uuid, Entry>,
}

impl Vault {
    // Un coffre neuf reçoit un identifiant aléatoire : il n'a pas de valeur par défaut sensée.
    #[allow(clippy::new_without_default)]
    #[must_use]
    pub fn new() -> Self {
        Self {
            id: Uuid::new_v4(),
            entries: BTreeMap::new(),
        }
    }

    #[must_use]
    pub fn id(&self) -> Uuid {
        self.id
    }

    /// Toutes les entrées, tombstones comprises, triées par identifiant.
    pub fn entries(&self) -> impl Iterator<Item = &Entry> {
        self.entries.values()
    }

    /// Entrée non supprimée portant cet identifiant.
    #[must_use]
    pub fn get(&self, id: Uuid) -> Option<&Entry> {
        self.entries.get(&id).filter(|entry| !entry.is_deleted())
    }

    /// Ajoute une entrée et renvoie son identifiant.
    pub fn add(&mut self, content: EntryContent, now: Timestamp) -> Uuid {
        let entry = Entry::new(content, now);
        let id = entry.id();
        self.entries.insert(id, entry);
        id
    }

    /// # Errors
    ///
    /// [`VaultError::EntryNotFound`] si l'entrée n'existe pas ou est supprimée.
    pub fn update(
        &mut self,
        id: Uuid,
        content: EntryContent,
        now: Timestamp,
    ) -> Result<(), VaultError> {
        self.live_entry_mut(id)?.update(content, now);
        Ok(())
    }

    /// # Errors
    ///
    /// [`VaultError::EntryNotFound`] si l'entrée n'existe pas ou est déjà supprimée.
    pub fn delete(&mut self, id: Uuid, now: Timestamp) -> Result<(), VaultError> {
        self.live_entry_mut(id)?.delete(now);
        Ok(())
    }

    fn live_entry_mut(&mut self, id: Uuid) -> Result<&mut Entry, VaultError> {
        self.entries
            .get_mut(&id)
            .filter(|entry| !entry.is_deleted())
            .ok_or(VaultError::EntryNotFound(id))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::EntryKind;

    fn titled(title: &str) -> EntryContent {
        EntryContent::new(EntryKind::Note, title.to_owned())
    }

    fn at(millis: i64) -> Timestamp {
        Timestamp::from_millis(millis)
    }

    #[test]
    fn new_vault_is_empty_with_a_random_v4_id() {
        let vault = Vault::new();
        assert_eq!(vault.id().get_version_num(), 4);
        assert_ne!(vault.id(), Vault::new().id());
        assert_eq!(vault.entries().count(), 0);
    }

    #[test]
    fn entries_are_sorted_by_id() {
        let mut vault = Vault::new();
        let mut ids = vec![
            vault.add(titled("a"), at(1_000)),
            vault.add(titled("b"), at(1_000)),
            vault.add(titled("c"), at(1_000)),
        ];
        ids.sort();
        let listed: Vec<Uuid> = vault.entries().map(Entry::id).collect();
        assert_eq!(listed, ids);
    }

    #[test]
    fn get_finds_a_live_entry() {
        let mut vault = Vault::new();
        vault.add(titled("a"), at(1_000));
        let id = vault.add(titled("b"), at(1_000));
        assert_eq!(vault.get(id).map(Entry::content), Some(&titled("b")));
    }

    #[test]
    fn get_ignores_unknown_and_deleted_entries() {
        let mut vault = Vault::new();
        let id = vault.add(titled("a"), at(1_000));
        vault.delete(id, at(2_000)).unwrap();
        assert_eq!(vault.get(id), None);
        assert_eq!(vault.get(Uuid::new_v4()), None);
    }

    #[test]
    fn update_changes_the_entry() {
        let mut vault = Vault::new();
        let id = vault.add(titled("a"), at(1_000));
        vault.update(id, titled("b"), at(2_000)).unwrap();
        let entry = vault.get(id).unwrap();
        assert_eq!(entry.content(), &titled("b"));
        assert_eq!(entry.history().len(), 1);
    }

    #[test]
    fn delete_keeps_a_tombstone() {
        let mut vault = Vault::new();
        let id = vault.add(titled("a"), at(1_000));
        vault.delete(id, at(2_000)).unwrap();
        let tombstone = vault.entries.get(&id).unwrap();
        assert!(tombstone.is_deleted());
        assert_eq!(tombstone.updated_at(), at(2_000));
    }

    #[test]
    fn unknown_entry_cannot_be_updated_or_deleted() {
        let mut vault = Vault::new();
        vault.add(titled("a"), at(1_000));
        let id = Uuid::new_v4();
        assert_eq!(
            vault.update(id, titled("a"), at(1_000)),
            Err(VaultError::EntryNotFound(id))
        );
        assert_eq!(
            vault.delete(id, at(1_000)),
            Err(VaultError::EntryNotFound(id))
        );
    }

    #[test]
    fn deleted_entry_cannot_be_updated_or_deleted_again() {
        let mut vault = Vault::new();
        let id = vault.add(titled("a"), at(1_000));
        vault.delete(id, at(2_000)).unwrap();
        assert_eq!(
            vault.update(id, titled("b"), at(3_000)),
            Err(VaultError::EntryNotFound(id))
        );
        assert_eq!(
            vault.delete(id, at(3_000)),
            Err(VaultError::EntryNotFound(id))
        );
        assert_eq!(vault.entries.get(&id).unwrap().updated_at(), at(2_000));
    }

    #[test]
    fn error_message_names_the_entry() {
        assert_eq!(
            VaultError::EntryNotFound(Uuid::nil()).to_string(),
            "entrée introuvable : 00000000-0000-0000-0000-000000000000"
        );
    }
}
