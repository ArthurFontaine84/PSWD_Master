//! Adaptateur `RemoteStore` HTTPS vers `pswd-server`. Le serveur est reconnu à l'empreinte de son
//! certificat, jamais par une autorité de certification.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, PoisonError};
use std::time::Duration;

use pswd_core::crypto::{FINGERPRINT_LEN, certificate_fingerprint};
use pswd_core::ports::{PortError, PushError, RemoteFile, RemoteStore, RemoteVersion};
use pswd_core::{ConnectionKey, FINGERPRINT_PREFIX_LEN, Secret};
use reqwest::StatusCode;
use reqwest::blocking::{Client, RequestBuilder, Response};
use reqwest::header::{CONTENT_TYPE, ETAG, IF_MATCH, IF_NONE_MATCH};
use rustls::client::danger::{HandshakeSignatureValid, ServerCertVerified, ServerCertVerifier};
use rustls::crypto::{CryptoProvider, ring, verify_tls12_signature, verify_tls13_signature};
use rustls::pki_types::{CertificateDer, ServerName, UnixTime};
use rustls::version::TLS13;
use rustls::{CertificateError, ClientConfig, DigitallySignedStruct, SignatureScheme};
use serde::Deserialize;
use serde::de::DeserializeOwned;
use uuid::Uuid;

/// Attente maximale d'une connexion, par adresse.
const CONNECT_TIMEOUT: Duration = Duration::from_secs(5);
/// Durée maximale d'une requête : un coffre peut peser 64 Mio.
const REQUEST_TIMEOUT: Duration = Duration::from_mins(2);

/// Appareil connecté à un serveur : ce qu'il doit retenir pour s'y adresser ensuite.
pub struct Connection {
    /// Empreinte complète du certificat, vérifié par la clé de connexion.
    pub fingerprint: [u8; FINGERPRINT_LEN],
    pub token: Secret,
    pub account_id: Uuid,
    pub device_id: Uuid,
    pub is_admin: bool,
}

/// Coffre dont le compte est membre ; `version` vaut `None` tant qu'il n'est pas publié.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RemoteVaultInfo {
    pub id: Uuid,
    pub name: String,
    pub version: Option<RemoteVersion>,
}

/// Connexion à un serveur, essayé à chacune de ses adresses dans l'ordre (locale, puis publique).
#[derive(Clone)]
pub struct ServerClient {
    http: Client,
    addresses: Vec<String>,
    observed: Arc<Observed>,
    token: Option<Secret>,
}

impl ServerClient {
    /// Client d'un appareil déjà connecté. `addresses` : `hôte:port`, par ordre de préférence.
    ///
    /// # Errors
    ///
    /// Si le client HTTPS ne peut pas être créé.
    pub fn new(
        addresses: Vec<String>,
        fingerprint: [u8; FINGERPRINT_LEN],
        token: Secret,
    ) -> Result<Self, PortError> {
        Self::build(addresses, Pin::Fingerprint(fingerprint), Some(token))
    }

    fn build(addresses: Vec<String>, pin: Pin, token: Option<Secret>) -> Result<Self, PortError> {
        let observed = Arc::new(Observed::default());
        let tls = tls_config(pin, Arc::clone(&observed))?;
        let built = Client::builder()
            .use_preconfigured_tls(tls)
            .connect_timeout(CONNECT_TIMEOUT)
            .timeout(REQUEST_TIMEOUT)
            .build();
        let http = built.map_err(client_unavailable)?;
        Ok(Self {
            http,
            addresses,
            observed,
            token,
        })
    }

