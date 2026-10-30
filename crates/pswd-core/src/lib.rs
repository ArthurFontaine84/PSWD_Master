#![forbid(unsafe_code)]
#![cfg_attr(coverage_nightly, feature(coverage_attribute))]

mod backup_file;
mod buffer;
mod connection_key;
pub mod crypto;
mod csv_format;
mod entry;
mod generator;
mod key_text;
mod merge;
pub mod ports;
mod recovery_key;
mod secret;
mod session;
mod tags;
mod timestamp;
mod utc;
mod vault;
mod vault_file;
mod vault_name;

pub use backup_file::{BackupFile, BackupFileError, MAX_VAULT_FILE_LEN};
pub use connection_key::{
    CONNECTION_SECRET_LEN, ConnectionKey, ConnectionKeyError, FINGERPRINT_PREFIX_LEN,
};
pub use csv_format::{CsvError, LineProblem, import_csv};
pub use entry::{Entry, EntryContent, EntryKind, EntrySnapshot};
pub use generator::{GeneratorError, PasswordOptions, generate_password};
pub use merge::{MergeError, merge};
pub use recovery_key::{RecoveryKey, RecoveryKeyError};
pub use secret::Secret;
pub use session::{Credentials, Session, SessionError, SyncStatus};
pub use tags::Tags;
pub use timestamp::Timestamp;
pub use utc::{backup_day, backup_file_name, format_timestamp, server_backup_file_name};
pub use vault::{Vault, VaultError};
pub use vault_name::{MAX_VAULT_NAME_LEN, is_valid_vault_name};

/// Points d'entrée de `cargo fuzz`, compilés seulement avec `--cfg fuzzing`.
#[cfg(fuzzing)]
pub mod fuzz {
    pub use crate::backup_file::fuzz::parse_backup;
    pub use crate::csv_format::fuzz::import_csv;
    pub use crate::vault_file::fuzz::{decode_payload, parse_file};
}
pub use vault_file::{FileError, RemoteMerge, VaultFile};
