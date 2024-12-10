use chrono::{DateTime, Utc};
use serde::Serialize;
use sqlx::FromRow;
use uuid::Uuid;

#[derive(Serialize, FromRow)]
pub struct StandardMessage {
    pub content: String,
    pub created_at: DateTime<Utc>,
}

#[derive(Serialize, FromRow)]
pub struct FullMessage {
    pub id: Uuid,
    pub content: String,
    pub author: Uuid,
    pub flagged: bool,
    pub published: bool,
    pub created_at: DateTime<Utc>,
}
