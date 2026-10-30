//! Doublures communes aux tests des modules.

use std::time::Duration;

use pswd_core::Secret;
use pswd_core::ports::{PortError, SecureClipboard};

use crate::state::Notifier;
use crate::sync::SyncReport;

/// Ni presse-papiers ni interface à prévenir.
pub struct Silent;

// Exclu de la couverture : ces méthodes ne font rien, il n'y a rien à vérifier ; les tests qui
// les utilisent ne copient rien et ne verrouillent pas par inactivité.
#[cfg_attr(coverage_nightly, coverage(off))]
impl Notifier for Silent {
    fn vault_locked(&self) {}
    fn vault_synced(&self, _: &SyncReport) {}
}

#[cfg_attr(coverage_nightly, coverage(off))]
impl SecureClipboard for Silent {
    fn copy_secret(&mut self, _: &Secret, _: Duration) -> Result<(), PortError> {
        Ok(())
    }
}
