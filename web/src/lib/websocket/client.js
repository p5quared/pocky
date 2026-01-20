import { connectionStore } from '../stores/connection.js';
import { handleMessage } from './handlers.js';

let ws = null;
let reconnectTimeout = null;
const RECONNECT_DELAY = 2000;

function getWsUrl() {
  return import.meta.env.VITE_WS_URL || 'ws://localhost:8080/ws';
}

export function connect() {
  if (ws && ws.readyState === WebSocket.OPEN) {
    return;
  }

  connectionStore.setConnecting();

  const url = getWsUrl();
  ws = new WebSocket(url);

  ws.onopen = () => {
    connectionStore.setConnected();
    if (reconnectTimeout) {
      clearTimeout(reconnectTimeout);
      reconnectTimeout = null;
    }
  };

  ws.onclose = () => {
    connectionStore.setDisconnected();
    ws = null;
    scheduleReconnect();
  };

  ws.onerror = (event) => {
    connectionStore.setError('WebSocket error');
    console.error('WebSocket error:', event);
  };

  ws.onmessage = (event) => {
    handleMessage(event.data);
  };
}

function scheduleReconnect() {
  if (reconnectTimeout) return;
  reconnectTimeout = setTimeout(() => {
    reconnectTimeout = null;
    connect();
  }, RECONNECT_DELAY);
}

export function disconnect() {
  if (reconnectTimeout) {
    clearTimeout(reconnectTimeout);
    reconnectTimeout = null;
  }
  if (ws) {
    ws.close();
    ws = null;
  }
}

export function send(message) {
  if (ws && ws.readyState === WebSocket.OPEN) {
    ws.send(JSON.stringify(message));
    return true;
  }
  return false;
}
