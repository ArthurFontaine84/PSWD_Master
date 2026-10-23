use std::collections::BTreeMap;
use std::fs::OpenOptions;
use std::io::Write;
use std::path::Path;

use pswd_core::{Entry, EntryContent, Timestamp, Vault, export_csv};

use crate::{AppError, VaultSession};

/// Entrées non supprimées dont le titre, l'identifiant, une URL ou un tag contient `text`, et
/// qui portent `tag` s'il est donné ; casse ignorée, triées par titre.
#[must_use]
pub fn search<'v>(vault: &'v Vault, text: &str, tag: Option<&str>) -> Vec<&'v Entry> {
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
    entries.sort_by_key(|entry| entry.content().title.to_lowercase());
    entries
}

fn matches_text(entry: &Entry, text: &str) -> bool {
    let content = entry.content();
    std::iter::once(&content.title)
        .chain(&content.username)
        .chain(&content.urls)
        .chain(content.tags.as_slice())
        .any(|field| field.to_lowercase().contains(text))
}

/// Nombre d'entrées non supprimées par tag, triés par tag.
#[must_use]
pub fn tag_counts(vault: &Vault) -> BTreeMap<&str, usize> {
    let mut counts = BTreeMap::new();
    for entry in vault.entries().filter(|entry| !entry.is_deleted()) {
        for tag in entry.content().tags.as_slice() {
            *counts.entry(tag.as_str()).or_default() += 1;
        }
    }
    counts
}

#[must_use]
pub fn live_count(vault: &Vault) -> usize {
    vault.entries().filter(|entry| !entry.is_deleted()).count()
}

/// Ajoute toutes les entrées en une seule écriture : un échec n'en ajoute aucune.
///
/// # Errors
///
/// Si l'écriture locale échoue.
pub fn add_entries(
    session: &mut VaultSession,
    contents: Vec<EntryContent>,
    now: Timestamp,
) -> Result<usize, AppError> {
    let count = contents.len();
    session.edit(|vault| {
        for content in contents {
            vault.add(content, now);
        }
    })?;
    Ok(count)
}

/// Écrit le coffre en clair (CSV) dans un nouveau fichier et renvoie le nombre d'entrées.
/// Un fichier existant n'est jamais écrasé.
///
/// # Errors
///
/// Si le fichier existe déjà ou ne peut pas être écrit.
pub fn export_to_file(vault: &Vault, file: &Path) -> Result<usize, AppError> {
    let csv = export_csv(vault).map_err(AppError::Csv)?;
    OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(file)
        .and_then(|mut output| output.write_all(&csv))
        .map_err(|error| {
            AppError::File(format!(
                "écriture de {} impossible : {error}",
                file.display()
            ))
        })?;
    Ok(live_count(vault))
}

#[cfg(test)]
mod tests {
    use std::fs;

    use pswd_core::crypto::KdfParams;
    use pswd_core::ports::PortError;
    use pswd_core::{Credentials, EntryKind, Secret, Session, SessionError, Tags};
    use pswd_infra::FileStore;
    use tempfile::TempDir;

    use super::*;

    fn at(millis: i64) -> Timestamp {
        Timestamp::from_millis(millis)
    }

    fn login(title: &str, username: &str, tags: &[&str]) -> EntryContent {
        EntryContent {
            username: Some(username.to_owned()),
            urls: vec![format!("https://{}.example", title.to_lowercase())],
            tags: Tags::new(tags.iter().copied()),
            ..EntryContent::new(EntryKind::Login, title.to_owned())
        }
    }

    fn sample_vault() -> Vault {
        let mut vault = Vault::new();
        vault.add(login("GitHub", "arthur", &["dev", "perso"]), at(1));
        vault.add(login("banque", "a.fontaine", &["perso"]), at(1));
        vault.add(
            EntryContent::new(EntryKind::Note, "Code Wi-Fi".to_owned()),
            at(1),
        );
        let deleted = vault.add(login("Ancienne", "arthur", &["perso"]), at(1));
        vault.delete(deleted, at(2)).unwrap();
        vault
    }

