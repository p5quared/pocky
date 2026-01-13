use std::collections::HashMap;
use std::sync::Arc;

use axum::{Router, routing::get};
use tokio::sync::{Mutex as TokioMutex, RwLock};
use tracing::info;

use adapters::{AppState, WebSocketNotifier, handle_connection};
use application::ports::in_::MatchmakingService;
use application::ports::out_::QueueNotifier;

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    let notifier = Arc::new(WebSocketNotifier::new());
    let game_store = Arc::new(RwLock::new(HashMap::new()));

    let queue_notifier: Arc<dyn QueueNotifier> = notifier.clone();
    let matchmaking_service = MatchmakingService::new(queue_notifier);

    let app_state = Arc::new(AppState::new(
        notifier,
        game_store,
        Arc::new(TokioMutex::new(matchmaking_service)),
    ));

    let app = Router::new().route("/ws", get(handle_connection)).with_state(app_state);

    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await.unwrap();
    info!("Server listening on 0.0.0.0:3000");
    axum::serve(listener, app).await.unwrap();
    info!("Server shut down");
}