    /// Connecte cet appareil avec une clé de connexion. Le serveur doit présenter le certificat
    /// annoncé par la clé. `account_name` nomme le compte créé par une clé administrateur.
    ///
    /// # Errors
    ///
    /// Si le serveur est injoignable, n'est pas celui de la clé, ou refuse la clé.
    pub fn connect(
        addresses: Vec<String>,
        key: &ConnectionKey,
        device_name: &str,
        account_name: Option<&str>,
    ) -> Result<Connection, PortError> {
        #[derive(Deserialize)]
        struct Connected {
            token: String,
            account_id: Uuid,
            device_id: Uuid,
            is_admin: bool,
        }

        let client = Self::build(addresses, Pin::Prefix(key.fingerprint_prefix()), None)?;
        let body = Secret::new(
            serde_json::json!({
                "key": key.encode().expose(),
                "device_name": device_name,
                "account_name": account_name,
            })
            .to_string(),
        );
        let response = client.send(&|base| {
            client
                .http
                .post(format!("{base}/v1/connect"))
                .header(CONTENT_TYPE, "application/json")
                .body(body.expose().to_owned())
        })?;
        match response.status() {
            StatusCode::CREATED => {
                let connected: Connected = parse(response)?;
                let fingerprint = client.observed.fingerprint().ok_or_else(invalid_response)?;
                Ok(Connection {
                    fingerprint,
                    token: Secret::new(connected.token),
                    account_id: connected.account_id,
                    device_id: connected.device_id,
                    is_admin: connected.is_admin,
                })
            }
            StatusCode::UNAUTHORIZED => Err(PortError::new(
                "clé de connexion invalide, expirée ou déjà utilisée",
            )),
            StatusCode::BAD_REQUEST => Err(PortError::new("nom d'appareil ou de compte invalide")),
            status => Err(status_error(status)),
        }
    }

    /// Réserve l'identifiant d'un nouveau coffre `name` sur le serveur, encore vide.
    ///
    /// # Errors
    ///
    /// Si le serveur est injoignable, refuse le nom ou répond de façon inattendue.
    pub fn create_vault(&self, name: &str) -> Result<Uuid, PortError> {
        #[derive(Deserialize)]
        struct Created {
            id: Uuid,
        }

        let body = serde_json::json!({ "name": name }).to_string();
        let response = self.send(&|base| {
            self.authorized(self.http.post(format!("{base}/v1/vaults")))
                .header(CONTENT_TYPE, "application/json")
                .body(body.clone())
        })?;
        match response.status() {
            StatusCode::CREATED => Ok(parse::<Created>(response)?.id),
            StatusCode::BAD_REQUEST => Err(PortError::new("nom de coffre refusé par le serveur")),
            status => Err(status_error(status)),
        }
    }

    /// Coffres dont le compte est membre.
    ///
    /// # Errors
    ///
    /// Si le serveur est injoignable ou répond de façon inattendue.
    pub fn list_vaults(&self) -> Result<Vec<RemoteVaultInfo>, PortError> {
        #[derive(Deserialize)]
        struct Listed {
            id: Uuid,
            name: String,
            version: Option<String>,
        }

        let response =
            self.send(&|base| self.authorized(self.http.get(format!("{base}/v1/vaults"))))?;
        match response.status() {
            StatusCode::OK => {
                let listed: Vec<Listed> = parse(response)?;
                Ok(listed
                    .into_iter()
                    .map(|vault| RemoteVaultInfo {
                        id: vault.id,
                        name: vault.name,
                        version: vault.version.map(RemoteVersion),
                    })
                    .collect())
            }
            status => Err(status_error(status)),
        }
    }

    /// Clé de connexion d'un nouvel appareil pour ce compte, à montrer une seule fois.
    ///
    /// # Errors
    ///
    /// Si le serveur est injoignable ou répond de façon inattendue.
    pub fn new_device_key(&self) -> Result<Secret, PortError> {
        self.issue_key(&|base| self.authorized(self.http.post(format!("{base}/v1/devices/keys"))))
    }

    /// Clé de connexion d'un nouveau membre nommé `name` ; réservé aux administrateurs.
    ///
    /// # Errors
    ///
    /// Si l'appareil n'est pas administrateur, si le nom est invalide, ou si le serveur est
    /// injoignable.
    pub fn add_member(&self, name: &str) -> Result<Secret, PortError> {
        let body = serde_json::json!({ "name": name }).to_string();
        self.issue_key(&|base| {
            self.authorized(self.http.post(format!("{base}/v1/members")))
                .header(CONTENT_TYPE, "application/json")
                .body(body.clone())
        })
    }

