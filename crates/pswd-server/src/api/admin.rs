//! Panneau d'administration : réservé aux appareils des comptes administrateurs.

use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::http::header::CONTENT_TYPE;
use axum::response::{IntoResponse, Response};
use axum::routing::{delete, get, post, put};
use axum::{Json, Router};
use pswd_core::Timestamp;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::{Admin, ApiError, Shared};
use crate::clock::now;
use crate::storage::{
    AccountDetails, AccountSummary, DeviceSummary, PendingKey, PendingPurpose, Restored,
    ServerVault, VersionSummary,
};

/// Longueur maximale de l'adresse publique (nom DNS ou adresse IP, port compris).
const MAX_ADDRESS_LEN: usize = 255;

pub(super) fn routes() -> Router<Shared> {
    Router::new()
        .route("/v1/admin/accounts", get(accounts))
        .route("/v1/admin/accounts/{id}", delete(delete_account))
        .route("/v1/admin/accounts/{id}/admin", put(set_admin))
        .route("/v1/admin/devices/{id}", delete(revoke_device))
        .route("/v1/admin/keys", get(keys))
        .route("/v1/admin/keys/{id}", delete(cancel_key))
        .route("/v1/admin/vaults", get(vaults))
        .route("/v1/admin/vaults/{id}", delete(delete_vault))
        .route("/v1/admin/vaults/{id}/members", post(attach))
        .route(
            "/v1/admin/vaults/{id}/members/{account}",
            delete(remove_member),
        )
        .route("/v1/admin/vaults/{id}/versions", get(versions))
        .route(
            "/v1/admin/vaults/{id}/versions/{version}/restore",
            post(restore),
        )
        .route("/v1/admin/settings", put(settings))
        .route("/v1/admin/backup", get(backup))
}

#[derive(Serialize)]
struct Device {
    id: Uuid,
    name: String,
    created_at: Timestamp,
}

impl From<DeviceSummary> for Device {
    fn from(device: DeviceSummary) -> Self {
        Self {
            id: device.id,
            name: device.name,
            created_at: device.created_at,
        }
    }
}

#[derive(Serialize)]
struct Account {
    id: Uuid,
    name: String,
    is_admin: bool,
    devices: Vec<Device>,
}

