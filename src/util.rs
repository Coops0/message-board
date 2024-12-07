use askama::Template;
use axum::extract::FromRequestParts;
use axum::http::request::Parts;
use axum::http::StatusCode;
use axum::response::{Html, IntoResponse, Response};
use minify_html::Cfg;
use std::net::IpAddr;
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

pub struct MinifiedHtml<H: Template>(pub H);

impl<H: Template> IntoResponse for MinifiedHtml<H> {
    fn into_response(self) -> Response {
        let html = self.0.render().expect("failed to render template");
        let minified_html = minify_html::minify(
            html.as_bytes(),
            &Cfg {
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
            },
        );

        Html(minified_html).into_response()
    }
}
