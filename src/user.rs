use crate::util::WE;
use crate::AppState;
use anyhow::anyhow;
use axum::extract::FromRequestParts;
use axum::http::header::{COOKIE, SET_COOKIE};
use axum::http::request::Parts;
use axum::response::{IntoResponse, Redirect, Response};
use axum::RequestPartsExt;
use base64::Engine;
use chrono::{DateTime, Utc};
use serde::Serialize;
use sqlx::FromRow;
use std::convert::Infallible;
use uuid::Uuid;

#[allow(dead_code)]
#[derive(FromRow, Clone, Serialize)]
pub struct User {
    pub id: Uuid,
    pub code: String,
    pub admin: bool,

    pub location_referral: Option<String>,
    #[allow(clippy::struct_field_names)]
    pub user_referral: Option<Uuid>,

    pub ip: String,
    #[allow(clippy::struct_field_names)]
    pub user_agent: Option<String>,

    pub banned: bool,
    pub created_at: DateTime<Utc>,
}

impl User {
    pub fn user_referral_redirect(&self) -> Redirect {
        Redirect::temporary(&format!("/u/{}", self.code))
    }

    pub fn encoded_id(&self) -> String {
        base64::engine::general_purpose::STANDARD.encode(self.id.to_string())
    }

    pub fn encryption_key(&self) -> Vec<u8> {
        let uuid_string = self.id.to_string();
        let uuid_first_bytes = &uuid_string.as_bytes()[0..16];

        uuid_first_bytes.to_vec()
    }
}

#[derive(Debug)]
pub struct MaybeLocalUserId(pub Option<Uuid>);

impl MaybeLocalUserId {
    pub fn make(mut self) -> Uuid {
        self.0.take().unwrap_or_else(Uuid::new_v4)
    }
}

#[axum::async_trait]
impl<S> FromRequestParts<S> for MaybeLocalUserId {
    type Rejection = Infallible;

    async fn from_request_parts(parts: &mut Parts, _state: &S) -> Result<Self, Self::Rejection> {
        Ok(Self(
            parts
                .headers
                .get(COOKIE)
                .and_then(|cookies| cookies.to_str().ok())
                .and_then(|cookies| cookies.split_once("__cf="))
                .and_then(|(_, uuid_and_end)| uuid_and_end.split(';').next())
                .and_then(|uuid| Uuid::parse_str(uuid).ok()),
        ))
    }
}

#[axum::async_trait]
impl FromRequestParts<AppState> for User {
    type Rejection = WE;

    async fn from_request_parts(
        parts: &mut Parts,
        AppState { pool, .. }: &AppState,
    ) -> Result<Self, Self::Rejection> {
        let Ok(MaybeLocalUserId(Some(local_user_id))) = parts.extract::<MaybeLocalUserId>().await
        else {
            return Err(WE(anyhow!("failed to get local user id")));
        };

        sqlx::query_as!(
            User,
            // language=postgresql
            "SELECT * FROM users WHERE id = $1 LIMIT 1",
            local_user_id
        )
        .fetch_one(pool)
        .await
        .map_err(Into::into)
    }
}

pub fn inject_uuid_cookie<R: IntoResponse>(response: R, user: &User) -> Response {
    let mut response = response.into_response();
    let cf_cookie_value = format!("__cf={}; Path=/; Max-Age=31536000", user.id)
        .parse()
        .expect("failed to parse user id cookie value?");

    let headers = response.headers_mut();

    if let Some((_, existing_header_value)) = headers
        .iter_mut()
        .filter(|(name, _)| name == &SET_COOKIE)
        .find(|(_, value)| value.to_str().is_ok_and(|val| val.contains("__cf=")))
    {
        *existing_header_value = cf_cookie_value;
        return response;
    }

    headers.insert(SET_COOKIE, cf_cookie_value);
    response
}
