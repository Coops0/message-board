mod messages;
mod util;

use axum::extract::{FromRequestParts, Request};
use axum::http::header::{COOKIE, SET_COOKIE};
use axum::http::request::Parts;
use axum::middleware::{from_fn, Next};
use axum::response::Response;
use axum::routing::get;
use axum::{RequestExt, Router};
use memory_serve::{load_assets, MemoryServe};
use sqlx::postgres::{PgConnectOptions, PgPoolOptions};
use std::env;
use tracing::{info, warn};
use uuid::Uuid;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let _ = dotenvy::dotenv();

    let pool = PgPoolOptions::new()
        .connect_lazy_with(env::var("DATABASE_URL")?.parse::<PgConnectOptions>()?);

    if let Err(why) = sqlx::migrate!().run(&pool).await {
        warn!("migrations failed: {:?}", why);
    } else {
        info!("migrations ran successfully / db connection valid");
    }

    let memory_router = MemoryServe::new(load_assets!("public")).into_router();

    let app = Router::new()
        .nest("/x/", memory_router)
        .layer(from_fn(force_cookie_middleware))
        .route("/", get(messages::index))
        .with_state(pool);

    let listener = tokio::net::TcpListener::bind("0.0.0.0:4000").await?;
    axum::serve(listener, app.into_make_service())
        .await
        .map_err(Into::into)
}

pub struct Fingerprint {
    pub id: Uuid,
    pub is_stored: bool,
}

impl Default for Fingerprint {
    fn default() -> Self {
        Self::new()
    }
}

impl Fingerprint {
    pub fn new() -> Self {
        Self {
            id: Uuid::new_v4(),
            is_stored: false,
        }
    }
}

impl From<Uuid> for Fingerprint {
    fn from(id: Uuid) -> Self {
        Self {
            id,
            is_stored: false,
        }
    }
}

#[axum::async_trait]
impl<S> FromRequestParts<S> for Fingerprint {
    type Rejection = ();

    async fn from_request_parts(parts: &mut Parts, _state: &S) -> Result<Self, Self::Rejection> {
        Ok(
            parts
            .headers
            .get(COOKIE)
            .and_then(|cookies| cookies.to_str().ok())
            .and_then(|cookies| cookies.split_once("__cf="))
            .and_then(|(_, fingerprint_and_end)| fingerprint_and_end.split_once(";"))
            .and_then(|(fingerprint, _)| Uuid::parse_str(fingerprint).ok())
            .unwrap_or_else(Uuid::new_v4)
            .into()
        )
    }
}

async fn force_cookie_middleware(mut request: Request, next: Next) -> Response {
    let fingerprint = request.extract_parts::<Fingerprint>().await.unwrap();
    let mut response = next.run(request).await;

    let cf_cookie_value = format!(
        "__cf={}; HttpOnly; Secure; SameSite=Strict; Expires=Fri, 31 Dec 9999 23:59:59 GMT",
        fingerprint.id
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
