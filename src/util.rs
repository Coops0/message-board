use axum::extract::FromRequestParts;
use axum::http::request::Parts;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use std::future::Future;
use std::net::IpAddr;
use std::pin::Pin;
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

pub struct ClientIp(pub IpAddr);

#[axum::async_trait]
impl<S> FromRequestParts<S> for ClientIp {
    type Rejection = ();

    async fn from_request_parts(parts: &mut Parts, _state: &S) -> Result<Self, Self::Rejection> {
        Ok(Self(
            parts
                .headers
                .get("CF-Connecting-IP")
                .and_then(|ip| ip.to_str().ok())
                .and_then(|ip| ip.parse::<IpAddr>().ok())
                // todo
                .unwrap_or_else(|| IpAddr::V4(std::net::Ipv4Addr::new(0, 0, 0, 0))),
                // .unwrap_or_else(|| panic!("failed to get client ip")),
        ))
    }
}
