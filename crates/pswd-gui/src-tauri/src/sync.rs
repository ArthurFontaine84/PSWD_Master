//! Résultat de la synchro du coffre ouvert, tel que l'interface le reçoit : jamais de secret.

use pswd_app::{AppError, SyncOutcome};
use pswd_core::SyncStatus;
use serde::Serialize;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum SyncState {
    Synced,
    NotConnected,
    /// Lié à un autre serveur : utilisé en local.
    Detached,
    /// Le serveur l'a retiré, ou en a retiré le compte : gardé en local, plus synchronisé.
    Removed,
    Offline,
    /// Le coffre a changé sur le serveur à chaque essai.
    Conflict,
    /// Copie du serveur inutilisable : elle n'est pas remplacée.
    Rejected,
    /// Erreur locale (configuration, écriture).
    Failed,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SyncReport {
    pub vault: String,
    pub state: SyncState,
    /// Message de l'erreur, en français.
    pub detail: Option<String>,
    /// La synchro a changé le contenu du coffre : l'interface le relit.
    pub changed: bool,
}

impl SyncReport {
    pub fn new(vault: String, outcome: Result<SyncOutcome, AppError>, changed: bool) -> Self {
        let (state, detail) = match outcome {
            Ok(SyncOutcome::NotConnected) => (SyncState::NotConnected, None),
            Ok(SyncOutcome::Detached) => (SyncState::Detached, None),
            Ok(SyncOutcome::Removed | SyncOutcome::Done(SyncStatus::Removed)) => {
                (SyncState::Removed, None)
            }
            Ok(SyncOutcome::Done(SyncStatus::Synced)) => (SyncState::Synced, None),
            Ok(SyncOutcome::Done(SyncStatus::Offline(error))) => {
                (SyncState::Offline, Some(error.to_string()))
            }
            Ok(SyncOutcome::Done(SyncStatus::Conflict)) => (SyncState::Conflict, None),
            Ok(SyncOutcome::Done(SyncStatus::Rejected(error))) => {
                (SyncState::Rejected, Some(error.to_string()))
            }
            Err(error) => (SyncState::Failed, Some(error.to_string())),
        };
        Self {
            vault,
            state,
            detail,
            changed,
        }
    }
}

#[cfg(test)]
mod tests {
    use pswd_core::FileError;
    use pswd_core::ports::PortError;
    use serde_json::json;

    use super::*;

    fn sent(outcome: Result<SyncOutcome, AppError>) -> serde_json::Value {
        serde_json::to_value(SyncReport::new("perso".to_owned(), outcome, false)).unwrap()
    }

    #[test]
    fn every_outcome_reaches_the_interface() {
        let report = |state: &str, detail: Option<&str>| json!({ "vault": "perso", "state": state, "detail": detail, "changed": false });
        let offline = PortError::new("serveur injoignable");
        let cases = [
            (Ok(SyncOutcome::NotConnected), report("notConnected", None)),
            (Ok(SyncOutcome::Detached), report("detached", None)),
            (Ok(SyncOutcome::Removed), report("removed", None)),
            (
                Ok(SyncOutcome::Done(SyncStatus::Removed)),
                report("removed", None),
            ),
            (
                Ok(SyncOutcome::Done(SyncStatus::Synced)),
                report("synced", None),
            ),
            (
                Ok(SyncOutcome::Done(SyncStatus::Offline(offline))),
                report("offline", Some("serveur injoignable")),
            ),
            (
                Ok(SyncOutcome::Done(SyncStatus::Conflict)),
                report("conflict", None),
            ),
            (
                Ok(SyncOutcome::Done(SyncStatus::Rejected(
                    FileError::Corrupted,
                ))),
                report("rejected", Some("coffre altéré ou corrompu")),
            ),
            (
                Err(AppError::NotConnected),
                report("failed", Some(&AppError::NotConnected.to_string())),
            ),
        ];
        for (outcome, expected) in cases {
            assert_eq!(sent(outcome), expected);
        }
    }
}
