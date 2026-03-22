//! Household management routes for `/api/household`.
//!
//! Provides endpoints for creating and managing households, inviting
//! members, managing roles, and configuring shared collections.

use crate::auth::types::{AuthIdentity, HouseholdRole};
use crate::household::HouseholdStore;

use axum::extract::State;
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::Json;
use serde::Deserialize;
use serde_json::json;
use std::sync::Arc;

/// Shared state for household routes.
#[derive(Clone)]
pub struct HouseholdState {
    pub store: Arc<HouseholdStore>,
}

/// Request body for creating a household.
#[derive(Debug, Deserialize)]
pub struct CreateHouseholdRequest {
    pub name: String,
    pub display_name: String,
}

/// POST /api/household — Create a new household.
pub async fn create_household(
    State(state): State<HouseholdState>,
    identity: axum::Extension<AuthIdentity>,
    Json(body): Json<CreateHouseholdRequest>,
) -> impl IntoResponse {
    let user_id = match &identity.user_id {
        Some(uid) => uid.clone(),
        None => {
            return (
                StatusCode::BAD_REQUEST,
                Json(json!({
                    "error": { "code": "HOUSEHOLD_NO_USER_ID", "message": "user identity required for household creation" }
                })),
            )
                .into_response();
        }
    };

    let household = state
        .store
        .create_household(&body.name, &user_id, &body.display_name)
        .await;

    (StatusCode::CREATED, Json(json!({ "data": household }))).into_response()
}

/// GET /api/household — Get the current user's household.
pub async fn get_household(
    State(state): State<HouseholdState>,
    identity: axum::Extension<AuthIdentity>,
) -> impl IntoResponse {
    let user_id = match &identity.user_id {
        Some(uid) => uid.clone(),
        None => {
            return (
                StatusCode::BAD_REQUEST,
                Json(json!({
                    "error": { "code": "HOUSEHOLD_NO_USER_ID", "message": "user identity required" }
                })),
            )
                .into_response();
        }
    };

    match state.store.get_user_household(&user_id).await {
        Some(household) => (StatusCode::OK, Json(json!({ "data": household }))).into_response(),
        None => (
            StatusCode::NOT_FOUND,
            Json(json!({
                "error": { "code": "HOUSEHOLD_NOT_FOUND", "message": "user is not a member of any household" }
            })),
        )
            .into_response(),
    }
}

/// Request body for inviting a member.
#[derive(Debug, Deserialize)]
pub struct InviteMemberRequest {
    pub email: String,
    pub role: HouseholdRole,
}

/// POST /api/household/invite — Create an invite for a new member.
pub async fn invite_member(
    State(state): State<HouseholdState>,
    identity: axum::Extension<AuthIdentity>,
    Json(body): Json<InviteMemberRequest>,
) -> impl IntoResponse {
    let user_id = match &identity.user_id {
        Some(uid) => uid.clone(),
        None => {
            return (
                StatusCode::BAD_REQUEST,
                Json(json!({
                    "error": { "code": "HOUSEHOLD_NO_USER_ID", "message": "user identity required" }
                })),
            )
                .into_response();
        }
    };

    // Only admins can invite.
    let role = state.store.get_user_role(&user_id).await;
    if role != Some(HouseholdRole::Admin) {
        return (
            StatusCode::FORBIDDEN,
            Json(json!({
                "error": { "code": "HOUSEHOLD_FORBIDDEN", "message": "only admins can invite members" }
            })),
        )
            .into_response();
    }

    let household = match state.store.get_user_household(&user_id).await {
        Some(h) => h,
        None => {
            return (
                StatusCode::NOT_FOUND,
                Json(json!({
                    "error": { "code": "HOUSEHOLD_NOT_FOUND", "message": "user has no household" }
                })),
            )
                .into_response();
        }
    };

    match state
        .store
        .create_invite(&household.id, &body.email, body.role, &user_id)
        .await
    {
        Some(invite) => {
            (StatusCode::CREATED, Json(json!({ "data": invite }))).into_response()
        }
        None => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({
                "error": { "code": "HOUSEHOLD_INVITE_FAILED", "message": "failed to create invite" }
            })),
        )
            .into_response(),
    }
}

