//! Cas d'usage partagés par la CLI et l'interface graphique. Aucune fonction ne pose de
//! question à l'utilisateur : chacune reçoit ce qu'il faut et renvoie un résultat.

#![forbid(unsafe_code)]
#![cfg_attr(coverage_nightly, feature(coverage_attribute))]

mod admin;
mod entries;
mod error;
mod idle_lock;
mod password;
mod random;
mod server;
mod sharing;
mod utc;
mod vaults;

use pswd_core::Session;
use pswd_infra::FileStore;

pub use admin::ServerBackup;
pub use entries::{add_entries, live_count, search, tag_counts};
pub use error::AppError;
pub use idle_lock::IdleLock;
pub use password::{MIN_CHARACTER_KINDS, MIN_PASSWORD_LENGTH, check_master_password};
pub use random::generate;
pub use server::{
    DEFAULT_PORT, Disconnected, Downloads, KeptReason, SyncOutcome, VaultResults, VaultState,
};
pub use utc::{backup_file_name, format_timestamp, now, server_backup_file_name};
pub use vaults::Vaults;

/// Coffre ouvert, enregistré dans un fichier local.
pub type VaultSession = Session<FileStore>;
