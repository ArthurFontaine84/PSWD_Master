//! Coffres : création, liste, lecture et publication conditionnelle.

use axum::Json;
use axum::body::Bytes;
use axum::extract::{Path, State};
use axum::http::header::{CONTENT_TYPE, ETAG, IF_MATCH, IF_NONE_MATCH};
use axum::http::{HeaderMap, HeaderValue, StatusCode};
use axum::response::{IntoResponse, Response};
use pswd_core::{VaultFile, is_valid_vault_name};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::{ApiError, Shared};
use crate::clock::now;
use crate::storage::Caller;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct CreateRequest {
    name: String,
    /// Coffre commun ; privé par défaut.
    #[serde(default)]
    shared: bool,
}

#[derive(Serialize)]
struct Created {
    id: Uuid,
}

#[derive(Serialize)]
struct Listed {
    id: Uuid,
    name: String,
    shared: bool,
    /// `ETag` de la version courante ; `null` si le coffre n'est pas encore publié.
    version: Option<String>,
}

pub(super) async fn create(
    State(state): State<Shared>,
    caller: Caller,
    Json(request): Json<CreateRequest>,
) -> Result<Response, ApiError> {
    if !is_valid_vault_name(&request.name) {
        return Err(ApiError::InvalidName);
    }
    let id = state
        .storage()
        .create_vault(caller.account, &request.name, request.shared)?;
    Ok((StatusCode::CREATED, Json(Created { id })).into_response())
}

pub(super) async fn list(
    State(state): State<Shared>,
    caller: Caller,
) -> Result<Response, ApiError> {
    let vaults = state.storage().list_vaults(caller.account)?;
    let listed: Vec<Listed> = vaults
        .into_iter()
        .map(|vault| Listed {
            id: vault.id,
            name: vault.name,
            shared: vault.shared,
            version: vault.version.map(etag),
        })
        .collect();
    Ok(Json(listed).into_response())
}

pub(super) async fn fetch(
    State(state): State<Shared>,
    caller: Caller,
    Path(id): Path<Uuid>,
) -> Result<Response, ApiError> {
    let Some(file) = state.storage().fetch(id, caller.account)? else {
        return Ok(StatusCode::NO_CONTENT.into_response());
    };
    let headers = [
        (CONTENT_TYPE, "application/octet-stream".to_owned()),
        (ETAG, etag(file.version)),
    ];
    Ok((headers, file.content).into_response())
}

pub(super) async fn publish(
    State(state): State<Shared>,
    caller: Caller,
    Path(id): Path<Uuid>,
    headers: HeaderMap,
    body: Bytes,
) -> Result<Response, ApiError> {
    let expected = expected_version(&headers)?;
    VaultFile::check_format(&body).map_err(|_| ApiError::InvalidVault)?;
    let version = state
        .storage()
        .publish(id, caller.account, &body, expected, now())?;
    Ok((StatusCode::NO_CONTENT, [(ETAG, etag(version))]).into_response())
}

/// Un compte supprime partout son coffre privé, ou le coffre commun dont il est le dernier
/// membre ; un coffre commun qui a d'autres membres se quitte.
pub(super) async fn delete(
    State(state): State<Shared>,
    caller: Caller,
    Path(id): Path<Uuid>,
) -> Result<StatusCode, ApiError> {
    state.storage().delete_own_vault(id, caller.account)?;
    Ok(StatusCode::NO_CONTENT)
}

fn etag(version: i64) -> String {
    format!("\"{version}\"")
}

/// Version attendue par une publication : `If-Match: "<version>"`, ou `If-None-Match: *` pour un
/// coffre jamais publié. Une publication sans condition est refusée.
fn expected_version(headers: &HeaderMap) -> Result<Option<i64>, ApiError> {
    if let Some(value) = headers.get(IF_MATCH) {
        return parse_etag(value)
            .map(Some)
            .ok_or(ApiError::PreconditionFailed);
    }
    match headers.get(IF_NONE_MATCH) {
        Some(value) if value == "*" => Ok(None),
        _ => Err(ApiError::PreconditionRequired),
    }
}

fn parse_etag(value: &HeaderValue) -> Option<i64> {
    value
        .to_str()
        .ok()?
        .strip_prefix('"')?
        .strip_suffix('"')?
        .parse()
        .ok()
}

#[cfg(test)]
mod tests {
    use axum::Router;
    use axum::body::Body;
    use axum::http::{Method, Request};
    use pswd_core::MAX_VAULT_FILE_LEN;

    use super::super::testing::*;
    use super::*;

    fn create_request(token: Option<&str>, name: &str) -> Request<Body> {
        let body = serde_json::json!({ "name": name });
        request("POST", "/v1/vaults", token, Some(body))
    }

    async fn create(app: &Router, token: &str) -> Uuid {
        create_named(app, token, "perso").await
    }

