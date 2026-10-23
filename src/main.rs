use axum::{Json, Router, routing::get};
use serde::Serialize;

const ADDR: &str = "127.0.0.1:3000";

#[derive(Serialize)]
struct HealthResponse {
    status: &'static str,
}

async fn health() -> Json<HealthResponse> {
    Json(HealthResponse { status: "ok" })
}

fn app() -> Router {
    Router::new().route("/health", get(health))
}

#[tokio::main]
async fn main() {
    let listener = tokio::net::TcpListener::bind(ADDR)
        .await
        .unwrap_or_else(|e| panic!("failed to bind {ADDR}: {e}"));
    println!("listening on http://{ADDR}");
    axum::serve(listener, app()).await.unwrap();
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::{Body, to_bytes};
    use axum::http::{Request, StatusCode, header};
    use axum::response::Response;
    use tower::ServiceExt; // .oneshot()

    async fn send(method: &str, uri: &str) -> Response {
        let req = Request::builder()
            .method(method)
            .uri(uri)
            .body(Body::empty())
            .unwrap();
        app().oneshot(req).await.unwrap()
    }

    #[tokio::test]
    async fn get_health_returns_ok_json() {
        let res = send("GET", "/health").await;
        assert_eq!(res.status(), StatusCode::OK);
        assert_eq!(res.headers()[header::CONTENT_TYPE], "application/json");
        let body = to_bytes(res.into_body(), usize::MAX).await.unwrap();
        assert_eq!(&body[..], br#"{"status":"ok"}"#);
    }

    #[tokio::test]
    async fn head_health_returns_200_without_body() {
        let res = send("HEAD", "/health").await;
        assert_eq!(res.status(), StatusCode::OK);
        let body = to_bytes(res.into_body(), usize::MAX).await.unwrap();
        assert!(body.is_empty());
    }

    #[tokio::test]
    async fn post_health_returns_405() {
        assert_eq!(
            send("POST", "/health").await.status(),
            StatusCode::METHOD_NOT_ALLOWED
        );
    }

    #[tokio::test]
    async fn unknown_path_returns_404() {
        assert_eq!(
            send("GET", "/does-not-exist").await.status(),
            StatusCode::NOT_FOUND
        );
    }
}