    fn issue_key(&self, request: &dyn Fn(&str) -> RequestBuilder) -> Result<Secret, PortError> {
        #[derive(Deserialize)]
        struct Issued {
            key: String,
        }

        let response = self.send(request)?;
        match response.status() {
            StatusCode::CREATED => Ok(Secret::new(parse::<Issued>(response)?.key)),
            StatusCode::BAD_REQUEST => Err(PortError::new("nom invalide")),
            status => Err(status_error(status)),
        }
    }

    /// Déconnecte cet appareil : son jeton est révoqué par le serveur.
    ///
    /// # Errors
    ///
    /// Si le serveur est injoignable ou répond de façon inattendue.
    pub fn disconnect(&self) -> Result<(), PortError> {
        let response = self
            .send(&|base| self.authorized(self.http.delete(format!("{base}/v1/devices/self"))))?;
        match response.status() {
            StatusCode::NO_CONTENT => Ok(()),
            status => Err(status_error(status)),
        }
    }

    /// Coffre `id` du serveur, comme distant de la synchro.
    #[must_use]
    pub fn vault(&self, id: Uuid) -> RemoteVault {
        RemoteVault {
            client: self.clone(),
            id,
        }
    }

    fn authorized(&self, request: RequestBuilder) -> RequestBuilder {
        match &self.token {
            Some(token) => request.bearer_auth(token.expose()),
            None => request,
        }
    }

    /// Envoie la requête à chaque adresse jusqu'à obtenir une réponse.
    fn send(&self, request: &dyn Fn(&str) -> RequestBuilder) -> Result<Response, PortError> {
        self.observed.mismatch.store(false, Ordering::Relaxed);
        for address in &self.addresses {
            if let Ok(response) = request(&format!("https://{address}")).send() {
                return Ok(response);
            }
        }
        if self.observed.mismatch.load(Ordering::Relaxed) {
            Err(PortError::new("le serveur n'est pas celui attendu"))
        } else {
            Err(PortError::new("serveur injoignable"))
        }
    }
}

/// Un coffre sur le serveur.
pub struct RemoteVault {
    client: ServerClient,
    id: Uuid,
}

impl RemoteVault {
    fn url(&self, base: &str) -> String {
        format!("{base}/v1/vaults/{}", self.id)
    }
}

impl RemoteStore for RemoteVault {
    fn fetch(&mut self) -> Result<Option<RemoteFile>, PortError> {
        let client = &self.client;
        let response = client.send(&|base| client.authorized(client.http.get(self.url(base))))?;
        match response.status() {
            StatusCode::OK => {
                let version = response
                    .headers()
                    .get(ETAG)
                    .and_then(|value| value.to_str().ok())
                    .map(|value| RemoteVersion(value.to_owned()))
                    .ok_or_else(invalid_response)?;
                let file = response.bytes().map_err(|_| interrupted())?.to_vec();
                Ok(Some(RemoteFile { file, version }))
            }
            StatusCode::NO_CONTENT => Ok(None),
            StatusCode::NOT_FOUND => Err(unknown_vault()),
            status => Err(status_error(status)),
        }
    }

    fn push(&mut self, file: &[u8], expected: Option<&RemoteVersion>) -> Result<(), PushError> {
        let (header, value) = match expected {
            Some(version) => (IF_MATCH, version.0.as_str()),
            None => (IF_NONE_MATCH, "*"),
        };
        let client = &self.client;
        let response = client
            .send(&|base| {
                client
                    .authorized(client.http.put(self.url(base)))
                    .header(&header, value)
                    .body(file.to_vec())
            })
            .map_err(PushError::Failed)?;
        match response.status() {
            StatusCode::NO_CONTENT => Ok(()),
            StatusCode::PRECONDITION_FAILED => Err(PushError::Conflict),
            StatusCode::NOT_FOUND => Err(PushError::Failed(unknown_vault())),
            status => Err(PushError::Failed(status_error(status))),
        }
    }
}

/// Corps JSON d'une réponse.
fn parse<T: DeserializeOwned>(response: Response) -> Result<T, PortError> {
    let body = response.bytes().map_err(|_| interrupted())?;
    serde_json::from_slice(&body).map_err(|_| invalid_response())
}

fn unknown_vault() -> PortError {
    PortError::new("coffre inconnu du serveur")
}

fn invalid_response() -> PortError {
    PortError::new("réponse du serveur invalide")
}

