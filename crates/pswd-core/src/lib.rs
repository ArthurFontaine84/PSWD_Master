#![forbid(unsafe_code)]

mod entry;
mod merge;
mod secret;
mod tags;
mod timestamp;
mod vault;

pub use entry::{Entry, EntryContent, EntryKind, EntrySnapshot};
pub use merge::{MergeError, merge};
pub use secret::Secret;
pub use tags::Tags;
pub use timestamp::Timestamp;
pub use vault::{Vault, VaultError};
