mod messages;
mod util;

use axum::routing::get;
use axum::Router;
use memory_serve::{load_assets, MemoryServe};
use sqlx::postgres::{PgConnectOptions, PgPoolOptions};
use std::env;
use std::future::Future;
use std::pin::Pin;
use axum::extract::FromRequestParts;
use axum::http::request::Parts;
use axum::middleware::AddExtension;
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

    let memory_router = MemoryServe::new(load_assets!("static")).into_router();

    let app = Router::new()
        .nest("/static/", memory_router)
        // .layer(layer)
        .route("/", get(messages::index))
        .with_state(pool);

    let listener = tokio::net::TcpListener::bind("0.0.0.0:4000").await?;
    axum::serve(listener, app.into_make_service())
        .await
        .map_err(Into::into)
}

pub struct Fingerprint(Uuid);

#[axum::async_trait]
impl<S> FromRequestParts<S> for Fingerprint {
    type Rejection = ();

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        todo!()
    }
}

async fn fingerprint_middleware() {
    let fingerprint = headers
        .get(COOKIE)
        .and_then(|cookies| cookies.to_str().ok())
        .and_then(|cookies| cookies.split_once("__cf="))
        .and_then(|(_, fingerprint_and_end)| fingerprint_and_end.split_once(";"))
        .and_then(|(fingerprint, _)| Uuid::parse_str(fingerprint).ok())
        .unwrap_or_else(Uuid::new_v4);
}