fn interrupted() -> PortError {
    PortError::new("connexion au serveur interrompue")
}

/// Refus ou réponse imprévue du serveur.
fn status_error(status: StatusCode) -> PortError {
    match status {
        StatusCode::UNAUTHORIZED => {
            PortError::new("appareil non reconnu par le serveur : déconnecté ou révoqué")
        }
        StatusCode::FORBIDDEN => PortError::new("action réservée aux administrateurs"),
        StatusCode::TOO_MANY_REQUESTS => {
            PortError::new("trop d'essais refusés : réessayez dans quelques minutes")
        }
        status => PortError::new(format!(
            "réponse inattendue du serveur ({})",
            status.as_u16()
        )),
    }
}

/// Erreur de `rustls` ou de `reqwest` à la création du client, qui ne peut pas survenir : TLS 1.3
/// est toujours pris en charge par le moteur `ring`, et la configuration TLS est fournie toute
/// prête.
// Exclue de la couverture : aucun test ne peut l'atteindre.
#[cfg_attr(coverage_nightly, coverage(off))]
fn client_unavailable<E>(_: E) -> PortError {
    PortError::new("client HTTPS indisponible")
}

fn tls_config(pin: Pin, observed: Arc<Observed>) -> Result<ClientConfig, PortError> {
    let provider = Arc::new(ring::default_provider());
    let verifier = PinnedCertificate {
        pin,
        provider: Arc::clone(&provider),
        observed,
    };
    let versions = ClientConfig::builder_with_provider(provider).with_protocol_versions(&[&TLS13]);
    let builder = versions.map_err(client_unavailable)?;
    let config = builder
        .dangerous()
        .with_custom_certificate_verifier(Arc::new(verifier))
        .with_no_client_auth();
    Ok(config)
}

/// Certificat attendu : connu en entier, ou seulement par son début (clé de connexion).
#[derive(Debug, Clone, Copy)]
enum Pin {
    Fingerprint([u8; FINGERPRINT_LEN]),
    Prefix([u8; FINGERPRINT_PREFIX_LEN]),
}

impl Pin {
    fn accepts(&self, fingerprint: &[u8; FINGERPRINT_LEN]) -> bool {
        match self {
            Self::Fingerprint(expected) => fingerprint == expected,
            Self::Prefix(prefix) => fingerprint.starts_with(prefix),
        }
    }
}

/// Ce que le vérificateur a vu, relu après la requête.
#[derive(Debug, Default)]
struct Observed {
    /// Une adresse a présenté un autre certificat.
    mismatch: AtomicBool,
    /// Empreinte du dernier certificat accepté.
    fingerprint: Mutex<Option<[u8; FINGERPRINT_LEN]>>,
}

impl Observed {
    fn fingerprint(&self) -> Option<[u8; FINGERPRINT_LEN]> {
        *self
            .fingerprint
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
    }
}

/// N'accepte que le certificat attendu, sans regarder ni son nom ni ses dates : le serveur est
/// joint par plusieurs adresses et son certificat n'expire pas.
#[derive(Debug)]
struct PinnedCertificate {
    pin: Pin,
    provider: Arc<CryptoProvider>,
    observed: Arc<Observed>,
}

