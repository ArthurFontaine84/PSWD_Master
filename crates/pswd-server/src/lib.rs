//! Serveur de synchronisation : garde les fichiers `vault.enc` opaques et leurs versions, sans
//! jamais détenir de clé.

#![forbid(unsafe_code)]
#![cfg_attr(coverage_nightly, feature(coverage_attribute))]

mod api;
mod clock;
mod limiter;
mod storage;
#[cfg(feature = "testing")]
pub mod testing;
mod tls;

pub use api::{issue_connection_key, router};
pub use storage::{
    Caller, KEPT_VERSIONS, KeyPurpose, NewDevice, Storage, StorageError, StoredFile, VaultSummary,
};
pub use tls::{Identity, ShutdownHandle, TlsError, load_or_create_identity, serve, server_config};