impl From<AccountDetails> for Account {
    fn from(account: AccountDetails) -> Self {
        Self {
            id: account.id,
            name: account.name,
            is_admin: account.is_admin,
            devices: account.devices.into_iter().map(Device::from).collect(),
        }
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct RoleRequest {
    is_admin: bool,
}

/// `purpose` : `admin`, `member` (nouveau compte `name`) ou `device` (appareil de plus pour le
/// compte `name`).
#[derive(Serialize)]
struct Key {
    id: Uuid,
    purpose: &'static str,
    name: Option<String>,
    expires_at: Timestamp,
}

impl From<PendingKey> for Key {
    fn from(key: PendingKey) -> Self {
        let (purpose, name) = match key.purpose {
            PendingPurpose::Admin => ("admin", None),
            PendingPurpose::Member { name } => ("member", Some(name)),
            PendingPurpose::Device { account } => ("device", Some(account)),
        };
        Self {
            id: key.id,
            purpose,
            name,
            expires_at: key.expires_at,
        }
    }
}

#[derive(Serialize)]
struct Member {
    id: Uuid,
    name: String,
}

impl From<AccountSummary> for Member {
    fn from(account: AccountSummary) -> Self {
        Self {
            id: account.id,
            name: account.name,
        }
    }
}

#[derive(Serialize)]
struct Vault {
    id: Uuid,
    name: String,
    shared: bool,
    /// `null` tant que le coffre n'est pas publié.
    version: Option<i64>,
    members: Vec<Member>,
}

impl From<ServerVault> for Vault {
    fn from(vault: ServerVault) -> Self {
        Self {
            id: vault.id,
            name: vault.name,
            shared: vault.shared,
            version: vault.version,
            members: vault.members.into_iter().map(Member::from).collect(),
        }
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct AttachRequest {
    account_id: Uuid,
}

#[derive(Serialize)]
struct KeptVersion {
    version: i64,
    created_at: Timestamp,
    size: i64,
}

impl From<VersionSummary> for KeptVersion {
    fn from(version: VersionSummary) -> Self {
        Self {
            version: version.version,
            created_at: version.created_at,
            size: version.size,
        }
    }
}

#[derive(Serialize)]
struct Created {
    id: Uuid,
    name: String,
}

impl From<Restored> for Created {
    fn from(restored: Restored) -> Self {
        Self {
            id: restored.id,
            name: restored.name,
        }
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct SettingsRequest {
    /// `null` efface l'adresse.
    public_address: Option<String>,
}

/// Nom DNS ou adresse IP, éventuellement suivi d'un port : ni espace, ni caractère de contrôle,
/// ni morceau d'URL.
fn is_valid_address(address: &str) -> bool {
    !address.is_empty()
        && address.len() <= MAX_ADDRESS_LEN
        && !address
            .chars()
            .any(|c| c.is_whitespace() || c.is_control() || "/?#@\\".contains(c))
}

async fn accounts(State(state): State<Shared>, _: Admin) -> Result<Response, ApiError> {
    let accounts = state.storage().account_details()?;
    let accounts: Vec<Account> = accounts.into_iter().map(Account::from).collect();
    Ok(Json(accounts).into_response())
}

async fn delete_account(
    State(state): State<Shared>,
    _: Admin,
    Path(account): Path<Uuid>,
) -> Result<StatusCode, ApiError> {
    state.storage().delete_account(account)?;
    Ok(StatusCode::NO_CONTENT)
}

async fn set_admin(
    State(state): State<Shared>,
    _: Admin,
    Path(account): Path<Uuid>,
    Json(request): Json<RoleRequest>,
) -> Result<StatusCode, ApiError> {
    state.storage().set_admin(account, request.is_admin)?;
    Ok(StatusCode::NO_CONTENT)
}

async fn revoke_device(
    State(state): State<Shared>,
    _: Admin,
    Path(device): Path<Uuid>,
) -> Result<StatusCode, ApiError> {
    state.storage().revoke_device(device)?;
    Ok(StatusCode::NO_CONTENT)
}

async fn keys(State(state): State<Shared>, _: Admin) -> Result<Response, ApiError> {
    let keys = state.storage().pending_keys(now())?;
    let keys: Vec<Key> = keys.into_iter().map(Key::from).collect();
    Ok(Json(keys).into_response())
}

async fn cancel_key(
    State(state): State<Shared>,
    _: Admin,
    Path(key): Path<Uuid>,
) -> Result<StatusCode, ApiError> {
    state.storage().cancel_key(key)?;
    Ok(StatusCode::NO_CONTENT)
}

async fn vaults(State(state): State<Shared>, _: Admin) -> Result<Response, ApiError> {
    let vaults = state.storage().server_vaults()?;
    let vaults: Vec<Vault> = vaults.into_iter().map(Vault::from).collect();
    Ok(Json(vaults).into_response())
}

async fn delete_vault(
    State(state): State<Shared>,
    _: Admin,
    Path(vault): Path<Uuid>,
) -> Result<StatusCode, ApiError> {
    state.storage().delete_vault(vault)?;
    Ok(StatusCode::NO_CONTENT)
}

/// Rattache un compte à un coffre qui n'a plus de membre.
async fn attach(
    State(state): State<Shared>,
    _: Admin,
    Path(vault): Path<Uuid>,
    Json(request): Json<AttachRequest>,
) -> Result<StatusCode, ApiError> {
    state.storage().attach(vault, request.account_id)?;
    Ok(StatusCode::NO_CONTENT)
}

async fn remove_member(
    State(state): State<Shared>,
    _: Admin,
    Path((vault, account)): Path<(Uuid, Uuid)>,
) -> Result<StatusCode, ApiError> {
    state.storage().remove_member(vault, account)?;
    Ok(StatusCode::NO_CONTENT)
}

async fn versions(
    State(state): State<Shared>,
    _: Admin,
    Path(vault): Path<Uuid>,
) -> Result<Response, ApiError> {
    let versions = state.storage().versions(vault)?;
    let versions: Vec<KeptVersion> = versions.into_iter().map(KeptVersion::from).collect();
    Ok(Json(versions).into_response())
}

/// Crée un nouveau coffre à partir d'une version conservée.
async fn restore(
    State(state): State<Shared>,
    _: Admin,
    Path((vault, version)): Path<(Uuid, i64)>,
) -> Result<Response, ApiError> {
    let restored = state.storage().restore_version(vault, version, now())?;
    Ok((StatusCode::CREATED, Json(Created::from(restored))).into_response())
}

async fn settings(
    State(state): State<Shared>,
    _: Admin,
    Json(request): Json<SettingsRequest>,
) -> Result<StatusCode, ApiError> {
    let address = request.public_address.as_deref();
    if !address.is_none_or(is_valid_address) {
        return Err(ApiError::BadRequest);
    }
    state.storage().set_public_address(address)?;
    Ok(StatusCode::NO_CONTENT)
}

/// Copie de toute la base : coffres chiffrés de tous les comptes, et clé privée du certificat.
async fn backup(State(state): State<Shared>, _: Admin) -> Result<Response, ApiError> {
    let copy = state.storage().backup()?;
    Ok(([(CONTENT_TYPE, "application/octet-stream")], copy).into_response())
}

#[cfg(test)]
mod tests {
    use serde_json::{Value, json};

    use super::super::testing::*;
    use super::*;

    /// Serveur avec Arthur (administrateur) et Camille.
    struct Server {
        app: Router,
        arthur: String,
        camille: String,
    }

    async fn server() -> Server {
        let (app, arthur) = app_with_admin().await;
        let camille = add_member(&app, &arthur, "Camille").await;
        Server {
            app,
            arthur,
            camille,
        }
    }

    impl Server {
        async fn call(&self, method: &str, uri: &str, body: Option<Value>) -> Response {
            send(&self.app, request(method, uri, Some(&self.arthur), body)).await
        }

        async fn status(&self, method: &str, uri: &str, body: Option<Value>) -> StatusCode {
            self.call(method, uri, body).await.status()
        }

        async fn get(&self, uri: &str) -> Value {
            let response = self.call("GET", uri, None).await;
            assert_eq!(response.status(), StatusCode::OK);
            json(response).await
        }

        /// Identifiant du compte `name` et de son premier appareil.
        async fn account(&self, name: &str) -> (String, String) {
            let accounts = self.get("/v1/admin/accounts").await;
            let account = accounts
                .as_array()
                .unwrap()
                .iter()
                .find(|account| account["name"] == name)
                .unwrap();
            let id = account["id"].as_str().unwrap().to_owned();
            let device = account["devices"][0]["id"].as_str().unwrap().to_owned();
            (id, device)
        }

        /// Coffre créé et publié par `token`.
        async fn published_vault(&self, token: &str, name: &str, shared: bool) -> String {
            let body = json!({ "name": name, "shared": shared });
            let response = send(
                &self.app,
                request("POST", "/v1/vaults", Some(token), Some(body)),
            );
            let id = json(response.await).await["id"]
                .as_str()
                .unwrap()
                .to_owned();
            let publication = axum::http::Request::put(format!("/v1/vaults/{id}"))
                .header("authorization", format!("Bearer {token}"))
                .header("if-none-match", "*")
                .body(axum::body::Body::from(VAULT.clone()))
                .unwrap();
            let response = send(&self.app, publication).await;
            assert_eq!(response.status(), StatusCode::NO_CONTENT);
            id
        }
    }

    #[test]
    fn addresses_must_be_host_names() {
        for address in ["192.168.1.10", "maison.example.org:8443", "[::1]:8443"] {
            assert!(is_valid_address(address), "{address}");
        }
        let too_long = "a".repeat(MAX_ADDRESS_LEN + 1);
        for address in [
            "",
            "a b",
            "a\tb",
            "a\u{7}b",
            "https://maison",
            "a?b",
            "a#b",
            "u@a",
            "a\\b",
            &too_long,
        ] {
            assert!(!is_valid_address(address), "{address:?}");
        }
    }

    /// Une clé administrateur ne vient que de `pswd-server reset-admin`, pas d'une route.
    #[test]
    fn administrator_keys_are_listed_without_name() {
        let key = Key::from(PendingKey {
            id: Uuid::new_v4(),
            purpose: PendingPurpose::Admin,
            expires_at: Timestamp::from_millis(1),
        });
        assert_eq!((key.purpose, key.name), ("admin", None));
    }

    #[tokio::test]
    async fn the_panel_is_for_administrators_only() {
        let server = server().await;
        let id = Uuid::new_v4();
        let routes = [
            ("GET", "/v1/admin/accounts".to_owned(), None),
            ("DELETE", format!("/v1/admin/accounts/{id}"), None),
            (
                "PUT",
                format!("/v1/admin/accounts/{id}/admin"),
                Some(json!({ "is_admin": true })),
            ),
            ("DELETE", format!("/v1/admin/devices/{id}"), None),
            ("GET", "/v1/admin/keys".to_owned(), None),
            ("DELETE", format!("/v1/admin/keys/{id}"), None),
            ("GET", "/v1/admin/vaults".to_owned(), None),
            ("DELETE", format!("/v1/admin/vaults/{id}"), None),
            (
                "POST",
                format!("/v1/admin/vaults/{id}/members"),
                Some(json!({ "account_id": id })),
            ),
            (
                "DELETE",
                format!("/v1/admin/vaults/{id}/members/{id}"),
                None,
            ),
            ("GET", format!("/v1/admin/vaults/{id}/versions"), None),
            (
                "POST",
                format!("/v1/admin/vaults/{id}/versions/1/restore"),
                None,
            ),
            (
                "PUT",
                "/v1/admin/settings".to_owned(),
                Some(json!({ "public_address": null })),
            ),
            ("GET", "/v1/admin/backup".to_owned(), None),
        ];
        for (method, uri, body) in routes {
            let response = send(
                &server.app,
                request(method, &uri, Some(&server.camille), body.clone()),
            )
            .await;
            assert_eq!(response.status(), StatusCode::FORBIDDEN, "{method} {uri}");
            let response = send(&server.app, request(method, &uri, None, body)).await;
            assert_eq!(
                response.status(),
                StatusCode::UNAUTHORIZED,
                "{method} {uri}"
            );
        }
    }

    #[tokio::test]
    async fn accounts_and_devices_are_managed() {
        let server = server().await;
        let accounts = server.get("/v1/admin/accounts").await;
        assert_eq!(accounts[0]["name"], "Arthur");
        assert_eq!(accounts[0]["is_admin"], true);
        assert_eq!(accounts[1]["devices"][0]["name"], "PC");
        assert!(accounts[1]["devices"][0]["created_at"].is_i64());

        let (arthur, _) = server.account("Arthur").await;
        let (camille, device) = server.account("Camille").await;
        let role = |is_admin: bool| Some(json!({ "is_admin": is_admin }));
        let arthur_role = format!("/v1/admin/accounts/{arthur}/admin");
        let camille_role = format!("/v1/admin/accounts/{camille}/admin");
        assert_eq!(
            server.status("PUT", &arthur_role, role(false)).await,
            StatusCode::CONFLICT
        );
        assert_eq!(
            server.status("PUT", &camille_role, role(true)).await,
            StatusCode::NO_CONTENT
        );
        let response = send(
            &server.app,
            request("GET", "/v1/admin/keys", Some(&server.camille), None),
        )
        .await;
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(
            server.status("PUT", &camille_role, role(false)).await,
            StatusCode::NO_CONTENT
        );
        let unknown = format!("/v1/admin/accounts/{}/admin", Uuid::new_v4());
        assert_eq!(
            server.status("PUT", &unknown, role(true)).await,
            StatusCode::NOT_FOUND
        );

        let revoke = format!("/v1/admin/devices/{device}");
        assert_eq!(
            server.status("DELETE", &revoke, None).await,
            StatusCode::NO_CONTENT
        );
        let response = send(
            &server.app,
            request("GET", "/v1/vaults", Some(&server.camille), None),
        )
        .await;
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
        assert_eq!(
            server.status("DELETE", &revoke, None).await,
            StatusCode::NOT_FOUND
        );

        let delete = |id: &str| format!("/v1/admin/accounts/{id}");
        assert_eq!(
            server.status("DELETE", &delete(&arthur), None).await,
            StatusCode::CONFLICT
        );
        assert_eq!(
            server.status("DELETE", &delete(&camille), None).await,
            StatusCode::NO_CONTENT
        );
        assert_eq!(
            server.status("DELETE", &delete(&camille), None).await,
            StatusCode::NOT_FOUND
        );
        assert_eq!(
            server
                .get("/v1/admin/accounts")
                .await
                .as_array()
                .unwrap()
                .len(),
            1
        );
    }

    #[tokio::test]
    async fn pending_keys_are_listed_and_cancelled() {
        let server = server().await;
        let response = server
            .call("POST", "/v1/members", Some(json!({ "name": "Dominique" })))
            .await;
        let key = json(response).await["key"].as_str().unwrap().to_owned();
        let keys = server.get("/v1/admin/keys").await;
        assert_eq!(keys.as_array().unwrap().len(), 1);
        assert_eq!(keys[0]["purpose"], "member");
        assert_eq!(keys[0]["name"], "Dominique");
        assert!(keys[0]["expires_at"].is_i64());

        let cancel = format!("/v1/admin/keys/{}", keys[0]["id"].as_str().unwrap());
        assert_eq!(
            server.status("DELETE", &cancel, None).await,
            StatusCode::NO_CONTENT
        );
        assert_eq!(
            server.status("DELETE", &cancel, None).await,
            StatusCode::NOT_FOUND
        );
        let body = json!({ "key": key, "device_name": "PC" });
        let response = send(
            &server.app,
            request("POST", "/v1/connect", None, Some(body)),
        )
        .await;
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);

        send(
            &server.app,
            request("POST", "/v1/devices/keys", Some(&server.camille), None),
        )
        .await;
        let keys = server.get("/v1/admin/keys").await;
        assert_eq!(
            (&keys[0]["purpose"], &keys[0]["name"]),
            (&json!("device"), &json!("Camille"))
        );
    }

    #[tokio::test]
    async fn versions_are_restored_as_new_vaults() {
        let server = server().await;
        let perso = server
            .published_vault(&server.camille, "perso", false)
            .await;
        let vaults = server.get("/v1/admin/vaults").await;
        assert_eq!(vaults[0]["name"], "perso");
        assert_eq!(vaults[0]["shared"], false);
        assert_eq!(vaults[0]["version"], 1);
        assert_eq!(vaults[0]["members"][0]["name"], "Camille");

        let versions = server
            .get(&format!("/v1/admin/vaults/{perso}/versions"))
            .await;
        assert_eq!(versions[0]["version"], 1);
        assert_eq!(versions[0]["size"], VAULT.len());
        assert!(versions[0]["created_at"].is_i64());

        let response = server
            .call(
                "POST",
                &format!("/v1/admin/vaults/{perso}/versions/1/restore"),
                None,
            )
            .await;
        assert_eq!(response.status(), StatusCode::CREATED);
        let restored = json(response).await;
        assert_eq!(restored["name"], "perso-v1");
        let response = send(
            &server.app,
            request("GET", "/v1/vaults", Some(&server.camille), None),
        )
        .await;
        assert_eq!(json(response).await.as_array().unwrap().len(), 2);

        let missing = format!("/v1/admin/vaults/{perso}/versions/2/restore");
        assert_eq!(
            server.status("POST", &missing, None).await,
            StatusCode::NOT_FOUND
        );
        let unknown = format!("/v1/admin/vaults/{}/versions", Uuid::new_v4());
        assert_eq!(
            server.status("GET", &unknown, None).await,
            StatusCode::NOT_FOUND
        );
    }

    #[tokio::test]
    async fn a_deleted_vault_is_gone_for_its_members() {
        let server = server().await;
        let perso = server
            .published_vault(&server.camille, "perso", false)
            .await;
        let delete = format!("/v1/admin/vaults/{perso}");
        assert_eq!(
            server.status("DELETE", &delete, None).await,
            StatusCode::NO_CONTENT
        );
        let response = send(
            &server.app,
            request(
                "GET",
                &format!("/v1/vaults/{perso}"),
                Some(&server.camille),
                None,
            ),
        )
        .await;
        assert_eq!(response.status(), StatusCode::GONE);
        assert_eq!(server.get("/v1/admin/vaults").await, json!([]));
        assert_eq!(
            server.status("DELETE", &delete, None).await,
            StatusCode::NOT_FOUND
        );
    }

    #[tokio::test]
    async fn members_are_removed_and_accounts_attached() {
        let server = server().await;
        let famille = server
            .published_vault(&server.arthur, "famille", true)
            .await;
        let (arthur, _) = server.account("Arthur").await;
        let (camille, _) = server.account("Camille").await;
        let members = format!("/v1/admin/vaults/{famille}/members");
        let attach = |account: &str| Some(json!({ "account_id": account }));

        assert_eq!(
            server.status("POST", &members, attach(&camille)).await,
            StatusCode::CONFLICT
        );
        let remove = format!("{members}/{arthur}");
        assert_eq!(
            server.status("DELETE", &remove, None).await,
            StatusCode::NO_CONTENT
        );
        assert_eq!(
            server.status("DELETE", &remove, None).await,
            StatusCode::NOT_FOUND
        );
        let unknown = Uuid::new_v4().to_string();
        assert_eq!(
            server.status("POST", &members, attach(&unknown)).await,
            StatusCode::BAD_REQUEST
        );
        assert_eq!(
            server.status("POST", &members, attach(&camille)).await,
            StatusCode::NO_CONTENT
        );
        let response = send(
            &server.app,
            request(
                "GET",
                &format!("/v1/vaults/{famille}"),
                Some(&server.camille),
                None,
            ),
        )
        .await;
        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn public_address_is_checked() {
        let server = server().await;
        let set = |address: Value| Some(json!({ "public_address": address }));
        let uri = "/v1/admin/settings";
        assert_eq!(
            server
                .status("PUT", uri, set(json!("https://maison")))
                .await,
            StatusCode::BAD_REQUEST
        );
        assert_eq!(
            server.status("PUT", uri, set(json!("maison:8443"))).await,
            StatusCode::NO_CONTENT
        );
        assert_eq!(
            server.get("/v1/server").await["public_address"],
            "maison:8443"
        );
        assert_eq!(
            server.status("PUT", uri, set(Value::Null)).await,
            StatusCode::NO_CONTENT
        );
        assert_eq!(
            server.get("/v1/server").await["public_address"],
            Value::Null
        );
    }

    #[tokio::test]
    async fn backup_is_a_sqlite_database() {
        let server = server().await;
        let response = server.call("GET", "/v1/admin/backup", None).await;
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(
            response.headers()["content-type"],
            "application/octet-stream"
        );
        assert!(body(response).await.starts_with(b"SQLite format 3\0"));
    }
}