/// Request body for accepting an invite.
#[derive(Debug, Deserialize)]
pub struct AcceptInviteRequest {
    pub invite_id: String,
    pub display_name: String,
}

/// POST /api/household/invite/accept — Accept a household invite.
pub async fn accept_invite(
    State(state): State<HouseholdState>,
    identity: axum::Extension<AuthIdentity>,
    Json(body): Json<AcceptInviteRequest>,
) -> impl IntoResponse {
    let user_id = match &identity.user_id {
        Some(uid) => uid.clone(),
        None => {
            return (
                StatusCode::BAD_REQUEST,
                Json(json!({
                    "error": { "code": "HOUSEHOLD_NO_USER_ID", "message": "user identity required" }
                })),
            )
                .into_response();
        }
    };

    match state
        .store
        .accept_invite(&body.invite_id, &user_id, &body.display_name)
        .await
    {
        Ok(member) => (StatusCode::OK, Json(json!({ "data": member }))).into_response(),
        Err(e) => (
            StatusCode::BAD_REQUEST,
            Json(json!({
                "error": { "code": "HOUSEHOLD_INVITE_ERROR", "message": e }
            })),
        )
            .into_response(),
    }
}

/// Request body for updating a member's role.
#[derive(Debug, Deserialize)]
pub struct UpdateRoleRequest {
    pub user_id: String,
    pub role: HouseholdRole,
}

/// PUT /api/household/members/role — Update a member's role.
pub async fn update_member_role(
    State(state): State<HouseholdState>,
    identity: axum::Extension<AuthIdentity>,
    Json(body): Json<UpdateRoleRequest>,
) -> impl IntoResponse {
    let requesting_user = match &identity.user_id {
        Some(uid) => uid.clone(),
        None => {
            return (
                StatusCode::BAD_REQUEST,
                Json(json!({
                    "error": { "code": "HOUSEHOLD_NO_USER_ID", "message": "user identity required" }
                })),
            )
                .into_response();
        }
    };

    // Only admins can change roles.
    let role = state.store.get_user_role(&requesting_user).await;
    if role != Some(HouseholdRole::Admin) {
        return (
            StatusCode::FORBIDDEN,
            Json(json!({
                "error": { "code": "HOUSEHOLD_FORBIDDEN", "message": "only admins can change roles" }
            })),
        )
            .into_response();
    }

    // Prevent admin from demoting themselves if they're the only admin.
    if body.user_id == requesting_user && body.role != HouseholdRole::Admin {
        let household = state.store.get_user_household(&requesting_user).await;
        if let Some(h) = household {
            let admin_count = h
                .members
                .iter()
                .filter(|m| m.role == HouseholdRole::Admin)
                .count();
            if admin_count <= 1 {
                return (
                    StatusCode::BAD_REQUEST,
                    Json(json!({
                        "error": { "code": "HOUSEHOLD_LAST_ADMIN", "message": "cannot demote the last admin" }
                    })),
                )
                    .into_response();
            }
        }
    }

    // Persist the role change.
    if !state.store.update_member_role(&body.user_id, body.role).await {
        return (
            StatusCode::NOT_FOUND,
            Json(json!({
                "error": { "code": "HOUSEHOLD_MEMBER_NOT_FOUND", "message": "member not found in any household" }
            })),
        )
            .into_response();
    }

    (
        StatusCode::OK,
        Json(json!({ "data": { "user_id": body.user_id, "role": body.role } })),
    )
        .into_response()
}

/// Request body for adding a shared collection.
#[derive(Debug, Deserialize)]
pub struct AddSharedCollectionRequest {
    pub collection: String,
}

