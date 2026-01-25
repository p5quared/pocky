import { writable, get } from 'svelte/store';

function createMatchmakingStore() {
  const { subscribe, set, update } = writable({
    status: 'idle', // idle | queued | matched | error
    playerId: null,
    queuedAt: null,
    matchedPlayers: [],
    queuedPlayers: [],
    queueCount: 0,
    error: null
  });

  return {
    subscribe,

    setEnqueued: (playerId) => {
      set({
        status: 'queued',
        playerId,
        queuedAt: Date.now(),
        matchedPlayers: [],
        error: null
      });
    },

    setMatched: (players) => {
      update(s => ({
        ...s,
        status: 'matched',
        matchedPlayers: players
      }));
    },

    setDequeued: () => {
      set({
        status: 'idle',
        playerId: null,
        queuedAt: null,
        matchedPlayers: [],
        error: null
      });
    },

    setAlreadyQueued: () => {
      update(s => ({ ...s, error: 'Already in queue' }));
    },

    setPlayerNotFound: () => {
      update(s => ({ ...s, error: 'Player not found' }));
    },

    setQueuedPlayers: (players, count) => {
      update(s => ({ ...s, queuedPlayers: players, queueCount: count }));
    },

    reset: () => {
      set({
        status: 'idle',
        playerId: null,
        queuedAt: null,
        matchedPlayers: [],
        queuedPlayers: [],
        queueCount: 0,
        error: null
      });
    },

    getPlayerId: () => get({ subscribe }).playerId
  };
}

export const matchmakingStore = createMatchmakingStore();
