//! Connexion des appareils, clés de connexion, déconnexion.

use std::net::SocketAddr;

use axum::Json;
use axum::extract::{ConnectInfo, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use pswd_core::ConnectionKey;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::{ApiError, Shared, digest, issue_connection_key, new_token};
use crate::clock::now;
use crate::storage::{Caller, KeyPurpose, NewDevice, StorageError};

/// Longueur maximale d'un nom de compte ou d'appareil, en caractères.
const MAX_NAME_CHARS: usize = 64;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct ConnectRequest {
    key: String,
    device_name: String,
    /// Nom du compte créé par une clé administrateur ; ignoré sinon.
    account_name: Option<String>,
}

#[derive(Serialize)]
struct Connected {
    token: String,
    account_id: Uuid,
    device_id: Uuid,
    is_admin: bool,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct MemberRequest {
    name: String,
}

#[derive(Serialize)]
struct IssuedKey {
    key: String,
}

/// Nom lisible : ni vide, ni espaces en bordure, ni caractère de contrôle.
fn is_valid_name(name: &str) -> bool {
    !name.is_empty()
        && name.trim() == name
        && name.chars().count() <= MAX_NAME_CHARS
        && !name.chars().any(char::is_control)
}

/// Échange une clé de connexion contre un jeton d'appareil, montré une seule fois.
pub(super) async fn connect(
    State(state): State<Shared>,
    ConnectInfo(address): ConnectInfo<SocketAddr>,
    Json(request): Json<ConnectRequest>,
) -> Result<Response, ApiError> {
    let ip = address.ip();
    state.check_not_blocked(ip)?;
    let names_valid = is_valid_name(&request.device_name)
        && request.account_name.as_deref().is_none_or(is_valid_name);
    if !names_valid {
        return Err(ApiError::InvalidName);
    }
    let Ok(key) = ConnectionKey::parse(&request.key) else {
        state.record_failure(ip);
        return Err(ApiError::Unauthorized);
    };
    let token = new_token()?;
    let token_hash = digest(token.expose().as_bytes());
    let device = NewDevice {
        name: &request.device_name,
        token_hash: &token_hash,
        account_name: request.account_name.as_deref(),
    };
    let redeemed = state
        .storage()
        .redeem_connection_key(&digest(key.secret()), now(), &device);
    let caller = match redeemed {
        Ok(caller) => caller,
        Err(StorageError::InvalidKey) => {
            state.record_failure(ip);
            return Err(ApiError::Unauthorized);
        }
        Err(other) => return Err(other.into()),
    };
    let connected = Connected {
        token: token.expose().to_owned(),
        account_id: caller.account,
        device_id: caller.device,
        is_admin: caller.is_admin,
    };
    Ok((StatusCode::CREATED, Json(connected)).into_response())
}

/// Clé de connexion d'un nouveau membre, réservée aux administrateurs.
pub(super) async fn add_member(
    State(state): State<Shared>,
    caller: Caller,
    Json(request): Json<MemberRequest>,
) -> Result<Response, ApiError> {
    if !caller.is_admin {
        return Err(ApiError::Forbidden);
    }
    if !is_valid_name(&request.name) {
        return Err(ApiError::InvalidName);
    }
    let purpose = KeyPurpose::Member { name: request.name };
    issued(&state, &purpose)
}

/// Clé de connexion d'un appareil de plus pour le compte de l'appelant.
pub(super) async fn new_device_key(
    State(state): State<Shared>,
    caller: Caller,
) -> Result<Response, ApiError> {
    issued(
        &state,
        &KeyPurpose::Device {
            account: caller.account,
        },
    )
}

fn issued(state: &Shared, purpose: &KeyPurpose) -> Result<Response, ApiError> {
    let key = issue_connection_key(&mut state.storage(), &state.fingerprint, purpose)?;
    let issued = IssuedKey {
        key: key.expose().to_owned(),
    };
    Ok((StatusCode::CREATED, Json(issued)).into_response())
}

/// Déconnexion : le jeton de l'appareil est révoqué.
pub(super) async fn disconnect(
    State(state): State<Shared>,
    caller: Caller,
) -> Result<Response, ApiError> {
    state.storage().revoke_device(caller.device)?;
    Ok(StatusCode::NO_CONTENT.into_response())
}

#[cfg(test)]
mod tests {
    use axum::Router;
    use serde_json::json;

    use super::super::testing::*;
    use super::{ConnectionKey, MAX_NAME_CHARS, Response, StatusCode, Uuid, is_valid_name};
    use crate::limiter::MAX_FAILURES;

    async fn try_connect(app: &Router, body: serde_json::Value) -> Response {
        send(app, request("POST", "/v1/connect", None, Some(body))).await
    }

    #[test]
    fn names_must_be_readable() {
        assert!(is_valid_name("PC du bureau"));
        assert!(is_valid_name(&"é".repeat(MAX_NAME_CHARS)));
        for name in ["", " PC", "PC ", "P\nC", &"é".repeat(MAX_NAME_CHARS + 1)] {
            assert!(!is_valid_name(name), "{name:?}");
        }
    }

    #[tokio::test]
    async fn admin_key_makes_the_first_device_an_administrator() {
        let (app, key) = fresh_app();
        let body = json!({ "key": key, "device_name": "PC", "account_name": "Arthur" });
        let response = try_connect(&app, body.clone()).await;
        assert_eq!(response.status(), StatusCode::CREATED);
        let connected = json(response).await;
        assert_eq!(connected["is_admin"], true);
        assert_eq!(connected["token"].as_str().unwrap().len(), 64);
        assert!(
            connected["account_id"]
                .as_str()
                .unwrap()
                .parse::<Uuid>()
                .is_ok()
        );
        assert!(
            connected["device_id"]
                .as_str()
                .unwrap()
                .parse::<Uuid>()
                .is_ok()
        );

        let response = try_connect(&app, body).await;
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn admin_key_requires_an_account_name() {
        let (app, key) = fresh_app();
        let response = try_connect(&app, json!({ "key": key, "device_name": "PC" })).await;
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        connect(&app, &key, Some("Arthur")).await;
    }

    #[tokio::test]
    async fn invalid_names_are_refused() {
        let (app, key) = fresh_app();
        for (device, account) in [("", "Arthur"), ("PC", " Arthur")] {
            let body = json!({ "key": key, "device_name": device, "account_name": account });
            assert_eq!(
                try_connect(&app, body).await.status(),
                StatusCode::BAD_REQUEST
            );
        }
        connect(&app, &key, Some("Arthur")).await;
    }

    #[tokio::test]
    async fn malformed_request_is_refused() {
        let (app, key) = fresh_app();
        let body = json!({ "key": key, "device_name": "PC", "account_name": "A", "role": "x" });
        let response = try_connect(&app, body).await;
        assert!(response.status().is_client_error());
        connect(&app, &key, Some("Arthur")).await;
    }

    #[tokio::test]
    async fn wrong_keys_are_refused_then_blocked() {
        let (app, key) = fresh_app();
        let unreadable = json!({ "key": "pas une clé", "device_name": "PC" });
        assert_eq!(
            try_connect(&app, unreadable).await.status(),
            StatusCode::UNAUTHORIZED
        );
        let other = issue_other_key();
        for _ in 1..MAX_FAILURES {
            let body = json!({ "key": other, "device_name": "PC", "account_name": "A" });
            assert_eq!(
                try_connect(&app, body).await.status(),
                StatusCode::UNAUTHORIZED
            );
        }
        let body = json!({ "key": key, "device_name": "PC", "account_name": "A" });
        assert_eq!(
            try_connect(&app, body).await.status(),
            StatusCode::TOO_MANY_REQUESTS
        );
    }

    /// Clé bien formée mais inconnue du serveur.
    fn issue_other_key() -> String {
        ConnectionKey::generate(&FINGERPRINT)
            .unwrap()
            .encode()
            .expose()
            .to_owned()
    }

    #[tokio::test]
    async fn member_gets_a_named_account() {
        let (app, admin) = app_with_admin().await;
        let response = send(
            &app,
            request(
                "POST",
                "/v1/members",
                Some(&admin),
                Some(json!({ "name": "Camille" })),
            ),
        )
        .await;
        assert_eq!(response.status(), StatusCode::CREATED);
        let key = json(response).await["key"].as_str().unwrap().to_owned();
        assert!(ConnectionKey::parse(&key).unwrap().matches(&FINGERPRINT));

        let body = json!({ "key": key, "device_name": "Téléphone" });
        let connected = json(try_connect(&app, body).await).await;
        assert_eq!(connected["is_admin"], false);
    }

    #[tokio::test]
    async fn only_administrators_add_members() {
        let (app, admin) = app_with_admin().await;
        let member = add_member(&app, &admin, "Camille").await;
        let body = Some(json!({ "name": "Dominique" }));
        let response = send(&app, request("POST", "/v1/members", Some(&member), body)).await;
        assert_eq!(response.status(), StatusCode::FORBIDDEN);
        let body = Some(json!({ "name": "" }));
        let response = send(&app, request("POST", "/v1/members", Some(&admin), body)).await;
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn device_key_adds_a_device_to_the_same_account() {
        let (app, admin) = app_with_admin().await;
        let member = add_member(&app, &admin, "Camille").await;
        let response = send(
            &app,
            request("POST", "/v1/devices/keys", Some(&member), None),
        )
        .await;
        assert_eq!(response.status(), StatusCode::CREATED);
        let key = json(response).await["key"].as_str().unwrap().to_owned();
        let second = connect(&app, &key, None).await;

        let name = Some(serde_json::json!({ "name": "perso" }));
        let vault =
            json(send(&app, request("POST", "/v1/vaults", Some(&member), name)).await).await;
        let listed =
            json(send(&app, request("GET", "/v1/vaults", Some(&second), None)).await).await;
        assert_eq!(listed[0]["id"], vault["id"]);
    }

    #[tokio::test]
    async fn disconnected_device_loses_access() {
        let (app, admin) = app_with_admin().await;
        let response = send(
            &app,
            request("DELETE", "/v1/devices/self", Some(&admin), None),
        )
        .await;
        assert_eq!(response.status(), StatusCode::NO_CONTENT);
        let response = send(&app, request("GET", "/v1/vaults", Some(&admin), None)).await;
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    }
}
