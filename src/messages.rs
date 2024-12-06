use crate::util::WR;
use askama_axum::Template;
use axum::extract::State;
use serde::{Deserialize, Serialize};
use sqlx::{FromRow, PgPool};
use uuid::Uuid;
use crate::Fingerprint;

#[derive(Template)]
#[template(path = "index.askama.html")]
pub struct MessageTemplate {
    messages: Vec<Message>
}

pub async fn index(State(pool): State<PgPool>, fingerprint: Fingerprint) -> WR<MessageTemplate> {
    let messages = fetch_messages(&pool, &fingerprint).await?;
    Ok(MessageTemplate { messages })
}

#[derive(Serialize, Deserialize, FromRow)]
pub struct Message {
    pub id: Uuid,
    pub content: String,
}

pub async fn fetch_messages(pool: &PgPool, fingerprint: &Fingerprint) -> WR<Vec<Message>> {
    sqlx::query_as::<_, Message>(
        // language=postgresql
        "SELECT content, id FROM messages
                           WHERE (published OR fingerprint = $1)
                           ORDER BY created_at DESC LIMIT 50",
    )
    .bind(fingerprint.id)
    .fetch_all(pool)
    .await
    .map_err(Into::into)
}
