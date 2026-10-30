#![forbid(unsafe_code)]
#![cfg_attr(coverage_nightly, feature(coverage_attribute))]

use std::net::{Ipv4Addr, SocketAddr, TcpListener};
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use clap::{Parser, Subcommand};
use pswd_core::Timestamp;
use pswd_server::{
    DATABASE_FILE, Identity, KeyPurpose, ShutdownHandle, Storage, adopt_folder_owner, backup,
    issue_connection_key, load_or_create_identity, load_settings, restore, router, serve,
    server_config,
};

/// Délai laissé aux requêtes en cours (une publication, par exemple) avant l'arrêt.
const SHUTDOWN_GRACE: Duration = Duration::from_secs(10);

/// Serveur de synchronisation des coffres pswd.
#[derive(Parser)]
#[command(version)]
struct Cli {
    /// Dossier des données du serveur ; il doit exister.
    #[arg(long, global = true, default_value = "/var/lib/pswd-server")]
    data_dir: PathBuf,
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Lance le serveur.
    Serve {
        /// Configuration (port d'écoute) ; absente, le port 8443 est pris.
        #[arg(long, default_value = "/etc/pswd-server/config.toml")]
        config: PathBuf,
        /// Adresse d'écoute, à la place de toutes les interfaces au port configuré
        /// (développement : 127.0.0.1:8443).
        #[arg(long)]
        listen: Option<SocketAddr>,
    },
    /// Affiche une nouvelle clé administrateur et annule les précédentes.
    ResetAdmin,
    /// Copie la base, certificat compris, dans un nouveau fichier du dossier donné, sans arrêter
    /// le serveur.
    Backup {
        /// Dossier de destination ; il doit exister.
        folder: PathBuf,
    },
    /// Remplace la base d'un serveur neuf, service arrêté, par une sauvegarde.
    Restore {
        /// Fichier `pswd-server-sauvegarde_<date>.db`.
        file: PathBuf,
    },
}

// Exclus de la couverture : point d'entrée du processus, qui ouvre la vraie base, le port réseau
// et attend les signaux du système ; le stockage, le certificat, les clés, les routes, l'arrêt,
// la configuration, la sauvegarde et la restauration sont testés dans la bibliothèque.
#[cfg_attr(coverage_nightly, coverage(off))]
#[tokio::main]
async fn main() -> ExitCode {
    let cli = Cli::parse();
    let result = match cli.command {
        Command::Serve { config, listen } => run(&cli.data_dir, &config, listen).await,
        Command::ResetAdmin => reset_admin(&cli.data_dir),
        Command::Backup { folder } => backup(&cli.data_dir, &folder, now())
            .map(|path| println!("Sauvegarde écrite : {}", path.display()))
            .map_err(|e| e.to_string()),
        Command::Restore { file } => restore(&cli.data_dir, &file)
            .map(|()| {
                println!("Base restaurée. Démarrez le service : sudo systemctl start pswd-server");
            })
            .map_err(|e| e.to_string()),
    };
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(message) => {
            eprintln!("pswd-server : {message}");
            ExitCode::FAILURE
        }
    }
}

#[cfg_attr(coverage_nightly, coverage(off))]
fn now() -> Timestamp {
    let millis = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |elapsed| {
            i64::try_from(elapsed.as_millis()).unwrap_or(i64::MAX)
        });
    Timestamp::from_millis(millis)
}

#[cfg_attr(coverage_nightly, coverage(off))]
fn open(data_dir: &Path) -> Result<(Storage, Identity), String> {
    if !data_dir.is_dir() {
        return Err(format!("dossier introuvable : {}", data_dir.display()));
    }
    let mut storage = Storage::open(&data_dir.join(DATABASE_FILE)).map_err(|e| e.to_string())?;
    let identity = load_or_create_identity(&mut storage).map_err(|e| e.to_string())?;
    adopt_folder_owner(data_dir).map_err(|e| e.to_string())?;
    Ok((storage, identity))
}

#[cfg_attr(coverage_nightly, coverage(off))]
async fn run(data_dir: &Path, config: &Path, listen: Option<SocketAddr>) -> Result<(), String> {
    let settings = load_settings(config).map_err(|e| e.to_string())?;
    let listen = listen.unwrap_or_else(|| SocketAddr::from((Ipv4Addr::UNSPECIFIED, settings.port)));
    let (storage, identity) = open(data_dir)?;
    let config = server_config(&identity).map_err(|e| e.to_string())?;
    let listener =
        TcpListener::bind(listen).map_err(|e| format!("écoute impossible sur {listen} : {e}"))?;
    let handle = ShutdownHandle::new();
    tokio::spawn(stop_on_signal(handle.clone()));
    eprintln!("pswd-server écoute sur {listen}");
    serve(
        listener,
        router(storage, identity.fingerprint()),
        config,
        handle,
    )
    .await
    .map_err(|e| e.to_string())
}

#[cfg_attr(coverage_nightly, coverage(off))]
fn reset_admin(data_dir: &Path) -> Result<(), String> {
    let (mut storage, identity) = open(data_dir)?;
    let key = issue_connection_key(&mut storage, &identity.fingerprint(), &KeyPurpose::Admin)
        .map_err(|e| e.to_string())?;
    println!("Clé administrateur (valable 7 jours, utilisable une fois) :");
    println!("{}", key.expose());
    Ok(())
}

#[cfg_attr(coverage_nightly, coverage(off))]
async fn stop_on_signal(handle: ShutdownHandle) {
    wait_for_signal().await;
    eprintln!("pswd-server : arrêt demandé");
    handle.graceful_shutdown(Some(SHUTDOWN_GRACE));
}

/// Ctrl+C, ou SIGTERM (envoyé par `systemd` pour arrêter le service).
#[cfg(unix)]
#[cfg_attr(coverage_nightly, coverage(off))]
async fn wait_for_signal() {
    use tokio::signal::unix::{SignalKind, signal};

    match signal(SignalKind::terminate()) {
        Ok(mut terminate) => {
            tokio::select! {
                _ = terminate.recv() => {}
                _ = tokio::signal::ctrl_c() => {}
            }
        }
        Err(_) => {
            let _ = tokio::signal::ctrl_c().await;
        }
    }
}

#[cfg(not(unix))]
#[cfg_attr(coverage_nightly, coverage(off))]
async fn wait_for_signal() {
    let _ = tokio::signal::ctrl_c().await;
}
