use crate::messages::{FullMessage, Message};
use crate::user::{inject_uuid_cookie, MaybeLocalUserId, User};
use crate::util::{ExistingMessages, MaybeUserAgent, MessageAndIvFromHeaders};
use crate::ws::WebsocketActorMessage;
use crate::{
    random_codes::generate_code,
    util::{ClientIp, MinifiedHtml, WR},
    AppState,
};
use aes::cipher::block_padding::Pkcs7;
use askama::Template;
use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::Response,
};
use cbc::cipher::{BlockDecryptMut, KeyIvInit};
use cbc::Decryptor;
use sqlx::PgPool;
use std::net::IpAddr;
use tokio::task;

#[derive(Template)]
#[template(path = "messages.component.askama.html")]
pub struct MessagesComponentTemplate {
    messages: Vec<Message>,
}

#[derive(Template)]
#[template(path = "messages.askama.html")]
pub struct MessagesPageTemplate {
    messages: Vec<Message>,
    user_id_encoded: String,
    admin: bool,
}

pub async fn user_referred_index(
    State(AppState { pool, .. }): State<AppState>,
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

pub async fn encoded_messages(
    State(AppState { pool, .. }): State<AppState>,
    user: User,
) -> WR<Response> {
    let messages_page = MessagesComponentTemplate {
        messages: Message::fetch_for(&pool, &user).await?,
    };

    Ok(inject_uuid_cookie(MinifiedHtml(messages_page), &user))
}

pub async fn location_referred_index(
    State(AppState { pool, .. }): State<AppState>,
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
        user_id_encoded: user.encoded_id(),
        admin: user.admin,
    };

    Ok(inject_uuid_cookie(MinifiedHtml(messages_page), &user))
}

async fn handle_new_user(
    pool: &PgPool,
    maybe_local_user_id: MaybeLocalUserId,
    ip: IpAddr,
    MaybeUserAgent(maybe_user_agent): MaybeUserAgent,
    referral_code: String,
) -> anyhow::Result<Response> {
    let user_id = sqlx::query_scalar!(
        "SELECT id FROM users WHERE code = $1 LIMIT 1",
        &referral_code
    )
    .fetch_one(pool)
    .await?;

    let user = sqlx::query_as!(
        User,
        "INSERT INTO users (id, code, user_referral, ip, user_agent)
         VALUES ($1, $2, $3, $4, $5)
         RETURNING *",
        maybe_local_user_id.make(),
        generate_code(),
        user_id,
        ip.to_string(),
        maybe_user_agent.as_deref()
    )
    .fetch_one(pool)
    .await?;

    Ok(inject_uuid_cookie(user.user_referral_redirect(), &user))
}

pub async fn create_message(
    State(AppState { pool, tx }): State<AppState>,
    user: User,
    header_content: Option<MessageAndIvFromHeaders>,
) -> StatusCode {
    task::spawn(async move {
        if user.banned {
            return;
        }

        let Some(MessageAndIvFromHeaders(encrypted_content_bytes, iv)) = header_content else {
            return;
        };

        let decryptor = Decryptor::<aes::Aes128>::new(
            user.encryption_key().as_slice().into(),
            iv.as_slice().into(),
        );
        let Ok(decrypted_content) = decryptor.decrypt_padded_vec_mut::<Pkcs7>(encrypted_content_bytes.as_slice())
        else {
            return;
        };

        let Ok(unclean_content) = String::from_utf8(decrypted_content) else {
            return;
        };

        let content = ammonia::clean(&unclean_content);
        if content.is_empty() {
            return;
        }

        let Ok(existing) = ExistingMessages::fetch_for(&pool, &user).await else {
            return;
        };

        if !existing.should_block_message(&content) {
            return;
        }

        let flagged = existing.should_flag_message(&content);
        let full_message = sqlx::query_as!(
            FullMessage,
            // language=postgresql
            "INSERT INTO messages (content, author, flagged, published)
             VALUES ($1, $2, $3, $4) RETURNING *",
            content,
            user.id,
            flagged,
            !flagged
        )
        .fetch_one(&pool)
        .await
        .expect("failed to insert message");

        tx.send(WebsocketActorMessage::Message(full_message))
            .await
            .expect("failed to send message");
    });

    StatusCode::NOT_FOUND
}
