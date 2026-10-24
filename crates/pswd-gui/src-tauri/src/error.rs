use pswd_app::AppError;
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
    fn new(code: &'static str, detail: impl Into<String>) -> Self {
        Self {
            code,
            detail: Some(detail.into()),
        }
    }
}

impl From<AppError> for UiError {
    fn from(error: AppError) -> Self {
        match error {
            AppError::Config(ConfigError::Invalid(message)) => Self::new("configInvalid", message),
            AppError::Config(ConfigError::File(error)) => {
                Self::new("configUnreadable", error.to_string())
            }
            // EXTENSION(anglais): les autres erreurs reçoivent leur code au fil des écrans ; en
            // attendant, le message français sert de détail.
            other => Self::new("unexpected", other.to_string()),
        }
    }
}

#[cfg(test)]
mod tests {
    use pswd_core::ports::PortError;

    use super::*;

    #[test]
    fn errors_get_a_stable_code_and_a_detail() {
        let cases = [
            (
                AppError::Config(ConfigError::Invalid("ligne 2".to_owned())),
                "configInvalid",
                "ligne 2",
            ),
            (
                AppError::Config(ConfigError::File(PortError::new("accès refusé"))),
                "configUnreadable",
                "accès refusé",
            ),
            (
                AppError::EmptyPassword,
                "unexpected",
                "le mot de passe maître ne peut pas être vide",
            ),
        ];
        for (error, code, detail) in cases {
            assert_eq!(
                UiError::from(error),
                UiError {
                    code,
                    detail: Some(detail.to_owned())
                }
            );
        }
    }
}
