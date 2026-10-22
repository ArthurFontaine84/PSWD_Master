//! Adaptateurs desktop des ports de `pswd-core`.

#![cfg_attr(coverage_nightly, feature(coverage_attribute))]

mod file_store;
mod git_remote;

pub use file_store::FileStore;
pub use git_remote::GitRemote;
