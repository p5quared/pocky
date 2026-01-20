import { matchmakingStore } from '../stores/matchmaking.js';
import { gameStore } from '../stores/game.js';

// Parse and handle incoming server messages
export function handleMessage(data) {
  let msg;
  try {
    msg = JSON.parse(data);
  } catch (e) {
    console.error('Failed to parse message:', e);
    return;
  }

  // Check for game notifications (tagged with "type")
  if (msg.type) {
    handleGameNotification(msg);
    return;
  }

  // Matchmaking messages (untagged)
  handleMatchmakingMessage(msg);
}

function handleMatchmakingMessage(msg) {
  if (msg.Enqueued !== undefined) {
    matchmakingStore.setEnqueued(msg.Enqueued);
  } else if (msg.Matched !== undefined) {
    matchmakingStore.setMatched(msg.Matched);
  } else if (msg.Dequeued !== undefined) {
    matchmakingStore.setDequeued();
  } else if (msg.AlreadyQueued !== undefined) {
    matchmakingStore.setAlreadyQueued();
  } else if (msg.PlayerNotFound !== undefined) {
    matchmakingStore.setPlayerNotFound();
  }
}

function handleGameNotification(msg) {
  switch (msg.type) {
    case 'countdown':
      gameStore.setCountdown(msg.game_id, msg.remaining);
      break;

    case 'game_started':
      gameStore.startGame(
        msg.game_id,
        msg.starting_price,
        msg.starting_balance,
        msg.players
      );
      break;

    case 'price_changed':
      gameStore.updatePrice(msg.price);
      break;

    case 'bid_placed':
      gameStore.addOrder('bid', msg.player_id, msg.bid_value);
      break;

    case 'ask_placed':
      gameStore.addOrder('ask', msg.player_id, msg.ask_value);
      break;

    case 'bid_filled':
      gameStore.fillBid(msg.player_id, msg.bid_value);
      break;

    case 'ask_filled':
      gameStore.fillAsk(msg.player_id, msg.ask_value);
      break;

    case 'game_ended':
      gameStore.endGame(msg.final_balances);
      break;
  }
}
