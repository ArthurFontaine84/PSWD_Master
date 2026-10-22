use std::cmp::Ordering;
use std::fmt;

use crate::entry::HISTORY_LIMIT;
use crate::{Entry, EntrySnapshot, Vault};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MergeError {
    DifferentVaults,
}

impl fmt::Display for MergeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::DifferentVaults => f.write_str("impossible de fusionner deux coffres différents"),
        }
    }
}

impl std::error::Error for MergeError {}

/// Fusionne deux copies d'un même coffre, entrée par entrée.
///
/// L'opération est commutative, associative et idempotente : toutes les copies convergent
/// quel que soit l'ordre des synchronisations.
///
/// # Errors
///
/// [`MergeError::DifferentVaults`] si les deux copies n'ont pas le même identifiant de coffre.
pub fn merge(local: Vault, remote: Vault) -> Result<Vault, MergeError> {
    if local.id != remote.id {
        return Err(MergeError::DifferentVaults);
    }
    let mut entries = local.entries;
    for (id, theirs) in remote.entries {
        let merged = match entries.remove(&id) {
            Some(ours) => merge_entry(ours, theirs),
            None => theirs,
        };
        entries.insert(id, merged);
    }
    Ok(Vault {
        id: local.id,
        entries,
    })
}

fn merge_entry(a: Entry, b: Entry) -> Entry {
    if a == b {
        return a;
    }
    let ordering = precedence(&a).cmp(&precedence(&b));
    let (mut winner, loser) = match ordering {
        Ordering::Less => (b, a),
        _ => (a, b),
    };
    winner.created_at = winner.created_at.min(loser.created_at);
    winner.deleted_at = winner.deleted_at.max(loser.deleted_at);

    let mut history = std::mem::take(&mut winner.history);
    history.extend(loser.history);
    // Une tombstone perdante est écartée par le filtre sur `deleted_at`.
    history.push(EntrySnapshot {
        content: loser.content,
        updated_at: loser.updated_at,
    });
    winner.history = normalize_history(history, &winner);
    winner
}

/// La version la plus récente gagne ; à date égale, une entrée vivante l'emporte sur une
/// tombstone, puis le contenu départage.
fn precedence(entry: &Entry) -> impl Ord + '_ {
    (entry.updated_at, !entry.deleted, entry.content.sort_key())
}

/// Ne garde que les versions postérieures à la dernière suppression et différentes de l'état
/// courant, triées de la plus récente à la plus ancienne, sans doublon, tronquées.
fn normalize_history(mut history: Vec<EntrySnapshot>, current: &Entry) -> Vec<EntrySnapshot> {
    history.retain(|snapshot| {
        current
            .deleted_at
            .is_none_or(|deleted_at| snapshot.updated_at > deleted_at)
            && !(snapshot.updated_at == current.updated_at && snapshot.content == current.content)
    });
    history.sort_by(|x, y| y.sort_key().cmp(&x.sort_key()));
    history.dedup();
    history.truncate(HISTORY_LIMIT);
    history
}

#[cfg(test)]
mod tests {
    use proptest::collection::vec;
    use proptest::prelude::*;
    use uuid::Uuid;

    use super::*;
    use crate::{EntryContent, EntryKind, Secret, Timestamp};

    const VAULT_ID: Uuid = Uuid::from_u128(1);
    const ENTRY_ID: Uuid = Uuid::from_u128(10);

    fn at(millis: i64) -> Timestamp {
        Timestamp::from_millis(millis)
    }

    fn titled(title: &str) -> EntryContent {
        EntryContent::new(EntryKind::Note, title.to_owned())
    }

    fn snapshot(title: &str, millis: i64) -> EntrySnapshot {
        EntrySnapshot {
            content: titled(title),
            updated_at: at(millis),
        }
    }

    fn live(title: &str, millis: i64, history: Vec<EntrySnapshot>) -> Entry {
        Entry {
            id: ENTRY_ID,
            content: titled(title),
            created_at: at(0),
            updated_at: at(millis),
            deleted: false,
            deleted_at: None,
            history,
        }
    }

