use crate::messages::Message;
use crate::user::{inject_uuid_cookie, MaybeLocalUserId, User};
use crate::util::{MaybeUserAgent, MessageFromHeaders};
use crate::{
    random_codes::generate_code,
    util,
    util::{ClientIp, MinifiedHtml, WR},
};
use askama::Template;
use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::Response,
};
use sqlx::{FromRow, PgPool};
use std::net::IpAddr;
use tokio::task;
use uuid::Uuid;

#[derive(Template)]
#[template(path = "messages.askama.html")]
pub struct MessagesPageTemplate {
    messages: Vec<Message>,
    user_id_encoded: String,
    admin: bool,
}

#[derive(FromRow, Debug)]
pub struct ExistingMessages {
    pub total_count: i64,
    pub flagged_count: i64,
    pub last_message_content: Option<String>,
}

pub async fn user_referred_index(
    State(pool): State<PgPool>,
    Path(referral_code): Path<String>,
    maybe_local_user_id: MaybeLocalUserId,
    user: Option<User>,
    ClientIp(ip): ClientIp,
    maybe_user_agent: MaybeUserAgent,
) -> WR<Response> {
    match user {
        Some(user) => handle_existing_user(&pool, user, referral_code).await,
        None => {
            handle_new_user(
                &pool,
                maybe_local_user_id,
                ip,
                maybe_user_agent,
                referral_code,
            )
            .await
        }
    }
    .map_err(Into::into)
}

pub async fn location_referred_index(
    State(pool): State<PgPool>,
    Path(location_code): Path<String>,
    local_user_id: MaybeLocalUserId,
    user: Option<User>,
    ClientIp(ip): ClientIp,
    MaybeUserAgent(maybe_user_agent): MaybeUserAgent,
) -> WR<Response> {
    if let Some(user) = user {
        return Ok(inject_uuid_cookie(user.user_referral_redirect(), &user));
    }

    let found_location_code = sqlx::query_scalar!(
        // language=postgresql
        "SELECT code FROM locations WHERE code = $1 LIMIT 1",
        &location_code
    )
    .fetch_one(&pool)
    .await?;

    let user = sqlx::query_as!(
        User,
        "INSERT INTO users (id, code, location_referral, ip, user_agent)
         VALUES ($1, $2, $3, $4, $5)
         RETURNING *",
        local_user_id.make(),
        generate_code(),
        found_location_code,
        ip.to_string(),
        maybe_user_agent.as_deref()
    )
    .fetch_one(&pool)
    .await?;

    Ok(inject_uuid_cookie(user.user_referral_redirect(), &user))
}

async fn handle_existing_user(
    pool: &PgPool,
    user: User,
    referral_code: String,
) -> anyhow::Result<Response> {
    if user.code != referral_code {
        return Ok(inject_uuid_cookie(user.user_referral_redirect(), &user));
    }

    let messages = Message::fetch_for(pool, &user).await?;
    let messages_page = MessagesPageTemplate {
        messages,
        user_id_encoded: user.encode_id(),
        admin: user.admin,
    };

    Ok(inject_uuid_cookie(MinifiedHtml(messages_page), &user))
}

async fn handle_new_user(
    pool: &PgPool,
    local_user_id: MaybeLocalUserId,
    ip: IpAddr,
    maybe_user_agent: MaybeUserAgent,
    referral_code: String,
) -> anyhow::Result<Response> {
    let user_id = sqlx::query_scalar!(
        "SELECT id FROM users WHERE code = $1 LIMIT 1",
        &referral_code
    )
    .fetch_one(pool)
    .await?;

    let user = create_new_user(pool, local_user_id.make(), user_id, ip, maybe_user_agent).await?;
    Ok(inject_uuid_cookie(user.user_referral_redirect(), &user))
}

async fn create_new_user(
    pool: &PgPool,
    id: Uuid,
    user_referral_id: Uuid,
    ip: IpAddr,
    MaybeUserAgent(user_agent): MaybeUserAgent,
) -> anyhow::Result<User> {
    sqlx::query_as!(
        User,
        "INSERT INTO users (id, code, user_referral, ip, user_agent)
         VALUES ($1, $2, $3, $4, $5)
         RETURNING *",
        id,
        generate_code(),
        user_referral_id,
        ip.to_string(),
        user_agent.as_deref()
    )
    .fetch_one(pool)
    .await
    .map_err(Into::into)
}

pub async fn create_message(
    State(pool): State<PgPool>,
    user: User,
    content: Option<MessageFromHeaders>,
) -> StatusCode {
    task::spawn(async move {
        if user.banned {
            return;
        }

        let Some(MessageFromHeaders(content)) = content else {
            return;
        };

        let Some(existing) = fetch_existing_messages(&pool, &user).await else {
            return;
        };

        if !util::should_block_message(&existing, &content) {
            return;
        }

        let flagged = util::should_flag_message(&existing, &content);
        insert_message(&pool, &content, &user, flagged).await;
    });

    StatusCode::NOT_FOUND
}

async fn fetch_existing_messages(pool: &PgPool, user: &User) -> Option<ExistingMessages> {
    sqlx::query_as!(
        ExistingMessages,
        // language=postgresql
        r#"WITH last_message AS (
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
        WHERE author = $1"#,
        user.id
    )
    .fetch_one(pool)
    .await
    .ok()
}

async fn insert_message(pool: &PgPool, content: &str, user: &User, flagged: bool) {
    sqlx::query!(
        // language=postgresql
        "INSERT INTO messages (content, author, flagged, published)
         VALUES ($1, $2, $3, $4)",
        content,
        user.id,
        flagged,
        !flagged
    )
    .execute(pool)
    .await
    .expect("failed to insert message");
}
