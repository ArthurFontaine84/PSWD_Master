//! Routes HTTP : traduisent les requêtes en appels à [`Storage`].

mod accounts;
mod sharing;
mod vaults;

use std::net::{IpAddr, SocketAddr};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::time::Instant;

use axum::Router;
use axum::extract::{ConnectInfo, DefaultBodyLimit, FromRequestParts};
use axum::http::StatusCode;
use axum::http::header::AUTHORIZATION;
use axum::http::request::Parts;
use axum::response::{IntoResponse, Response};
use axum::routing::{delete, get, post};
use pswd_core::crypto::FINGERPRINT_LEN;
use pswd_core::{ConnectionKey, MAX_VAULT_FILE_LEN, Secret, Timestamp};
use sha2::{Digest, Sha256};
use zeroize::Zeroizing;

use crate::clock::now;
use crate::limiter::Limiter;
use crate::storage::{Caller, KeyPurpose, Storage, StorageError, random_unavailable};

/// Taille maximale d'une requête JSON.
const MAX_JSON_LEN: usize = 16 * 1024;
/// Durée de validité d'une clé de connexion : 7 jours.
const KEY_VALIDITY_MILLIS: i64 = 7 * 24 * 60 * 60 * 1000;
const TOKEN_LEN: usize = 32;

/// État partagé par les routes ; public seulement pour l'extracteur [Caller].
pub struct AppState {
    storage: Mutex<Storage>,
    limiter: Mutex<Limiter>,
    fingerprint: [u8; FINGERPRINT_LEN],
}

type Shared = Arc<AppState>;

impl AppState {
    /// Une requête qui a paniqué n'a pas pu laisser la base à moitié modifiée (transactions) :
    /// le verrou reste utilisable.
    fn storage(&self) -> MutexGuard<'_, Storage> {
        self.storage.lock().unwrap_or_else(PoisonError::into_inner)
    }

    fn limiter(&self) -> MutexGuard<'_, Limiter> {
        self.limiter.lock().unwrap_or_else(PoisonError::into_inner)
    }

    fn check_not_blocked(&self, ip: IpAddr) -> Result<(), ApiError> {
        if self.limiter().is_blocked(ip, Instant::now()) {
            Err(ApiError::TooManyAttempts)
        } else {
            Ok(())
        }
    }

    fn record_failure(&self, ip: IpAddr) {
        self.limiter().record_failure(ip, Instant::now());
    }
}

/// `fingerprint` : empreinte du certificat du serveur, inscrite dans les clés de connexion.
pub fn router(storage: Storage, fingerprint: [u8; FINGERPRINT_LEN]) -> Router {
    let state = Arc::new(AppState {
        storage: Mutex::new(storage),
        limiter: Mutex::new(Limiter::default()),
        fingerprint,
    });
    Router::new()
        .route("/v1/connect", post(accounts::connect))
        .route("/v1/members", post(accounts::add_member))
        .route("/v1/devices/keys", post(accounts::new_device_key))
        .route("/v1/devices/self", delete(accounts::disconnect))
        .route("/v1/accounts", get(sharing::accounts))
        .route("/v1/vaults", get(vaults::list).post(vaults::create))
        .route("/v1/vaults/{id}/members", get(sharing::members))
        .route("/v1/vaults/{id}/members/self", delete(sharing::leave))
        .route("/v1/vaults/{id}/invitations", post(sharing::invite))
        .route("/v1/invitations", get(sharing::received))
        .route("/v1/invitations/{id}", delete(sharing::delete_invitation))
        .route("/v1/invitations/{id}/accept", post(sharing::accept))
        .route(
            "/v1/vaults/{id}",
            get(vaults::fetch)
                .put(vaults::publish)
                .layer(DefaultBodyLimit::max(MAX_VAULT_FILE_LEN)),
        )
        .layer(DefaultBodyLimit::max(MAX_JSON_LEN))
        .with_state(state)
}

