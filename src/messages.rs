use crate::util::WR;
use crate::FullFingerprint;
use askama_axum::Template;
use axum::extract::State;
use axum::http::StatusCode;
use serde::{Deserialize, Serialize};
use sqlx::{FromRow, PgPool};
use tracing::warn;

#[derive(Template)]
#[template(path = "index.askama.html")]
pub struct MessageTemplate {
    messages: Vec<Message>,
}

pub async fn index(State(pool): State<PgPool>, fingerprint: FullFingerprint) -> WR<MessageTemplate> {
    let messages = fetch_messages(&pool, &fingerprint).await?;
    Ok(MessageTemplate { messages })
}

#[derive(Serialize, Deserialize, FromRow)]
pub struct Message {
    pub content: String,
}

pub async fn fetch_messages(pool: &PgPool, fingerprint: &FullFingerprint) -> WR<Vec<Message>> {
    sqlx::query_as::<_, Message>(
        // language=postgresql
        "SELECT content FROM messages
                           WHERE (published OR id = $1)
                           ORDER BY created_at DESC LIMIT 50",
    )
    .bind(fingerprint.id)
    .fetch_all(pool)
    .await
    .map_err(Into::into)
}

pub async fn create_message(State(pool): State<PgPool>, fingerprint: FullFingerprint) -> StatusCode {
    let insertion_result = sqlx::query(
        // language=postgresql
        "INSERT INTO messages (content, associated_fingerprint, flag_score, flagged, published)
                           VALUES ($1, $2, $3, $4, $5)",
    )
    .bind("") // todo
    .bind(fingerprint.id)
    .bind(0) // todo
    .bind(false) // todo
    .bind(true) // todo
    .execute(&pool)
    .await;

    if let Err(why) = insertion_result {
        warn!("failed to insert message: {why:?}");
    }

    StatusCode::NOT_FOUND
}
