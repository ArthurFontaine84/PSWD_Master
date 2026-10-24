//! Cas d'usage partagés par la CLI et l'interface graphique. Aucune fonction ne pose de
//! question à l'utilisateur : chacune reçoit ce qu'il faut et renvoie un résultat.

#![forbid(unsafe_code)]
#![cfg_attr(coverage_nightly, feature(coverage_attribute))]

mod entries;
mod error;
mod idle_lock;
mod random;
mod utc;
mod vaults;

use pswd_core::Session;
use pswd_infra::FileStore;

pub use entries::{add_entries, export_to_file, live_count, search, tag_counts};
pub use error::AppError;
pub use idle_lock::IdleLock;
pub use random::generate;
pub use utc::{backup_stamp, format_timestamp, now};
pub use vaults::Vaults;

/// Coffre ouvert, enregistré dans un fichier local.
pub type VaultSession = Session<FileStore>;
