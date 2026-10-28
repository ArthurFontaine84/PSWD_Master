#![forbid(unsafe_code)]
#![cfg_attr(coverage_nightly, feature(coverage_attribute))]

use std::net::{SocketAddr, TcpListener};
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::time::Duration;

use clap::{Parser, Subcommand};
use pswd_server::{
    Identity, KeyPurpose, ShutdownHandle, Storage, issue_connection_key, load_or_create_identity,
    router, serve, server_config,
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
        /// Adresse d'écoute.
        #[arg(long, default_value = "127.0.0.1:8443")]
        listen: SocketAddr,
    },
    /// Affiche une nouvelle clé administrateur et annule les précédentes.
    ResetAdmin,
}

// Exclus de la couverture : point d'entrée du processus, qui ouvre la vraie base, le port réseau
// et attend les signaux du système ; le stockage, le certificat, les clés, les routes et l'arrêt
// sont testés dans la bibliothèque.
#[cfg_attr(coverage_nightly, coverage(off))]
#[tokio::main]
async fn main() -> ExitCode {
    let cli = Cli::parse();
    let result = match cli.command {
        Command::Serve { listen } => run(&cli.data_dir, listen).await,
        Command::ResetAdmin => reset_admin(&cli.data_dir),
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
fn open(data_dir: &Path) -> Result<(Storage, Identity), String> {
    if !data_dir.is_dir() {
        return Err(format!("dossier introuvable : {}", data_dir.display()));
    }
    let mut storage = Storage::open(&data_dir.join("pswd.db")).map_err(|e| e.to_string())?;
    let identity = load_or_create_identity(&mut storage).map_err(|e| e.to_string())?;
    Ok((storage, identity))
}

#[cfg_attr(coverage_nightly, coverage(off))]
async fn run(data_dir: &Path, listen: SocketAddr) -> Result<(), String> {
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