/// POST /api/household/shared-collections — Add a shared collection.
pub async fn add_shared_collection(
    State(state): State<HouseholdState>,
    identity: axum::Extension<AuthIdentity>,
    Json(body): Json<AddSharedCollectionRequest>,
) -> impl IntoResponse {
    let user_id = match &identity.user_id {
        Some(uid) => uid.clone(),
        None => {
            return (
                StatusCode::BAD_REQUEST,
                Json(json!({
                    "error": { "code": "HOUSEHOLD_NO_USER_ID", "message": "user identity required" }
                })),
            )
                .into_response();
        }
    };

    // Only admins can manage shared collections.
    let role = state.store.get_user_role(&user_id).await;
    if role != Some(HouseholdRole::Admin) {
        return (
            StatusCode::FORBIDDEN,
            Json(json!({
                "error": { "code": "HOUSEHOLD_FORBIDDEN", "message": "only admins can manage shared collections" }
            })),
        )
            .into_response();
    }

    let household = match state.store.get_user_household(&user_id).await {
        Some(h) => h,
        None => {
            return (
                StatusCode::NOT_FOUND,
                Json(json!({
                    "error": { "code": "HOUSEHOLD_NOT_FOUND", "message": "no household found" }
                })),
            )
                .into_response();
        }
    };

    match state
        .store
        .add_shared_collection(&household.id, &body.collection)
        .await
    {
        Ok(()) => (
            StatusCode::OK,
            Json(json!({ "data": { "collection": body.collection, "shared": true } })),
        )
            .into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({
                "error": { "code": "HOUSEHOLD_ERROR", "message": e }
            })),
        )
            .into_response(),
    }
}

