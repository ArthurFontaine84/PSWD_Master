#![forbid(unsafe_code)]
#![cfg_attr(coverage_nightly, feature(coverage_attribute))]

mod app;
mod cli;
mod commands;
mod editing;
mod setup;
mod shell;
mod terminal;
#[cfg(test)]
mod testing;

use std::process::ExitCode;
use std::time::Duration;

use clap::Parser;
use pswd_core::Secret;
use pswd_core::crypto::KdfParams;
use pswd_core::ports::{PortError, SecureClipboard};
use pswd_infra::{KeyringStore, Locations, SystemClipboard, TimedClipboard};

use crate::app::{App, CliError, system_clock};
use crate::cli::Cli;
use crate::terminal::StdTerminal;

// Exclu de la couverture : point d'entrée du processus, qui branche les vrais adaptateurs
// (console, trousseau, presse-papiers, dossiers de l'utilisateur) ; les commandes sont testées
// via `App`.
#[cfg_attr(coverage_nightly, coverage(off))]
fn main() -> ExitCode {
    let cli = Cli::parse();
    match run(cli) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("erreur : {error}");
            ExitCode::FAILURE
        }
    }
}

#[cfg_attr(coverage_nightly, coverage(off))]
fn run(cli: Cli) -> Result<(), CliError> {
    let mut app = App {
        terminal: StdTerminal,
        keyring: KeyringStore::system()?,
        clipboard: LazyClipboard(None),
        locations: Locations::system()?,
        kdf: KdfParams::DEFAULT,
        clock: system_clock,
    };
    app.run(cli.command)
}

/// Presse-papiers ouvert à la première copie : une machine sans presse-papiers (serveur) peut
/// utiliser toutes les autres commandes.
struct LazyClipboard(Option<TimedClipboard<SystemClipboard>>);

// Exclu de la couverture : même raison que `main`.
#[cfg_attr(coverage_nightly, coverage(off))]
impl SecureClipboard for LazyClipboard {
    fn copy_secret(&mut self, value: &Secret, ttl: Duration) -> Result<(), PortError> {
        if self.0.is_none() {
            self.0 = Some(TimedClipboard::new(SystemClipboard::new()?));
        }
        match &mut self.0 {
            Some(clipboard) => clipboard.copy_secret(value, ttl),
            None => Ok(()),
        }
    }
}
