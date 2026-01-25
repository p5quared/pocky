use axum::{Router, routing::get};
use tower_http::cors::{Any, CorsLayer};
use tracing::info;

use adapters::{create_app_state, get_queue, handle_connection};

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    let app = Router::new()
        .route("/ws", get(handle_connection))
        .route("/ping", get(|| async { "pong" }))
        .route("/queue", get(get_queue))
        .layer(cors)
        .with_state(create_app_state());

    let listener = tokio::net::TcpListener::bind("0.0.0.0:8080").await.unwrap();
    info!("Server listening on 0.0.0.0:8080");
    axum::serve(listener, app).await.unwrap();
    info!("Server shut down");
}
