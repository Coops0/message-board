use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use tracing::warn;

pub struct WE(anyhow::Error);

impl<E: Into<anyhow::Error>> From<E> for WE {
    fn from(e: E) -> Self {
        Self(e.into())
    }
}

impl IntoResponse for WE {
    fn into_response(self) -> Response {
        warn!("error!! -> {:?}", self.0);
        StatusCode::UPGRADE_REQUIRED.into_response()
    }
}

pub type WR<T> = Result<T, WE>;
