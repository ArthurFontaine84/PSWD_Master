use std::fmt;

use pswd_core::ports::PortError;
use pswd_core::{CsvError, FileError, GeneratorError, SessionError};
use pswd_infra::ConfigError;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AppError {
    Config(ConfigError),
    Session(SessionError),
    Port(PortError),
    Csv(CsvError),
    Generator(GeneratorError),
    EmptyPassword,
    NothingToBackUp,
    InvalidBackup(FileError),
    EmptyBackup,
    NotInBackup(String),
    File(String),
    RandomUnavailable,
}

impl fmt::Display for AppError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Config(error) => error.fmt(f),
            Self::Session(error) => error.fmt(f),
            Self::Port(error) => error.fmt(f),
            Self::Csv(error) => error.fmt(f),
            Self::Generator(error) => error.fmt(f),
            Self::EmptyPassword => f.write_str("le mot de passe maître ne peut pas être vide"),
            Self::NothingToBackUp => f.write_str("aucun coffre à sauvegarder : utiliser init"),
            Self::InvalidBackup(error) => write!(f, "fichier de sauvegarde inutilisable : {error}"),
            Self::EmptyBackup => f.write_str("cette sauvegarde ne contient aucun coffre"),
            Self::NotInBackup(name) => {
                write!(f, "cette sauvegarde ne contient pas de coffre « {name} »")
            }
            Self::File(message) => f.write_str(message),
            Self::RandomUnavailable => f.write_str("générateur aléatoire du système indisponible"),
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
                "en-tête CSV attendu : title,kind,username,password,urls,notes,tags",
            ),
            (
                AppError::from(GeneratorError::InvalidLength),
                "la longueur doit être comprise entre 8 et 128",
            ),
            (
                AppError::EmptyPassword,
                "le mot de passe maître ne peut pas être vide",
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
        ];
        for (error, message) in messages {
            assert_eq!(error.to_string(), message);
        }
    }
}
