<script>
  import { onMount, onDestroy } from 'svelte';
  import { connectionStore } from '../lib/stores/connection.js';
  import { matchmakingStore } from '../lib/stores/matchmaking.js';
  import { connect, send } from '../lib/websocket/client.js';
  import { joinQueue, leaveQueue } from '../lib/websocket/messages.js';

  let elapsedTime = 0;
  let timerInterval = null;

  $: isConnected = $connectionStore.status === 'connected';
  $: isQueued = $matchmakingStore.status === 'queued';

  onMount(() => {
    connect();
  });

  onDestroy(() => {
    if (timerInterval) clearInterval(timerInterval);
  });

  function handleJoinQueue() {
    if (send(joinQueue())) {
      elapsedTime = 0;
      timerInterval = setInterval(() => {
        elapsedTime++;
      }, 1000);
    }
  }

  function handleLeaveQueue() {
    send(leaveQueue());
    if (timerInterval) {
      clearInterval(timerInterval);
      timerInterval = null;
    }
    elapsedTime = 0;
  }

  function formatTime(seconds) {
    const mins = Math.floor(seconds / 60);
    const secs = seconds % 60;
    return `${mins}:${secs.toString().padStart(2, '0')}`;
  }
</script>

<div class="matchmaking">
  <h1>Pocky</h1>
  <p class="subtitle">Real-time Stock Trading Game</p>

  <div class="status-indicator" class:connected={isConnected}>
    <span class="dot"></span>
    {$connectionStore.status}
  </div>

  {#if $connectionStore.error}
    <p class="error">{$connectionStore.error}</p>
  {/if}

  {#if $matchmakingStore.error}
    <p class="error">{$matchmakingStore.error}</p>
  {/if}

  <div class="queue-section">
    {#if isQueued}
      <div class="queue-info">
        <p>Waiting for opponent...</p>
        <p class="timer">{formatTime(elapsedTime)}</p>
        <p class="player-id">Your ID: {$matchmakingStore.playerId?.slice(0, 8)}...</p>
      </div>
      <button class="btn btn-secondary" on:click={handleLeaveQueue}>
        Leave Queue
      </button>
    {:else}
      <button
        class="btn btn-primary"
        on:click={handleJoinQueue}
        disabled={!isConnected}
      >
        Join Queue
      </button>
    {/if}
  </div>
</div>

<style>
  .matchmaking {
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    min-height: 100vh;
    padding: 20px;
    text-align: center;
  }

  h1 {
    font-size: 48px;
    margin-bottom: 8px;
    color: #e94560;
  }

  .subtitle {
    color: #888;
    margin-bottom: 32px;
  }

  .status-indicator {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 8px 16px;
    background: #16213e;
    border-radius: 20px;
    margin-bottom: 24px;
    font-size: 14px;
    text-transform: capitalize;
  }

  .dot {
    width: 8px;
    height: 8px;
    border-radius: 50%;
    background: #888;
  }

  .status-indicator.connected .dot {
    background: #4ade80;
  }

  .error {
    color: #e94560;
    margin-bottom: 16px;
  }

  .queue-section {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 16px;
  }

  .queue-info {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 8px;
  }

  .timer {
    font-size: 32px;
    font-weight: 600;
    font-variant-numeric: tabular-nums;
  }

  .player-id {
    font-size: 12px;
    color: #666;
    font-family: monospace;
  }

  .btn {
    padding: 12px 32px;
    border: none;
    border-radius: 8px;
    font-size: 16px;
    font-weight: 600;
    cursor: pointer;
    transition: all 0.2s;
  }

  .btn:disabled {
    opacity: 0.5;
    cursor: not-allowed;
  }

  .btn-primary {
    background: #e94560;
    color: white;
  }

  .btn-primary:hover:not(:disabled) {
    background: #d63d56;
  }

  .btn-secondary {
    background: #16213e;
    color: #888;
    border: 1px solid #0f3460;
  }

  .btn-secondary:hover:not(:disabled) {
    background: #1a2a4e;
    color: #fff;
  }
</style>