/// Crée une clé de connexion valable 7 jours et renvoie son texte, à montrer une seule fois.
///
/// # Errors
///
/// Si l'aléa du système manque ou si l'écriture dans la base échoue.
pub fn issue_connection_key(
    storage: &mut Storage,
    fingerprint: &[u8; FINGERPRINT_LEN],
    purpose: &KeyPurpose,
) -> Result<Secret, StorageError> {
    let key = ConnectionKey::generate(fingerprint).map_err(random_unavailable)?;
    let expires_at = Timestamp::from_millis(now().as_millis().saturating_add(KEY_VALIDITY_MILLIS));
    storage.add_connection_key(&digest(key.secret()), purpose, expires_at)?;
    Ok(key.encode())
}

/// Empreinte SHA-256 sous laquelle jetons et clés sont rangés.
fn digest(secret: &[u8]) -> [u8; 32] {
    Sha256::digest(secret).into()
}

/// Jeton d'appareil : 32 octets aléatoires en hexadécimal.
fn new_token() -> Result<Secret, StorageError> {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut bytes = Zeroizing::new([0; TOKEN_LEN]);
    getrandom::fill(bytes.as_mut_slice()).map_err(random_unavailable)?;
    let mut text = String::with_capacity(TOKEN_LEN * 2);
    for &byte in bytes.iter() {
        text.push(char::from(HEX[usize::from(byte >> 4)]));
        text.push(char::from(HEX[usize::from(byte & 0x0F)]));
    }
    Ok(Secret::new(text))
}

/// Adresse IP du client, fournie par le serveur HTTP.
async fn client_ip(parts: &mut Parts, state: &Shared) -> Result<IpAddr, ApiError> {
    let ConnectInfo(address) = ConnectInfo::<SocketAddr>::from_request_parts(parts, state)
        .await
        .map_err(|_| ApiError::Internal)?;
    Ok(address.ip())
}

/// Appareil authentifié par `Authorization: Bearer <jeton>`. Un jeton invalide compte comme un
/// essai raté pour l'adresse IP.
impl FromRequestParts<Shared> for Caller {
    type Rejection = ApiError;

    async fn from_request_parts(parts: &mut Parts, state: &Shared) -> Result<Self, ApiError> {
        let ip = client_ip(parts, state).await?;
        state.check_not_blocked(ip)?;
        let token = parts
            .headers
            .get(AUTHORIZATION)
            .and_then(|value| value.to_str().ok())
            .and_then(|value| value.strip_prefix("Bearer "))
            .ok_or(ApiError::Unauthorized)?;
        let caller = state.storage().authenticate(&digest(token.as_bytes()))?;
        caller.ok_or_else(|| {
            state.record_failure(ip);
            ApiError::Unauthorized
        })
    }
}

#[derive(Debug, PartialEq, Eq)]
pub enum ApiError {
    NotFound,
    /// Le compte a quitté le coffre ou en a été retiré.
    Gone,
    /// Coffre privé, ou compte invité inconnu.
    InvalidInvitation,
    /// Compte déjà membre ou déjà invité.
    AlreadyMember,
    PreconditionFailed,
    PreconditionRequired,
    InvalidVault,
    InvalidName,
    Unauthorized,
    Forbidden,
    TooManyAttempts,
    Internal,
}

impl From<StorageError> for ApiError {
    fn from(error: StorageError) -> Self {
        match error {
            StorageError::NotFound => Self::NotFound,
            StorageError::Gone => Self::Gone,
            StorageError::NotShared | StorageError::UnknownAccount => Self::InvalidInvitation,
            StorageError::AlreadyMember => Self::AlreadyMember,
            StorageError::Conflict => Self::PreconditionFailed,
            StorageError::MissingAccountName => Self::InvalidName,
            other => {
                eprintln!("erreur interne : {other}");
                Self::Internal
            }
        }
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let status = match self {
            Self::NotFound => StatusCode::NOT_FOUND,
            Self::Gone => StatusCode::GONE,
            Self::AlreadyMember => StatusCode::CONFLICT,
            Self::PreconditionFailed => StatusCode::PRECONDITION_FAILED,
            Self::PreconditionRequired => StatusCode::PRECONDITION_REQUIRED,
            Self::InvalidVault | Self::InvalidName | Self::InvalidInvitation => {
                StatusCode::BAD_REQUEST
            }
            Self::Unauthorized => StatusCode::UNAUTHORIZED,
            Self::Forbidden => StatusCode::FORBIDDEN,
            Self::TooManyAttempts => StatusCode::TOO_MANY_REQUESTS,
            Self::Internal => StatusCode::INTERNAL_SERVER_ERROR,
        };
        status.into_response()
    }
}

