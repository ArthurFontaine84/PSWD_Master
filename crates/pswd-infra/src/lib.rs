//! Adaptateurs desktop des ports de `pswd-core`.

#![cfg_attr(coverage_nightly, feature(coverage_attribute))]

mod backup;
mod clipboard;
mod config;
mod file_store;

pub use backup::{read_backup, write_backup};
pub use clipboard::{ClipboardAccess, TimedClipboard};
#[cfg(not(any(target_os = "android", target_os = "ios")))]
pub use clipboard::{LazyClipboard, SystemClipboard};
pub use config::{Config, ConfigError, ConfigFile, Locations, Theme, VaultConfig};
pub use file_store::FileStore;
