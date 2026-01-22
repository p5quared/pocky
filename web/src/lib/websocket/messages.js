// Outgoing message builders

export function joinQueue() {
  return { type: 'join_queue' };
}

export function leaveQueue() {
  return { type: 'leave_queue' };
}

export function placeBid(gameId, value) {
  return { type: 'place_bid', game_id: gameId, value };
}

export function placeAsk(gameId, value) {
  return { type: 'place_ask', game_id: gameId, value };
}

export function cancelBid(gameId, price) {
  return { type: 'cancel_bid', game_id: gameId, price };
}

export function cancelAsk(gameId, price) {
  return { type: 'cancel_ask', game_id: gameId, price };
}
