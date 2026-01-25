use std::sync::Arc;

use axum::Json;
use axum::extract::State;
use serde::Serialize;

use domain::PlayerId;

use super::state::AppState;

#[derive(Serialize)]
pub struct GetQueueResponse {
    players: Vec<PlayerId>,
    count: usize,
}

pub async fn get_queue(State(state): State<Arc<AppState>>) -> Json<GetQueueResponse> {
    let matchmaking = state.matchmaking_service.lock().await;
    let players = matchmaking.get_queue();
    let count = players.len();
    Json(GetQueueResponse { players, count })
}
