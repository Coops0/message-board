use chrono::{DateTime, Utc};
use crate::user::User;
use serde::Serialize;
use sqlx::{FromRow, PgPool, Postgres};
use uuid::Uuid;

#[derive(Serialize)]
pub enum Message {
    Standard(StandardMessage),
    Full(FullMessage),
}

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

impl Message {
    pub async fn fetch_for(pool: &PgPool, user: &User) -> sqlx::Result<Vec<Self>> {
        let mut query = sqlx::QueryBuilder::<Postgres>::new("SELECT ");
        if user.admin {
            query.push("* ");
        } else {
            query.push("content, created_at ");
        }

        query.push("FROM messages ");
        if !user.admin {
            query.push_bind("WHERE published OR author = ");
            query.push_bind(user.id);
        }

        query.push(" ORDER BY created_at DESC LIMIT 50");

        if user.admin {
            query
                .build_query_as::<FullMessage>()
                .fetch_all(pool)
                .await
                .map(|messages| messages.into_iter().map(Message::Full).collect())
        } else {
            query
                .build_query_as::<StandardMessage>()
                .fetch_all(pool)
                .await
                .map(|messages| messages.into_iter().map(Message::Standard).collect())
        }
    }
}