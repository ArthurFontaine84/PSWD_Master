use pswd_app::AppError;
use pswd_core::{CsvError, FileError, GeneratorError, LineProblem, SessionError, VaultError};
use pswd_infra::ConfigError;
use serde::Serialize;

/// Erreur transmise à l'interface : un code stable, traduit côté interface, et un détail
/// facultatif (nom de coffre, message système). Jamais de secret.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct UiError {
    code: &'static str,
    detail: Option<String>,
}

impl UiError {
    pub fn new(code: &'static str, detail: impl Into<String>) -> Self {
        Self {
            code,
            detail: Some(detail.into()),
        }
    }

    pub fn code(code: &'static str) -> Self {
        Self { code, detail: None }
    }

    /// Aucun coffre ouvert : il a été verrouillé, à la main ou par inactivité.
    pub fn vault_locked() -> Self {
        Self::code("vaultLocked")
    }
}

impl From<AppError> for UiError {
    fn from(error: AppError) -> Self {
        match error {
            AppError::Config(ConfigError::Invalid(message)) => Self::new("configInvalid", message),
            AppError::Config(ConfigError::File(error)) => {
                Self::new("configUnreadable", error.to_string())
            }
            AppError::Config(ConfigError::UnknownVault(name)) => Self::new("unknownVault", name),
            AppError::Config(ConfigError::VaultExists(name)) => Self::new("vaultExists", name),
            AppError::Config(ConfigError::InvalidVaultName(name)) => {
                Self::new("invalidVaultName", name)
            }
            AppError::PasswordTooShort => Self::code("passwordTooShort"),
            AppError::PasswordTooSimple => Self::code("passwordTooSimple"),
            AppError::Session(SessionError::File(FileError::WrongPassword)) => {
                Self::code("wrongPassword")
            }
            AppError::Session(SessionError::File(FileError::WrongRecoveryKey)) => {
                Self::code("wrongRecoveryKey")
            }
            AppError::Session(SessionError::LocalVaultExists) => Self::code("vaultFolderExists"),
            AppError::Session(SessionError::File(error)) => {
                Self::new("vaultUnreadable", error.to_string())
            }
            AppError::Session(SessionError::Local(error)) => {
                Self::new("saveFailed", error.to_string())
            }
            AppError::Generator(GeneratorError::InvalidLength) => Self::code("invalidLength"),
            AppError::Csv(CsvError::InvalidHeader) => Self::code("csvInvalidHeader"),
            AppError::Csv(CsvError::InvalidLine { line, problem }) => {
                let code = match problem {
                    LineProblem::FieldCount => "csvFieldCount",
                    LineProblem::UnknownKind => "csvUnknownKind",
                    LineProblem::InvalidUtf8 => "csvInvalidUtf8",
                };
                Self::new(code, line.to_string())
            }
            AppError::NothingToBackUp => Self::code("nothingToBackUp"),
            AppError::EmptyBackup => Self::code("emptyBackup"),
            AppError::NotInBackup(name) => Self::new("notInBackup", name),
            AppError::InvalidBackup(error) => Self::new("invalidBackup", error.to_string()),
            AppError::BackupFile(error) => Self::new("invalidBackup", error.to_string()),
            AppError::Port(error) => Self::new("ioError", error.to_string()),
            AppError::File(message) => Self::new("ioError", message),
            // EXTENSION(anglais): les autres erreurs reçoivent leur code au fil des écrans ; en
            // attendant, le message français sert de détail.
            other => Self::new("unexpected", other.to_string()),
        }
    }
}

impl From<SessionError> for UiError {
    fn from(error: SessionError) -> Self {
        AppError::Session(error).into()
    }
}

impl From<VaultError> for UiError {
    fn from(error: VaultError) -> Self {
        match error {
            VaultError::EntryNotFound(id) => Self::new("entryNotFound", id.to_string()),
        }
    }
}

#[cfg(test)]
mod tests {
    use pswd_core::BackupFileError;
    use pswd_core::ports::PortError;

    use super::*;

