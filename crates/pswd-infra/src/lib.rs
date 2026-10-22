//! Adaptateurs desktop des ports de `pswd-core`.

#![cfg_attr(coverage_nightly, feature(coverage_attribute))]

mod clipboard;
mod file_store;
mod git_remote;
mod keyring_store;

pub use clipboard::{ClipboardAccess, SystemClipboard, TimedClipboard};
pub use file_store::FileStore;
pub use git_remote::GitRemote;
pub use keyring_store::KeyringStore;
