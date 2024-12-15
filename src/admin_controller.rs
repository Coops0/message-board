use crate::{
    fallback, messages::FullMessage, user::User, util::WR, ws::WebsocketActorMessage, AppState
};
use axum::{
    extract::{Path, Request, State},
    middleware::{from_fn_with_state, Next},
    response::Response,
    routing::{get, patch},
    Json, RequestExt, Router
};
use serde::Deserialize;
use uuid::Uuid;

pub fn admin_controller(state: AppState) -> Router<AppState> {
    Router::new()
        .route("/user/:id", get(get_user).patch(update_user))
        .route("/message/:id", patch(update_message))
        .layer(from_fn_with_state(state, verify_admin_layer))
}

async fn verify_admin_layer(
    State(state): State<AppState>,
    mut request: Request,
    next: Next
) -> Response {
    let user = request
        .extract_parts_with_state::<User, AppState>(&state)
        .await;

    if let Ok(user) = user {
        if user.admin {
            return next.run(request).await;
        }
    }

    fallback().await
}

async fn get_user(
    State(AppState { pool, .. }): State<AppState>,
    Path(id): Path<Uuid>
) -> WR<Json<Option<User>>> {
    sqlx::query_as!(
        User,
        // language=postgresql
        "SELECT * FROM users WHERE id = $1 LIMIT 1",
        id
    )
    .fetch_optional(&pool)
    .await
    .map(Json)
    .map_err(Into::into)
}

#[derive(Deserialize)]
struct PatchUserPayload {
    #[serde(default)]
    banned: Option<bool>
}

async fn update_user(
    State(AppState { pool, .. }): State<AppState>,
    Path(id): Path<Uuid>,
    Json(payload): Json<PatchUserPayload>
) -> WR<Json<User>> {
    sqlx::query_as!(
        User,
        // language=postgresql
        "UPDATE users 
        SET banned = COALESCE($2, banned)
        WHERE id = $1 RETURNING *
        ",
        id,
        payload.banned
    )
    .fetch_one(&pool)
    .await
    .map(Json)
    .map_err(Into::into)
}

#[derive(Deserialize)]
struct PatchMessagePayload {
    #[serde(default)]
    pub content: Option<String>,
    #[serde(default)]
    pub flagged: Option<bool>,
    #[serde(default)]
    pub published: Option<bool>
}

async fn update_message(
    State(AppState { pool, tx }): State<AppState>,
    Path(id): Path<Uuid>,
    Json(payload): Json<PatchMessagePayload>
) -> WR<Json<FullMessage>> {
    let updated_message = sqlx::query_as!(
        FullMessage,
        // language=postgresql
        "UPDATE messages
        SET content = COALESCE($2, content),
            flagged = COALESCE($3, flagged),
            published = COALESCE($4, published)
        WHERE id = $1 RETURNING *",
        id,
        payload.content,
        payload.flagged,
        payload.published
    )
    .fetch_one(&pool)
    .await?;

    let _ = tx
        .send(WebsocketActorMessage::Message {
            message: updated_message.clone(),
            is_update: true
        })
        .await;

    Ok(Json(updated_message))
}
