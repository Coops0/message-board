mod messages;
mod util;

use axum::extract::{FromRequestParts, Request, State};
use axum::http::header::{COOKIE, SET_COOKIE, USER_AGENT};
use axum::http::request::Parts;
use axum::middleware::{from_fn_with_state, Next};
use axum::response::{Redirect, Response};
use axum::routing::{get, post};
use axum::{RequestExt, RequestPartsExt, Router};
use chrono::{DateTime, Utc};
use memory_serve::{load_assets, MemoryServe};
use sqlx::postgres::{PgConnectOptions, PgPoolOptions};
use sqlx::{FromRow, PgPool, Pool};
use std::env;
use tracing::{info, warn};
use uuid::Uuid;
use crate::util::ClientIp;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let _ = dotenvy::dotenv();

    let pool = PgPoolOptions::new()
        .connect_lazy_with(env::var("DATABASE_URL")?.parse::<PgConnectOptions>()?);

    if let Err(why) = sqlx::migrate!().run(&pool).await {
        warn!("migrations failed: {why:?}");
    } else {
        info!("migrations ran successfully / db connection valid");
    }

    let memory_router = MemoryServe::new(load_assets!("public")).into_router();

    let app = Router::new()
        .nest("/x/", memory_router)
        .layer(from_fn_with_state(
            Pool::clone(&pool),
            force_cookie_middleware,
        ))
        .route("/", get(messages::index))
        .route("/%20", post(messages::create_message))
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

#[derive(FromRow)]
pub struct FullFingerprint {
    pub id: Uuid,
    pub ip: String,
    pub user_agent: String,
    pub last_seen: DateTime<Utc>,
    pub created_at: DateTime<Utc>,
}

struct LocalFingerprint(Uuid);

impl LocalFingerprint {
    async fn create_or_update(
        &mut self,
        pool: &PgPool,
        ip: &ClientIp,
        user_agent: &str,
    ) -> sqlx::Result<FullFingerprint> {
        sqlx::query_as::<_, FullFingerprint>(
            // language=postgresql
            "INSERT INTO fingerprints (id, ip, user_agent)
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

impl From<Uuid> for LocalFingerprint {
    fn from(id: Uuid) -> Self {
        Self(id)
    }
}

#[axum::async_trait]
impl<S> FromRequestParts<S> for LocalFingerprint {
    type Rejection = ();

    async fn from_request_parts(parts: &mut Parts, _state: &S) -> Result<Self, Self::Rejection> {
        Ok(parts
            .headers
            .get(COOKIE)
            .and_then(|cookies| cookies.to_str().ok())
            .and_then(|cookies| cookies.split_once("__cf="))
            .and_then(|(_, fingerprint_and_end)| fingerprint_and_end.split_once(";"))
            .and_then(|(fingerprint, _)| Uuid::parse_str(fingerprint).ok())
            .unwrap_or_else(Uuid::new_v4)
            .into())
    }
}

#[axum::async_trait]
impl FromRequestParts<PgPool> for FullFingerprint {
    type Rejection = ();

    async fn from_request_parts(parts: &mut Parts, pool: &PgPool) -> Result<Self, Self::Rejection> {
        let client_ip = parts.extract::<ClientIp>().await?;
        let user_agent = parts.headers.get(USER_AGENT)
            .and_then(|ua| ua.to_str().ok())
            .unwrap_or_default()
            .to_string();

        let mut local_fingerprint = parts.extract::<LocalFingerprint>().await?;
        Ok(local_fingerprint
            .create_or_update(pool, &client_ip, &user_agent)
            .await
            .expect("failed to create or update fingerprint"))
    }
}

async fn force_cookie_middleware(
    State(pool): State<PgPool>,
    mut request: Request,
    next: Next,
) -> Response {
    let full_fingerprint = request
        .extract_parts_with_state::<FullFingerprint, PgPool>(&pool)
        .await
        .unwrap();

    let mut response = next.run(request).await;

    let cf_cookie_value = format!(
        "__cf={}; HttpOnly; Secure; SameSite=Strict; Expires=Fri, 31 Dec 9999 23:59:59 GMT",
        full_fingerprint.id
    )
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

    headers.insert(SET_COOKIE, cf_cookie_value);
    response
}
