mod controller;
mod messages;
mod random_codes;
mod user;
mod util;

use crate::util::WebErrorExtensionMarker;
use axum::extract::{OriginalUri, Request};
use axum::middleware::{from_fn, Next};
use axum::response::{IntoResponse, Redirect, Response};
use axum::routing::get;
use axum::{RequestExt, Router};
use sqlx::postgres::{PgConnectOptions, PgPoolOptions};
use std::env;
use tracing::level_filters::LevelFilter;
use tracing::{info, warn};
use tracing_subscriber::EnvFilter;

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
        .route("/l/:code", get(controller::location_referred_index))
        .route("/u/:code", get(controller::user_referred_index))
        .route("/favicon.ico", get(controller::create_message))
        .route(
            "/cgi-bin/cloudflare-verify.php",
            get(controller::encoded_messages),
        )
        .layer(from_fn(intercept_web_error))
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

async fn intercept_web_error(mut request: Request, next: Next) -> Response {
    let original_uri = request.extract_parts::<OriginalUri>().await.unwrap();

    let response = next.run(request).await;
    if response
        .extensions()
        .get::<WebErrorExtensionMarker>()
        .is_some()
    {
        fallback(original_uri).await.into_response()
    } else {
        response
    }
}
