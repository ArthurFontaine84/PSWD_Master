use std::path::PathBuf;

use clap::{Parser, Subcommand};

/// Gestionnaire de mots de passe local-first, synchronisé par Git.
#[derive(Debug, Parser)]
#[command(name = "pswd", version)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Debug, PartialEq, Eq, Subcommand)]
pub enum Command {
    /// Crée un coffre et le publie sur un dépôt distant vide.
    Init {
        /// Nom du coffre (lettres, chiffres, - et _).
        name: String,
    },
    /// Rattache cette machine à un coffre existant.
    Join {
        /// Nom local du coffre (lettres, chiffres, - et _).
        name: String,
        /// URL du dépôt distant.
        url: String,
    },
    /// Déverrouille un coffre et ouvre le shell interactif.
    Open {
        /// Nom du coffre (par défaut : le coffre par défaut).
        name: Option<String>,
    },
    /// Synchronise un coffre avec son dépôt distant.
    Sync {
        /// Nom du coffre (par défaut : le coffre par défaut).
        name: Option<String>,
    },
    /// Change le mot de passe maître (l'ancien est demandé).
    Passwd {
        /// Nom du coffre (par défaut : le coffre par défaut).
        name: Option<String>,
    },
    /// Choisit un nouveau mot de passe maître grâce à la clé de récupération.
    Recover {
        /// Nom du coffre (par défaut : le coffre par défaut).
        name: Option<String>,
    },
    /// Importe des entrées depuis un fichier CSV (tout ou rien).
    Import {
        file: PathBuf,
        /// Nom du coffre (par défaut : le coffre par défaut).
        #[arg(long)]
        vault: Option<String>,
    },
    /// Exporte un coffre en clair dans un nouveau fichier CSV.
    Export {
        file: PathBuf,
        /// Nom du coffre (par défaut : le coffre par défaut).
        #[arg(long)]
        vault: Option<String>,
    },
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(args: &[&str]) -> Result<Command, clap::Error> {
        Cli::try_parse_from(std::iter::once("pswd").chain(args.iter().copied()))
            .map(|cli| cli.command)
    }

    #[test]
    fn commands_are_parsed() {
        assert_eq!(
            parse(&["init", "perso"]).unwrap(),
            Command::Init {
                name: "perso".to_owned()
            }
        );
        assert_eq!(
            parse(&["join", "travail", "https://example.com/coffre.git"]).unwrap(),
            Command::Join {
                name: "travail".to_owned(),
                url: "https://example.com/coffre.git".to_owned()
            }
        );
        assert_eq!(parse(&["open"]).unwrap(), Command::Open { name: None });
        assert_eq!(parse(&["sync"]).unwrap(), Command::Sync { name: None });
        assert_eq!(
            parse(&["passwd", "perso"]).unwrap(),
            Command::Passwd {
                name: Some("perso".to_owned())
            }
        );
        assert_eq!(
            parse(&["recover"]).unwrap(),
            Command::Recover { name: None }
        );
        assert_eq!(
            parse(&["import", "a.csv", "--vault", "perso"]).unwrap(),
            Command::Import {
                file: PathBuf::from("a.csv"),
                vault: Some("perso".to_owned())
            }
        );
        assert_eq!(
            parse(&["export", "b.csv"]).unwrap(),
            Command::Export {
                file: PathBuf::from("b.csv"),
                vault: None
            }
        );
        assert_eq!(
            parse(&["open", "perso"]).unwrap(),
            Command::Open {
                name: Some("perso".to_owned())
            }
        );
    }

    #[test]
    fn missing_or_unknown_arguments_are_rejected() {
        for args in [&[][..], &["init"], &["join", "travail"], &["inconnue"]] {
            assert!(parse(args).is_err(), "{args:?}");
        }
    }
}
