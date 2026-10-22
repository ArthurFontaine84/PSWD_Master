#![forbid(unsafe_code)]
#![cfg_attr(coverage_nightly, feature(coverage_attribute))]

mod buffer;
pub mod crypto;
mod csv_format;
mod entry;
mod generator;
mod merge;
pub mod ports;
mod recovery_key;
mod secret;
mod session;
mod tags;
mod timestamp;
mod vault;
mod vault_file;

pub use csv_format::{CsvError, LineProblem, export_csv, import_csv};
pub use entry::{Entry, EntryContent, EntryKind, EntrySnapshot};
pub use generator::{GeneratorError, PasswordOptions, generate_password};
pub use merge::{MergeError, merge};
pub use recovery_key::{RecoveryKey, RecoveryKeyError};
pub use secret::Secret;
pub use session::{Credentials, Session, SessionError, SyncStatus};
pub use tags::Tags;
pub use timestamp::Timestamp;
pub use vault::{Vault, VaultError};

/// Points d'entrée de `cargo fuzz`, compilés seulement avec `--cfg fuzzing`.
#[cfg(fuzzing)]
pub mod fuzz {
    pub use crate::csv_format::fuzz::import_csv;
    pub use crate::vault_file::fuzz::{decode_payload, parse_file};
}
pub use vault_file::{FileError, RemoteMerge, VaultFile};
