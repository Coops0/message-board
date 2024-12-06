use axum::response::{IntoResponse, Response};
use tracing::warn;

pub struct WE(anyhow::Error);

impl From<anyhow::Error> for WE {
    fn from(e: anyhow::Error) -> Self {
        Self(e)
    }
}

impl IntoResponse for WE {
    fn into_response(self) -> Response {
        warn!("error!! -> {:?}", self.0);
        (400, "you clicked the wrong one").into_response()
    }
}

pub type WR<T> = Result<T, WE>;