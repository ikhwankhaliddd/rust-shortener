use std::collections::HashMap;
use std::collections::hash_map::Entry;
use std::sync::{Arc, RwLock};

use axum::Json;
use axum::extract::State;
use axum::http::StatusCode;
use rand::RngExt;
use rand::distr::Alphanumeric;
use serde::{Deserialize, Serialize};
use url::Url;

const CODE_LEN: usize = 6;

#[derive(Clone, Serialize)]
pub struct Link {
    pub code: String,
    pub url: String,
}

#[derive(Clone, Default)]
pub struct AppState {
    links: Arc<RwLock<HashMap<String, Link>>>,
}

#[derive(Deserialize)]
pub struct CreateLink {
    pub url: String,
}

pub async fn create_link(
    State(state): State<AppState>,
    Json(input): Json<CreateLink>,
) -> Result<(StatusCode, Json<Link>), (StatusCode, &'static str)> {
    let url = normalize_url(&input.url).ok_or((
        StatusCode::UNPROCESSABLE_ENTITY,
        "url must be an absolute http or https URL",
    ))?;
    let mut links = state.links.write().expect("link store lock poisoned");
    loop {
        if let Entry::Vacant(slot) = links.entry(generate_code()) {
            let link = Link {
                code: slot.key().clone(),
                url,
            };
            slot.insert(link.clone());
            return Ok((StatusCode::CREATED, Json(link)));
        }
    }
}

fn generate_code() -> String {
    rand::rng()
        .sample_iter(Alphanumeric)
        .take(CODE_LEN)
        .map(char::from)
        .collect()
}

fn normalize_url(raw: &str) -> Option<String> {
    let url = Url::parse(raw).ok()?;
    match (url.scheme(), url.host()) {
        ("http" | "https", Some(_)) => Some(url.into()),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app;
    use axum::Router;
    use axum::body::{Body, to_bytes};
    use axum::http::{Request, StatusCode, header};
    use axum::response::Response;
    use tower::ServiceExt;

    async fn post_link(app: &Router, body: &str) -> Response {
        let req = Request::post("/links")
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(body.to_owned()))
            .unwrap();
        app.clone().oneshot(req).await.unwrap()
    }

    async fn json(res: Response) -> serde_json::Value {
        let bytes = to_bytes(res.into_body(), usize::MAX).await.unwrap();
        serde_json::from_slice(&bytes).unwrap()
    }

    #[test]
    fn generate_code_is_six_ascii_alphanumerics() {
        for _ in 0..1000 {
            let code = generate_code();
            assert_eq!(code.len(), 6);
            assert!(code.chars().all(|c| c.is_ascii_alphanumeric()), "{code}");
        }
    }

    #[tokio::test]
    async fn post_links_returns_201_with_code_and_url() {
        let app = app();
        let res = post_link(
            &app,
            r#"{"url":"https://mindwalker.ai/products/mindgraph"}"#,
        )
        .await;
        assert_eq!(res.status(), StatusCode::CREATED);
        assert_eq!(res.headers()[header::CONTENT_TYPE], "application/json");
        let body = json(res).await;
        assert_eq!(body["url"], "https://mindwalker.ai/products/mindgraph");
        let code = body["code"].as_str().unwrap();
        assert_eq!(code.len(), 6);
        assert!(code.chars().all(|c| c.is_ascii_alphanumeric()));
        assert_eq!(body.as_object().unwrap().len(), 2);
    }

    #[tokio::test]
    async fn post_links_rejects_non_http_urls() {
        let app = app();
        for bad in [
            "not a url",
            "/relative/path",
            "javascript:alert(1)",
            "ftp://example.com/file",
            "mailto:a@b.c",
            "",
        ] {
            let res = post_link(&app, &serde_json::json!({ "url": bad }).to_string()).await;
            assert_eq!(
                res.status(),
                StatusCode::UNPROCESSABLE_ENTITY,
                "url = {bad:?}"
            );
        }
    }

    #[tokio::test]
    async fn post_links_rejects_malformed_bodies() {
        let app = app();
        assert_eq!(
            post_link(&app, "{}").await.status(),
            StatusCode::UNPROCESSABLE_ENTITY
        );
        assert_eq!(
            post_link(&app, "not json").await.status(),
            StatusCode::BAD_REQUEST
        );
        let no_ct = Request::post("/links")
            .body(Body::from(r#"{"url":"https://a.b"}"#))
            .unwrap();
        assert_eq!(
            app.clone().oneshot(no_ct).await.unwrap().status(),
            StatusCode::UNSUPPORTED_MEDIA_TYPE
        );
    }

    #[tokio::test]
    async fn post_links_percent_encodes_non_ascii_url() {
        let app = app();
        let res = post_link(
            &app,
            r#"{"url":"https://id.wikipedia.org/wiki/Bahasa_Indonésia"}"#,
        )
        .await;
        assert_eq!(res.status(), StatusCode::CREATED);
        assert_eq!(
            json(res).await["url"],
            "https://id.wikipedia.org/wiki/Bahasa_Indon%C3%A9sia"
        );
    }
}
