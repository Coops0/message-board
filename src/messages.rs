use crate::random_codes::generate_code;
use crate::util::{ClientIp, MinifiedHtml, WR};
use crate::{fallback, inject_uuid_cookie, LocalUserId, User};
use askama::Template;
use axum::extract::{OriginalUri, Path, State};
use axum::http::header::USER_AGENT;
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Redirect, Response};
use base64::Engine;
use rustrict::{Censor, Type};
use serde::Serialize;
use sqlx::{FromRow, PgPool};
use tokio::task;
use tracing::{info, warn};
use uuid::Uuid;

#[derive(Template)]
#[template(path = "messages.askama.html")]
pub struct MessageTemplate {
    messages: Vec<Message>,
    user_id_encoded: String,
    admin: bool,
}

pub async fn user_referred_index(
    State(pool): State<PgPool>,
    Path(referral_user_code): Path<String>,
    original_uri: OriginalUri,
    local_user_id: Option<LocalUserId>,
    user: Option<User>,
    ClientIp(ip): ClientIp,
    headers: HeaderMap,
) -> Response {
    if let Some(u) = user {
        if u.code != referral_user_code {
            return inject_uuid_cookie(Redirect::temporary(&u.referral_uri()), &u);
        }

        let messages = match fetch_messages(&pool, &u).await {
            Ok(messages) => messages,
            Err(why) => {
                warn!("failed to fetch messages: {why:?}");
                return inject_uuid_cookie(fallback(original_uri).await, &u);
            }
        };

        return inject_uuid_cookie(
            MinifiedHtml(MessageTemplate {
                messages,
                user_id_encoded: base64::engine::general_purpose::STANDARD
                    .encode(u.id.to_string().as_bytes()), // convert to string for proper encoding
                admin: u.admin,
            }),
            &u,
        );
    }

    let Ok(found_user_referral_id) = sqlx::query_scalar!(
        "SELECT id FROM users WHERE code = $1 LIMIT 1",
        &referral_user_code
    )
    .fetch_one(&pool)
    .await
    else {
        return fallback(original_uri).await.into_response();
    };

    let local_user_id = local_user_id.unwrap_or_default();
    let user_agent = headers
        .get(USER_AGENT)
        .and_then(|ua| ua.to_str().ok())
        .map(ToString::to_string);

    let created_user = sqlx::query_as!(
        User,
        // language=postgresql
        "INSERT INTO users (id, code, user_referral, ip, user_agent)
                           VALUES ($1, $2, $3, $4, $5)
                           RETURNING *",
        local_user_id.0,
        generate_code(),
        found_user_referral_id,
        ip.to_string(),
        user_agent.as_deref()
    )
    .fetch_one(&pool)
    .await
    .expect("failed to insert user");

    inject_uuid_cookie(
        Redirect::temporary(&created_user.referral_uri()),
        &created_user,
    )
}

pub async fn location_referred_index(
    State(pool): State<PgPool>,
    Path(location_code): Path<String>,
    original_uri: OriginalUri,
    local_user_id: Option<LocalUserId>,
    user: Option<User>,
    ClientIp(ip): ClientIp,
    headers: HeaderMap,
) -> Response {
    if let Some(u) = user {
        return inject_uuid_cookie(Redirect::temporary(&u.referral_uri()), &u);
    }

    let Ok(found_location_code) = sqlx::query_scalar!(
        // language=postgresql
        "SELECT code FROM locations WHERE code = $1 LIMIT 1",
        &location_code
    )
    .fetch_one(&pool)
    .await
    else {
        return fallback(original_uri).await.into_response();
    };

    let local_user_id = local_user_id.unwrap_or_default();
    let user_agent = headers
        .get(USER_AGENT)
        .and_then(|ua| ua.to_str().ok())
        .map(ToString::to_string);

    let user = sqlx::query_as!(
        User,
        // language=postgresql
        "INSERT INTO users (id, code, location_referral, ip, user_agent)
                           VALUES ($1, $2, $3, $4, $5)
                           RETURNING *",
        local_user_id.0,
        generate_code(),
        found_location_code,
        ip.to_string(),
        user_agent.as_deref()
    )
    .fetch_one(&pool)
    .await
    .expect("failed to insert user");

    inject_uuid_cookie(Redirect::temporary(&user.referral_uri()), &user)
}

#[derive(Serialize)]
pub enum Message {
    Standard(StandardMessage),
    Full(FullMessage),
}

#[derive(Serialize, FromRow)]
pub struct StandardMessage {
    pub content: String,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Serialize, FromRow)]
pub struct FullMessage {
    pub id: Uuid,
    pub content: String,
    pub author: Uuid,
    pub flagged: bool,
    pub published: bool,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

async fn fetch_messages(pool: &PgPool, user: &User) -> WR<Vec<Message>> {
    if user.admin {
        return sqlx::query_as!(
            FullMessage,
            // language=postgresql
            "SELECT * FROM messages ORDER BY created_at DESC LIMIT 80",
        )
        .fetch_all(pool)
        .await
        .map(|messages| messages.into_iter().map(Message::Full).collect())
        .map_err(Into::into);
    }

    sqlx::query_as!(
        StandardMessage,
        // language=postgresql
        "SELECT content, created_at FROM messages
                           WHERE published OR author = $1
                           ORDER BY created_at DESC LIMIT 50",
        user.id
    )
    .fetch_all(pool)
    .await
    .map(|messages| messages.into_iter().map(Message::Standard).collect())
    .map_err(Into::into)
}

#[derive(FromRow, Debug)]
struct ExistingMessages {
    total_count: i64,
    flagged_count: i64,
    last_message_content: Option<String>,
}

async fn create_message_inner(pool: PgPool, user: User, headers: HeaderMap) -> Option<()> {
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

    let content = ammonia::clean(&String::from_utf8(content_bytes).ok()?);
    info!("msg content: {content}");
    if content.is_empty() {
        info!("empty message");
        return None;
    }

    let existing_messages = sqlx::query_as!(
        ExistingMessages,
        // language=postgresql
        r#"
            WITH last_message AS (
              SELECT content
              FROM messages
              WHERE author = $1
              ORDER BY created_at DESC
              LIMIT 1
            )
            SELECT
              COUNT(*) as "total_count!",
              COUNT(*) FILTER (WHERE flagged AND NOT published) as "flagged_count!",
              (SELECT content FROM last_message) as last_message_content
            FROM messages
            WHERE author = $1
        "#,
        user.id
    )
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

    if let Err(why) = sqlx::query!(
        // language=postgresql
        "INSERT INTO messages (content, author, flagged, published)
                           VALUES ($1, $2, $3, $4)",
        content,
        user.id,
        flagged,
        !flagged
    )
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
