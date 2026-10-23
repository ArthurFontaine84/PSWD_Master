use std::fmt;
use std::time::{SystemTime, UNIX_EPOCH};

use pswd_core::crypto::KdfParams;
use pswd_core::ports::{PortError, SecretStore, SecureClipboard};
use pswd_core::{
    CsvError, GeneratorError, RecoveryKeyError, SessionError, SyncStatus, Timestamp, VaultError,
};
use pswd_infra::{ConfigError, Locations};

use crate::cli::Command;
use crate::commands::Unlock;
use crate::terminal::Terminal;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CliError {
    Config(ConfigError),
    Session(SessionError),
    Port(PortError),
    Input(String),
    InputClosed,
    EmptyPassword,
    PasswordMismatch,
    EntryNotFound(String),
    AmbiguousEntry(Vec<String>),
    NoPassword(String),
    Generator(GeneratorError),
    RandomUnavailable,
    EmptyTitle,
    UnknownKind(String),
    Vault(VaultError),
    RecoveryKey(RecoveryKeyError),
    Csv(CsvError),
    File(String),
}

impl fmt::Display for CliError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Config(error) => error.fmt(f),
            Self::Session(error) => error.fmt(f),
            Self::Port(error) => error.fmt(f),
            Self::Input(message) => write!(f, "saisie impossible : {message}"),
            Self::InputClosed => f.write_str("saisie interrompue"),
            Self::EmptyPassword => f.write_str("le mot de passe maître ne peut pas être vide"),
            Self::PasswordMismatch => f.write_str("les deux saisies ne correspondent pas"),
            Self::EntryNotFound(reference) => {
                write!(f, "aucune entrée ne correspond à « {reference} »")
            }
            Self::AmbiguousEntry(candidates) => write!(
                f,
                "plusieurs entrées correspondent : {} ; préciser l'identifiant",
                candidates.join(", ")
            ),
            Self::NoPassword(title) => write!(f, "l'entrée « {title} » n'a pas de mot de passe"),
            Self::Generator(error) => error.fmt(f),
            Self::RandomUnavailable => f.write_str("générateur aléatoire du système indisponible"),
            Self::EmptyTitle => f.write_str("le titre ne peut pas être vide"),
            Self::UnknownKind(kind) => write!(f, "type inconnu « {kind} » : login ou note"),
            Self::Vault(error) => error.fmt(f),
            Self::RecoveryKey(error) => error.fmt(f),
            Self::Csv(error) => error.fmt(f),
            Self::File(message) => f.write_str(message),
        }
    }
}

impl std::error::Error for CliError {}

impl From<ConfigError> for CliError {
    fn from(error: ConfigError) -> Self {
        Self::Config(error)
    }
}

impl From<SessionError> for CliError {
    fn from(error: SessionError) -> Self {
        Self::Session(error)
    }
}

impl From<PortError> for CliError {
    fn from(error: PortError) -> Self {
        Self::Port(error)
    }
}

impl From<GeneratorError> for CliError {
    fn from(error: GeneratorError) -> Self {
        Self::Generator(error)
    }
}

/// Tout ce dont les commandes ont besoin : remplacé par des doublures dans les tests.
pub struct App<T, K, C> {
    pub terminal: T,
    pub keyring: K,
    pub clipboard: C,
    pub locations: Locations,
    pub kdf: KdfParams,
    pub clock: fn() -> Timestamp,
}

impl<T: Terminal, K: SecretStore, C: SecureClipboard> App<T, K, C> {
    /// # Errors
    ///
    /// Toute erreur de la commande, à afficher à l'utilisateur.
    pub fn run(&mut self, command: Command) -> Result<(), CliError> {
        match command {
            Command::Init { name } => self.init(&name),
            Command::Join { name, url } => self.join(&name, &url),
            Command::Open { name } => self.open(name.as_deref()),
            Command::Sync { name } => self.sync(name.as_deref()),
            Command::Passwd { name } => self.change_password(name.as_deref(), Unlock::Password),
            Command::Recover { name } => self.change_password(name.as_deref(), Unlock::RecoveryKey),
            Command::Import { file, vault } => self.import(&file, vault.as_deref()),
            Command::Export { file, vault } => self.export(&file, vault.as_deref()),
        }
    }
}

