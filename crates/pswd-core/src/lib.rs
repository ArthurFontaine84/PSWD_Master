#![forbid(unsafe_code)]
#![cfg_attr(coverage_nightly, feature(coverage_attribute))]

pub mod crypto;
mod entry;
mod merge;
mod recovery_key;
mod secret;
mod tags;
mod timestamp;
mod vault;
mod vault_file;

pub use entry::{Entry, EntryContent, EntryKind, EntrySnapshot};
pub use merge::{MergeError, merge};
pub use recovery_key::{RecoveryKey, RecoveryKeyError};
pub use secret::Secret;
pub use tags::Tags;
pub use timestamp::Timestamp;
pub use vault::{Vault, VaultError};
#[cfg(fuzzing)]
pub use vault_file::fuzz;
pub use vault_file::{FileError, VaultFile};
