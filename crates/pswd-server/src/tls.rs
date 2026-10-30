//! Certificat du serveur et configuration HTTPS.

use std::fmt;
use std::io;
use std::net::{SocketAddr, TcpListener};
use std::sync::Arc;

use axum::Router;
use axum_server::Handle;
use axum_server::tls_rustls::RustlsConfig;
use pswd_core::crypto::{FINGERPRINT_LEN, certificate_fingerprint};
use rcgen::{CertificateParams, KeyPair, date_time_ymd};
use rustls::ServerConfig;
use rustls::crypto::ring;
use rustls::pki_types::{CertificateDer, PrivateKeyDer, PrivatePkcs8KeyDer};
use rustls::version::TLS13;
use zeroize::Zeroizing;

use crate::storage::{Storage, StorageError};

/// Nom inscrit dans le certificat. L'application ne le vérifie pas : elle reconnaît le serveur à
/// l'empreinte du certificat.
const CERTIFICATE_NAME: &str = "pswd-server";

#[derive(Debug)]
pub enum TlsError {
    Generation,
    /// Certificat ou clé privée enregistrés illisibles.
    InvalidIdentity,
    Storage(StorageError),
}

impl fmt::Display for TlsError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Generation => f.write_str("création du certificat impossible"),
            Self::InvalidIdentity => f.write_str("certificat du serveur illisible"),
            Self::Storage(error) => error.fmt(f),
        }
    }
}

impl std::error::Error for TlsError {}

impl From<StorageError> for TlsError {
    fn from(error: StorageError) -> Self {
        Self::Storage(error)
    }
}

/// Certificat auto-signé du serveur et sa clé privée (PKCS#8, forme DER).
pub struct Identity {
    certificate: Vec<u8>,
    private_key: Zeroizing<Vec<u8>>,
}

impl Identity {
    /// Crée un certificat sans expiration pratique : l'application l'épingle, le changer
    /// obligerait tous les appareils à se reconnecter.
    ///
    /// # Errors
    ///
    /// [`TlsError::Generation`] si la bibliothèque de certificats échoue.
    pub fn generate() -> Result<Self, TlsError> {
        let key = KeyPair::generate().map_err(generation_failed)?;
        let mut params =
            CertificateParams::new(vec![CERTIFICATE_NAME.to_owned()]).map_err(generation_failed)?;
        params.not_after = date_time_ymd(9999, 12, 31);
        let certificate = params.self_signed(&key).map_err(generation_failed)?;
        Ok(Self {
            certificate: certificate.der().to_vec(),
            private_key: Zeroizing::new(key.serialize_der()),
        })
    }

    pub(crate) fn from_parts(certificate: Vec<u8>, private_key: Zeroizing<Vec<u8>>) -> Self {
        Self {
            certificate,
            private_key,
        }
    }

    #[must_use]
    pub fn certificate(&self) -> &[u8] {
        &self.certificate
    }

    pub(crate) fn private_key(&self) -> &Zeroizing<Vec<u8>> {
        &self.private_key
    }

    #[must_use]
    pub fn fingerprint(&self) -> [u8; FINGERPRINT_LEN] {
        certificate_fingerprint(&self.certificate)
    }
}

/// Erreur de `rcgen`, qui ne peut pas survenir : paramètres fixes et valides, aléa du système.
// Exclue de la couverture : aucun test ne peut l'atteindre.
#[cfg_attr(coverage_nightly, coverage(off))]
fn generation_failed<E>(_: E) -> TlsError {
    TlsError::Generation
}

/// Certificat enregistré dans la base, créé au premier démarrage.
///
/// # Errors
///
/// Si la base est illisible ou si la création échoue.
pub fn load_or_create_identity(storage: &mut Storage) -> Result<Identity, TlsError> {
    if let Some(identity) = storage.identity()? {
        return Ok(identity);
    }
    let identity = Identity::generate()?;
    storage.save_identity(&identity)?;
    Ok(identity)
}

