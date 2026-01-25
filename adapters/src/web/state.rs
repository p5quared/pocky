use std::collections::HashMap;
use std::sync::Arc;

use tokio::sync::{Mutex as TokioMutex, RwLock};

use application::ports::in_::MatchmakingService;
use application::ports::in_::game_service::GameStore;
use application::ports::out_::QueueNotifier;

use super::websocket::WebSocketNotifier;

pub struct AppState {
    pub notifier: Arc<WebSocketNotifier>,
    pub game_store: GameStore,
    pub matchmaking_service: Arc<TokioMutex<MatchmakingService>>,
}

impl AppState {
    pub fn new(
        notifier: Arc<WebSocketNotifier>,
        game_store: GameStore,
        matchmaking_service: Arc<TokioMutex<MatchmakingService>>,
    ) -> Self {
        Self {
            notifier,
            game_store,
            matchmaking_service,
        }
    }
}

pub fn create_app_state() -> Arc<AppState> {
    let notifier = Arc::new(WebSocketNotifier::new());
    let game_store = Arc::new(RwLock::new(HashMap::new()));
    let queue_notifier: Arc<dyn QueueNotifier> = notifier.clone();
    let matchmaking_service = MatchmakingService::new(queue_notifier);

    Arc::new(AppState::new(
        notifier,
        game_store,
        Arc::new(TokioMutex::new(matchmaking_service)),
    ))
}
