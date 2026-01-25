export async function fetchQueue() {
  const wsUrl = import.meta.env.VITE_WS_URL || 'ws://localhost:8080/ws';
  const baseUrl = wsUrl.replace('wss://', 'https://').replace('ws://', 'http://').replace('/ws', '');
  const response = await fetch(`${baseUrl}/queue`);
  if (!response.ok) throw new Error(`Failed to fetch queue: ${response.status}`);
  return response.json();
}