    fn tombstone(millis: i64) -> Entry {
        Entry {
            id: ENTRY_ID,
            content: titled(""),
            created_at: at(0),
            updated_at: at(millis),
            deleted: true,
            deleted_at: Some(at(millis)),
            history: Vec::new(),
        }
    }

    fn vault_with(entries: impl IntoIterator<Item = Entry>) -> Vault {
        Vault {
            id: VAULT_ID,
            entries: entries.into_iter().map(|entry| (entry.id, entry)).collect(),
        }
    }

    fn merged_entry(a: Entry, b: Entry) -> Entry {
        let merged = merge(vault_with([a]), vault_with([b])).unwrap();
        merged.entries[&ENTRY_ID].clone()
    }

    #[test]
    fn different_vaults_are_rejected() {
        let other = Vault {
            id: Uuid::from_u128(2),
            ..vault_with([])
        };
        assert_eq!(
            merge(vault_with([]), other),
            Err(MergeError::DifferentVaults)
        );
    }

    #[test]
    fn error_message_is_explicit() {
        assert_eq!(
            MergeError::DifferentVaults.to_string(),
            "impossible de fusionner deux coffres différents"
        );
    }

    #[test]
    fn entries_present_on_one_side_are_kept() {
        let mine = Entry {
            id: Uuid::from_u128(20),
            ..live("mine", 1, Vec::new())
        };
        let theirs = Entry {
            id: Uuid::from_u128(30),
            ..live("theirs", 1, Vec::new())
        };
        let merged = merge(vault_with([mine.clone()]), vault_with([theirs.clone()])).unwrap();
        assert_eq!(merged, vault_with([mine, theirs]));
    }

    #[test]
    fn identical_entries_are_kept_as_is() {
        let entry = live("a", 2, vec![snapshot("old", 1)]);
        assert_eq!(merged_entry(entry.clone(), entry.clone()), entry);
    }

    #[test]
    fn latest_version_wins_and_the_other_goes_to_history() {
        let older = live("v1", 1, Vec::new());
        let newer = live("v2", 2, Vec::new());
        let expected = live("v2", 2, vec![snapshot("v1", 1)]);
        assert_eq!(merged_entry(older.clone(), newer.clone()), expected);
        assert_eq!(merged_entry(newer, older), expected);
    }

    #[test]
    fn same_date_is_decided_by_content() {
        let a = live("a", 1, Vec::new());
        let b = live("b", 1, Vec::new());
        let expected = live("b", 1, vec![snapshot("a", 1)]);
        assert_eq!(merged_entry(a.clone(), b.clone()), expected);
        assert_eq!(merged_entry(b, a), expected);
    }

    #[test]
    fn live_entry_beats_a_tombstone_of_the_same_date() {
        let merged = merged_entry(tombstone(2), live("a", 2, vec![snapshot("old", 1)]));
        assert!(!merged.is_deleted());
        assert_eq!(merged.content, titled("a"));
        assert_eq!(merged.deleted_at, Some(at(2)));
        assert_eq!(merged.history, []);
    }

    #[test]
    fn later_deletion_wins_and_erases_history() {
        let merged = merged_entry(live("a", 1, vec![snapshot("old", 0)]), tombstone(2));
        assert_eq!(merged, tombstone(2));
    }

    #[test]
    fn modification_after_deletion_revives_without_older_versions() {
        let revived = live("v3", 3, vec![snapshot("v1", 1)]);
        let merged = merged_entry(tombstone(2), revived);
        assert!(!merged.is_deleted());
        assert_eq!(merged.content, titled("v3"));
        assert_eq!(merged.history, []);
    }

    #[test]
    fn histories_are_united_without_duplicating_the_current_version() {
        let a = live("v3", 3, vec![snapshot("v2", 2), snapshot("v1", 1)]);
        let b = live("v3", 3, vec![snapshot("other", 2)]);
        let merged = merged_entry(a, b);
        assert_eq!(
            merged.history,
            [snapshot("v2", 2), snapshot("other", 2), snapshot("v1", 1)]
        );
    }

