//! Sauvegarde en un seul fichier `.pswd` : les `vault.enc` de plusieurs coffres, toujours
//! chiffrés, rangés par nom. Sert là où une sauvegarde ne peut pas être un dossier (mobile).

use std::collections::BTreeMap;
use std::fmt;

use crate::{FileError, Timestamp, VaultFile, is_valid_vault_name};

const MAGIC: [u8; 8] = *b"PSWDSAVE";
const FORMAT_VERSION: u8 = 1;
/// Bornes contre un fichier piégé ; largement au-dessus d'un usage réel.
const MAX_BACKUP_VAULTS: usize = 1_000;
/// Taille maximale d'un fichier `vault.enc`, dans une sauvegarde comme sur le serveur.
pub const MAX_VAULT_FILE_LEN: usize = 64 * 1024 * 1024;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BackupFileError {
    Malformed,
    UnsupportedVersion(u8),
    /// Le nom n'est pas cité : il vient peut-être d'un fichier quelconque.
    InvalidName,
    DuplicateName(String),
    TooLarge,
    InvalidVault(String, FileError),
}

impl fmt::Display for BackupFileError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Malformed => f.write_str("fichier de sauvegarde mal formé"),
            Self::UnsupportedVersion(version) => {
                write!(f, "version de sauvegarde non prise en charge : {version}")
            }
            Self::InvalidName => f.write_str("nom de coffre invalide dans la sauvegarde"),
            Self::DuplicateName(name) => write!(f, "coffre {name} présent deux fois"),
            Self::TooLarge => write!(
                f,
                "sauvegarde trop grande ({MAX_BACKUP_VAULTS} coffres de {} Mio au plus)",
                MAX_VAULT_FILE_LEN / (1024 * 1024)
            ),
            Self::InvalidVault(name, error) => write!(f, "coffre {name} : {error}"),
        }
    }
}

impl std::error::Error for BackupFileError {}

/// Contenu d'un fichier `.pswd` : sa date et le fichier chiffré de chaque coffre.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BackupFile<'a> {
    created_at: Timestamp,
    vaults: BTreeMap<&'a str, &'a [u8]>,
}

impl<'a> BackupFile<'a> {
    #[must_use]
    pub fn new(created_at: Timestamp) -> Self {
        Self {
            created_at,
            vaults: BTreeMap::new(),
        }
    }

    /// Ajoute un coffre ; son fichier est vérifié (structure, version) sans aucun secret.
    ///
    /// # Errors
    ///
    /// Si le nom est invalide ou déjà présent, si une borne est dépassée, ou si `file` n'est
    /// pas un fichier de coffre.
    pub fn add(&mut self, name: &'a str, file: &'a [u8]) -> Result<(), BackupFileError> {
        if !is_valid_vault_name(name) {
            return Err(BackupFileError::InvalidName);
        }
        if self.vaults.contains_key(name) {
            return Err(BackupFileError::DuplicateName(name.to_owned()));
        }
        if self.vaults.len() >= MAX_BACKUP_VAULTS || file.len() > MAX_VAULT_FILE_LEN {
            return Err(BackupFileError::TooLarge);
        }
        VaultFile::check_format(file)
            .map_err(|error| BackupFileError::InvalidVault(name.to_owned(), error))?;
        self.vaults.insert(name, file);
        Ok(())
    }

    #[must_use]
    pub fn created_at(&self) -> Timestamp {
        self.created_at
    }

    /// Noms des coffres, triés.
    pub fn names(&self) -> impl Iterator<Item = &'a str> + '_ {
        self.vaults.keys().copied()
    }

