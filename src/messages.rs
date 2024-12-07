use crate::util::{MinifiedHtml, WR};
use crate::User;
use askama::Template;
use axum::extract::State;
use axum::http::{HeaderMap, StatusCode};
use axum::response::{Redirect, Response};
use base64::Engine;
use rustrict::{Censor, Type};
use serde::{Deserialize, Serialize};
use sqlx::{FromRow, PgPool};
use tokio::task;
use tracing::{info, warn};

#[derive(Template)]
#[template(path = "index.askama.html")]
pub struct MessageTemplate {
    messages: Vec<Message>,
    user_id_encoded: String,
}

pub async fn user_referred_index(
    State(pool): State<PgPool>,
    user: User,
) -> WR<MinifiedHtml<MessageTemplate>> {
    info!(
        "got index req, user created at {}",
        user.created_at.to_rfc2822()
    );

    let messages = fetch_messages(&pool, &user).await?;
    Ok(MinifiedHtml(MessageTemplate {
        messages,
        user_id_encoded: base64::engine::general_purpose::STANDARD
            .encode(user.id.to_string().as_bytes()), // convert to string for proper encoding
    }))
}

pub async fn location_referred_index(State(pool): State<PgPool>, user: User) -> WR<Response> {
    
}

#[derive(Serialize, Deserialize, FromRow)]
pub struct Message {
    pub content: String,
}

async fn fetch_messages(pool: &PgPool, user: &User) -> WR<Vec<Message>> {
    sqlx::query_as::<_, Message>(
        // language=postgresql
        "SELECT content FROM messages
                           WHERE (published OR author = $1)
                           ORDER BY created_at DESC LIMIT 50",
    )
    .bind(user.id)
    .fetch_all(pool)
    .await
    .map_err(Into::into)
}

#[derive(FromRow, Debug)]
struct ExistingMessages {
    total_count: i64,
    flagged_count: i64,
    last_message_content: Option<String>,
}

async fn create_message_inner(
    pool: PgPool,
    user: User,
    headers: HeaderMap,
) -> Option<()> {
    if user.banned {
        info!("banned user");
        return None;
    }

    let content_bytes = base64::engine::general_purpose::STANDARD
        .decode(headers.get("CF-Cache-Identifier")?.as_bytes())
        .ok()?;

    info!("got message w len {}", content_bytes.len());
    if content_bytes.len() > 1024 {
        info!("message too long!");
        return None;
    }

    let content = String::from_utf8(content_bytes).ok()?;
    info!("msg content: {content}");

    let existing_messages = sqlx::query_as::<_, ExistingMessages>(
        // language=postgresql
        "
            WITH last_message AS (
              SELECT content
              FROM messages
              WHERE author = $1
              ORDER BY created_at DESC
              LIMIT 1
            )
            SELECT
              COUNT(*) as total_count,
              COUNT(*) FILTER (WHERE flagged AND NOT published) as flagged_count,
              (SELECT content FROM last_message) as last_message_content
            FROM messages
            WHERE author = $1
        ",
    )
    .bind(user.id)
    .fetch_one(&pool)
    .await
    .ok()?;

    info!("existing messages for user: {existing_messages:?}");

    if existing_messages.total_count > 400 {
        info!("too many messages for user");
        return None;
    }

    if let Some(last_message_content) = &existing_messages.last_message_content {
        if last_message_content == &content {
            info!("duplicate message");
            return None;
        }
    }

    let flagged = if existing_messages.flagged_count > 25 {
        true
    } else {
        let profanity_type = Censor::from_str(&content).analyze();
        info!("profanity type: {:?}", profanity_type);
        // profanity_type.is((Type::MODERATE_OR_HIGHER
        //     & (Type::OFFENSIVE | Type::SEXUAL | Type::SPAM))
        //     | Type::SEVERE)
        profanity_type.is(Type::SEVERE)
    };

    if flagged {
        info!("flagged");
    }

    if let Err(why) = sqlx::query(
        // language=postgresql
        "INSERT INTO messages (content, author, flagged, published)
                           VALUES ($1, $2, $3, $4)",
    )
    .bind(&content)
    .bind(user.id)
    .bind(flagged)
    .bind(!flagged)
    .execute(&pool)
    .await
    {
        warn!("failed to insert message: {why:?}");
    } else {
        info!("created message");
    }

    Some(())
}

pub async fn create_message(
    State(pool): State<PgPool>,
    user: User,
    headers: HeaderMap,
) -> StatusCode {
    #[allow(clippy::let_underscore_future)]
    let _ = task::spawn(create_message_inner(pool, user, headers));
    StatusCode::NOT_FOUND
}
