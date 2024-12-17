use chrono::{DateTime, Utc};
use serde::Serialize;
use sqlx::FromRow;
use uuid::Uuid;

#[derive(Serialize, FromRow, Clone)]
pub struct StandardMessage {
    pub id: Uuid,
    pub content: String,
    pub created_at: DateTime<Utc>,
    pub author: Uuid
}

#[derive(Serialize, FromRow, Clone)]
pub struct FullMessage {
    pub id: Uuid,
    pub content: String,
    pub author: Uuid,
    pub published: bool,
    pub score: f32,
    pub created_at: DateTime<Utc>
}
