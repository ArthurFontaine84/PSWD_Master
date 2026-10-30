//! Coffres communs : comptes, membres, invitations et départs.

use axum::Json;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::{ApiError, Shared};
use crate::clock::now;
use crate::storage::{self, AccountSummary, Caller, ReceivedInvitation, SentInvitation};

#[derive(Serialize)]
struct Account {
    id: Uuid,
    name: String,
}

impl From<AccountSummary> for Account {
    fn from(account: AccountSummary) -> Self {
        Self {
            id: account.id,
            name: account.name,
        }
    }
}

#[derive(Serialize)]
struct Invitation {
    id: Uuid,
    account: Account,
}

impl From<SentInvitation> for Invitation {
    fn from(invitation: SentInvitation) -> Self {
        Self {
            id: invitation.id,
            account: invitation.account.into(),
        }
    }
}

#[derive(Serialize)]
struct Members {
    members: Vec<Account>,
    invitations: Vec<Invitation>,
}

impl From<storage::Members> for Members {
    fn from(members: storage::Members) -> Self {
        Self {
            members: members.members.into_iter().map(Account::from).collect(),
            invitations: members
                .invitations
                .into_iter()
                .map(Invitation::from)
                .collect(),
        }
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct InviteRequest {
    account_id: Uuid,
}

#[derive(Serialize)]
struct Created {
    id: Uuid,
}

#[derive(Serialize)]
struct Received {
    id: Uuid,
    vault_id: Uuid,
    vault_name: String,
    invited_by: String,
}

impl From<ReceivedInvitation> for Received {
    fn from(invitation: ReceivedInvitation) -> Self {
        Self {
            id: invitation.id,
            vault_id: invitation.vault_id,
            vault_name: invitation.vault_name,
            invited_by: invitation.invited_by,
        }
    }
}

/// Comptes du serveur, pour choisir qui inviter.
pub(super) async fn accounts(State(state): State<Shared>, _: Caller) -> Result<Response, ApiError> {
    let accounts = state.storage().list_accounts()?;
    let accounts: Vec<Account> = accounts.into_iter().map(Account::from).collect();
    Ok(Json(accounts).into_response())
}

pub(super) async fn members(
    State(state): State<Shared>,
    caller: Caller,
    Path(vault): Path<Uuid>,
) -> Result<Response, ApiError> {
    let members = state.storage().members(vault, caller.account)?;
    Ok(Json(Members::from(members)).into_response())
}

pub(super) async fn invite(
    State(state): State<Shared>,
    caller: Caller,
    Path(vault): Path<Uuid>,
    Json(request): Json<InviteRequest>,
) -> Result<Response, ApiError> {
    let id = state
        .storage()
        .invite(vault, caller.account, request.account_id, now())?;
    Ok((StatusCode::CREATED, Json(Created { id })).into_response())
}

/// Invitations reçues par le compte de l'appareil.
pub(super) async fn received(
    State(state): State<Shared>,
    caller: Caller,
) -> Result<Response, ApiError> {
    let invitations = state.storage().received_invitations(caller.account)?;
    let invitations: Vec<Received> = invitations.into_iter().map(Received::from).collect();
    Ok(Json(invitations).into_response())
}

pub(super) async fn accept(
    State(state): State<Shared>,
    caller: Caller,
    Path(invitation): Path<Uuid>,
) -> Result<StatusCode, ApiError> {
    state
        .storage()
        .accept_invitation(invitation, caller.account)?;
    Ok(StatusCode::NO_CONTENT)
}

/// Refus de l'invité, ou annulation par un membre du coffre.
pub(super) async fn delete_invitation(
    State(state): State<Shared>,
    caller: Caller,
    Path(invitation): Path<Uuid>,
) -> Result<StatusCode, ApiError> {
    state
        .storage()
        .delete_invitation(invitation, caller.account)?;
    Ok(StatusCode::NO_CONTENT)
}

pub(super) async fn leave(
    State(state): State<Shared>,
    caller: Caller,
    Path(vault): Path<Uuid>,
) -> Result<StatusCode, ApiError> {
    state.storage().leave(vault, caller.account)?;
    Ok(StatusCode::NO_CONTENT)
}

#[cfg(test)]
mod tests {
    use axum::Router;
    use serde_json::{Value, json};

    use super::super::testing::*;
    use super::*;

    /// Serveur avec Arthur (administrateur) et Camille ; Arthur a un coffre commun `famille`.
    struct Family {
        app: Router,
        arthur: String,
        camille: String,
        famille: String,
    }

    async fn family() -> Family {
        let (app, arthur) = app_with_admin().await;
        let camille = add_member(&app, &arthur, "Camille").await;
        let body = json!({ "name": "famille", "shared": true });
        let response = send(
            &app,
            request("POST", "/v1/vaults", Some(&arthur), Some(body)),
        )
        .await;
        let famille = json(response).await["id"].as_str().unwrap().to_owned();
        Family {
            app,
            arthur,
            camille,
            famille,
        }
    }

    async fn call(
        app: &Router,
        method: &str,
        uri: &str,
        token: &str,
        body: Option<Value>,
    ) -> Response {
        send(app, request(method, uri, Some(token), body)).await
    }

    /// Identifiant du compte `name`, tel que la liste des comptes le donne.
    async fn account_id(family: &Family, name: &str) -> Value {
        let response = call(&family.app, "GET", "/v1/accounts", &family.arthur, None).await;
        let accounts = json(response).await;
        accounts
            .as_array()
            .unwrap()
            .iter()
            .find(|account| account["name"] == name)
            .unwrap()["id"]
            .clone()
    }

    async fn invite(family: &Family, token: &str, account: Value) -> Response {
        let uri = format!("/v1/vaults/{}/invitations", family.famille);
        call(
            &family.app,
            "POST",
            &uri,
            token,
            Some(json!({ "account_id": account })),
        )
        .await
    }

    async fn invite_camille(family: &Family) -> String {
        let camille = account_id(family, "Camille").await;
        let response = invite(family, &family.arthur, camille).await;
        assert_eq!(response.status(), StatusCode::CREATED);
        json(response).await["id"].as_str().unwrap().to_owned()
    }

    #[tokio::test]
    async fn accounts_are_listed_for_any_device() {
        let family = family().await;
        let response = call(&family.app, "GET", "/v1/accounts", &family.camille, None).await;
        assert_eq!(response.status(), StatusCode::OK);
        let names: Vec<Value> = json(response)
            .await
            .as_array()
            .unwrap()
            .iter()
            .map(|account| account["name"].clone())
            .collect();
        assert_eq!(names, [json!("Arthur"), json!("Camille")]);
    }

    #[tokio::test]
    async fn an_invited_account_accepts_and_becomes_a_member() {
        let family = family().await;
        let invitation = invite_camille(&family).await;

        let response = call(&family.app, "GET", "/v1/invitations", &family.camille, None).await;
        assert_eq!(
            json(response).await,
            json!([{
                "id": invitation,
                "vault_id": family.famille,
                "vault_name": "famille",
                "invited_by": "Arthur",
            }])
        );
        let members = format!("/v1/vaults/{}/members", family.famille);
        let response = call(&family.app, "GET", &members, &family.arthur, None).await;
        let listed = json(response).await;
        assert_eq!(listed["invitations"][0]["id"], invitation.as_str());
        assert_eq!(listed["invitations"][0]["account"]["name"], "Camille");
        assert_eq!(
            call(&family.app, "GET", &members, &family.camille, None)
                .await
                .status(),
            StatusCode::NOT_FOUND
        );

        let accept = format!("/v1/invitations/{invitation}/accept");
        let response = call(&family.app, "POST", &accept, &family.camille, None).await;
        assert_eq!(response.status(), StatusCode::NO_CONTENT);
        let response = call(&family.app, "GET", &members, &family.camille, None).await;
        let listed = json(response).await;
        assert_eq!(listed["members"].as_array().unwrap().len(), 2);
        assert_eq!(listed["invitations"], json!([]));
        let response = call(&family.app, "GET", "/v1/vaults", &family.camille, None).await;
        assert_eq!(json(response).await[0]["shared"], true);
        assert_eq!(
            call(&family.app, "POST", &accept, &family.camille, None)
                .await
                .status(),
            StatusCode::NOT_FOUND
        );
    }

    #[tokio::test]
    async fn invitations_are_checked() {
        let family = family().await;
        let camille = account_id(&family, "Camille").await;
        let arthur = account_id(&family, "Arthur").await;
        let status = |response: Response| response.status();
        assert_eq!(
            status(invite(&family, &family.camille, camille.clone()).await),
            StatusCode::NOT_FOUND
        );
        assert_eq!(
            status(invite(&family, &family.arthur, json!(Uuid::new_v4())).await),
            StatusCode::BAD_REQUEST
        );
        assert_eq!(
            status(invite(&family, &family.arthur, arthur).await),
            StatusCode::CONFLICT
        );
        let body = json!({ "name": "perso" });
        let response = call(
            &family.app,
            "POST",
            "/v1/vaults",
            &family.arthur,
            Some(body),
        )
        .await;
        let private = json(response).await["id"].as_str().unwrap().to_owned();
        let uri = format!("/v1/vaults/{private}/invitations");
        let body = json!({ "account_id": camille });
        assert_eq!(
            status(call(&family.app, "POST", &uri, &family.arthur, Some(body)).await),
            StatusCode::BAD_REQUEST
        );
    }

    #[tokio::test]
    async fn invitations_are_refused_or_cancelled() {
        let family = family().await;
        let invitation = invite_camille(&family).await;
        let uri = format!("/v1/invitations/{invitation}");
        let response = call(&family.app, "DELETE", &uri, &family.camille, None).await;
        assert_eq!(response.status(), StatusCode::NO_CONTENT);
        let response = call(&family.app, "DELETE", &uri, &family.arthur, None).await;
        assert_eq!(response.status(), StatusCode::NOT_FOUND);

        let invitation = invite_camille(&family).await;
        let uri = format!("/v1/invitations/{invitation}");
        let response = call(&family.app, "DELETE", &uri, &family.arthur, None).await;
        assert_eq!(response.status(), StatusCode::NO_CONTENT);
        let response = call(&family.app, "GET", "/v1/invitations", &family.camille, None).await;
        assert_eq!(json(response).await, json!([]));
    }

    #[tokio::test]
    async fn a_member_who_left_gets_gone() {
        let family = family().await;
        let invitation = invite_camille(&family).await;
        let accept = format!("/v1/invitations/{invitation}/accept");
        call(&family.app, "POST", &accept, &family.camille, None).await;
        let leave = format!("/v1/vaults/{}/members/self", family.famille);
        let response = call(&family.app, "DELETE", &leave, &family.camille, None).await;
        assert_eq!(response.status(), StatusCode::NO_CONTENT);
        for (method, uri) in [
            ("GET", format!("/v1/vaults/{}", family.famille)),
            ("GET", format!("/v1/vaults/{}/members", family.famille)),
            ("DELETE", leave.clone()),
        ] {
            let response = call(&family.app, method, &uri, &family.camille, None).await;
            assert_eq!(response.status(), StatusCode::GONE, "{method} {uri}");
        }
        let response = call(&family.app, "GET", "/v1/vaults", &family.camille, None).await;
        assert_eq!(json(response).await, json!([]));
    }

    #[tokio::test]
    async fn a_private_vault_cannot_be_left() {
        let family = family().await;
        let body = json!({ "name": "perso" });
        let response = call(
            &family.app,
            "POST",
            "/v1/vaults",
            &family.arthur,
            Some(body),
        )
        .await;
        let private = json(response).await["id"].as_str().unwrap().to_owned();
        let leave = format!("/v1/vaults/{private}/members/self");
        let response = call(&family.app, "DELETE", &leave, &family.arthur, None).await;
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn sharing_requires_a_token() {
        let family = family().await;
        let vault = &family.famille;
        let id = Uuid::new_v4();
        for (method, uri) in [
            ("GET", "/v1/accounts".to_owned()),
            ("GET", format!("/v1/vaults/{vault}/members")),
            ("DELETE", format!("/v1/vaults/{vault}/members/self")),
            ("POST", format!("/v1/vaults/{vault}/invitations")),
            ("GET", "/v1/invitations".to_owned()),
            ("DELETE", format!("/v1/invitations/{id}")),
            ("POST", format!("/v1/invitations/{id}/accept")),
        ] {
            let response = send(&family.app, request(method, &uri, None, None)).await;
            assert_eq!(
                response.status(),
                StatusCode::UNAUTHORIZED,
                "{method} {uri}"
            );
        }
    }
}
