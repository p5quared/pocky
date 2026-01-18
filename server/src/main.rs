use axum::{Router, routing::get};
use tracing::info;

use adapters::{create_app_state, handle_connection};

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    let app = Router::new()
        .route("/ws", get(handle_connection))
        .route("/ping", get(|| async { "pong" }))
        .with_state(create_app_state());

    let listener = tokio::net::TcpListener::bind("0.0.0.0:8080").await.unwrap();
    info!("Server listening on 0.0.0.0:8080");
    axum::serve(listener, app).await.unwrap();
    info!("Server shut down");
}
