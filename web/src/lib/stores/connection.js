import { writable } from 'svelte/store';

function createConnectionStore() {
  const { subscribe, set, update } = writable({
    status: 'disconnected', // disconnected | connecting | connected
    error: null
  });

  return {
    subscribe,
    setConnecting: () => set({ status: 'connecting', error: null }),
    setConnected: () => set({ status: 'connected', error: null }),
    setDisconnected: () => update(s => ({ ...s, status: 'disconnected' })),
    setError: (error) => update(s => ({ ...s, error })),
    reset: () => set({ status: 'disconnected', error: null })
  };
}

export const connectionStore = createConnectionStore();
