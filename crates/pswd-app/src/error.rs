use std::fmt;

use pswd_core::ports::PortError;
use pswd_core::{BackupFileError, CsvError, FileError, GeneratorError, SessionError};
use pswd_infra::ConfigError;

use crate::password::{MIN_CHARACTER_KINDS, MIN_PASSWORD_LENGTH};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AppError {
    Config(ConfigError),
    Session(SessionError),
    Port(PortError),
    Csv(CsvError),
    Generator(GeneratorError),
    PasswordTooShort,
    PasswordTooSimple,
    NothingToBackUp,
    InvalidBackup(FileError),
    BackupFile(BackupFileError),
    EmptyBackup,
    NotInBackup(String),
    File(String),
    RandomUnavailable,
    NotConnected,
    AlreadyConnected,
    /// Le coffre n'est pas publié sur le serveur de l'appareil (à publier, détaché ou retiré).
    NotPublished(String),
    /// La copie locale du coffre a des modifications que le serveur n'a pas.
    Unpublished(String),
}

impl fmt::Display for AppError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Config(error) => error.fmt(f),
            Self::Session(error) => error.fmt(f),
            Self::Port(error) => error.fmt(f),
            Self::Csv(error) => error.fmt(f),
            Self::Generator(error) => error.fmt(f),
            Self::PasswordTooShort => write!(
                f,
                "le mot de passe maître doit contenir au moins {MIN_PASSWORD_LENGTH} caractères"
            ),
            Self::PasswordTooSimple => write!(
                f,
                "le mot de passe maître doit mélanger au moins {MIN_CHARACTER_KINDS} types de \
                 caractères parmi : minuscules, majuscules, chiffres, symboles"
            ),
            Self::NothingToBackUp => f.write_str("aucun coffre à sauvegarder : utiliser init"),
            Self::InvalidBackup(error) => write!(f, "fichier de sauvegarde inutilisable : {error}"),
            Self::BackupFile(error) => error.fmt(f),
            Self::EmptyBackup => f.write_str("cette sauvegarde ne contient aucun coffre"),
            Self::NotInBackup(name) => {
                write!(f, "cette sauvegarde ne contient pas de coffre « {name} »")
            }
            Self::File(message) => f.write_str(message),
            Self::RandomUnavailable => f.write_str("générateur aléatoire du système indisponible"),
            Self::NotConnected => f.write_str("cet appareil n'est connecté à aucun serveur"),
            Self::AlreadyConnected => f.write_str(
                "cet appareil est déjà connecté à un serveur : s'en déconnecter d'abord",
            ),
            Self::NotPublished(name) => write!(
                f,
                "le coffre « {name} » n'est pas publié sur le serveur de cet appareil"
            ),
            Self::Unpublished(name) => write!(
                f,
                "le coffre « {name} » a des modifications que le serveur n'a pas : le \
                 synchroniser d'abord"
            ),
        }
    }
}

impl std::error::Error for AppError {}

impl From<ConfigError> for AppError {
    fn from(error: ConfigError) -> Self {
        Self::Config(error)
    }
}

impl From<SessionError> for AppError {
    fn from(error: SessionError) -> Self {
        Self::Session(error)
    }
}

impl From<PortError> for AppError {
    fn from(error: PortError) -> Self {
        Self::Port(error)
    }
}

impl From<GeneratorError> for AppError {
    fn from(error: GeneratorError) -> Self {
        Self::Generator(error)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn errors_have_explicit_messages() {
        let messages = [
            (
                AppError::from(ConfigError::UnknownVault("perso".to_owned())),
                "coffre inconnu : perso",
            ),
            (
                AppError::from(SessionError::NoLocalVault),
                "aucun coffre local : utiliser init",
            ),
            (
                AppError::from(PortError::new("disque plein")),
                "disque plein",
            ),
            (
                AppError::Csv(CsvError::InvalidHeader),
                "en-tête CSV attendu : Titre / Title;Type / Kind;Nom d'utilisateur / Username;\
                Mot de passe / Password;Urls;Notes;Tags",
            ),
            (
                AppError::from(GeneratorError::InvalidLength),
                "la longueur doit être comprise entre 8 et 24",
            ),
            (
                AppError::PasswordTooShort,
                "le mot de passe maître doit contenir au moins 12 caractères",
            ),
            (
                AppError::PasswordTooSimple,
                "le mot de passe maître doit mélanger au moins 3 types de caractères parmi : \
                 minuscules, majuscules, chiffres, symboles",
            ),
            (
                AppError::NothingToBackUp,
                "aucun coffre à sauvegarder : utiliser init",
            ),
            (
                AppError::InvalidBackup(FileError::Malformed),
                "fichier de sauvegarde inutilisable : fichier de coffre mal formé",
            ),
            (
                AppError::BackupFile(BackupFileError::Malformed),
                "fichier de sauvegarde mal formé",
            ),
            (
                AppError::EmptyBackup,
                "cette sauvegarde ne contient aucun coffre",
            ),
            (
                AppError::NotInBackup("perso".to_owned()),
                "cette sauvegarde ne contient pas de coffre « perso »",
            ),
            (
                AppError::File("écriture de a.csv impossible : disque plein".to_owned()),
                "écriture de a.csv impossible : disque plein",
            ),
            (
                AppError::RandomUnavailable,
                "générateur aléatoire du système indisponible",
            ),
            (
                AppError::NotConnected,
                "cet appareil n'est connecté à aucun serveur",
            ),
            (
                AppError::AlreadyConnected,
                "cet appareil est déjà connecté à un serveur : s'en déconnecter d'abord",
            ),
            (
                AppError::NotPublished("famille".to_owned()),
                "le coffre « famille » n'est pas publié sur le serveur de cet appareil",
            ),
            (
                AppError::Unpublished("perso".to_owned()),
                "le coffre « perso » a des modifications que le serveur n'a pas : le \
                 synchroniser d'abord",
            ),
        ];
        for (error, message) in messages {
            assert_eq!(error.to_string(), message);
        }
    }
}
