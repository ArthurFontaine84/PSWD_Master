//! Adaptateurs desktop des ports de `pswd-core`.

#![cfg_attr(coverage_nightly, feature(coverage_attribute))]

mod clipboard;
mod config;
mod file_store;

pub use clipboard::{ClipboardAccess, SystemClipboard, TimedClipboard};
pub use config::{Config, ConfigError, ConfigFile, Locations, VaultConfig};
pub use file_store::FileStore;
