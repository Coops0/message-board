use crate::{
    censor, censor::{score_content, CensorOutcome}, messages::{FullMessage, StandardMessage}, user::{inject_uuid_cookie, MaybeLocalUserId, User}, util::{
        clean, generate_code, ClientIp, MaybeUserAgent, MessageAndIvFromHeaders, MinifiedHtml, OptionalExtractor, WR
    }, ws::WebsocketActorMessage, AppState
};
use aes::cipher::block_padding::Pkcs7;
use askama::Template;
use axum::{
    extract::{Path, State}, http::StatusCode, response::{Html, Response}
};
use cbc::{
    cipher::{BlockDecryptMut, KeyIvInit}, Decryptor
};
use rustrict::Censor;
use sqlx::PgPool;
use std::{net::IpAddr, time::Duration};
use tokio::{sync::mpsc::Sender, task, time::sleep};

#[derive(Template)]
#[template(path = "user-messages.askama.html")]
pub struct UserMessagesPageTemplate {
    messages: Vec<StandardMessage>,
    user_id_encoded: String
}

pub async fn user_referred_index(
    State(AppState { pool, tx }): State<AppState>,
    Path(referral_code): Path<String>,
    maybe_local_user_id: MaybeLocalUserId,
    OptionalExtractor(user): OptionalExtractor<User>,
    ClientIp(ip): ClientIp,
    maybe_user_agent: MaybeUserAgent
) -> WR<Response> {
    match user {
        Some(user) => handle_existing_user(&pool, tx, user, referral_code).await,
        None => {
            handle_new_user(&pool, maybe_local_user_id, ip, maybe_user_agent, referral_code).await
        }
    }
    .map_err(Into::into)
}

