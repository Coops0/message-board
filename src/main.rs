mod messages;
mod random_codes;
mod util;

use axum::extract::{FromRequestParts, OriginalUri};
use axum::http::header::{COOKIE, SET_COOKIE};
use axum::http::request::Parts;
use axum::response::{IntoResponse, Redirect, Response};
use axum::routing::get;
use axum::{RequestPartsExt, Router};
use chrono::{DateTime, Utc};
use sqlx::postgres::{PgConnectOptions, PgPoolOptions};
use sqlx::{FromRow, PgPool};
use std::env;
use tracing::level_filters::LevelFilter;
use tracing::{info, warn};
use tracing_subscriber::EnvFilter;
use uuid::Uuid;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let _ = dotenvy::dotenv();

    let filter = EnvFilter::builder()
        .with_default_directive(LevelFilter::INFO.into())
        .from_env()?
        .add_directive("message_board=debug".parse()?);

    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .compact()
        .init();

    let pool = PgPoolOptions::new()
        .connect_lazy_with(env::var("DATABASE_URL")?.parse::<PgConnectOptions>()?);

    info!("attempting to run migrations...");
    if let Err(why) = sqlx::migrate!().run(&pool).await {
        warn!("migrations failed: {why:?}");
    } else {
        info!("migrations ran successfully / db connection valid");
    }

    let app = Router::new()
        .route("/l/:code", get(messages::location_referred_index))
        .route("/u/:code", get(messages::user_referred_index))
        .route("/favicon.ico", get(messages::create_message))
        .with_state(pool)
        .fallback(fallback);

    let listener = tokio::net::TcpListener::bind("0.0.0.0:4000").await?;
    axum::serve(listener, app.into_make_service())
        .await
        .map_err(Into::into)
}

pub async fn fallback(OriginalUri(original_uri): OriginalUri) -> Redirect {
    Redirect::temporary(original_uri.path())
}

#[derive(FromRow, Debug)]
pub struct User {
    pub id: Uuid,
    pub code: String,
    pub admin: bool,

    pub location_referral: Option<String>,
    pub user_referral: Option<Uuid>,

    pub ip: String,
    pub user_agent: Option<String>,

    pub banned: bool,
    pub created_at: DateTime<Utc>,
}

impl User {
    pub fn referral_uri(&self) -> String {
        format!("/u/{}", self.code)
    }
}

#[derive(Debug)]
pub struct LocalUserId(pub Uuid);

impl Default for LocalUserId {
    fn default() -> Self {
        Self(Uuid::new_v4())
    }
}

#[axum::async_trait]
impl<S> FromRequestParts<S> for LocalUserId {
    type Rejection = ();

    async fn from_request_parts(parts: &mut Parts, _state: &S) -> Result<Self, Self::Rejection> {
        let existing_id = parts
            .headers
            .get(COOKIE)
            .and_then(|cookies| cookies.to_str().ok())
            .and_then(|cookies| cookies.split_once("__cf="))
            .and_then(|(_, uuid_and_end)| uuid_and_end.split(';').next())
            .and_then(|uuid| Uuid::parse_str(uuid).ok());

        match existing_id {
            Some(id) => Ok(Self(id)),
            None => Err(()),
        }
    }
}

#[axum::async_trait]
impl FromRequestParts<PgPool> for User {
    type Rejection = ();

    async fn from_request_parts(parts: &mut Parts, pool: &PgPool) -> Result<Self, Self::Rejection> {
        let local_user_id = parts.extract::<LocalUserId>().await?;

        let user = sqlx::query_as!(
            User,
            // language=postgresql
            "SELECT * FROM users WHERE id = $1 LIMIT 1",
            local_user_id.0
        )
        .fetch_one(pool)
        .await
        .map_err(|_| ())?;

        Ok(user)
    }
}

pub fn inject_uuid_cookie<R: IntoResponse>(response: R, user: &User) -> Response {
    let mut response = response.into_response();
    let cf_cookie_value = format!("__cf={}; Path=/; Max-Age=31536000", user.id)
        .parse()
        .unwrap();

    let headers = response.headers_mut();

    for (name, value) in headers.iter_mut() {
        if name != SET_COOKIE {
            continue;
        }

        let Ok(val) = value.to_str() else {
            continue;
        };

        if val.contains("__cf=") {
            *value = cf_cookie_value;
            return response;
        }
    }

    headers.insert(SET_COOKIE, cf_cookie_value.clone());
    response
}
