//! Ports : ce dont `pswd-core` a besoin du monde extérieur, implémenté par `pswd-infra` (ou
//! par une future application mobile).

use std::fmt;
use std::time::Duration;

use crate::Secret;

/// Erreur d'un adaptateur. Son message est affiché tel quel : il ne doit jamais contenir de
/// secret.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PortError {
    message: String,
    removed: bool,
}

impl PortError {
    #[must_use]
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            removed: false,
        }
    }

    /// Le distant a retiré le coffre, ou en a retiré l'utilisateur : il ne faut plus le
    /// republier.
    #[must_use]
    pub fn removed(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            removed: true,
        }
    }

    #[must_use]
    pub fn is_removed(&self) -> bool {
        self.removed
    }
}

impl fmt::Display for PortError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for PortError {}

/// Copie locale du fichier `vault.enc`.
pub trait LocalStore {
    /// `None` si aucun coffre n'existe localement.
    ///
    /// # Errors
    ///
    /// Si la copie locale existe mais ne peut pas être lue.
    fn load(&self) -> Result<Option<Vec<u8>>, PortError>;

    /// Remplace la copie locale d'un seul coup (fichier temporaire puis renommage) : une
    /// interruption ne laisse jamais un fichier à moitié écrit.
    ///
    /// # Errors
    ///
    /// Si l'écriture échoue ; l'ancienne copie reste alors intacte.
    fn save_atomic(&mut self, file: &[u8]) -> Result<(), PortError>;
}

/// Identifiant opaque d'une version du fichier distant, attribué par le distant.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RemoteVersion(pub String);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RemoteFile {
    pub file: Vec<u8>,
    pub version: RemoteVersion,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PushError {
    /// Le distant a changé depuis `expected` : il faut récupérer et fusionner avant de pousser.
    Conflict,
    Failed(PortError),
}

/// Copie distante du coffre.
// EXTENSION(remote-transport): l'adaptateur HTTP de `pswd-server` (étape 3).
pub trait RemoteStore {
    /// `None` si le distant ne contient pas encore de coffre. L'adaptateur limite lui-même la
    /// durée d'attente.
    ///
    /// # Errors
    ///
    /// Si le distant est injoignable : le coffre est alors utilisé hors ligne.
    fn fetch(&mut self) -> Result<Option<RemoteFile>, PortError>;

    /// Publie `file` seulement si le distant est encore à la version `expected` (`None` : le
    /// distant doit être vide).
    ///
    /// # Errors
    ///
    /// [`PushError::Conflict`] si le distant a bougé, [`PushError::Failed`] sinon.
    fn push(&mut self, file: &[u8], expected: Option<&RemoteVersion>) -> Result<(), PushError>;
}

/// Presse-papiers qui s'efface tout seul.
// EXTENSION(mobile): à réimplémenter avec le presse-papiers de la plateforme.
pub trait SecureClipboard {
    /// Copie `value`, puis l'efface après `ttl` si le presse-papiers la contient encore.
    ///
    /// # Errors
    ///
    /// Si le presse-papiers est inaccessible.
    fn copy_secret(&mut self, value: &Secret, ttl: Duration) -> Result<(), PortError>;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn port_error_displays_its_message() {
        assert_eq!(
            PortError::new("serveur injoignable").to_string(),
            "serveur injoignable"
        );
        assert_eq!(
            PortError::removed("coffre retiré").to_string(),
            "coffre retiré"
        );
    }

    #[test]
    fn only_a_removal_says_so() {
        assert!(!PortError::new("serveur injoignable").is_removed());
        assert!(PortError::removed("coffre retiré").is_removed());
        assert_ne!(PortError::new("x"), PortError::removed("x"));
    }
}