/// Heure courante : `pswd-core` ne lit jamais l'horloge lui-même.
pub fn system_clock() -> Timestamp {
    let millis = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |elapsed| {
            i64::try_from(elapsed.as_millis()).unwrap_or(i64::MAX)
        });
    Timestamp::from_millis(millis)
}

/// Phrase décrivant l'état de la synchronisation.
pub fn describe(status: &SyncStatus) -> String {
    match status {
        SyncStatus::Synced => "Synchronisé avec le dépôt distant.".to_owned(),
        SyncStatus::Offline(error) => format!(
            "⚠ Hors ligne ({error}) : les modifications sont enregistrées localement et seront \
             publiées à la prochaine synchronisation."
        ),
        SyncStatus::Conflict => "⚠ Le dépôt distant change sans cesse : les modifications \
                                 seront publiées à la prochaine synchronisation."
            .to_owned(),
        SyncStatus::Rejected(error) => format!(
            "⚠ Fichier distant refusé ({error}) : il n'a pas été modifié, le coffre local reste \
             utilisable."
        ),
    }
}

#[cfg(test)]
mod tests {
    use pswd_core::FileError;

    use super::*;

    #[test]
    fn system_clock_gives_the_current_time() {
        // 1ᵉʳ janvier 2026 en millisecondes.
        assert!(system_clock() > Timestamp::from_millis(1_767_225_600_000));
    }

    #[test]
    fn every_status_is_described() {
        let offline = describe(&SyncStatus::Offline(PortError::new("réseau coupé")));
        assert!(offline.contains("réseau coupé"), "{offline}");
        assert_eq!(
            describe(&SyncStatus::Synced),
            "Synchronisé avec le dépôt distant."
        );
        assert!(describe(&SyncStatus::Conflict).starts_with("⚠ Le dépôt distant change"));
        let rejected = describe(&SyncStatus::Rejected(FileError::Corrupted));
        assert!(rejected.contains("coffre altéré ou corrompu"), "{rejected}");
    }

    #[test]
    fn errors_have_explicit_messages() {
        let messages = [
            (
                CliError::Config(ConfigError::NoDefaultVault),
                "aucun coffre par défaut : préciser son nom ou utiliser init",
            ),
            (
                CliError::Session(SessionError::RemoteEmpty),
                "le dépôt distant ne contient aucun coffre : utiliser init",
            ),
            (
                CliError::Port(PortError::new("trousseau : verrouillé")),
                "trousseau : verrouillé",
            ),
            (
                CliError::Input("console absente".to_owned()),
                "saisie impossible : console absente",
            ),
            (CliError::InputClosed, "saisie interrompue"),
            (
                CliError::EmptyPassword,
                "le mot de passe maître ne peut pas être vide",
            ),
            (
                CliError::PasswordMismatch,
                "les deux saisies ne correspondent pas",
            ),
            (
                CliError::EntryNotFound("gmail".to_owned()),
                "aucune entrée ne correspond à « gmail »",
            ),
            (
                CliError::AmbiguousEntry(vec!["a (1234)".to_owned(), "a (5678)".to_owned()]),
                "plusieurs entrées correspondent : a (1234), a (5678) ; préciser l'identifiant",
            ),
            (
                CliError::NoPassword("Wi-Fi".to_owned()),
                "l'entrée « Wi-Fi » n'a pas de mot de passe",
            ),
            (
                CliError::Generator(GeneratorError::InvalidLength),
                "la longueur doit être comprise entre 8 et 128",
            ),
            (
                CliError::RandomUnavailable,
                "générateur aléatoire du système indisponible",
            ),
            (CliError::EmptyTitle, "le titre ne peut pas être vide"),
            (
                CliError::UnknownKind("carte".to_owned()),
                "type inconnu « carte » : login ou note",
            ),
            (
                CliError::Vault(VaultError::EntryNotFound(uuid::Uuid::nil())),
                "entrée introuvable : 00000000-0000-0000-0000-000000000000",
            ),
            (
                CliError::RecoveryKey(RecoveryKeyError::WrongLength),
                "la clé de récupération doit compter 52 caractères",
            ),
            (
                CliError::Csv(CsvError::InvalidHeader),
                "en-tête CSV attendu : title,kind,username,password,urls,notes,tags",
            ),
            (
                CliError::File("écriture de a.csv impossible : disque plein".to_owned()),
                "écriture de a.csv impossible : disque plein",
            ),
        ];
        for (error, message) in messages {
            assert_eq!(error.to_string(), message);
        }
    }
}
