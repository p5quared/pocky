use std::sync::Arc;

use axum::{Router, routing::get};
use tokio::sync::Mutex as TokioMutex;
use tracing::info;

use adapters::{AppState, InMemory, TokioGameScheduler, WebSocketNotifier, handle_connection};
use application::ports::in_::{GameService, MatchmakingService};
use application::ports::out_::{GameEventNotifier, GameEventScheduler, GameRepository, QueueNotifier};

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    let ws_adapter = Arc::new(WebSocketNotifier::new());
    let repository = Arc::new(InMemory::new());

    // Game service dependencies
    let notifier: Arc<dyn GameEventNotifier> = ws_adapter.clone();
    let repo: Arc<dyn GameRepository> = repository.clone();
    let scheduler: Arc<dyn GameEventScheduler> = Arc::new(TokioGameScheduler::new(notifier.clone(), repo.clone()));

    let game_service = GameService::new(notifier, repo, scheduler);

    // Matchmaking service dependencies
    let queue_notifier: Arc<dyn QueueNotifier> = ws_adapter.clone();

    let matchmaking_service = MatchmakingService::new(queue_notifier);

    let app_state = Arc::new(AppState::new(
        ws_adapter,
        Arc::new(TokioMutex::new(game_service)),
        Arc::new(TokioMutex::new(matchmaking_service)),
    ));

    let app = Router::new().route("/ws", get(handle_connection)).with_state(app_state);

    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await.unwrap();
    info!("Server listening on 0.0.0.0:3000");
    axum::serve(listener, app).await.unwrap();
    info!("Server shut down");
}