pub async fn location_referred_index(
    State(AppState { pool, .. }): State<AppState>,
    Path(location_code): Path<String>,
    maybe_local_user_id: MaybeLocalUserId,
    OptionalExtractor(maybe_user): OptionalExtractor<User>,
    ClientIp(ip): ClientIp,
    MaybeUserAgent(maybe_user_agent): MaybeUserAgent
) -> WR<Response> {
    if let Some(user) = maybe_user {
        return Ok(inject_uuid_cookie(user.user_referral_redirect(), &user));
    }

    let location_code = location_code.to_lowercase();

    let found_location_code = sqlx::query_scalar!(
        // language=postgresql
        "SELECT code FROM locations WHERE code = $1 LIMIT 1",
        &location_code
    )
    .fetch_one(&pool)
    .await?;

    let local_user_id = maybe_local_user_id.make();
    let new_user_code = generate_code();

    let user = sqlx::query_as!(
        User,
        "INSERT INTO users (id, code, location_referral, ip, user_agent)
         VALUES ($1, $2, $3, $4, $5)
         RETURNING *",
        local_user_id,
        new_user_code,
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
    tx: Sender<WebsocketActorMessage>,
    user: User,
    referral_code: String
) -> anyhow::Result<Response> {
    if user.code != referral_code {
        return Ok(inject_uuid_cookie(user.user_referral_redirect(), &user));
    }

    if !user.admin {
        let mut messages = sqlx::query_as!(
            StandardMessage,
            // language=postgresql
            "SELECT id, content, created_at, author FROM messages
                                   WHERE (published OR author = $1)
                                   ORDER BY created_at DESC LIMIT 50",
            user.id
        )
        .fetch_all(pool)
        .await?;

        messages.reverse();

        let page_template =
            UserMessagesPageTemplate { messages, user_id_encoded: user.encoded_id() };

        return Ok(inject_uuid_cookie(MinifiedHtml(page_template), &user));
    }

    let mut messages = sqlx::query_as!(
        FullMessage,
        // language=postgresql
        "SELECT * FROM messages ORDER BY created_at DESC LIMIT 300"
    )
    .fetch_all(pool)
    .await?;

    messages.reverse();

    // this is so dumb but askama gets confused with vue templating syntax and fails to compile
    let admin_page = include_str!("../templates/admin-messages.vue")
        .replace("'{{ MESSAGES }}'", &serde_json::to_string(&messages)?)
        .replace("'{{ USER_ID }}'", &user.id.to_string())
        .replace("'{{ VUE_GLOBAL_SCRIPT }}'", include_str!("../assets/vue.global.prod.js"))
        .replace("'{{ TAILWIND_STYLES }}'", include_str!("../assets/ts.css"));

    #[allow(clippy::let_underscore_future)]
    let _ = task::spawn(async move {
        // give time for page to load
        sleep(Duration::from_secs(2)).await;
        let _ = tx.send(WebsocketActorMessage::RequestCount { id: user.id }).await;
    });

    Ok(inject_uuid_cookie(Html(admin_page), &user))
}

async fn handle_new_user(
    pool: &PgPool,
    maybe_local_user_id: MaybeLocalUserId,
    ip: IpAddr,
    MaybeUserAgent(maybe_user_agent): MaybeUserAgent,
    referral_code: String
) -> anyhow::Result<Response> {
    let referrer_user =
        sqlx::query_as!(User, "SELECT * FROM users WHERE code = $1 LIMIT 1", &referral_code)
            .fetch_one(pool)
            .await?;

    let local_user_id = maybe_local_user_id.make();
    let new_user_code = generate_code();

    let user = sqlx::query_as!(
        User,
        "INSERT INTO users (id, code, user_referral, ip, user_agent, banned)
         VALUES ($1, $2, $3, $4, $5, $6)
         RETURNING *",
        local_user_id,
        new_user_code,
        referrer_user.id,
        ip.to_string(),
        maybe_user_agent.as_deref(),
        referrer_user.banned
    )
    .fetch_one(pool)
    .await?;

    Ok(inject_uuid_cookie(user.user_referral_redirect(), &user))
}

pub async fn create_message(
    State(AppState { pool, tx }): State<AppState>,
    user: User,
    OptionalExtractor(header_content): OptionalExtractor<MessageAndIvFromHeaders>
) -> StatusCode {
    task::spawn(async move {
        let Some(MessageAndIvFromHeaders(encrypted_content_bytes, iv)) = header_content else {
            return;
        };

        let decryptor = Decryptor::<aes::Aes128>::new(
            user.encryption_key().as_slice().into(),
            iv.as_slice().into()
        );

        // first & last 8 bytes are noise
        let Some(cleaned_encrypted_content_bytes) =
            encrypted_content_bytes.get(8..encrypted_content_bytes.len() - 8)
        else {
            return;
        };

        let Ok(decrypted_content) =
            decryptor.decrypt_padded_vec_mut::<Pkcs7>(cleaned_encrypted_content_bytes)
        else {
            return;
        };

        let Ok(unclean_content) = String::from_utf8(decrypted_content) else {
            return;
        };

        let content = clean(&unclean_content);
        if content.is_empty() || (!user.admin && content.len() > 320) {
            return;
        }

        let profanity_type = Censor::from_str(&content).analyze();
        let score = score_content(profanity_type);

        let published = match censor::censor(&pool, &user, &content, score, profanity_type).await {
            CensorOutcome::Allow => true,
            CensorOutcome::Hide => false,
            CensorOutcome::Block => return
        };

        let full_message = sqlx::query_as!(
            FullMessage,
            // language=postgresql
            "INSERT INTO messages (content, author, published, score)
             VALUES ($1, $2, $3, $4) RETURNING *",
            content,
            user.id,
            published,
            score
        )
        .fetch_one(&pool)
        .await
        .expect("failed to insert message");

        tx.send(WebsocketActorMessage::Message { message: full_message, is_update: false })
            .await
            .expect("failed to send message");
    });

    StatusCode::NOT_FOUND
}