#[cfg(test)]
pub(crate) mod testing {
    use axum::body::Body;
    use axum::extract::connect_info::MockConnectInfo;
    use axum::http::Request;
    use http_body_util::BodyExt;
    use tower::ServiceExt;

    use super::*;

    pub(crate) const FINGERPRINT: [u8; FINGERPRINT_LEN] = [7; FINGERPRINT_LEN];

    fn with_client_address(app: Router) -> Router {
        app.layer(MockConnectInfo(SocketAddr::from((
            [192, 168, 1, 20],
            50_000,
        ))))
    }

    /// Serveur neuf et la clé administrateur affichée à l'installation.
    pub(crate) fn fresh_app() -> (Router, String) {
        let mut storage = Storage::open_in_memory().unwrap();
        let key = issue_connection_key(&mut storage, &FINGERPRINT, &KeyPurpose::Admin).unwrap();
        (
            with_client_address(router(storage, FINGERPRINT)),
            key.expose().to_owned(),
        )
    }

    /// Serveur avec un administrateur connecté ; renvoie son jeton.
    pub(crate) async fn app_with_admin() -> (Router, String) {
        let (app, key) = fresh_app();
        let token = connect(&app, &key, Some("Arthur")).await;
        (app, token)
    }

    pub(crate) async fn send(app: &Router, request: Request<Body>) -> Response {
        app.clone().oneshot(request).await.unwrap()
    }

    pub(crate) async fn body(response: Response) -> Vec<u8> {
        response
            .into_body()
            .collect()
            .await
            .unwrap()
            .to_bytes()
            .to_vec()
    }

    pub(crate) async fn json(response: Response) -> serde_json::Value {
        serde_json::from_slice(&body(response).await).unwrap()
    }

    pub(crate) fn request(
        method: &str,
        uri: &str,
        token: Option<&str>,
        body: Option<serde_json::Value>,
    ) -> Request<Body> {
        let mut builder = Request::builder().method(method).uri(uri);
        if let Some(token) = token {
            builder = builder.header("authorization", format!("Bearer {token}"));
        }
        match body {
            Some(json) => builder
                .header("content-type", "application/json")
                .body(Body::from(json.to_string()))
                .unwrap(),
            None => builder.body(Body::empty()).unwrap(),
        }
    }

    /// Connecte un appareil avec `key` et renvoie son jeton.
    pub(crate) async fn connect(app: &Router, key: &str, account_name: Option<&str>) -> String {
        let body = serde_json::json!({
            "key": key,
            "device_name": "PC",
            "account_name": account_name,
        });
        let response = send(app, request("POST", "/v1/connect", None, Some(body))).await;
        assert_eq!(response.status(), StatusCode::CREATED);
        json(response).await["token"].as_str().unwrap().to_owned()
    }

    /// Ajoute un membre et connecte son premier appareil ; renvoie son jeton.
    pub(crate) async fn add_member(app: &Router, admin: &str, name: &str) -> String {
        let body = serde_json::json!({ "name": name });
        let response = send(app, request("POST", "/v1/members", Some(admin), Some(body))).await;
        assert_eq!(response.status(), StatusCode::CREATED);
        let key = json(response).await["key"].as_str().unwrap().to_owned();
        connect(app, &key, None).await
    }
}

#[cfg(test)]
mod tests {
    use std::thread;

    use axum::http::Request;

    use super::testing::*;
    use super::*;

    #[test]
    fn tokens_are_random_hexadecimal() {
        let first = new_token().unwrap();
        let second = new_token().unwrap();
        assert_eq!(first.expose().len(), TOKEN_LEN * 2);
        let digits: String = first
            .expose()
            .chars()
            .filter(|c| matches!(c, '0'..='9' | 'a'..='f'))
            .collect();
        assert_eq!(digits, first.expose());
        assert_ne!(first.expose(), second.expose());
    }

