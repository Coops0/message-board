use crate::user::User;
use chrono::{DateTime, Utc};
use serde::Serialize;
use sqlx::{FromRow, PgPool, Postgres};
use tokio_util::bytes::{BufMut, BytesMut};
use tokio_util::codec::Encoder;
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

        query.push("FROM messages");
        if !user.admin {
            query.push_bind(" WHERE published OR author = ");
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

pub struct MessageEncoder;
pub struct MessagesListEncoder;

fn write_str(dst: &mut BytesMut, s: &str) -> anyhow::Result<()> {
    dst.put_u32(u32::try_from(s.len())?);
    dst.put_slice(s.as_bytes());

    Ok(())
}

impl Encoder<Message> for MessageEncoder {
    type Error = anyhow::Error;

    fn encode(&mut self, message: Message, dst: &mut BytesMut) -> Result<(), Self::Error> {
        match message {
            Message::Standard(StandardMessage {
                content,
                created_at,
            }) => {
                write_str(dst, &content)?;
                write_str(dst, &created_at.to_string())?;
            }
            Message::Full(FullMessage {
                id,
                content,
                author,
                flagged,
                published,
                created_at,
            }) => {
                write_str(dst, &content)?;
                write_str(dst, &created_at.to_string())?;
                
                write_str(dst, &id.to_string())?;
                write_str(dst, &author.to_string())?;
                dst.put_u8(u8::from(flagged));
                dst.put_u8(u8::from(published));
            }
        }

        Ok(())
    }
}

impl Encoder<Vec<Message>> for MessagesListEncoder {
    type Error = anyhow::Error;

    fn encode(&mut self, messages: Vec<Message>, dst: &mut BytesMut) -> Result<(), Self::Error> {
        dst.put_u16(u16::try_from(messages.len())?);

        let mut encoder = MessageEncoder;
        for message in messages {
            encoder.encode(message, dst)?;
        }

        Ok(())
    }
}
