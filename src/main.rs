mod admin_controller;
mod censor;
mod controller;
mod messages;
mod user;
mod util;
mod ws;

use crate::{
    user::{inject_uuid_cookie, User}, util::{OptionalExtractor, WebErrorExtensionMarker}, ws::WebsocketActorMessage
};
use axum::{
    extract::{Request, State}, http::{header::WWW_AUTHENTICATE, HeaderMap, StatusCode}, middleware::{from_fn_with_state, Next}, response::{IntoResponse, Response}, routing::{any, get}, RequestExt, Router
};
use base64::{prelude::BASE64_STANDARD, Engine};
use sqlx::{
    postgres::{PgConnectOptions, PgPoolOptions}, PgPool
};
use std::env;
use tokio::sync::{mpsc, mpsc::Sender};
use tracing::{info, level_filters::LevelFilter, warn};
use tracing_subscriber::EnvFilter;

#[derive(Clone)]
pub struct AppState {
    pool: PgPool,
    tx: Sender<WebsocketActorMessage>
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let _ = dotenvy::dotenv();

    let filter = EnvFilter::builder()
        .with_default_directive(LevelFilter::INFO.into())
        .from_env()?
        .add_directive("message_board=debug".parse()?);

    tracing_subscriber::fmt().with_env_filter(filter).compact().init();

    let pg_connect_opts = env::var("DATABASE_URL")?.parse::<PgConnectOptions>()?;
    let pool = PgPoolOptions::new().connect_lazy_with(pg_connect_opts.clone());

    info!("attempting to run migrations with db host {}...", pg_connect_opts.get_host());
    if let Err(why) = sqlx::migrate!().run(&pool).await {
        warn!("migrations failed: {why:?}");
    } else {
        info!("migrations ran successfully / db connection valid");
    }

    let (tx, rx) = mpsc::channel(100);

    let state = AppState { pool, tx };

    let app = Router::new()
        .route("/l/{code}", get(controller::location_referred_index))
        .route("/u/{code}", get(controller::user_referred_index))
        .route("/favicon.ico", get(controller::create_message))
        .route("/-", any(ws::ws_route))
        .nest("/admin", admin_controller::admin_controller(AppState::clone(&state)))
        .fallback(inner_fallback)
        .layer(from_fn_with_state(AppState::clone(&state), intercept_web_error))
        .with_state(state);

    #[allow(clippy::let_underscore_future)]
    let _ = tokio::spawn(ws::socket_owner_actor(rx));

    let listener = tokio::net::TcpListener::bind("0.0.0.0:5000").await?;
    axum::serve(listener, app.into_make_service()).await.map_err(Into::into)
}

async fn inner_fallback(
    State(_state): State<AppState>, // need this for the user extractor to work
    OptionalExtractor(maybe_user): OptionalExtractor<User>,
    headers: HeaderMap
) -> Response {
    if let Some(user) = maybe_user {
        return inject_uuid_cookie(user.user_referral_redirect(), &user);
    };

    if let Some(login) = headers
        .get("Authorization")
        .and_then(|auth| auth.to_str().ok())
        .and_then(|auth| auth.strip_prefix("Basic "))
        .and_then(|auth| BASE64_STANDARD.decode(auth).ok())
        .and_then(|auth| String::from_utf8(auth).ok())
    {
        info!("BASIC AUTH: {login}");
    }

    fallback().await
}

#[allow(clippy::unused_async, clippy::missing_panics_doc)]
pub async fn fallback() -> Response {
    let mut res = StatusCode::UNAUTHORIZED.into_response();
    res.headers_mut().insert(WWW_AUTHENTICATE, "Basic".parse().unwrap());

    res
}

async fn intercept_web_error(
    State(state): State<AppState>,
    mut request: Request,
    next: Next
) -> Response {
    let maybe_user = request.extract_parts_with_state::<User, AppState>(&state).await.ok();

    let response = next.run(request).await;
    if response.extensions().get::<WebErrorExtensionMarker>().is_none() {
        return response;
    }

    let Some(user) = maybe_user else {
        return fallback().await;
    };

    inject_uuid_cookie(user.user_referral_redirect(), &user)
}
