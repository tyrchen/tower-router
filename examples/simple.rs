use anyhow::Result;
use axum_core::{body::Body, extract::Request};
use http_body_util::BodyExt;
use tower_router::{routing::get, Router};
use tower_service::Service;

#[tokio::main]
async fn main() -> Result<()> {
    let mut router = Router::<()>::new().route("/", get(|| async { "Hello, world!" }));

    let res = router
        .call(
            Request::builder()
                .uri("/")
                .body(Body::empty())
                .expect("request should be built"),
        )
        .await
        .expect("request should succeed");

    let body = res.into_body().collect().await?.to_bytes();
    let body = String::from_utf8(body.into())?;
    println!("body: {:?}", body);
    Ok(())
}
