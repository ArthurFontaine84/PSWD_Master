use std::fmt;
use std::time::Duration;

use pswd_app::{AppError, Vaults};
use pswd_core::crypto::KdfParams;
use pswd_core::ports::{PortError, SecureClipboard};
use pswd_core::{RecoveryKeyError, SessionError, Timestamp, VaultError};
use pswd_infra::ConfigError;

use crate::cli::Command;
use crate::commands::Unlock;
use crate::terminal::Terminal;

/// Erreurs propres au terminal ; les autres viennent des cas d'usage ([`AppError`]).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CliError {
    App(AppError),
    Input(String),
    InputClosed,
    PasswordMismatch,
    EntryNotFound(String),
    AmbiguousEntry(Vec<String>),
    NoPassword(String),
    EmptyTitle,
    UnknownKind(String),
    Vault(VaultError),
    RecoveryKey(RecoveryKeyError),
}

impl fmt::Display for CliError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::App(error) => error.fmt(f),
            Self::Input(message) => write!(f, "saisie impossible : {message}"),
            Self::InputClosed => f.write_str("saisie interrompue"),
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
            Self::EmptyTitle => f.write_str("le titre ne peut pas être vide"),
            Self::UnknownKind(kind) => write!(f, "type inconnu « {kind} » : login ou note"),
            Self::Vault(error) => error.fmt(f),
            Self::RecoveryKey(error) => error.fmt(f),
        }
    }
}

impl std::error::Error for CliError {}

impl From<AppError> for CliError {
    fn from(error: AppError) -> Self {
        Self::App(error)
    }
}

impl From<ConfigError> for CliError {
    fn from(error: ConfigError) -> Self {
        Self::App(error.into())
    }
}

impl From<SessionError> for CliError {
    fn from(error: SessionError) -> Self {
        Self::App(error.into())
    }
}

impl From<PortError> for CliError {
    fn from(error: PortError) -> Self {
        Self::App(error.into())
    }
}

/// Tout ce dont les commandes ont besoin : remplacé par des doublures dans les tests.
pub struct App<T, C> {
    pub terminal: T,
    pub clipboard: C,
    pub vaults: Vaults,
    pub kdf: KdfParams,
    pub clock: fn() -> Timestamp,
    /// Délai de verrouillage imposé ; `None` : celui de la configuration.
    pub auto_lock: Option<Duration>,
}

impl<T: Terminal, C: SecureClipboard> App<T, C> {
    /// # Errors
    ///
    /// Toute erreur de la commande, à afficher à l'utilisateur.
    pub fn run(&mut self, command: Command) -> Result<(), CliError> {
        match command {
            Command::Init { name } => self.init(&name),
            Command::Open { name } => self.open(&name),
            Command::Passwd { name } => self.change_password(&name, Unlock::Password),
            Command::Recover { name } => self.change_password(&name, Unlock::RecoveryKey),
            Command::Import { name, file } => self.import(&name, &file),
            Command::Delete { name } => self.delete(&name),
            Command::Backup { destination, vault } => self.backup(&destination, vault.as_deref()),
            Command::Restore { backup, name } => self.restore(&backup, name.as_deref()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn errors_have_explicit_messages() {
        let messages = [
            (
                CliError::from(ConfigError::UnknownVault("perso".to_owned())),
                "coffre inconnu : perso",
            ),
            (
                CliError::from(SessionError::NoLocalVault),
                "aucun coffre local : utiliser init",
            ),
            (
                CliError::from(PortError::new("presse-papiers indisponible")),
                "presse-papiers indisponible",
            ),
            (
                CliError::from(AppError::PasswordTooShort),
                "le mot de passe maître doit contenir au moins 12 caractères",
            ),
            (
                CliError::Input("console absente".to_owned()),
                "saisie impossible : console absente",
            ),
            (CliError::InputClosed, "saisie interrompue"),
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
        ];
        for (error, message) in messages {
            assert_eq!(error.to_string(), message);
        }
    }
}
