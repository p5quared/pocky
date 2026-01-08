use std::sync::Arc;

use axum::{Router, routing::get};
use tokio::sync::Mutex as TokioMutex;
use tracing::info;

use adapters::{
    AppState, InMemory, InMemoryQueueRepository, TokioGameScheduler, WebSocketAdapter, WebSocketQueueNotifier,
    handle_connection,
};
use application::ports::in_::{GameService, MatchmakingQueueService};
use application::ports::out_::queue::{QueueNotifier, QueueRepository};
use application::ports::out_::{GameEventNotifier, GameEventScheduler, GameRepository};

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    let ws_adapter = Arc::new(WebSocketAdapter::new());
    let repository = Arc::new(InMemory::new());

    // Game service dependencies
    let notifier: Arc<dyn GameEventNotifier> = ws_adapter.clone();
    let repo: Arc<dyn GameRepository> = repository.clone();
    let scheduler: Arc<dyn GameEventScheduler> = Arc::new(TokioGameScheduler::new(notifier.clone(), repo.clone()));

    let game_service = GameService::new(notifier, repo, scheduler);

    // Matchmaking service dependencies
    let queue_repo: Arc<dyn QueueRepository> = Arc::new(InMemoryQueueRepository::new());
    let queue_notifier: Arc<dyn QueueNotifier> = Arc::new(WebSocketQueueNotifier::new(ws_adapter.clone()));

    let matchmaking_service = MatchmakingQueueService::new(queue_repo, queue_notifier);

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
