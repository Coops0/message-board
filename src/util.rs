use anyhow::Context;
use askama::Template;
use axum::extract::FromRequestParts;
use axum::http::header::USER_AGENT;
use axum::http::request::Parts;
use axum::response::{Html, IntoResponse, Response};
use base64::Engine;
use minify_html::Cfg;
use rustrict::{Censor, Type};
use sqlx::FromRow;
use std::convert::Infallible;
use std::net::IpAddr;
use tracing::{info, warn};

#[derive(Debug)]
pub struct WE(anyhow::Error);

impl<E: Into<anyhow::Error>> From<E> for WE {
    fn from(e: E) -> Self {
        Self(e.into())
    }
}

#[derive(Clone)]
pub struct WebErrorExtensionMarker;

impl IntoResponse for WE {
    fn into_response(self) -> Response {
        warn!("error!! -> {:?}", self.0);
        // this will never be shown to the user
        let mut res = ().into_response();
        res.extensions_mut().insert(WebErrorExtensionMarker);

        res
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

pub struct MinifiedHtml<H: Template>(pub H);

const MINIFY_CFG: Cfg = Cfg {
    do_not_minify_doctype: false,
    ensure_spec_compliant_unquoted_attribute_values: false,
    keep_closing_tags: false,
    keep_html_and_head_opening_tags: false,
    keep_spaces_between_attributes: false,
    keep_comments: false,
    keep_input_type_text_attr: false,
    keep_ssi_comments: false,
    preserve_brace_template_syntax: false,
    preserve_chevron_percent_template_syntax: false,
    minify_css: true,
    minify_js: true,
    remove_bangs: true,
    remove_processing_instructions: true,
};

impl<H: Template> IntoResponse for MinifiedHtml<H> {
    fn into_response(self) -> Response {
        let html = self.0.render().expect("failed to render template");
        let minified_html = minify_html::minify(html.as_bytes(), &MINIFY_CFG);

        Html(minified_html).into_response()
    }
}

pub struct MaybeUserAgent(pub Option<String>);

#[axum::async_trait]
impl<S> FromRequestParts<S> for MaybeUserAgent
where
    S: Send + Sync,
{
    type Rejection = Infallible;

    async fn from_request_parts(parts: &mut Parts, _: &S) -> Result<Self, Self::Rejection> {
        Ok(Self(
            parts
                .headers
                .get(USER_AGENT)
                .and_then(|ua| ua.to_str().ok())
                .map(str::to_string),
        ))
    }
}

pub struct MessageFromHeaders(pub String);

#[axum::async_trait]
impl<S> FromRequestParts<S> for MessageFromHeaders
where
    S: Send + Sync,
{
    type Rejection = WE;

    async fn from_request_parts(parts: &mut Parts, _: &S) -> Result<Self, Self::Rejection> {
        let raw_header = parts
            .headers
            .get("CF-Cache-Identifier")
            .context("failed to get header")?
            .as_bytes();

        let content_bytes = base64::engine::general_purpose::STANDARD.decode(raw_header)?;

        if content_bytes.len() > 1024 {
            return Err(WE(anyhow::anyhow!("content too long")));
        }

        let content = ammonia::clean(&String::from_utf8(content_bytes)?);
        if content.is_empty() {
            Err(WE(anyhow::anyhow!("content is empty")))
        } else {
            Ok(Self(content))
        }
    }
}

#[derive(FromRow, Debug)]
pub struct ExistingMessages {
    pub total_count: i64,
    pub flagged_count: i64,
    pub last_message_content: Option<String>,
}

impl ExistingMessages {
    pub async fn fetch_for(pool: &sqlx::PgPool, user: &crate::user::User) -> anyhow::Result<Self> {
        sqlx::query_as!(
            ExistingMessages,
            // language=postgresql
            r#"WITH last_message AS (
            SELECT content
            FROM messages
            WHERE author = $1
            ORDER BY created_at DESC
            LIMIT 1
        )
        SELECT
            COUNT(*) as "total_count!",
            COUNT(*) FILTER (WHERE flagged AND NOT published) as "flagged_count!",
            (SELECT content FROM last_message) as last_message_content
        FROM messages
        WHERE author = $1"#,
            user.id
        )
        .fetch_one(pool)
        .await
        .map_err(Into::into)
    }

    pub fn should_flag_message(&self, content: &str) -> bool {
        if self.flagged_count > 25 {
            return true;
        }

        let profanity_type = Censor::from_str(content).analyze();
        info!("profanity type type: {:?}", profanity_type);

        profanity_type.is(Type::SEVERE)
    }

    pub fn should_block_message(&self, content: &str) -> bool {
        if self.total_count > 400 {
            return false;
        }

        if let Some(last_content) = &self.last_message_content {
            if last_content == content {
                return false;
            }
        }

        true
    }
}
