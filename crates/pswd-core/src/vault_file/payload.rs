use std::collections::BTreeMap;
use std::io;

use serde::{Deserialize, Serialize, Serializer};
use uuid::Uuid;
use zeroize::Zeroizing;

use super::{FileError, internal_error};
use crate::{Entry, Vault};

#[derive(Serialize)]
struct PayloadRef<'a> {
    vault_id: Uuid,
    entries: EntryList<'a>,
}

/// Les entrées sont écrites sous forme de liste : leur identifiant y figure déjà.
struct EntryList<'a>(&'a BTreeMap<Uuid, Entry>);

impl Serialize for EntryList<'_> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.collect_seq(self.0.values())
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Payload {
    vault_id: Uuid,
    entries: Vec<Entry>,
}

/// JSON du coffre, dans un buffer effacé à la libération.
pub(super) fn encode(vault: &Vault) -> Result<Zeroizing<Vec<u8>>, FileError> {
    let payload = PayloadRef {
        vault_id: vault.id,
        entries: EntryList(&vault.entries),
    };
    // La taille est calculée d'abord : le buffer n'est jamais réalloué, donc aucune copie du
    // JSON n'est abandonnée en mémoire sans être effacée.
    let mut counter = ByteCounter(0);
    serde_json::to_writer(&mut counter, &payload).map_err(internal_error)?;
    let mut json = Zeroizing::new(Vec::with_capacity(counter.0));
    serde_json::to_writer(&mut *json, &payload).map_err(internal_error)?;
    Ok(json)
}

pub(super) fn decode(json: &[u8]) -> Result<Vault, FileError> {
    // Le détail de l'erreur serde n'est pas repris : il peut citer un extrait du contenu.
    let payload: Payload = serde_json::from_slice(json).map_err(|_| FileError::Malformed)?;
    let mut entries = BTreeMap::new();
    for entry in payload.entries {
        if entries.insert(entry.id, entry).is_some() {
            return Err(FileError::Malformed);
        }
    }
    Ok(Vault {
        id: payload.vault_id,
        entries,
    })
}

struct ByteCounter(usize);

