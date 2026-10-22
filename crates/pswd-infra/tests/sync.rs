//! Synchronisation de bout en bout : deux appareils partagent un dépôt Git nu local.

// Fichier de test : un échec doit faire paniquer le test, y compris dans les fonctions d'aide.
#![allow(clippy::unwrap_used)]

use pswd_core::crypto::KdfParams;
use pswd_core::ports::LocalStore;
use pswd_core::{
    Credentials, EntryContent, EntryKind, Secret, Session, SessionError, SyncStatus, Timestamp,
    Vault, VaultFile,
};
use pswd_infra::{FileStore, GitRemote};
use tempfile::TempDir;

type Device = Session<FileStore, GitRemote>;

struct Setup {
    dir: TempDir,
    url: String,
}

impl Setup {
    fn new() -> Self {
        let dir = TempDir::new().unwrap();
        let remote = dir.path().join("remote.git");
        git2::Repository::init_bare(&remote).unwrap();
        let url = remote.to_str().unwrap().to_owned();
        Self { dir, url }
    }

    fn stores(&self, device: &str) -> (FileStore, GitRemote) {
        let root = self.dir.path().join(device);
        let local = FileStore::new(root.join("vault.enc"));
        let remote = GitRemote::open(&root.join("repo.git"), &self.url, None).unwrap();
        (local, remote)
    }

    fn create(&self, device: &str) -> Device {
        let (local, remote) = self.stores(device);
        Session::create(local, remote, &secret("maître"), fast_params(), at(0))
            .unwrap()
            .0
    }

    fn join(&self, device: &str) -> Device {
        let (local, remote) = self.stores(device);
        Session::join(local, remote, Credentials::Password(&secret("maître"))).unwrap()
    }

    fn open(&self, device: &str, password: &str) -> Result<Device, SessionError> {
        let (local, remote) = self.stores(device);
        Session::open(local, remote, Credentials::Password(&secret(password)))
    }

    fn local_file(&self, device: &str) -> Vec<u8> {
        FileStore::new(self.dir.path().join(device).join("vault.enc"))
            .load()
            .unwrap()
            .unwrap()
    }
}

fn secret(value: &str) -> Secret {
    Secret::new(value.to_owned())
}

fn fast_params() -> KdfParams {
    KdfParams::new(8 * 1024, 1, 1).unwrap()
}

fn at(millis: i64) -> Timestamp {
    Timestamp::from_millis(millis)
}

fn add(device: &mut Device, title: &str) {
    device
        .edit(|vault| vault.add(EntryContent::new(EntryKind::Note, title.to_owned()), at(1)))
        .unwrap();
    assert_eq!(device.status(), &SyncStatus::Synced);
}

fn titles(vault: &Vault) -> Vec<String> {
    let mut titles: Vec<String> = vault
        .entries()
        .map(|entry| entry.content().title.clone())
        .collect();
    titles.sort_unstable();
    titles
}

#[test]
fn changes_travel_between_two_devices() {
    let setup = Setup::new();
    let mut a = setup.create("a");
    let mut b = setup.join("b");
    assert_eq!(b.vault().id(), a.vault().id());

    add(&mut a, "depuis A");
    b.sync().unwrap();
    assert_eq!(titles(b.vault()), ["depuis A"]);

    add(&mut b, "depuis B");
    a.sync().unwrap();
    assert_eq!(titles(a.vault()), ["depuis A", "depuis B"]);
}

#[test]
fn concurrent_edits_are_merged() {
    let setup = Setup::new();
    let mut a = setup.create("a");
    let mut b = setup.join("b");
    add(&mut a, "A");
    // B n'a pas vu la modification de A : sa publication est refusée, puis fusionnée.
    add(&mut b, "B");
    assert_eq!(titles(b.vault()), ["A", "B"]);
    a.sync().unwrap();
    assert_eq!(titles(a.vault()), ["A", "B"]);
}

#[test]
fn password_change_reaches_the_other_device() {
    let setup = Setup::new();
    let mut a = setup.create("a");
    let mut b = setup.join("b");
    a.change_password(&secret("nouveau"), fast_params(), at(5))
        .unwrap();
    b.sync().unwrap();
    drop(b);
    assert_eq!(
        setup.open("b", "maître").err(),
        Some(SessionError::File(pswd_core::FileError::WrongPassword))
    );
    assert!(setup.open("b", "nouveau").is_ok());
}

#[test]
fn reopened_vault_keeps_its_entries() {
    let setup = Setup::new();
    let mut a = setup.create("a");
    add(&mut a, "gardée");
    drop(a);
    let reopened = setup.open("a", "maître").unwrap();
    assert_eq!(titles(reopened.vault()), ["gardée"]);
    assert_eq!(reopened.status(), &SyncStatus::Synced);
}

#[test]
fn second_init_on_the_same_repository_is_refused() {
    let setup = Setup::new();
    setup.create("a");
    let (local, remote) = setup.stores("b");
    assert_eq!(
        Session::create(local, remote, &secret("autre"), fast_params(), at(0)).err(),
        Some(SessionError::RemoteVaultExists)
    );
}

#[test]
fn local_copy_is_an_encrypted_vault_file() {
    let setup = Setup::new();
    let mut a = setup.create("a");
    add(&mut a, "secrète");
    let file = setup.local_file("a");
    assert!(file.starts_with(b"PSWD"));
    assert!(!file.windows(7).any(|window| window == b"secr\xC3\xA8te"));
    let (_, vault) = VaultFile::unlock_with_password(&file, &secret("maître")).unwrap();
    assert_eq!(titles(&vault), ["secrète"]);
}

#[test]
fn unreachable_remote_leaves_the_vault_usable() {
    let setup = Setup::new();
    let mut a = setup.create("a");
    add(&mut a, "avant");
    drop(a);
    let local = FileStore::new(setup.dir.path().join("a").join("vault.enc"));
    let missing = setup.dir.path().join("disparu.git");
    let remote = GitRemote::open(
        &setup.dir.path().join("a").join("repo.git"),
        missing.to_str().unwrap(),
        None,
    )
    .unwrap();
    let mut a = Session::open(local, remote, Credentials::Password(&secret("maître"))).unwrap();
    assert!(matches!(a.status(), SyncStatus::Offline(_)));
    a.edit(|vault| {
        vault.add(
            EntryContent::new(EntryKind::Note, "hors ligne".to_owned()),
            at(2),
        )
    })
    .unwrap();
    assert!(a.has_pending_changes());
    assert_eq!(titles(a.vault()), ["avant", "hors ligne"]);
}