    fn titles(entries: &[&Entry]) -> Vec<String> {
        entries
            .iter()
            .map(|entry| entry.content().title.clone())
            .collect()
    }

    #[test]
    fn search_ignores_case_and_deleted_entries_and_sorts_by_title() {
        let vault = sample_vault();
        assert_eq!(
            titles(&search(&vault, "", None)),
            ["banque", "Code Wi-Fi", "GitHub"]
        );
        assert_eq!(titles(&search(&vault, "ARTHUR", None)), ["GitHub"]);
        assert_eq!(titles(&search(&vault, "banque.example", None)), ["banque"]);
        assert_eq!(titles(&search(&vault, "DEV", None)), ["GitHub"]);
        assert_eq!(titles(&search(&vault, "wi-fi", None)), ["Code Wi-Fi"]);
        assert_eq!(search(&vault, "absent", None), Vec::<&Entry>::new());
    }

    #[test]
    fn search_can_be_limited_to_a_tag() {
        let vault = sample_vault();
        assert_eq!(
            titles(&search(&vault, "", Some(" Perso "))),
            ["banque", "GitHub"]
        );
        assert_eq!(titles(&search(&vault, "git", Some("perso"))), ["GitHub"]);
        assert_eq!(search(&vault, "", Some("travail")), Vec::<&Entry>::new());
    }

    #[test]
    fn tags_are_counted_on_live_entries() {
        let vault = sample_vault();
        let counts: Vec<(&str, usize)> = tag_counts(&vault).into_iter().collect();
        assert_eq!(counts, [("dev", 1), ("perso", 2)]);
        assert_eq!(live_count(&vault), 3);
    }

    fn session_in(dir: &TempDir) -> VaultSession {
        let local = FileStore::new(dir.path().join("vault.enc"));
        let params = KdfParams::new(8 * 1024, 1, 1).unwrap();
        Session::create(local, &Secret::new("maître".to_owned()), params, at(0))
            .unwrap()
            .0
    }

    #[test]
    fn added_entries_are_saved_together() {
        let dir = TempDir::new().unwrap();
        let mut session = session_in(&dir);
        let contents = vec![login("a", "x", &[]), login("b", "y", &[])];
        assert_eq!(add_entries(&mut session, contents, at(3)), Ok(2));
        let reopened = Session::open(
            FileStore::new(dir.path().join("vault.enc")),
            Credentials::Password(&Secret::new("maître".to_owned())),
        )
        .unwrap();
        assert_eq!(live_count(reopened.vault()), 2);
    }

    #[test]
    fn failed_save_adds_nothing() {
        let dir = TempDir::new().unwrap();
        let mut session = session_in(&dir);
        // Un dossier à la place du fichier temporaire empêche l'écriture.
        fs::create_dir(dir.path().join("vault.enc.tmp")).unwrap();
        let result = add_entries(&mut session, vec![login("a", "x", &[])], at(3));
        assert!(
            matches!(
                result,
                Err(AppError::Session(SessionError::Local(PortError { .. })))
            ),
            "{result:?}"
        );
        assert_eq!(live_count(session.vault()), 0);
    }

    #[test]
    fn export_writes_a_new_csv_file() {
        let dir = TempDir::new().unwrap();
        let file = dir.path().join("export.csv");
        assert_eq!(export_to_file(&sample_vault(), &file), Ok(3));
        let csv = fs::read_to_string(&file).unwrap();
        assert!(csv.starts_with("title,kind,username,password,urls,notes,tags\r\n"));
        assert!(csv.contains("GitHub,login,arthur,"), "{csv}");
        assert!(!csv.contains("Ancienne"), "{csv}");
    }

    #[test]
    fn export_never_overwrites_a_file() {
        let dir = TempDir::new().unwrap();
        let file = dir.path().join("export.csv");
        fs::write(&file, "à garder").unwrap();
        let error = export_to_file(&sample_vault(), &file).unwrap_err();
        assert!(error.to_string().starts_with("écriture de "), "{error}");
        assert_eq!(fs::read_to_string(&file).unwrap(), "à garder");
    }
}
