use std::sync::Arc;

use axum::extract::State;
use axum::Json;
use serde::Serialize;

use domain::PlayerId;

use super::state::AppState;

#[derive(Serialize)]
pub struct QueueResponse {
    players: Vec<PlayerId>,
    count: usize,
}

pub async fn get_queue(State(state): State<Arc<AppState>>) -> Json<QueueResponse> {
    let matchmaking = state.matchmaking_service.lock().await;
    let players = matchmaking.get_queue();
    let count = players.len();
    Json(QueueResponse { players, count })
}
