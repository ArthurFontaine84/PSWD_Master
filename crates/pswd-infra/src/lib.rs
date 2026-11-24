//! Adaptateurs desktop des ports de `pswd-core`.

#![cfg_attr(coverage_nightly, feature(coverage_attribute))]

mod admin;
mod clipboard;
mod config;
mod file_store;
mod remote;
mod server_file;
mod sharing;

pub use admin::{
    AccountInfo, DeviceInfo, KeyPurpose, PendingKey, RestoredVault, ServerInfo, ServerVaultInfo,
    VaultVersion,
};
pub use clipboard::{ClipboardAccess, TimedClipboard};
#[cfg(not(any(target_os = "android", target_os = "ios")))]
pub use clipboard::{LazyClipboard, SystemClipboard};
pub use config::{
    Config, ConfigError, ConfigFile, Locations, RemoteLink, SyncedState, Theme, VaultConfig,
};
pub use file_store::FileStore;
pub use remote::{Connection, RemoteVault, RemoteVaultInfo, Removal, ServerClient};
pub use server_file::{ServerConfig, ServerFile};
pub use sharing::{Account, ReceivedInvitation, SentInvitation, VaultMembers};