    fn check(cases: Vec<(AppError, &'static str, Option<&str>)>) {
        for (error, code, detail) in cases {
            assert_eq!(
                UiError::from(error),
                UiError {
                    code,
                    detail: detail.map(str::to_owned)
                }
            );
        }
    }

    #[test]
    fn vault_errors_get_a_stable_code_and_a_detail() {
        check(vec![
            (
                AppError::Config(ConfigError::Invalid("ligne 2".to_owned())),
                "configInvalid",
                Some("ligne 2"),
            ),
            (
                AppError::Config(ConfigError::File(PortError::new("accès refusé"))),
                "configUnreadable",
                Some("accès refusé"),
            ),
            (
                AppError::Config(ConfigError::UnknownVault("perso".to_owned())),
                "unknownVault",
                Some("perso"),
            ),
            (
                AppError::Config(ConfigError::VaultExists("perso".to_owned())),
                "vaultExists",
                Some("perso"),
            ),
            (
                AppError::Config(ConfigError::InvalidVaultName("a/b".to_owned())),
                "invalidVaultName",
                Some("a/b"),
            ),
            (AppError::PasswordTooShort, "passwordTooShort", None),
            (AppError::PasswordTooSimple, "passwordTooSimple", None),
            (
                AppError::Session(SessionError::File(FileError::WrongPassword)),
                "wrongPassword",
                None,
            ),
            (
                AppError::Session(SessionError::LocalVaultExists),
                "vaultFolderExists",
                None,
            ),
            (
                AppError::Session(SessionError::File(FileError::Corrupted)),
                "vaultUnreadable",
                Some("coffre altéré ou corrompu"),
            ),
            (
                AppError::Session(SessionError::Local(PortError::new("disque plein"))),
                "saveFailed",
                Some("disque plein"),
            ),
            (
                AppError::Generator(GeneratorError::InvalidLength),
                "invalidLength",
                None,
            ),
        ]);
    }

    #[test]
    fn file_errors_get_a_stable_code_and_a_detail() {
        check(vec![
            (
                AppError::Session(SessionError::File(FileError::WrongRecoveryKey)),
                "wrongRecoveryKey",
                None,
            ),
            (
                AppError::Csv(CsvError::InvalidHeader),
                "csvInvalidHeader",
                None,
            ),
            (
                AppError::Csv(CsvError::InvalidLine {
                    line: 3,
                    problem: LineProblem::FieldCount,
                }),
                "csvFieldCount",
                Some("3"),
            ),
            (
                AppError::Csv(CsvError::InvalidLine {
                    line: 4,
                    problem: LineProblem::UnknownKind,
                }),
                "csvUnknownKind",
                Some("4"),
            ),
            (
                AppError::Csv(CsvError::InvalidLine {
                    line: 5,
                    problem: LineProblem::InvalidUtf8,
                }),
                "csvInvalidUtf8",
                Some("5"),
            ),
            (AppError::NothingToBackUp, "nothingToBackUp", None),
            (AppError::EmptyBackup, "emptyBackup", None),
            (
                AppError::NotInBackup("perso".to_owned()),
                "notInBackup",
                Some("perso"),
            ),
            (
                AppError::InvalidBackup(FileError::Malformed),
                "invalidBackup",
                Some("fichier de coffre mal formé"),
            ),
            (
                AppError::BackupFile(BackupFileError::Malformed),
                "invalidBackup",
                Some("fichier de sauvegarde mal formé"),
            ),
            (
                AppError::Port(PortError::new("disque absent")),
                "ioError",
                Some("disque absent"),
            ),
            (
                AppError::File("coffre introuvable".to_owned()),
                "ioError",
                Some("coffre introuvable"),
            ),
            (
                AppError::Csv(CsvError::Internal),
                "unexpected",
                Some("erreur interne de lecture CSV"),
            ),
        ]);
    }

    #[test]
    fn missing_entries_and_session_errors_are_converted() {
        let id = uuid::Uuid::nil();
        assert_eq!(
            UiError::from(VaultError::EntryNotFound(id)),
            UiError::new("entryNotFound", id.to_string())
        );
        assert_eq!(
            UiError::from(SessionError::LocalVaultExists),
            UiError::code("vaultFolderExists")
        );
    }
}