/// GET /api/household/shared-collections — List shared collections.
pub async fn list_shared_collections(
    State(state): State<HouseholdState>,
    identity: axum::Extension<AuthIdentity>,
) -> impl IntoResponse {
    let user_id = match &identity.user_id {
        Some(uid) => uid.clone(),
        None => {
            return (
                StatusCode::BAD_REQUEST,
                Json(json!({
                    "error": { "code": "HOUSEHOLD_NO_USER_ID", "message": "user identity required" }
                })),
            )
                .into_response();
        }
    };

    match state.store.get_user_household(&user_id).await {
        Some(household) => (
            StatusCode::OK,
            Json(json!({ "data": household.shared_collections })),
        )
            .into_response(),
        None => (
            StatusCode::NOT_FOUND,
            Json(json!({
                "error": { "code": "HOUSEHOLD_NOT_FOUND", "message": "no household found" }
            })),
        )
            .into_response(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::auth::middleware::auth_middleware;
    use crate::test_helpers::{create_auth_state, generate_test_token};
    use axum::body::Body;
    use axum::http::Request;
    use axum::routing::{get, post, put};
    use axum::Router;
    use http_body_util::BodyExt;
    use tower::ServiceExt;

    async fn setup_household_app() -> (Router, String, Arc<HouseholdStore>) {
        let store = Arc::new(HouseholdStore::new());
        let (auth_state, provider) = create_auth_state();

        let household_state = HouseholdState {
            store: Arc::clone(&store),
        };

        let app = Router::new()
            .route("/api/household", post(create_household).get(get_household))
            .route("/api/household/invite", post(invite_member))
            .route("/api/household/invite/accept", post(accept_invite))
            .route("/api/household/members/role", put(update_member_role))
            .route(
                "/api/household/shared-collections",
                post(add_shared_collection).get(list_shared_collections),
            )
            .with_state(household_state)
            .layer(axum::middleware::from_fn_with_state(
                auth_state,
                auth_middleware,
            ));

        let token = generate_test_token(&provider).await;
        (app, token, store)
    }

    use crate::test_helpers::{auth_request, body_json};

    fn app_with_identity(store: Arc<HouseholdStore>, identity: crate::auth::types::AuthIdentity) -> Router {
        let household_state = HouseholdState { store };
        Router::new()
            .route("/api/household", post(create_household).get(get_household))
            .route("/api/household/invite", post(invite_member))
            .route("/api/household/invite/accept", post(accept_invite))
            .route("/api/household/members/role", put(update_member_role))
            .route(
                "/api/household/shared-collections",
                post(add_shared_collection).get(list_shared_collections),
            )
            .with_state(household_state)
            .layer(axum::Extension(identity))
    }

    fn test_identity(user_id: &str) -> crate::auth::types::AuthIdentity {
        crate::auth::types::AuthIdentity {
            token_id: "test-token".into(),
            user_id: Some(user_id.into()),
            household_id: None,
            role: None,
            created_at: chrono::Utc::now(),
            expires_at: chrono::Utc::now() + chrono::Duration::days(30),
        }
    }

    fn json_request(method: &str, uri: &str, body: Option<String>) -> Request<Body> {
        let builder = Request::builder()
            .method(method)
            .uri(uri)
            .header("Content-Type", "application/json");
        match body {
            Some(b) => builder.body(Body::from(b)).unwrap(),
            None => builder.body(Body::empty()).unwrap(),
        }
    }

    #[tokio::test]
    async fn household_routes_require_auth() {
        let (app, _token, _store) = setup_household_app().await;
        let req = Request::builder()
            .uri("/api/household")
            .body(Body::empty())
            .unwrap();
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn create_household_returns_201() {
        let store = Arc::new(HouseholdStore::new());
        let app = app_with_identity(Arc::clone(&store), test_identity("user-a"));
        let req = json_request(
            "POST",
            "/api/household",
            Some(r#"{"name":"Test Family","display_name":"User A"}"#.into()),
        );
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::CREATED);
        let json = body_json(resp).await;
        assert_eq!(json["data"]["name"], "Test Family");
        assert_eq!(json["data"]["members"][0]["user_id"], "user-a");
        assert_eq!(json["data"]["members"][0]["role"], "Admin");
    }

    #[tokio::test]
    async fn get_household_after_create() {
        let store = Arc::new(HouseholdStore::new());
        store.create_household("My House", "user-a", "User A").await;
        let app = app_with_identity(Arc::clone(&store), test_identity("user-a"));
        let req = json_request("GET", "/api/household", None);
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let json = body_json(resp).await;
        assert_eq!(json["data"]["name"], "My House");
    }

    #[tokio::test]
    async fn get_household_returns_404_when_none() {
        let store = Arc::new(HouseholdStore::new());
        let app = app_with_identity(Arc::clone(&store), test_identity("orphan"));
        let req = json_request("GET", "/api/household", None);
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::NOT_FOUND);
        let json = body_json(resp).await;
        assert_eq!(json["error"]["code"], "HOUSEHOLD_NOT_FOUND");
    }

    #[tokio::test]
    async fn invite_member_creates_invite() {
        let store = Arc::new(HouseholdStore::new());
        store.create_household("Family", "admin-user", "Admin").await;
        let app = app_with_identity(Arc::clone(&store), test_identity("admin-user"));
        let req = json_request(
            "POST",
            "/api/household/invite",
            Some(r#"{"email":"new@test.com","role":"Member"}"#.into()),
        );
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::CREATED);
        let json = body_json(resp).await;
        assert_eq!(json["data"]["email"], "new@test.com");
        assert_eq!(json["data"]["role"], "Member");
    }

    #[tokio::test]
    async fn invite_member_forbidden_for_non_admin() {
        let store = Arc::new(HouseholdStore::new());
        let household = store.create_household("Family", "admin-user", "Admin").await;
        let invite = store
            .create_invite(&household.id, "m@test.com", HouseholdRole::Member, "admin-user")
            .await
            .unwrap();
        store.accept_invite(&invite.id, "member-user", "Member").await.unwrap();
        let app = app_with_identity(Arc::clone(&store), test_identity("member-user"));
        let req = json_request(
            "POST",
            "/api/household/invite",
            Some(r#"{"email":"other@test.com","role":"Member"}"#.into()),
        );
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::FORBIDDEN);
        let json = body_json(resp).await;
        assert_eq!(json["error"]["code"], "HOUSEHOLD_FORBIDDEN");
    }

    #[tokio::test]
    async fn accept_invite_adds_member() {
        let store = Arc::new(HouseholdStore::new());
        let household = store.create_household("Family", "admin-user", "Admin").await;
        let invite = store
            .create_invite(&household.id, "new@test.com", HouseholdRole::Member, "admin-user")
            .await
            .unwrap();
        let app = app_with_identity(Arc::clone(&store), test_identity("new-user"));
        let body = serde_json::json!({ "invite_id": invite.id, "display_name": "New User" });
        let req = json_request("POST", "/api/household/invite/accept", Some(body.to_string()));
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let json = body_json(resp).await;
        assert_eq!(json["data"]["user_id"], "new-user");
        assert_eq!(json["data"]["role"], "Member");
        let updated = store.get_household(&household.id).await.unwrap();
        assert_eq!(updated.members.len(), 2);
    }

    #[tokio::test]
    async fn update_member_role_succeeds() {
        let store = Arc::new(HouseholdStore::new());
        let household = store.create_household("Family", "admin-user", "Admin").await;
        let invite = store
            .create_invite(&household.id, "m@test.com", HouseholdRole::Member, "admin-user")
            .await
            .unwrap();
        store.accept_invite(&invite.id, "member-user", "Member").await.unwrap();
        let app = app_with_identity(Arc::clone(&store), test_identity("admin-user"));
        let req = json_request(
            "PUT",
            "/api/household/members/role",
            Some(r#"{"user_id":"member-user","role":"Guest"}"#.into()),
        );
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let json = body_json(resp).await;
        assert_eq!(json["data"]["role"], "Guest");
        let role = store.get_user_role("member-user").await;
        assert_eq!(role, Some(HouseholdRole::Guest));
    }

    #[tokio::test]
    async fn last_admin_cannot_be_demoted() {
        let store = Arc::new(HouseholdStore::new());
        store.create_household("Family", "sole-admin", "Admin").await;
        let app = app_with_identity(Arc::clone(&store), test_identity("sole-admin"));
        let req = json_request(
            "PUT",
            "/api/household/members/role",
            Some(r#"{"user_id":"sole-admin","role":"Member"}"#.into()),
        );
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
        let json = body_json(resp).await;
        assert_eq!(json["error"]["code"], "HOUSEHOLD_LAST_ADMIN");
    }

    #[tokio::test]
    async fn add_shared_collection_succeeds() {
        let store = Arc::new(HouseholdStore::new());
        let household = store.create_household("Family", "admin-user", "Admin").await;
        let app = app_with_identity(Arc::clone(&store), test_identity("admin-user"));
        let req = json_request(
            "POST",
            "/api/household/shared-collections",
            Some(r#"{"collection":"shopping-list"}"#.into()),
        );
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let json = body_json(resp).await;
        assert_eq!(json["data"]["collection"], "shopping-list");
        assert_eq!(json["data"]["shared"], true);
        assert!(store.is_shared_collection(&household.id, "shopping-list").await);
    }

    #[tokio::test]
    async fn list_shared_collections_returns_all() {
        let store = Arc::new(HouseholdStore::new());
        let household = store.create_household("Family", "admin-user", "Admin").await;
        store.add_shared_collection(&household.id, "calendar").await.unwrap();
        store.add_shared_collection(&household.id, "tasks").await.unwrap();
        let app = app_with_identity(Arc::clone(&store), test_identity("admin-user"));
        let req = json_request("GET", "/api/household/shared-collections", None);
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let json = body_json(resp).await;
        let collections = json["data"].as_array().unwrap();
        assert_eq!(collections.len(), 2);
        assert!(collections.contains(&serde_json::json!("calendar")));
        assert!(collections.contains(&serde_json::json!("tasks")));
    }

    #[tokio::test]
    async fn shared_collection_forbidden_for_non_admin() {
        let store = Arc::new(HouseholdStore::new());
        let household = store.create_household("Family", "admin-user", "Admin").await;
        let invite = store
            .create_invite(&household.id, "m@test.com", HouseholdRole::Member, "admin-user")
            .await
            .unwrap();
        store.accept_invite(&invite.id, "member-user", "Member").await.unwrap();
        let app = app_with_identity(Arc::clone(&store), test_identity("member-user"));
        let req = json_request(
            "POST",
            "/api/household/shared-collections",
            Some(r#"{"collection":"secret"}"#.into()),
        );
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::FORBIDDEN);
        let json = body_json(resp).await;
        assert_eq!(json["error"]["code"], "HOUSEHOLD_FORBIDDEN");
    }
}