/// Configuration TLS du serveur : TLS 1.3 seulement, moteur cryptographique `ring`.
///
/// # Errors
///
/// [`TlsError::InvalidIdentity`] si le certificat ou la clé sont illisibles.
pub fn server_config(identity: &Identity) -> Result<Arc<ServerConfig>, TlsError> {
    let certificate = CertificateDer::from(identity.certificate.clone());
    let key = PrivateKeyDer::Pkcs8(PrivatePkcs8KeyDer::from(identity.private_key.to_vec()));
    let config = ServerConfig::builder_with_provider(Arc::new(ring::default_provider()))
        .with_protocol_versions(&[&TLS13])
        .map_err(unsupported_protocols)?
        .with_no_client_auth()
        .with_single_cert(vec![certificate], key)
        .map_err(|_| TlsError::InvalidIdentity)?;
    Ok(Arc::new(config))
}

/// TLS 1.3 est toujours pris en charge par le moteur `ring` de `rustls`.
// Exclue de la couverture : aucun test ne peut l'atteindre.
#[cfg_attr(coverage_nightly, coverage(off))]
fn unsupported_protocols<E>(_: E) -> TlsError {
    TlsError::Generation
}

/// Permet d'arrêter le serveur depuis une autre tâche.
pub type ShutdownHandle = Handle<SocketAddr>;

/// Sert `router` en HTTPS sur `listener`, jusqu'à l'arrêt demandé par `handle`.
///
/// # Errors
///
/// Si l'écoute échoue.
pub async fn serve(
    listener: TcpListener,
    router: Router,
    config: Arc<ServerConfig>,
    handle: ShutdownHandle,
) -> io::Result<()> {
    listener.set_nonblocking(true)?;
    axum_server::from_tcp_rustls(listener, RustlsConfig::from_config(config))?
        .handle(handle)
        .serve(router.into_make_service_with_connect_info::<SocketAddr>())
        .await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn identity_is_created_once() {
        let mut storage = Storage::open_in_memory().unwrap();
        let first = load_or_create_identity(&mut storage).unwrap();
        let second = load_or_create_identity(&mut storage).unwrap();
        assert_eq!(first.certificate(), second.certificate());
        assert_eq!(
            first.private_key().as_slice(),
            second.private_key().as_slice()
        );
    }

    #[test]
    fn each_server_gets_its_own_certificate() {
        let first = Identity::generate().unwrap();
        let second = Identity::generate().unwrap();
        assert_ne!(first.fingerprint(), second.fingerprint());
        assert_eq!(
            first.fingerprint(),
            certificate_fingerprint(first.certificate())
        );
    }

    #[test]
    fn generated_identity_makes_a_valid_configuration() {
        assert!(server_config(&Identity::generate().unwrap()).is_ok());
    }

    #[test]
    fn unreadable_private_key_is_refused() {
        let identity = Identity::generate().unwrap();
        let broken = Identity::from_parts(identity.certificate.clone(), Zeroizing::new(vec![1, 2]));
        let error = server_config(&broken).err().unwrap();
        assert!(matches!(error, TlsError::InvalidIdentity));
        assert_eq!(error.to_string(), "certificat du serveur illisible");
    }

    #[test]
    fn storage_failure_is_reported() {
        let mut storage = Storage::open_in_memory().unwrap();
        storage.break_for_tests("identity");
        let error = load_or_create_identity(&mut storage).err().unwrap();
        assert!(matches!(
            error,
            TlsError::Storage(StorageError::Database(_))
        ));
        assert!(error.to_string().starts_with("base de données : "));
    }

    #[tokio::test]
    async fn server_stops_on_request() {
        let identity = Identity::generate().unwrap();
        let config = server_config(&identity).unwrap();
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let app = crate::router(Storage::open_in_memory().unwrap(), identity.fingerprint());
        let handle = ShutdownHandle::new();
        let server = tokio::spawn(serve(listener, app, config, handle.clone()));
        assert!(handle.listening().await.is_some());
        handle.shutdown();
        assert!(server.await.unwrap().is_ok());
    }

    #[test]
    fn generation_error_has_a_message() {
        assert_eq!(
            TlsError::Generation.to_string(),
            "création du certificat impossible"
        );
    }
}