    /// Le fichier chiffré du coffre `name`.
    #[must_use]
    pub fn vault(&self, name: &str) -> Option<&'a [u8]> {
        self.vaults.get(name).copied()
    }

    /// Lit un fichier `.pswd` en entier, chaque coffre compris, avant de rendre quoi que ce soit.
    ///
    /// # Errors
    ///
    /// Si le fichier est mal formé, d'une version inconnue, ou contient un coffre refusé par
    /// [`BackupFile::add`].
    pub fn parse(data: &'a [u8]) -> Result<Self, BackupFileError> {
        let mut reader = Reader(data);
        if reader.take(MAGIC.len())? != MAGIC {
            return Err(BackupFileError::Malformed);
        }
        let version = reader.u8()?;
        if version != FORMAT_VERSION {
            return Err(BackupFileError::UnsupportedVersion(version));
        }
        let mut backup = Self::new(Timestamp::from_millis(reader.i64()?));
        let mut previous: Option<&str> = None;
        for _ in 0..reader.u16()? {
            let name_len = reader.u8()?;
            let name = std::str::from_utf8(reader.take(usize::from(name_len))?)
                .map_err(|_| BackupFileError::InvalidName)?;
            let file_len = usize::try_from(reader.u32()?).unwrap_or(usize::MAX);
            if file_len > MAX_VAULT_FILE_LEN {
                return Err(BackupFileError::TooLarge);
            }
            // Noms dans l'ordre : chaque sauvegarde n'a qu'une écriture possible.
            if previous.is_some_and(|previous| previous > name) {
                return Err(BackupFileError::Malformed);
            }
            backup.add(name, reader.take(file_len)?)?;
            previous = Some(name);
        }
        if !reader.0.is_empty() {
            return Err(BackupFileError::Malformed);
        }
        Ok(backup)
    }

    #[must_use]
    pub fn encode(&self) -> Vec<u8> {
        let mut out = Vec::new();
        out.extend_from_slice(&MAGIC);
        out.push(FORMAT_VERSION);
        out.extend_from_slice(&self.created_at.as_millis().to_le_bytes());
        // Les tailles sont bornées par `add` : les conversions ne peuvent pas échouer.
        let count = u16::try_from(self.vaults.len()).unwrap_or(u16::MAX);
        out.extend_from_slice(&count.to_le_bytes());
        for (name, file) in &self.vaults {
            out.push(u8::try_from(name.len()).unwrap_or(u8::MAX));
            out.extend_from_slice(name.as_bytes());
            let file_len = u32::try_from(file.len()).unwrap_or(u32::MAX);
            out.extend_from_slice(&file_len.to_le_bytes());
            out.extend_from_slice(file);
        }
        out
    }
}

struct Reader<'a>(&'a [u8]);

impl<'a> Reader<'a> {
    fn take(&mut self, len: usize) -> Result<&'a [u8], BackupFileError> {
        let (head, tail) = self
            .0
            .split_at_checked(len)
            .ok_or(BackupFileError::Malformed)?;
        self.0 = tail;
        Ok(head)
    }

    fn array<const N: usize>(&mut self) -> Result<[u8; N], BackupFileError> {
        let (head, tail) = self
            .0
            .split_first_chunk()
            .ok_or(BackupFileError::Malformed)?;
        self.0 = tail;
        Ok(*head)
    }

    fn u8(&mut self) -> Result<u8, BackupFileError> {
        self.array().map(u8::from_le_bytes)
    }

    fn u16(&mut self) -> Result<u16, BackupFileError> {
        self.array().map(u16::from_le_bytes)
    }

    fn u32(&mut self) -> Result<u32, BackupFileError> {
        self.array().map(u32::from_le_bytes)
    }

    fn i64(&mut self) -> Result<i64, BackupFileError> {
        self.array().map(i64::from_le_bytes)
    }
}

#[cfg(fuzzing)]
pub mod fuzz {
    use super::BackupFile;

