#![forbid(unsafe_code)]
#![cfg_attr(coverage_nightly, feature(coverage_attribute))]

mod app;
mod backup;
mod cli;
mod commands;
mod editing;
mod setup;
mod shell;
mod terminal;
#[cfg(test)]
mod testing;

use std::process::ExitCode;

use clap::Parser;
use pswd_app::Vaults;
use pswd_core::crypto::KdfParams;
use pswd_infra::{LazyClipboard, Locations};

use crate::app::{App, CliError};
use crate::cli::Cli;
use crate::terminal::StdTerminal;

// Exclu de la couverture : point d'entrée du processus, qui branche les vrais adaptateurs
// (console, presse-papiers, dossiers de l'utilisateur) ; les commandes sont testées
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
        clipboard: LazyClipboard::default(),
        vaults: Vaults::new(Locations::system()?),
        kdf: KdfParams::DEFAULT,
        clock: pswd_app::now,
        auto_lock: None,
    };
    app.run(cli.command)
}
