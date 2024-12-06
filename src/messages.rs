use crate::util::WR;
use askama_axum::Template;
use axum::extract::State;
use axum::http::header::COOKIE;
use axum::http::HeaderMap;
use serde::{Deserialize, Serialize};
use sqlx::{FromRow, PgPool};
use uuid::Uuid;

#[derive(Template)]
#[template(path = "index.askama.html")]
pub struct MessageTemplate {}

pub async fn index() -> MessageTemplate {
    MessageTemplate {}
}

#[derive(Serialize, Deserialize, FromRow)]
pub struct Message {
    pub id: Uuid,
    pub content: String,
}

pub async fn fetch_messages(State(pool): State<PgPool>) -> WR<Vec<Message>> {
    sqlx::query_as::<_, Message>(
        // language=postgresql
        "SELECT content, id FROM messages
                           WHERE (published OR fingerprint = $1)
                           ORDER BY created_at DESC LIMIT 50",
    )
    .bind(&fingerprint)
    .fetch_all(&pool)
    .await
    .map_err(Into::into)
}
