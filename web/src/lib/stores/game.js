import { writable, derived, get } from 'svelte/store';
import { matchmakingStore } from './matchmaking.js';

function createGameStore() {
  const { subscribe, set, update } = writable({
    phase: null, // null | countdown | running | ended
    gameId: null,
    countdown: 0,
    startingBalance: 0,
    startingPrice: 0,
    players: {}, // { [playerId]: { priceHistory, currentPrice, purchasePrices, salePrices } }
    finalBalances: [] // { playerId, balance }
  });

  let gameStartTime = null;

  return {
    subscribe,

    setCountdown: (gameId, remaining) => {
      update(s => ({
        ...s,
        phase: 'countdown',
        gameId,
        countdown: remaining
      }));
    },

    startGame: (gameId, startingPrice, startingBalance, playerIds) => {
      gameStartTime = Date.now();
      const players = {};
      playerIds.forEach(id => {
        players[id] = {
          priceHistory: [{ time: 0, value: startingPrice }],
          currentPrice: startingPrice,
          purchasePrices: [],
          salePrices: []
        };
      });
      set({
        phase: 'running',
        gameId,
        countdown: 0,
        startingBalance,
        startingPrice,
        players,
        finalBalances: []
      });
    },

    updatePrice: (playerId, price) => {
      update(s => {
        const elapsed = gameStartTime ? (Date.now() - gameStartTime) / 1000 : 0;
        const player = s.players[playerId];
        if (!player) return s;
        return {
          ...s,
          players: {
            ...s.players,
            [playerId]: {
              ...player,
              currentPrice: price,
              priceHistory: [...player.priceHistory, { time: elapsed, value: price }]
            }
          }
        };
      });
    },

    fillBid: (playerId, fillPrice) => {
      update(s => {
        const player = s.players[playerId];
        if (!player) return s;
        return {
          ...s,
          players: {
            ...s.players,
            [playerId]: {
              ...player,
              purchasePrices: [...player.purchasePrices, fillPrice]
            }
          }
        };
      });
    },

    fillAsk: (playerId, fillPrice) => {
      update(s => {
        const player = s.players[playerId];
        if (!player) return s;
        return {
          ...s,
          players: {
            ...s.players,
            [playerId]: {
              ...player,
              salePrices: [...player.salePrices, fillPrice]
            }
          }
        };
      });
    },

    endGame: (finalBalances) => {
      update(s => ({
        ...s,
        phase: 'ended',
        finalBalances: finalBalances.map(([playerId, balance]) => ({ playerId, balance }))
      }));
    },

    reset: () => {
      gameStartTime = null;
      set({
        phase: null,
        gameId: null,
        countdown: 0,
        startingBalance: 0,
        startingPrice: 0,
        players: {},
        finalBalances: []
      });
    },

    getGameId: () => get({ subscribe }).gameId
  };
}

export const gameStore = createGameStore();

// Helper function to compute player stats from fill prices
export function computePlayerStats(player, startingBalance) {
  const shares = player.purchasePrices.length - player.salePrices.length;
  const totalPurchased = player.purchasePrices.reduce((a, b) => a + b, 0);
  const totalSold = player.salePrices.reduce((a, b) => a + b, 0);
  const balance = startingBalance - totalPurchased + totalSold;
  const costBasis = player.purchasePrices.length > 0
    ? Math.round(totalPurchased / player.purchasePrices.length)
    : null;
  return { balance, shares, costBasis };
}

// Derived store for current player's P/L calculation
export const profitLoss = derived(gameStore, $game => {
  if (!$game.phase) return 0;
  const myId = matchmakingStore.getPlayerId();
  const player = $game.players[myId];
  if (!player) return 0;
  const stats = computePlayerStats(player, $game.startingBalance);
  const portfolioValue = stats.balance + (stats.shares * player.currentPrice);
  return portfolioValue - $game.startingBalance;
});
