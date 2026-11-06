//! Doublures communes aux tests des modules.

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
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

    fn clear_now(&mut self) {}
}

/// Copies reçues par un presse-papiers factice.
pub type Copies = Arc<Mutex<Vec<(String, Duration)>>>;

/// Presse-papiers qui note les copies et les effacements immédiats, partagés avec le test.
#[derive(Default)]
pub struct FakeClipboard {
    pub copies: Copies,
    pub clears: Arc<AtomicUsize>,
    pub broken: bool,
}

impl SecureClipboard for FakeClipboard {
    fn copy_secret(&mut self, value: &Secret, ttl: Duration) -> Result<(), PortError> {
        if self.broken {
            return Err(PortError::new("presse-papiers indisponible"));
        }
        self.copies
            .lock()
            .unwrap()
            .push((value.expose().to_owned(), ttl));
        Ok(())
    }

    fn clear_now(&mut self) {
        self.clears.fetch_add(1, Ordering::SeqCst);
    }
}
