mod messages;
mod util;

use crate::util::ClientIp;
use axum::extract::{FromRequestParts, Request, State};
use axum::http::header::{COOKIE, SET_COOKIE, USER_AGENT};
use axum::http::request::Parts;
use axum::middleware::{from_fn_with_state, Next};
use axum::response::{IntoResponse, Redirect, Response};
use axum::routing::get;
use axum::{RequestExt, RequestPartsExt, Router};
use chrono::{DateTime, Utc};
use sqlx::postgres::{PgConnectOptions, PgPoolOptions};
use sqlx::{FromRow, PgPool, Pool};
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
        .layer(from_fn_with_state(
            Pool::clone(&pool),
            force_cookie_middleware,
        ))
        .with_state(pool)
        .fallback(fallback);

    let listener = tokio::net::TcpListener::bind("0.0.0.0:4000").await?;
    axum::serve(listener, app.into_make_service())
        .await
        .map_err(Into::into)
}

async fn fallback(request: Request) -> Redirect {
    Redirect::temporary(request.uri().path())
}

#[derive(FromRow, Debug)]
pub struct User {
    pub id: Uuid,
    pub ip: String,
    pub user_agent: String,
    pub banned: bool,
    pub last_seen: DateTime<Utc>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug)]
struct LocalUserId(Uuid);

impl LocalUserId {
    async fn create_or_update(
        &mut self,
        pool: &PgPool,
        ip: &ClientIp,
        user_agent: &str,
    ) -> sqlx::Result<User> {
        sqlx::query_as::<_, User>(
            // language=postgresql
            "INSERT INTO users (id, ip, user_agent)
                            VALUES ($1, $2, $3)
                            ON CONFLICT (id) DO UPDATE
                                SET last_seen = NOW(), ip = $2, user_agent = $3
                            RETURNING *
                                ",
        )
        .bind(self.0)
        .bind(ip.0.to_string())
        .bind(user_agent)
        .fetch_one(pool)
        .await
    }
}

impl From<Uuid> for LocalUserId {
    fn from(id: Uuid) -> Self {
        Self(id)
    }
}

#[axum::async_trait]
impl<S> FromRequestParts<S> for LocalUserId {
    type Rejection = ();

    async fn from_request_parts(parts: &mut Parts, _state: &S) -> Result<Self, Self::Rejection> {
        Ok(parts
            .headers
            .get(COOKIE)
            .and_then(|cookies| cookies.to_str().ok())
            .and_then(|cookies| cookies.split_once("__cf="))
            .and_then(|(_, uuid_and_end)| uuid_and_end.split(';').next())
            .and_then(|uuid| Uuid::parse_str(uuid).ok())
            .unwrap_or_else(Uuid::new_v4)
            .into())
    }
}

#[axum::async_trait]
impl FromRequestParts<PgPool> for User {
    type Rejection = ();

    async fn from_request_parts(parts: &mut Parts, pool: &PgPool) -> Result<Self, Self::Rejection> {
        let client_ip = parts.extract::<ClientIp>().await?;
        let user_agent = parts
            .headers
            .get(USER_AGENT)
            .and_then(|ua| ua.to_str().ok())
            .unwrap_or_default()
            .to_string();

        let mut local_user_id = parts.extract::<LocalUserId>().await?;
        Ok(local_user_id
            .create_or_update(pool, &client_ip, &user_agent)
            .await
            .expect("failed to create or update user"))
    }
}

pub async fn insert_cookies_into_response<R: IntoResponse>(response: R, user: User) -> Response {
    let mut response = response.into_response();
    let headers = response.headers_mut();

    let cf_cookie_value = format!("__cf={}; Path=/; Max-Age=31536000", user.id)
        .parse()
        .unwrap();

    headers.insert(SET_COOKIE, cf_cookie_value);
    response
}

async fn force_cookie_middleware(
    State(pool): State<PgPool>,
    mut request: Request,
    next: Next,
) -> Response {
    let user = request
        .extract_parts_with_state::<User, PgPool>(&pool)
        .await
        .unwrap();

    let mut response = next.run(request).await;

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
