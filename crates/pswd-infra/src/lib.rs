//! Adaptateurs desktop des ports de `pswd-core`.

#![cfg_attr(coverage_nightly, feature(coverage_attribute))]

mod backup;
mod clipboard;
mod config;
mod file_store;

pub use backup::{BACKUPS_DIR, backup_vault_file, backup_vault_names, create_backup};
pub use clipboard::{ClipboardAccess, SystemClipboard, TimedClipboard};
pub use config::{Config, ConfigError, ConfigFile, Locations, VaultConfig};
pub use file_store::FileStore;
