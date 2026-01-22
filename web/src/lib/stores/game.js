import { writable, derived, get } from 'svelte/store';
import { matchmakingStore } from './matchmaking.js';

function createGameStore() {
  const { subscribe, set, update } = writable({
    phase: null, // null | countdown | running | ended
    gameId: null,
    countdown: 0,
    currentPrice: 0,
    priceHistory: [], // { time, value }
    balance: 0,
    startingBalance: 0,
    shares: 0,
    openOrders: [], // { type: 'bid'|'ask', playerId, value }
    cursorPrice: 0,
    players: [],
    finalBalances: [], // { playerId, balance }
    balanceHistory: [] // { time, value }
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

    startGame: (gameId, startingPrice, startingBalance, players) => {
      gameStartTime = Date.now();
      set({
        phase: 'running',
        gameId,
        countdown: 0,
        currentPrice: startingPrice,
        priceHistory: [{ time: 0, value: startingPrice }],
        balance: startingBalance,
        startingBalance,
        shares: 0,
        openOrders: [],
        cursorPrice: startingPrice,
        players,
        finalBalances: [],
        balanceHistory: [{ time: 0, value: startingBalance }]
      });
    },

    updatePrice: (price) => {
      update(s => {
        const elapsed = gameStartTime ? (Date.now() - gameStartTime) / 1000 : 0;
        return {
          ...s,
          currentPrice: price,
          priceHistory: [...s.priceHistory, { time: elapsed, value: price }]
        };
      });
    },

    addOrder: (type, playerId, value) => {
      update(s => ({
        ...s,
        openOrders: [...s.openOrders, { type, playerId, value }]
      }));
    },

    fillBid: (playerId, bidValue) => {
      update(s => {
        const myId = matchmakingStore.getPlayerId();
        const isMine = playerId === myId;
        const newBalance = isMine ? s.balance - bidValue : s.balance;
        let elapsed = gameStartTime ? (Date.now() - gameStartTime) / 1000 : 0;
        // Ensure strictly increasing time for chart library
        const lastTime = s.balanceHistory.length > 0 ? s.balanceHistory[s.balanceHistory.length - 1].time : -1;
        if (elapsed <= lastTime) {
          elapsed = lastTime + 0.001;
        }
        return {
          ...s,
          openOrders: s.openOrders.filter(
            o => !(o.type === 'bid' && o.playerId === playerId && o.value === bidValue)
          ),
          balance: newBalance,
          shares: isMine ? s.shares + 1 : s.shares,
          balanceHistory: isMine ? [...s.balanceHistory, { time: elapsed, value: newBalance }] : s.balanceHistory
        };
      });
    },

    fillAsk: (playerId, askValue) => {
      update(s => {
        const myId = matchmakingStore.getPlayerId();
        const isMine = playerId === myId;
        const newBalance = isMine ? s.balance + askValue : s.balance;
        let elapsed = gameStartTime ? (Date.now() - gameStartTime) / 1000 : 0;
        // Ensure strictly increasing time for chart library
        const lastTime = s.balanceHistory.length > 0 ? s.balanceHistory[s.balanceHistory.length - 1].time : -1;
        if (elapsed <= lastTime) {
          elapsed = lastTime + 0.001;
        }
        return {
          ...s,
          openOrders: s.openOrders.filter(
            o => !(o.type === 'ask' && o.playerId === playerId && o.value === askValue)
          ),
          balance: newBalance,
          shares: isMine ? s.shares - 1 : s.shares,
          balanceHistory: isMine ? [...s.balanceHistory, { time: elapsed, value: newBalance }] : s.balanceHistory
        };
      });
    },

    cancelOrder: (type, playerId, price) => {
      update(s => ({
        ...s,
        openOrders: s.openOrders.filter(
          o => !(o.type === type && o.playerId === playerId && o.value === price)
        )
      }));
    },

    endGame: (finalBalances) => {
      update(s => ({
        ...s,
        phase: 'ended',
        finalBalances: finalBalances.map(([playerId, balance]) => ({ playerId, balance }))
      }));
    },

    moveCursor: (delta) => {
      update(s => ({
        ...s,
        cursorPrice: Math.max(1, s.cursorPrice + delta)
      }));
    },

    setCursor: (value) => {
      update(s => ({
        ...s,
        cursorPrice: Math.max(1, value)
      }));
    },

    reset: () => {
      gameStartTime = null;
      set({
        phase: null,
        gameId: null,
        countdown: 0,
        currentPrice: 0,
        priceHistory: [],
        balance: 0,
        startingBalance: 0,
        shares: 0,
        openOrders: [],
        cursorPrice: 0,
        players: [],
        finalBalances: [],
        balanceHistory: []
      });
    },

    getGameId: () => get({ subscribe }).gameId
  };
}

export const gameStore = createGameStore();

// Derived store for P/L calculation
export const profitLoss = derived(gameStore, $game => {
  if (!$game.phase) return 0;
  const portfolioValue = $game.balance + ($game.shares * $game.currentPrice);
  return portfolioValue - $game.startingBalance;
});