    #[test]
    fn issued_key_points_to_the_server_certificate() {
        let mut storage = Storage::open_in_memory().unwrap();
        let text = issue_connection_key(&mut storage, &FINGERPRINT, &KeyPurpose::Admin).unwrap();
        assert!(
            ConnectionKey::parse(text.expose())
                .unwrap()
                .matches(&FINGERPRINT)
        );
    }

    #[test]
    fn issued_key_fails_on_a_broken_database() {
        let mut storage = Storage::open_in_memory().unwrap();
        storage.break_for_tests("connection_keys");
        assert!(matches!(
            issue_connection_key(&mut storage, &FINGERPRINT, &KeyPurpose::Admin),
            Err(StorageError::Database(_))
        ));
    }

    #[tokio::test]
    async fn requests_without_client_address_fail() {
        let app = router(Storage::open_in_memory().unwrap(), FINGERPRINT);
        let response = send(&app, request("GET", "/v1/vaults", Some("jeton"), None)).await;
        assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);
    }

    #[tokio::test]
    async fn missing_or_malformed_token_is_unauthorized() {
        let (app, _) = app_with_admin().await;
        let malformed = Request::get("/v1/vaults")
            .header("authorization", "Basic abc")
            .body(axum::body::Body::empty())
            .unwrap();
        for request in [request("GET", "/v1/vaults", None, None), malformed] {
            let response = send(&app, request).await;
            assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
            assert_eq!(body(response).await, Vec::<u8>::new());
        }
    }

    #[tokio::test]
    async fn repeated_invalid_tokens_block_the_address() {
        let (app, admin) = app_with_admin().await;
        for _ in 0..crate::limiter::MAX_FAILURES {
            let response = send(&app, request("GET", "/v1/vaults", Some("faux"), None)).await;
            assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
        }
        let response = send(&app, request("GET", "/v1/vaults", Some(&admin), None)).await;
        assert_eq!(response.status(), StatusCode::TOO_MANY_REQUESTS);
    }

    #[tokio::test]
    async fn oversized_json_is_refused() {
        let (app, admin) = app_with_admin().await;
        let name = "a".repeat(MAX_JSON_LEN);
        let body = serde_json::json!({ "name": name });
        let response = send(
            &app,
            request("POST", "/v1/members", Some(&admin), Some(body)),
        )
        .await;
        assert_eq!(response.status(), StatusCode::PAYLOAD_TOO_LARGE);
    }

    #[tokio::test]
    async fn storage_failure_is_an_internal_error() {
        let mut storage = Storage::open_in_memory().unwrap();
        let key = issue_connection_key(&mut storage, &FINGERPRINT, &KeyPurpose::Admin).unwrap();
        storage.break_for_tests("devices");
        let app = router(storage, FINGERPRINT).layer(axum::extract::connect_info::MockConnectInfo(
            SocketAddr::from(([127, 0, 0, 1], 1)),
        ));
        let response = send(&app, request("GET", "/v1/vaults", Some("jeton"), None)).await;
        assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);
        let body =
            serde_json::json!({ "key": key.expose(), "device_name": "PC", "account_name": "A" });
        let response = send(&app, request("POST", "/v1/connect", None, Some(body))).await;
        assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);
    }

    #[test]
    fn poisoned_lock_stays_usable() {
        let state = Arc::new(AppState {
            storage: Mutex::new(Storage::open_in_memory().unwrap()),
            limiter: Mutex::new(Limiter::default()),
            fingerprint: FINGERPRINT,
        });
        let clone = Arc::clone(&state);
        let panicked = thread::spawn(move || {
            let _storage = clone.storage.lock().unwrap();
            let _limiter = clone.limiter.lock().unwrap();
            panic!("requête interrompue");
        })
        .join();
        assert!(panicked.is_err());
        assert!(state.storage.is_poisoned());
        assert!(state.storage().authenticate(b"jeton").is_ok());
        let ip = IpAddr::from([127, 0, 0, 1]);
        state.record_failure(ip);
        assert!(state.check_not_blocked(ip).is_ok());
    }
}