    async fn create_named(app: &Router, token: &str, name: &str) -> Uuid {
        let response = send(app, create_request(Some(token), name)).await;
        assert_eq!(response.status(), StatusCode::CREATED);
        json(response).await["id"]
            .as_str()
            .unwrap()
            .parse()
            .unwrap()
    }

    fn put(
        id: Uuid,
        token: &str,
        condition: Option<(&str, &str)>,
        content: Vec<u8>,
    ) -> Request<Body> {
        let mut request = Request::builder()
            .method(Method::PUT)
            .uri(format!("/v1/vaults/{id}"))
            .header("authorization", format!("Bearer {token}"));
        if let Some((name, value)) = condition {
            request = request.header(name, value);
        }
        request.body(Body::from(content)).unwrap()
    }

    fn get(id: Uuid, token: &str) -> Request<Body> {
        request("GET", &format!("/v1/vaults/{id}"), Some(token), None)
    }

    fn header<'a>(response: &'a Response, name: &str) -> &'a str {
        response.headers()[name].to_str().unwrap()
    }

    const FIRST: Option<(&str, &str)> = Some(("if-none-match", "*"));

    #[tokio::test]
    async fn created_vault_is_empty() {
        let (app, token) = app_with_admin().await;
        let id = create(&app, &token).await;
        let response = send(&app, get(id, &token)).await;
        assert_eq!(response.status(), StatusCode::NO_CONTENT);
        assert_eq!(body(response).await, Vec::<u8>::new());
    }

    #[tokio::test]
    async fn unknown_vault_is_not_found() {
        let (app, token) = app_with_admin().await;
        let response = send(&app, get(Uuid::new_v4(), &token)).await;
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn vault_of_another_account_looks_unknown() {
        let (app, admin) = app_with_admin().await;
        let member = add_member(&app, &admin, "Camille").await;
        let id = create(&app, &admin).await;
        send(&app, put(id, &admin, FIRST, VAULT.clone())).await;
        assert_eq!(
            send(&app, get(id, &member)).await.status(),
            StatusCode::NOT_FOUND
        );
        let response = send(
            &app,
            put(id, &member, Some(("if-match", "\"1\"")), VAULT.clone()),
        )
        .await;
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn an_account_deletes_its_own_vaults_everywhere() {
        let (app, token) = app_with_admin().await;
        let private = create(&app, &token).await;
        send(&app, put(private, &token, FIRST, VAULT.clone())).await;
        let body = serde_json::json!({ "name": "famille", "shared": true });
        let response = send(
            &app,
            request("POST", "/v1/vaults", Some(&token), Some(body)),
        )
        .await;
        let shared = json(response).await["id"].as_str().unwrap().to_owned();
        let delete =
            |id: String| request("DELETE", &format!("/v1/vaults/{id}"), Some(&token), None);

        let response = send(&app, delete(shared)).await;
        assert_eq!(response.status(), StatusCode::NO_CONTENT);
        let response = send(&app, delete(private.to_string())).await;
        assert_eq!(response.status(), StatusCode::NO_CONTENT);
        let response = send(&app, get(private, &token)).await;
        assert_eq!(response.status(), StatusCode::GONE);
        assert_eq!(
            json(response).await,
            serde_json::json!({ "reason": "deleted_everywhere" })
        );
        let response = send(&app, delete(Uuid::new_v4().to_string())).await;
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn vaults_require_a_token() {
        let (app, token) = app_with_admin().await;
        let id = create(&app, &token).await;
        for request in [
            create_request(None, "perso"),
            request("GET", "/v1/vaults", None, None),
            request("GET", &format!("/v1/vaults/{id}"), None, None),
            put(id, "faux", FIRST, VAULT.clone()),
            request("DELETE", &format!("/v1/vaults/{id}"), None, None),
        ] {
            assert_eq!(send(&app, request).await.status(), StatusCode::UNAUTHORIZED);
        }
    }

    #[tokio::test]
    async fn list_shows_the_account_vaults_and_versions() {
        let (app, admin) = app_with_admin().await;
        let member = add_member(&app, &admin, "Camille").await;
        let published = create_named(&app, &admin, "perso").await;
        send(&app, put(published, &admin, FIRST, VAULT.clone())).await;
        let creation = request(
            "POST",
            "/v1/vaults",
            Some(&admin),
            Some(serde_json::json!({ "name": "travail", "shared": true })),
        );
        let response = send(&app, creation).await;
        let empty: Uuid = json(response).await["id"]
            .as_str()
            .unwrap()
            .parse()
            .unwrap();
        create(&app, &member).await;

        let response = send(&app, request("GET", "/v1/vaults", Some(&admin), None)).await;
        assert_eq!(response.status(), StatusCode::OK);
        let mut expected = vec![
            serde_json::json!({ "id": published, "name": "perso", "shared": false, "version": "\"1\"" }),
            serde_json::json!({ "id": empty, "name": "travail", "shared": true, "version": null }),
        ];
        expected.sort_by_key(|vault| vault["id"].as_str().unwrap().to_owned());
        assert_eq!(json(response).await, serde_json::Value::Array(expected));
    }

    #[tokio::test]
    async fn vault_name_must_be_valid() {
        let (app, token) = app_with_admin().await;
        for name in ["", "coffre perso", "a/b", "con"] {
            let response = send(&app, create_request(Some(&token), name)).await;
            assert_eq!(response.status(), StatusCode::BAD_REQUEST, "{name:?}");
        }
        let response = send(&app, request("GET", "/v1/vaults", Some(&token), None)).await;
        assert_eq!(json(response).await, serde_json::json!([]));
    }

    #[tokio::test]
    async fn invalid_identifier_is_refused() {
        let (app, token) = app_with_admin().await;
        let response = send(&app, request("GET", "/v1/vaults/abc", Some(&token), None)).await;
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn published_vault_is_returned_with_its_version() {
        let (app, token) = app_with_admin().await;
        let id = create(&app, &token).await;
        let response = send(&app, put(id, &token, FIRST, VAULT.clone())).await;
        assert_eq!(response.status(), StatusCode::NO_CONTENT);
        assert_eq!(header(&response, "etag"), "\"1\"");

        let response = send(&app, get(id, &token)).await;
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(header(&response, "etag"), "\"1\"");
        assert_eq!(
            header(&response, "content-type"),
            "application/octet-stream"
        );
        assert_eq!(body(response).await, *VAULT);
    }

    #[tokio::test]
    async fn publication_follows_the_current_version() {
        let (app, token) = app_with_admin().await;
        let id = create(&app, &token).await;
        send(&app, put(id, &token, FIRST, VAULT.clone())).await;
        let response = send(
            &app,
            put(id, &token, Some(("if-match", "\"1\"")), VAULT.clone()),
        )
        .await;
        assert_eq!(response.status(), StatusCode::NO_CONTENT);
        assert_eq!(header(&response, "etag"), "\"2\"");
    }

    #[tokio::test]
    async fn outdated_publication_is_refused() {
        let (app, token) = app_with_admin().await;
        let id = create(&app, &token).await;
        send(&app, put(id, &token, FIRST, VAULT.clone())).await;
        for condition in [FIRST, Some(("if-match", "\"2\""))] {
            let response = send(&app, put(id, &token, condition, VAULT.clone())).await;
            assert_eq!(response.status(), StatusCode::PRECONDITION_FAILED);
        }
        assert_eq!(header(&send(&app, get(id, &token)).await, "etag"), "\"1\"");
    }

    #[tokio::test]
    async fn malformed_version_is_never_matched() {
        let (app, token) = app_with_admin().await;
        let id = create(&app, &token).await;
        send(&app, put(id, &token, FIRST, VAULT.clone())).await;
        for value in ["1", "\"1", "1\"", "\"un\"", "W/\"1\"", "\"é\""] {
            let response = send(
                &app,
                put(id, &token, Some(("if-match", value)), VAULT.clone()),
            )
            .await;
            assert_eq!(
                response.status(),
                StatusCode::PRECONDITION_FAILED,
                "{value}"
            );
        }
    }

    #[tokio::test]
    async fn unconditional_publication_is_refused() {
        let (app, token) = app_with_admin().await;
        let id = create(&app, &token).await;
        for condition in [None, Some(("if-none-match", "\"1\""))] {
            let response = send(&app, put(id, &token, condition, VAULT.clone())).await;
            assert_eq!(response.status(), StatusCode::PRECONDITION_REQUIRED);
        }
        assert_eq!(
            send(&app, get(id, &token)).await.status(),
            StatusCode::NO_CONTENT
        );
    }

    #[tokio::test]
    async fn invalid_vault_file_is_refused() {
        let (app, token) = app_with_admin().await;
        let id = create(&app, &token).await;
        let mut truncated = VAULT.clone();
        truncated.truncate(10);
        let response = send(&app, put(id, &token, FIRST, truncated)).await;
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        assert_eq!(
            send(&app, get(id, &token)).await.status(),
            StatusCode::NO_CONTENT
        );
    }

    #[tokio::test]
    async fn oversized_vault_is_refused() {
        let (app, token) = app_with_admin().await;
        let id = create(&app, &token).await;
        let mut largest = VAULT.clone();
        largest.resize(MAX_VAULT_FILE_LEN, 0);
        let response = send(&app, put(id, &token, FIRST, largest.clone())).await;
        assert_ne!(response.status(), StatusCode::PAYLOAD_TOO_LARGE);
        largest.push(0);
        let response = send(&app, put(id, &token, FIRST, largest)).await;
        assert_eq!(response.status(), StatusCode::PAYLOAD_TOO_LARGE);
    }

    #[tokio::test]
    async fn publishing_to_an_unknown_vault_fails() {
        let (app, token) = app_with_admin().await;
        let response = send(&app, put(Uuid::new_v4(), &token, FIRST, VAULT.clone())).await;
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
    }
}
