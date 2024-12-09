mod controller;
mod messages;
mod random_codes;
mod user;
mod util;

use crate::user::{inject_uuid_cookie, User};
use crate::util::WebErrorExtensionMarker;
use axum::extract::{OriginalUri, Request, State};
use axum::middleware::{from_fn_with_state, Next};
use axum::response::{IntoResponse, Redirect, Response};
use axum::routing::get;
use axum::{RequestExt, Router};
use sqlx::postgres::{PgConnectOptions, PgPoolOptions};
use sqlx::PgPool;
use std::env;
use axum::http::StatusCode;
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
        .layer(from_fn_with_state(
            PgPool::clone(&pool),
            intercept_web_error,
        ))
        .with_state(pool)
        .fallback(fallback);

    let listener = tokio::net::TcpListener::bind("0.0.0.0:4000").await?;
    axum::serve(listener, app.into_make_service())
        .await
        .map_err(Into::into)
}

pub async fn fallback(OriginalUri(original_uri): OriginalUri) -> Response {
    // Redirect::temporary(original_uri.path())
    (StatusCode::INTERNAL_SERVER_ERROR, "nah").into_response()
}

async fn intercept_web_error(
    State(pool): State<PgPool>,
    mut request: Request,
    next: Next,
) -> Response {
    let original_uri = request.extract_parts::<OriginalUri>().await.unwrap();
    let maybe_user = request
        .extract_parts_with_state::<User, PgPool>(&pool)
        .await
        .ok();

    let response = next.run(request).await;
    if response
        .extensions()
        .get::<WebErrorExtensionMarker>()
        .is_none()
    {
        return response;
    }

    let Some(user) = maybe_user else {
        return fallback(original_uri).await;
    };

    inject_uuid_cookie(user.user_referral_redirect(), &user)
}