impl ServerCertVerifier for PinnedCertificate {
    fn verify_server_cert(
        &self,
        end_entity: &CertificateDer<'_>,
        _intermediates: &[CertificateDer<'_>],
        _server_name: &ServerName<'_>,
        _ocsp_response: &[u8],
        _now: UnixTime,
    ) -> Result<ServerCertVerified, rustls::Error> {
        let fingerprint = certificate_fingerprint(end_entity);
        if self.pin.accepts(&fingerprint) {
            *self
                .observed
                .fingerprint
                .lock()
                .unwrap_or_else(PoisonError::into_inner) = Some(fingerprint);
            Ok(ServerCertVerified::assertion())
        } else {
            self.observed.mismatch.store(true, Ordering::Relaxed);
            Err(rustls::Error::InvalidCertificate(
                CertificateError::ApplicationVerificationFailure,
            ))
        }
    }

    // Exclue de la couverture : le client n'accepte que TLS 1.3, `rustls` ne l'appelle jamais.
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn verify_tls12_signature(
        &self,
        message: &[u8],
        cert: &CertificateDer<'_>,
        dss: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, rustls::Error> {
        verify_tls12_signature(
            message,
            cert,
            dss,
            &self.provider.signature_verification_algorithms,
        )
    }

    fn verify_tls13_signature(
        &self,
        message: &[u8],
        cert: &CertificateDer<'_>,
        dss: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, rustls::Error> {
        verify_tls13_signature(
            message,
            cert,
            dss,
            &self.provider.signature_verification_algorithms,
        )
    }

    fn supported_verify_schemes(&self) -> Vec<SignatureScheme> {
        self.provider
            .signature_verification_algorithms
            .supported_schemes()
    }
}

#[cfg(test)]
mod tests {
    use std::io;
    use std::net::TcpListener;
    use std::sync::LazyLock;

    use axum::Router;
    use axum::body::{Body, Bytes};
    use futures_util::{StreamExt, stream};
    use pswd_core::crypto::KdfParams;
    use pswd_core::{Timestamp, Vault, VaultFile};
    use pswd_server::{
        Identity, KeyPurpose, ShutdownHandle, Storage, issue_connection_key, router, serve,
        server_config,
    };

    use super::*;

    static VAULT: LazyLock<Vec<u8>> = LazyLock::new(|| {
        let params = KdfParams::new(8 * 1024, 1, 1).unwrap();
        let password = Secret::new("maître".to_owned());
        let (file, _) =
            VaultFile::create(&password, params, Timestamp::from_millis(1_000)).unwrap();
        file.encrypt(&Vault::new()).unwrap()
    });

    /// Lance un serveur HTTPS dans son propre thread ; renvoie son adresse et son empreinte.
    fn start(app: impl FnOnce([u8; FINGERPRINT_LEN]) -> Router) -> (String, [u8; FINGERPRINT_LEN]) {
        let identity = Identity::generate().unwrap();
        let fingerprint = identity.fingerprint();
        let config = server_config(&identity).unwrap();
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap().to_string();
        let server = serve(listener, app(fingerprint), config, ShutdownHandle::new());
        let runtime = tokio::runtime::Runtime::new().unwrap();
        // Le serveur tourne jusqu'à la fin du processus de test.
        std::thread::spawn(move || runtime.block_on(server));
        (address, fingerprint)
    }

    /// Vrai serveur neuf ; renvoie son adresse, son empreinte et sa clé administrateur.
    fn real_server() -> (String, [u8; FINGERPRINT_LEN], ConnectionKey) {
        let mut key = None;
        let (address, fingerprint) = start(|fingerprint| {
            let mut storage = Storage::open_in_memory().unwrap();
            let text =
                issue_connection_key(&mut storage, &fingerprint, &KeyPurpose::Admin).unwrap();
            key = Some(ConnectionKey::parse(text.expose()).unwrap());
            router(storage, fingerprint)
        });
        (address, fingerprint, key.unwrap())
    }

    fn client_of(address: &str, connection: &Connection) -> ServerClient {
        ServerClient::new(
            vec![address.to_owned()],
            connection.fingerprint,
            connection.token.clone(),
        )
        .unwrap()
    }

    /// Client de l'administrateur d'un vrai serveur neuf, et l'adresse de ce serveur.
    fn admin() -> (ServerClient, String) {
        let (address, _, key) = real_server();
        let connection =
            ServerClient::connect(vec![address.clone()], &key, "PC", Some("Arthur")).unwrap();
        (client_of(&address, &connection), address)
    }

    /// Adresse qui coupe toute connexion. Un port simplement libéré pourrait être repris par
    /// le serveur d'un autre test.
    fn closed_address() -> String {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap().to_string();
        std::thread::spawn(move || {
            for connection in listener.incoming() {
                drop(connection);
            }
        });
        address
    }

    /// Faux serveur qui répond toujours la même chose.
    fn fake_server<H, T>(response: H) -> ServerClient
    where
        H: axum::handler::Handler<T, ()>,
        T: 'static,
    {
        let (address, fingerprint) = start(|_| Router::new().fallback(response));
        ServerClient::new(vec![address], fingerprint, Secret::new("jeton".to_owned())).unwrap()
    }

    #[test]
    fn admin_key_connects_the_first_device() {
        let (address, fingerprint, key) = real_server();
        let connection =
            ServerClient::connect(vec![address.clone()], &key, "PC", Some("Arthur")).unwrap();
        assert!(connection.is_admin);
        assert_eq!(connection.fingerprint, fingerprint);
        assert_eq!(connection.token.expose().len(), 64);
        let client = client_of(&address, &connection);
        assert_eq!(client.list_vaults().unwrap(), []);
    }

    #[test]
    fn connection_key_is_used_once() {
        let (address, _, key) = real_server();
        ServerClient::connect(vec![address.clone()], &key, "PC", Some("Arthur")).unwrap();
        assert_eq!(
            ServerClient::connect(vec![address], &key, "PC", Some("Arthur")).err(),
            Some(PortError::new(
                "clé de connexion invalide, expirée ou déjà utilisée"
            ))
        );
    }

    #[test]
    fn admin_key_needs_an_account_name() {
        let (address, _, key) = real_server();
        assert_eq!(
            ServerClient::connect(vec![address], &key, "PC", None).err(),
            Some(PortError::new("nom d'appareil ou de compte invalide"))
        );
    }

    #[test]
    fn key_of_another_server_is_refused() {
        let (address, _, _) = real_server();
        let key = ConnectionKey::generate(&[0; FINGERPRINT_LEN]).unwrap();
        assert_eq!(
            ServerClient::connect(vec![address], &key, "PC", Some("Arthur")).err(),
            Some(PortError::new("le serveur n'est pas celui attendu"))
        );
    }

    #[test]
    fn vault_round_trip() {
        let (client, _) = admin();
        let mut vault = client.vault(client.create_vault("perso").unwrap());
        assert_eq!(vault.fetch().unwrap(), None);

        vault.push(&VAULT, None).unwrap();
        let fetched = vault.fetch().unwrap().unwrap();
        assert_eq!(fetched.file, *VAULT);
        assert_eq!(fetched.version, RemoteVersion("\"1\"".to_owned()));

        vault.push(&VAULT, Some(&fetched.version)).unwrap();
        assert_eq!(
            vault.fetch().unwrap().unwrap().version,
            RemoteVersion("\"2\"".to_owned())
        );
    }

    #[test]
    fn list_shows_vaults_and_versions() {
        let (client, _) = admin();
        let published = client.create_vault("perso").unwrap();
        client.vault(published).push(&VAULT, None).unwrap();
        let empty = client.create_vault("travail").unwrap();

        let mut expected = vec![
            RemoteVaultInfo {
                id: published,
                name: "perso".to_owned(),
                version: Some(RemoteVersion("\"1\"".to_owned())),
            },
            RemoteVaultInfo {
                id: empty,
                name: "travail".to_owned(),
                version: None,
            },
        ];
        expected.sort_by_key(|vault| vault.id);
        let mut listed = client.list_vaults().unwrap();
        listed.sort_by_key(|vault| vault.id);
        assert_eq!(listed, expected);
    }

    #[test]
    fn outdated_push_is_a_conflict() {
        let (client, _) = admin();
        let mut vault = client.vault(client.create_vault("perso").unwrap());
        assert_eq!(
            vault.push(&VAULT, Some(&RemoteVersion("\"1\"".to_owned()))),
            Err(PushError::Conflict)
        );
        vault.push(&VAULT, None).unwrap();
        assert_eq!(vault.push(&VAULT, None), Err(PushError::Conflict));
    }

    #[test]
    fn unknown_vault_is_reported() {
        let (client, _) = admin();
        let mut vault = client.vault(Uuid::new_v4());
        let unknown = PortError::new("coffre inconnu du serveur");
        assert_eq!(vault.fetch(), Err(unknown.clone()));
        assert_eq!(vault.push(&VAULT, None), Err(PushError::Failed(unknown)));
    }

    #[test]
    fn invalid_vault_name_is_refused_by_the_server() {
        let (client, _) = admin();
        assert_eq!(
            client.create_vault("coffre perso"),
            Err(PortError::new("nom de coffre refusé par le serveur"))
        );
    }

    #[test]
    fn invalid_vault_file_is_refused_by_the_server() {
        let (client, _) = admin();
        let mut vault = client.vault(client.create_vault("perso").unwrap());
        assert_eq!(
            vault.push(b"pas un coffre", None),
            Err(PushError::Failed(PortError::new(
                "réponse inattendue du serveur (400)"
            )))
        );
    }

    #[test]
    fn members_are_added_by_administrators_only() {
        let (admin, address) = admin();
        let key = admin.add_member("Camille").unwrap();
        let key = ConnectionKey::parse(key.expose()).unwrap();
        let connection =
            ServerClient::connect(vec![address.clone()], &key, "Téléphone", None).unwrap();
        assert!(!connection.is_admin);

        let member = client_of(&address, &connection);
        assert_eq!(
            member.add_member("Dominique").err(),
            Some(PortError::new("action réservée aux administrateurs"))
        );
        assert_eq!(
            admin.add_member(" ").err(),
            Some(PortError::new("nom invalide"))
        );
    }

    #[test]
    fn new_device_joins_the_same_account() {
        let (client, address) = admin();
        let id = client.create_vault("perso").unwrap();
        let key = ConnectionKey::parse(client.new_device_key().unwrap().expose()).unwrap();
        let connection =
            ServerClient::connect(vec![address.clone()], &key, "Portable", None).unwrap();
        assert!(connection.is_admin);
        let second = client_of(&address, &connection);
        assert_eq!(second.list_vaults().unwrap()[0].id, id);
    }

    #[test]
    fn disconnected_device_is_no_longer_recognized() {
        let (client, _) = admin();
        client.disconnect().unwrap();
        assert_eq!(
            client.list_vaults().err(),
            Some(PortError::new(
                "appareil non reconnu par le serveur : déconnecté ou révoqué"
            ))
        );
    }

    #[test]
    fn other_certificate_is_refused() {
        let (address, fingerprint, _) = real_server();
        let mut other = fingerprint;
        other[0] ^= 1;
        let client =
            ServerClient::new(vec![address], other, Secret::new("jeton".to_owned())).unwrap();
        let mismatch = PortError::new("le serveur n'est pas celui attendu");
        assert_eq!(client.create_vault("perso"), Err(mismatch.clone()));
        assert_eq!(
            client.vault(Uuid::new_v4()).push(&VAULT, None),
            Err(PushError::Failed(mismatch))
        );
    }

    #[test]
    fn unreachable_server_is_offline() {
        let client = ServerClient::new(
            vec![closed_address()],
            [0; FINGERPRINT_LEN],
            Secret::new("jeton".to_owned()),
        )
        .unwrap();
        let offline = PortError::new("serveur injoignable");
        assert_eq!(client.vault(Uuid::new_v4()).fetch(), Err(offline.clone()));
        assert_eq!(client.disconnect(), Err(offline));
    }

    #[test]
    fn next_address_is_tried_when_one_fails() {
        let (admin, address) = admin();
        let (impostor, _, _) = real_server();
        let client = ServerClient {
            addresses: vec![closed_address(), impostor, address],
            ..admin
        };
        let mut vault = client.vault(client.create_vault("perso").unwrap());
        assert_eq!(vault.fetch().unwrap(), None);
    }

    #[test]
    fn mismatch_is_forgotten_by_the_next_request() {
        let (admin, address) = admin();
        let (impostor, _, _) = real_server();
        let client = ServerClient {
            addresses: vec![impostor, address],
            ..admin
        };
        client.create_vault("perso").unwrap();
        let offline = ServerClient {
            addresses: vec![closed_address()],
            ..client
        };
        assert_eq!(
            offline.vault(Uuid::new_v4()).fetch(),
            Err(PortError::new("serveur injoignable"))
        );
    }

    #[test]
    fn connection_requests_carry_no_token() {
        use axum::http::HeaderMap;
        use axum::http::header::AUTHORIZATION;

        let authorized = Arc::new(AtomicBool::new(true));
        let seen = Arc::clone(&authorized);
        let (address, fingerprint) = start(move |_| {
            Router::new().fallback(move |headers: HeaderMap| async move {
                seen.store(headers.contains_key(AUTHORIZATION), Ordering::Relaxed);
                StatusCode::NO_CONTENT
            })
        });
        let prefix = ConnectionKey::generate(&fingerprint)
            .unwrap()
            .fingerprint_prefix();
        let anonymous = ServerClient::build(vec![address], Pin::Prefix(prefix), None).unwrap();
        assert_eq!(anonymous.disconnect(), Ok(()));
        assert!(!authorized.load(Ordering::Relaxed));
    }

    #[test]
    fn refusals_are_explained() {
        let cases = [
            (
                StatusCode::UNAUTHORIZED,
                "appareil non reconnu par le serveur : déconnecté ou révoqué",
            ),
            (StatusCode::FORBIDDEN, "action réservée aux administrateurs"),
            (
                StatusCode::TOO_MANY_REQUESTS,
                "trop d'essais refusés : réessayez dans quelques minutes",
            ),
            (
                StatusCode::IM_A_TEAPOT,
                "réponse inattendue du serveur (418)",
            ),
        ];
        for (status, message) in cases {
            assert_eq!(status_error(status), PortError::new(message));
        }
    }

    #[test]
    fn unexpected_statuses_are_reported() {
        let client = fake_server(|| async { StatusCode::IM_A_TEAPOT });
        let teapot = PortError::new("réponse inattendue du serveur (418)");
        assert_eq!(client.create_vault("perso"), Err(teapot.clone()));
        assert_eq!(client.list_vaults(), Err(teapot.clone()));
        assert_eq!(client.new_device_key().err(), Some(teapot.clone()));
        assert_eq!(client.disconnect(), Err(teapot.clone()));
        let mut vault = client.vault(Uuid::new_v4());
        assert_eq!(vault.fetch(), Err(teapot.clone()));
        assert_eq!(
            vault.push(&VAULT, None),
            Err(PushError::Failed(teapot.clone()))
        );

        let key = ConnectionKey::generate(&client.observed.fingerprint().unwrap()).unwrap();
        assert_eq!(
            ServerClient::connect(client.addresses.clone(), &key, "PC", None).err(),
            Some(teapot)
        );
    }

    #[test]
    fn invalid_responses_are_refused() {
        let created = fake_server(|| async { (StatusCode::CREATED, "pas du JSON") });
        let invalid = PortError::new("réponse du serveur invalide");
        assert_eq!(created.create_vault("perso"), Err(invalid.clone()));
        assert_eq!(created.add_member("Camille").err(), Some(invalid.clone()));
        let key = ConnectionKey::generate(&created.observed.fingerprint().unwrap()).unwrap();
        assert_eq!(
            ServerClient::connect(created.addresses.clone(), &key, "PC", None).err(),
            Some(invalid.clone())
        );

        let ok = fake_server(|| async { "sans version" });
        assert_eq!(ok.list_vaults(), Err(invalid.clone()));
        assert_eq!(ok.vault(Uuid::new_v4()).fetch(), Err(invalid));
    }

    /// Corps coupé après les en-têtes, une fois la réponse partie.
    fn cut_body() -> Body {
        let start = stream::once(async { Ok(Bytes::from_static(b"{")) });
        let cut = stream::once(async {
            tokio::time::sleep(Duration::from_millis(100)).await;
            Err(io::Error::other("coupure"))
        });
        Body::from_stream(start.chain(cut))
    }

    #[test]
    fn interrupted_responses_are_reported() {
        let interrupted = PortError::new("connexion au serveur interrompue");
        let created = fake_server(|| async { (StatusCode::CREATED, cut_body()) });
        assert_eq!(created.create_vault("perso"), Err(interrupted.clone()));
        assert_eq!(created.new_device_key().err(), Some(interrupted.clone()));
        let key = ConnectionKey::generate(&created.observed.fingerprint().unwrap()).unwrap();
        assert_eq!(
            ServerClient::connect(created.addresses.clone(), &key, "PC", None).err(),
            Some(interrupted.clone())
        );

        let ok = fake_server(|| async { ([(ETAG, "\"1\"")], cut_body()) });
        assert_eq!(ok.list_vaults(), Err(interrupted.clone()));
        assert_eq!(ok.vault(Uuid::new_v4()).fetch(), Err(interrupted));
    }
}
