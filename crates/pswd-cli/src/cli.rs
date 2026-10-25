use std::path::PathBuf;

use clap::{Parser, Subcommand};

/// Gestionnaire de mots de passe local-first.
#[derive(Debug, Parser)]
#[command(name = "pswd", version)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Debug, PartialEq, Eq, Subcommand)]
pub enum Command {
    /// Crée un coffre local.
    Init {
        /// Nom du coffre (lettres, chiffres, - et _).
        name: String,
    },
    /// Déverrouille un coffre et ouvre le shell interactif.
    Open {
        /// Nom du coffre.
        name: String,
    },
    /// Change le mot de passe maître (l'ancien est demandé).
    Passwd {
        /// Nom du coffre.
        name: String,
    },
    /// Choisit un nouveau mot de passe maître grâce à la clé de récupération.
    Recover {
        /// Nom du coffre.
        name: String,
    },
    /// Importe des entrées depuis un fichier CSV (tout ou rien).
    Import {
        /// Nom du coffre.
        name: String,
        file: PathBuf,
    },
    /// Supprime un coffre de cette machine (le nom est à retaper).
    Delete {
        /// Nom du coffre.
        name: String,
    },
    /// Sauvegarde les coffres dans un nouveau fichier .pswd (clé USB, disque externe).
    Backup {
        /// Dossier où écrire le fichier, par exemple la racine de la clé USB.
        destination: PathBuf,
        /// Seulement ce coffre (par défaut : tous).
        #[arg(long)]
        vault: Option<String>,
    },
    /// Restaure un coffre d'une sauvegarde, comme nouveau coffre local.
    Restore {
        /// Fichier .pswd de la sauvegarde.
        backup: PathBuf,
        /// Coffre à restaurer (par défaut : demandé).
        name: Option<String>,
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
            parse(&["open", "perso"]).unwrap(),
            Command::Open {
                name: "perso".to_owned()
            }
        );
        assert_eq!(
            parse(&["passwd", "perso"]).unwrap(),
            Command::Passwd {
                name: "perso".to_owned()
            }
        );
        assert_eq!(
            parse(&["recover", "perso"]).unwrap(),
            Command::Recover {
                name: "perso".to_owned()
            }
        );
        assert_eq!(
            parse(&["import", "perso", "a.csv"]).unwrap(),
            Command::Import {
                name: "perso".to_owned(),
                file: PathBuf::from("a.csv")
            }
        );
        assert_eq!(
            parse(&["delete", "perso"]).unwrap(),
            Command::Delete {
                name: "perso".to_owned()
            }
        );
        assert_eq!(
            parse(&["backup", "E:/", "--vault", "perso"]).unwrap(),
            Command::Backup {
                destination: PathBuf::from("E:/"),
                vault: Some("perso".to_owned())
            }
        );
        assert_eq!(
            parse(&["restore", "E:/pswd-sauvegarde_2026-10-23_14-30-00.pswd"]).unwrap(),
            Command::Restore {
                backup: PathBuf::from("E:/pswd-sauvegarde_2026-10-23_14-30-00.pswd"),
                name: None
            }
        );
    }

    #[test]
    fn missing_or_unknown_arguments_are_rejected() {
        for args in [
            &[][..],
            &["init"],
            &["open"],
            &["passwd"],
            &["recover"],
            &["import", "a.csv"],
            &["backup"],
            &["restore"],
            &["sync"],
        ] {
            assert!(parse(args).is_err(), "{args:?}");
        }
    }
}
