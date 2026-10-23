use std::io::{self, BufRead, Write};

use pswd_core::Secret;

use crate::app::CliError;

/// Échanges avec l'utilisateur.
pub trait Terminal {
    /// # Errors
    ///
    /// Si l'entrée standard est fermée ou illisible.
    fn prompt(&mut self, label: &str) -> Result<String, CliError>;

    /// Saisie masquée.
    ///
    /// # Errors
    ///
    /// Si l'entrée standard est fermée ou illisible.
    fn prompt_secret(&mut self, label: &str) -> Result<Secret, CliError>;

    fn print(&mut self, text: &str);
}

/// Le vrai terminal.
pub struct StdTerminal;

// Exclu de la couverture : lit le clavier et écrit dans la console ; les commandes sont testées
// avec un terminal scénarisé.
#[cfg_attr(coverage_nightly, coverage(off))]
impl Terminal for StdTerminal {
    fn prompt(&mut self, label: &str) -> Result<String, CliError> {
        print!("{label}");
        io::stdout().flush().map_err(input_error)?;
        let mut line = String::new();
        if io::stdin()
            .lock()
            .read_line(&mut line)
            .map_err(input_error)?
            == 0
        {
            return Err(CliError::InputClosed);
        }
        Ok(line.trim_end_matches(['\r', '\n']).to_owned())
    }

    fn prompt_secret(&mut self, label: &str) -> Result<Secret, CliError> {
        rpassword::prompt_password(label)
            .map(Secret::new)
            .map_err(input_error)
    }

    fn print(&mut self, text: &str) {
        println!("{text}");
    }
}

#[cfg_attr(coverage_nightly, coverage(off))]
#[allow(clippy::needless_pass_by_value)]
fn input_error(error: io::Error) -> CliError {
    CliError::Input(error.to_string())
}