    /// # Panics
    ///
    /// Si une sauvegarde acceptée ne se réencode pas à l'identique : le fuzzer signale un bug.
    pub fn parse_backup(data: &[u8]) {
        if let Ok(backup) = BackupFile::parse(data) {
            assert_eq!(backup.encode(), data);
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::LazyLock;

    use proptest::collection::btree_map;
    use proptest::prelude::*;

    use super::*;
    use crate::crypto::KdfParams;
    use crate::{Secret, Vault};

    /// Un vrai `vault.enc`, chiffré une fois pour tous les tests.
    static VAULT_FILE: LazyLock<Vec<u8>> = LazyLock::new(|| {
        let params = KdfParams::new(8 * 1024, 1, 1).unwrap();
        let password = Secret::new("maître".to_owned());
        let (file, _) = VaultFile::create(&password, params, Timestamp::from_millis(500)).unwrap();
        file.encrypt(&Vault::new()).unwrap()
    });

    /// Fichiers de coffre distincts : le payload chiffré est opaque pour la vérification.
    fn vault_file(suffix: &[u8]) -> Vec<u8> {
        let mut file = VAULT_FILE.clone();
        file.extend_from_slice(suffix);
        file
    }

    fn at(millis: i64) -> Timestamp {
        Timestamp::from_millis(millis)
    }

    #[test]
    fn backup_is_read_back_with_its_vaults_in_order() {
        let (travail, perso) = (vault_file(b"t"), vault_file(b"p"));
        let mut backup = BackupFile::new(at(1_700_000_000_000));
        backup.add("travail", &travail).unwrap();
        backup.add("perso", &perso).unwrap();
        let encoded = backup.encode();
        let read = BackupFile::parse(&encoded).unwrap();
        assert_eq!(read, backup);
        assert_eq!(read.created_at(), at(1_700_000_000_000));
        assert_eq!(read.names().collect::<Vec<_>>(), ["perso", "travail"]);
        assert_eq!(read.vault("perso"), Some(perso.as_slice()));
        assert_eq!(read.vault("autre"), None);
    }

    #[test]
    fn layout_follows_the_specification() {
        let file = vault_file(b"");
        let mut backup = BackupFile::new(at(-2));
        backup.add("ab", &file).unwrap();
        let encoded = backup.encode();
        let mut expected = b"PSWDSAVE\x01".to_vec();
        expected.extend_from_slice(&(-2_i64).to_le_bytes());
        expected.extend_from_slice(&[1, 0, 2, b'a', b'b']);
        expected.extend_from_slice(&u32::try_from(file.len()).unwrap().to_le_bytes());
        expected.extend_from_slice(&file);
        assert_eq!(encoded, expected);
    }

    #[test]
    fn empty_backup_is_valid() {
        let encoded = BackupFile::new(at(0)).encode();
        assert_eq!(encoded.len(), 8 + 1 + 8 + 2);
        assert_eq!(BackupFile::parse(&encoded).unwrap().names().count(), 0);
    }

    #[test]
    fn invalid_vaults_are_refused_when_added() {
        let file = vault_file(b"");
        let mut backup = BackupFile::new(at(0));
        assert_eq!(backup.add("a/b", &file), Err(BackupFileError::InvalidName));
        assert_eq!(
            backup.add("perso", b"pas un coffre"),
            Err(BackupFileError::InvalidVault(
                "perso".to_owned(),
                FileError::Malformed
            ))
        );
        backup.add("perso", &file).unwrap();
        assert_eq!(
            backup.add("perso", &file),
            Err(BackupFileError::DuplicateName("perso".to_owned()))
        );
        assert_eq!(backup.names().collect::<Vec<_>>(), ["perso"]);
    }

    #[test]
    fn sizes_are_bounded() {
        let file = vault_file(b"");
        let names: Vec<String> = (0..=MAX_BACKUP_VAULTS).map(|i| format!("c{i}")).collect();
        let mut backup = BackupFile::new(at(0));
        for name in &names[..MAX_BACKUP_VAULTS] {
            backup.add(name, &file).unwrap();
        }
        assert_eq!(
            backup.add(&names[MAX_BACKUP_VAULTS], &file),
            Err(BackupFileError::TooLarge)
        );

        let mut largest = file.clone();
        largest.resize(MAX_VAULT_FILE_LEN, 0);
        let mut backup = BackupFile::new(at(0));
        backup.add("max", &largest).unwrap();
        let mut too_large = largest.clone();
        too_large.push(0);
        assert_eq!(
            backup.add("trop", &too_large),
            Err(BackupFileError::TooLarge)
        );
    }

    /// Sauvegarde d'un coffre, octets modifiables à la main.
    fn single(name: &str) -> Vec<u8> {
        let file = vault_file(b"");
        let mut backup = BackupFile::new(at(0));
        backup.add(name, &file).unwrap();
        backup.encode()
    }

    /// Position de la taille du fichier du premier coffre (nom « ab »).
    const FILE_LEN_AT: usize = 8 + 1 + 8 + 2 + 1 + 2;

    /// Abîme une sauvegarde valide.
    type Damage = Box<dyn Fn(&mut Vec<u8>)>;

    #[test]
    fn damaged_files_are_refused() {
        let valid = single("ab");
        let cases: [(&str, Damage, BackupFileError); 8] = [
            ("vide", Box::new(Vec::clear), BackupFileError::Malformed),
            (
                "magie",
                Box::new(|f| f[0] = b'X'),
                BackupFileError::Malformed,
            ),
            (
                "version",
                Box::new(|f| f[8] = 2),
                BackupFileError::UnsupportedVersion(2),
            ),
            (
                "tronqué",
                Box::new(|f| f.truncate(f.len() - 1)),
                BackupFileError::Malformed,
            ),
            (
                "en trop",
                Box::new(|f| f.push(0)),
                BackupFileError::Malformed,
            ),
            (
                "nom non UTF-8",
                Box::new(|f| f[20] = 0xFF),
                BackupFileError::InvalidName,
            ),
            (
                "nom invalide",
                Box::new(|f| f[20] = b'/'),
                BackupFileError::InvalidName,
            ),
            (
                "coffre trop grand",
                Box::new(|f| f[FILE_LEN_AT..FILE_LEN_AT + 4].copy_from_slice(&[0xFF; 4])),
                BackupFileError::TooLarge,
            ),
        ];
        for (label, damage, expected) in cases {
            let mut file = valid.clone();
            damage(&mut file);
            assert_eq!(BackupFile::parse(&file), Err(expected), "{label}");
        }
    }

    #[test]
    fn size_at_the_limit_is_not_too_large() {
        let mut file = single("ab");
        let len = u32::try_from(MAX_VAULT_FILE_LEN).unwrap();
        file[FILE_LEN_AT..FILE_LEN_AT + 4].copy_from_slice(&len.to_le_bytes());
        assert_eq!(BackupFile::parse(&file), Err(BackupFileError::Malformed));
    }

    #[test]
    fn damaged_vault_inside_is_refused() {
        let mut file = single("ab");
        file[FILE_LEN_AT + 4] = b'X';
        assert_eq!(
            BackupFile::parse(&file),
            Err(BackupFileError::InvalidVault(
                "ab".to_owned(),
                FileError::Malformed
            ))
        );
    }

    /// Deux coffres « ab » puis « cd », le nom du second remplacé par `second`.
    fn pair(second: [u8; 2]) -> Vec<u8> {
        let (ab, cd) = (vault_file(b"1"), vault_file(b"2"));
        let mut backup = BackupFile::new(at(0));
        backup.add("ab", &ab).unwrap();
        backup.add("cd", &cd).unwrap();
        let mut file = backup.encode();
        let at = FILE_LEN_AT + 4 + ab.len() + 1;
        file[at..at + 2].copy_from_slice(&second);
        file
    }

    #[test]
    fn names_must_be_sorted_and_unique() {
        assert!(BackupFile::parse(&pair(*b"cd")).is_ok());
        assert_eq!(
            BackupFile::parse(&pair(*b"aa")),
            Err(BackupFileError::Malformed)
        );
        assert_eq!(
            BackupFile::parse(&pair(*b"ab")),
            Err(BackupFileError::DuplicateName("ab".to_owned()))
        );
    }

    #[test]
    fn errors_have_explicit_messages() {
        let messages = [
            (
                BackupFileError::Malformed,
                "fichier de sauvegarde mal formé",
            ),
            (
                BackupFileError::UnsupportedVersion(3),
                "version de sauvegarde non prise en charge : 3",
            ),
            (
                BackupFileError::InvalidName,
                "nom de coffre invalide dans la sauvegarde",
            ),
            (
                BackupFileError::DuplicateName("perso".to_owned()),
                "coffre perso présent deux fois",
            ),
            (
                BackupFileError::TooLarge,
                "sauvegarde trop grande (1000 coffres de 64 Mio au plus)",
            ),
            (
                BackupFileError::InvalidVault("perso".to_owned(), FileError::Malformed),
                "coffre perso : fichier de coffre mal formé",
            ),
        ];
        for (error, message) in messages {
            assert_eq!(error.to_string(), message);
        }
    }

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(64))]

        #[test]
        fn any_backup_round_trips(
            created_at in any::<i64>(),
            vaults in btree_map("[A-Za-z0-9_-]{1,64}", proptest::collection::vec(any::<u8>(), 0..64), 0..8),
        ) {
            let files: Vec<(String, Vec<u8>)> = vaults
                .into_iter()
                .filter(|(name, _)| is_valid_vault_name(name))
                .map(|(name, suffix)| (name, vault_file(&suffix)))
                .collect();
            let mut backup = BackupFile::new(at(created_at));
            for (name, file) in &files {
                backup.add(name, file).unwrap();
            }
            let encoded = backup.encode();
            prop_assert_eq!(BackupFile::parse(&encoded), Ok(backup));
        }
    }
}
