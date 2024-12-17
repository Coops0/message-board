use anyhow::Context;
use askama::Template;
use axum::{
    extract::FromRequestParts, http::{header::USER_AGENT, request::Parts}, response::{Html, IntoResponse, Response}
};
use base64::Engine;
use minify_html::Cfg;
use rand::prelude::SliceRandom;
use std::{cell::LazyCell, convert::Infallible, net::IpAddr};
use tracing::warn;

#[derive(Debug)]
pub struct WE(pub anyhow::Error);

impl<E: Into<anyhow::Error>> From<E> for WE {
    fn from(e: E) -> Self {
        Self(e.into())
    }
}

#[derive(Clone)]
pub struct WebErrorExtensionMarker;

impl IntoResponse for WE {
    fn into_response(self) -> Response {
        warn!("error on request -> {:?}", self.0);
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
                .unwrap_or_else(|| {
                    #[cfg(not(debug_assertions))]
                    panic!("failed to get client ip");

                    #[cfg(debug_assertions)]
                    IpAddr::V4(std::net::Ipv4Addr::new(0, 0, 0, 0))
                })
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
    remove_processing_instructions: true
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
    S: Send + Sync
{
    type Rejection = Infallible;

    async fn from_request_parts(parts: &mut Parts, _: &S) -> Result<Self, Self::Rejection> {
        Ok(Self(parts.headers.get(USER_AGENT).and_then(|ua| ua.to_str().ok()).map(ammonia::clean)))
    }
}

pub struct MessageAndIvFromHeaders(pub Vec<u8>, pub Vec<u8>);

#[axum::async_trait]
impl<S> FromRequestParts<S> for MessageAndIvFromHeaders
where
    S: Send + Sync
{
    type Rejection = WE;

    async fn from_request_parts(parts: &mut Parts, _: &S) -> Result<Self, Self::Rejection> {
        let raw_content_header =
            parts.headers.get("CF-Cache-Identifier").context("failed to get header")?.as_bytes();

        let raw_iv_header = parts
            .headers
            .get("Uses-Agent")
            .and_then(|ua| ua.to_str().ok())
            .and_then(|ua| ua.split_once("Mozilla/5.0 (Windows NT 10.0; Win64; x64; "))
            .and_then(|(_, iv)| iv.split(')').next())
            .context("failed to get iv")?
            .as_bytes();

        let content_bytes = base64::engine::general_purpose::STANDARD.decode(raw_content_header)?;
        if content_bytes.len() > 1024 {
            return Err(WE(anyhow::anyhow!("content too long")));
        }

        let iv_bytes = base64::engine::general_purpose::STANDARD.decode(raw_iv_header)?;
        if iv_bytes.len() != 16 {
            return Err(WE(anyhow::anyhow!("iv length is not 16")));
        }

        Ok(Self(content_bytes, iv_bytes))
    }
}

const WORDS_STRING_LIST: &str = include_str!("../assets/all_english_words_clean.txt");
#[allow(clippy::declare_interior_mutable_const)]
const WORDS_ARRAY: LazyCell<Vec<&str>> = LazyCell::new(|| WORDS_STRING_LIST.lines().collect());

pub fn generate_code() -> String {
    let mut rng = rand::thread_rng();

    #[allow(clippy::borrow_interior_mutable_const)]
    let words =
        WORDS_ARRAY.choose_multiple(&mut rng, 2).map(ToString::to_string).collect::<Vec<String>>();

    let [first_word, second_word] = &words[..] else {
        unreachable!();
    };

    format!("{first_word}.{second_word}")
}