    #[test]
    fn merged_history_is_truncated() {
        let a = live("now", 100, (0..10).map(|i| snapshot("a", i)).collect());
        let b = live("now", 100, (10..20).map(|i| snapshot("b", i)).collect());
        let merged = merged_entry(a, b);
        let dates: Vec<i64> = merged
            .history
            .iter()
            .map(|s| s.updated_at.as_millis())
            .collect();
        assert_eq!(dates, [19, 18, 17, 16, 15, 14, 13, 12, 11, 10]);
    }

    #[test]
    fn earliest_creation_date_is_kept() {
        let a = Entry {
            created_at: at(5),
            ..live("a", 10, Vec::new())
        };
        let b = Entry {
            created_at: at(3),
            ..live("b", 20, Vec::new())
        };
        assert_eq!(merged_entry(a, b).created_at, at(3));
    }

    fn arb_content() -> impl Strategy<Value = EntryContent> {
        (
            prop_oneof![Just(EntryKind::Login), Just(EntryKind::Note)],
            "[ab]",
            proptest::option::of("[xy]"),
        )
            .prop_map(|(kind, title, password)| EntryContent {
                password: password.map(Secret::new),
                ..EntryContent::new(kind, title)
            })
    }

    fn arb_snapshot() -> impl Strategy<Value = EntrySnapshot> {
        (arb_content(), 0..6_i64).prop_map(|(content, millis)| EntrySnapshot {
            content,
            updated_at: at(millis),
        })
    }

    /// Entrée cohérente : sur un petit domaine pour provoquer collisions et égalités.
    fn arb_entry() -> impl Strategy<Value = Entry> {
        (
            0..4_u128,
            arb_content(),
            0..3_i64,
            0..6_i64,
            any::<bool>(),
            proptest::option::of(0..6_i64),
            vec(arb_snapshot(), 0..12),
        )
            .prop_map(
                |(id, content, created, updated, deleted, deleted_at, history)| {
                    let updated_at = at(created.max(updated));
                    let mut entry = Entry {
                        id: Uuid::from_u128(id),
                        content,
                        created_at: at(created),
                        updated_at,
                        deleted,
                        deleted_at: deleted_at.map(at).filter(|&d| d < updated_at),
                        history: Vec::new(),
                    };
                    if deleted {
                        entry.content = EntryContent::new(entry.content.kind, String::new());
                        entry.deleted_at = Some(updated_at);
                    } else {
                        let older = history
                            .into_iter()
                            .filter(|snapshot| snapshot.updated_at < updated_at)
                            .collect();
                        entry.history = normalize_history(older, &entry);
                    }
                    entry
                },
            )
    }

    fn arb_vault() -> impl Strategy<Value = Vault> {
        vec(arb_entry(), 0..6).prop_map(vault_with)
    }

    proptest! {
        #[test]
        fn merge_is_commutative(a in arb_vault(), b in arb_vault()) {
            prop_assert_eq!(merge(a.clone(), b.clone()), merge(b, a));
        }

        #[test]
        fn merge_is_associative(a in arb_vault(), b in arb_vault(), c in arb_vault()) {
            let left = merge(merge(a.clone(), b.clone()).unwrap(), c.clone()).unwrap();
            let right = merge(a, merge(b, c).unwrap()).unwrap();
            prop_assert_eq!(left, right);
        }

        #[test]
        fn merge_is_idempotent(a in arb_vault()) {
            prop_assert_eq!(merge(a.clone(), a.clone()), Ok(a));
        }

        #[test]
        fn no_entry_is_lost(a in arb_vault(), b in arb_vault()) {
            let merged = merge(a.clone(), b.clone()).unwrap();
            for id in a.entries.keys().chain(b.entries.keys()) {
                let candidates: Vec<&Entry> =
                    a.entries.get(id).into_iter().chain(b.entries.get(id)).collect();
                let latest = candidates.iter().map(|entry| entry.updated_at).max();
                let latest_is_live = candidates
                    .iter()
                    .any(|entry| Some(entry.updated_at) == latest && !entry.deleted);
                let result = &merged.entries[id];
                prop_assert_eq!(Some(result.updated_at), latest);
                prop_assert_eq!(result.deleted, !latest_is_live);
            }
        }
    }
}