impl io::Write for ByteCounter {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.0 += buf.len();
        Ok(buf.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use std::io::Write;

    use proptest::collection::vec;
    use proptest::prelude::*;

    use super::*;
    use crate::{EntryContent, EntryKind, EntrySnapshot, Secret, Tags, Timestamp};

    fn sample_vault() -> Vault {
        let entry = Entry {
            id: Uuid::from_u128(10),
            content: EntryContent {
                username: Some("arthur".to_owned()),
                password: Some(Secret::new("hunter2".to_owned())),
                urls: vec!["https://github.com".to_owned()],
                tags: Tags::new(["dev"]),
                ..EntryContent::new(EntryKind::Login, "GitHub".to_owned())
            },
            created_at: Timestamp::from_millis(1_000),
            updated_at: Timestamp::from_millis(2_000),
            deleted: false,
            deleted_at: None,
            history: vec![EntrySnapshot {
                content: EntryContent::new(EntryKind::Note, "Ancien".to_owned()),
                updated_at: Timestamp::from_millis(1_000),
            }],
        };
        Vault {
            id: Uuid::from_u128(1),
            entries: BTreeMap::from([(entry.id, entry)]),
        }
    }

    const SAMPLE_JSON: &str = concat!(
        r#"{"vault_id":"00000000-0000-0000-0000-000000000001","entries":[{"#,
        r#""id":"00000000-0000-0000-0000-00000000000a","#,
        r#""content":{"kind":"login","title":"GitHub","username":"arthur","password":"hunter2","#,
        r#""urls":["https://github.com"],"notes":null,"tags":["dev"]},"#,
        r#""created_at":1000,"updated_at":2000,"deleted":false,"deleted_at":null,"#,
        r#""history":[{"content":{"kind":"note","title":"Ancien","username":null,"#,
        r#""password":null,"urls":[],"notes":null,"tags":[]},"updated_at":1000}]}]}"#,
    );

    #[test]
    fn json_format_is_stable() {
        let json = encode(&sample_vault()).unwrap();
        assert_eq!(std::str::from_utf8(&json).unwrap(), SAMPLE_JSON);
        assert_eq!(decode(SAMPLE_JSON.as_bytes()), Ok(sample_vault()));
    }

    #[test]
    fn buffer_is_allocated_once_at_the_right_size() {
        let json = encode(&sample_vault()).unwrap();
        assert_eq!(json.capacity(), json.len());
    }

    #[test]
    fn duplicated_entry_is_rejected() {
        let entry = r#"{"id":"00000000-0000-0000-0000-00000000000a","content":{"kind":"note","title":"","username":null,"password":null,"urls":[],"notes":null,"tags":[]},"created_at":0,"updated_at":0,"deleted":false,"deleted_at":null,"history":[]}"#;
        let json = format!(
            r#"{{"vault_id":"00000000-0000-0000-0000-000000000001","entries":[{entry},{entry}]}}"#
        );
        assert_eq!(decode(json.as_bytes()), Err(FileError::Malformed));
    }

    #[test]
    fn unknown_fields_are_rejected() {
        let json = SAMPLE_JSON.replacen(r#""deleted":false"#, r#""deleted":false,"color":1"#, 1);
        assert_eq!(decode(json.as_bytes()), Err(FileError::Malformed));
    }

    #[test]
    fn invalid_json_is_malformed() {
        assert_eq!(decode(b"not json"), Err(FileError::Malformed));
        assert_eq!(
            decode(br#"{"vault_id":"00000000-0000-0000-0000-000000000001"}"#),
            Err(FileError::Malformed)
        );
    }

    #[test]
    fn counter_counts_written_bytes() {
        let mut counter = ByteCounter(0);
        counter.write_all(b"abc").unwrap();
        counter.flush().unwrap();
        assert_eq!(counter.0, 3);
    }

    fn arb_text() -> impl Strategy<Value = String> {
        "(?s).{0,8}"
    }

    fn arb_content() -> impl Strategy<Value = EntryContent> {
        (
            prop_oneof![Just(EntryKind::Login), Just(EntryKind::Note)],
            arb_text(),
            proptest::option::of(arb_text()),
            proptest::option::of(arb_text()),
            vec(arb_text(), 0..3),
            proptest::option::of(arb_text()),
            vec(arb_text(), 0..3),
        )
            .prop_map(|(kind, title, username, password, urls, notes, tags)| {
                EntryContent {
                    kind,
                    title,
                    username,
                    password: password.map(Secret::new),
                    urls,
                    notes,
                    tags: Tags::new(tags),
                }
            })
    }

    fn arb_entry() -> impl Strategy<Value = Entry> {
        (
            any::<u128>(),
            arb_content(),
            (any::<i64>(), any::<i64>()),
            any::<bool>(),
            proptest::option::of(any::<i64>()),
            vec((arb_content(), any::<i64>()), 0..3),
        )
            .prop_map(
                |(id, content, (created, updated), deleted, deleted_at, history)| Entry {
                    id: Uuid::from_u128(id),
                    content,
                    created_at: Timestamp::from_millis(created),
                    updated_at: Timestamp::from_millis(updated),
                    deleted,
                    deleted_at: deleted_at.map(Timestamp::from_millis),
                    history: history
                        .into_iter()
                        .map(|(content, millis)| EntrySnapshot {
                            content,
                            updated_at: Timestamp::from_millis(millis),
                        })
                        .collect(),
                },
            )
    }

    proptest! {
        #[test]
        fn decode_reverses_encode(id in any::<u128>(), entries in vec(arb_entry(), 0..4)) {
            let vault = Vault {
                id: Uuid::from_u128(id),
                entries: entries.into_iter().map(|entry| (entry.id, entry)).collect(),
            };
            let json = encode(&vault).unwrap();
            prop_assert_eq!(decode(&json), Ok(vault));
        }
    }
}
