<script>
  import { onDestroy } from 'svelte';
  import { gameStore, profitLoss, computePlayerStats } from '../lib/stores/game.js';
  import { matchmakingStore } from '../lib/stores/matchmaking.js';
  import { send } from '../lib/websocket/client.js';
  import { placeBid, placeAsk } from '../lib/websocket/messages.js';
  import InfoBox from '../lib/components/InfoBox.svelte';
  import PlayerCard from '../lib/components/PlayerCard.svelte';

  $: myId = $matchmakingStore.playerId;
  $: isEnded = $gameStore.phase === 'ended';
  $: isCountdown = $gameStore.phase === 'countdown';
  $: isRunning = $gameStore.phase === 'running';
  $: myResult = $gameStore.finalBalances.find(b => b.playerId === myId);
  $: otherResults = $gameStore.finalBalances.filter(b => b.playerId !== myId);

  // Timer state
  let timeRemaining = 0;
  let timerInterval = null;

  function updateTimer() {
    if ($gameStore.gameStartTime && $gameStore.gameDuration) {
      const elapsed = (Date.now() - $gameStore.gameStartTime) / 1000;
      timeRemaining = Math.max(0, Math.ceil($gameStore.gameDuration - elapsed));
    }
  }

  // Start/stop timer based on game phase
  $: if (isRunning && !timerInterval) {
    updateTimer();
    timerInterval = setInterval(updateTimer, 100);
  } else if (!isRunning && timerInterval) {
    clearInterval(timerInterval);
    timerInterval = null;
  }

  onDestroy(() => {
    if (timerInterval) clearInterval(timerInterval);
  });

  function formatTime(seconds) {
    const mins = Math.floor(seconds / 60);
    const secs = seconds % 60;
    return `${mins}:${secs.toString().padStart(2, '0')}`;
  }

  $: myPlayerData = $gameStore.players[myId];
  $: myStats = myPlayerData ? computePlayerStats(myPlayerData, $gameStore.startingBalance) : null;

  function handleKeydown(event) {
    if (isEnded || isCountdown) return;

    switch (event.key) {
      case 'b':
      case 'B':
        handleBid();
        break;
      case 's':
      case 'S':
        handleAsk();
        break;
    }
  }

  function handleBid() {
    const gameId = gameStore.getGameId();
    const price = myPlayerData?.currentPrice;
    if (gameId && price > 0) {
      send(placeBid(gameId, price));
    }
  }

  function handleAsk() {
    const gameId = gameStore.getGameId();
    const price = myPlayerData?.currentPrice;
    if (gameId && price > 0) {
      send(placeAsk(gameId, price));
    }
  }

  function handleReturn() {
    gameStore.reset();
    matchmakingStore.reset();
  }

  function formatPL(value) {
    const sign = value >= 0 ? '+' : '';
    return `${sign}${value}`;
  }
</script>

<svelte:window on:keydown={handleKeydown} />

<div class="game">
  {#if isCountdown}
    <div class="countdown-overlay">
      <div class="countdown-number">{$gameStore.countdown}</div>
      <p>Game starting...</p>
    </div>
  {/if}

  {#if isEnded}
    <div class="results-overlay">
      <h2>Game Over</h2>
      <div class="results">
        {#if myResult}
          <div class="result-item mine">
            <span>You</span>
            <span class="balance">${myResult.balance}</span>
          </div>
        {/if}
        {#each otherResults as result}
          <div class="result-item">
            <span>{result.playerId.slice(0, 8)}...</span>
            <span class="balance">${result.balance}</span>
          </div>
        {/each}
      </div>
      <button class="btn btn-primary" on:click={handleReturn}>
        Return to Lobby
      </button>
    </div>
  {/if}

  <div class="players-grid">
    {#each Object.entries($gameStore.players) as [playerId, playerData]}
      <PlayerCard
        {playerId}
        {playerData}
        isCurrentPlayer={playerId === myId}
        startingBalance={$gameStore.startingBalance}
      />
    {/each}
  </div>

  <div class="info-row">
    <InfoBox label="Time" value={formatTime(timeRemaining)} />
    <InfoBox label="Price" value={myPlayerData?.currentPrice ?? 0} />
    <InfoBox label="Balance" value={`$${myStats?.balance ?? 0}`} />
    <InfoBox label="Shares" value={myStats?.shares ?? 0} />
    <InfoBox label="P/L" value={formatPL($profitLoss)} />
  </div>

  <div class="controls">
    <div class="trade-controls">
      <button
        class="btn btn-bid"
        on:click={handleBid}
        disabled={isEnded || isCountdown}
      >
        BID (B)
      </button>
      <button
        class="btn btn-ask"
        on:click={handleAsk}
        disabled={isEnded || isCountdown}
      >
        ASK (S)
      </button>
    </div>
  </div>
</div>

<style>
  .game {
    padding: 20px;
    max-width: 1400px;
    margin: 0 auto;
    position: relative;
    background: #0a0a0a;
    min-height: 100vh;
  }

  .countdown-overlay,
  .results-overlay {
    position: fixed;
    top: 0;
    left: 0;
    right: 0;
    bottom: 0;
    background: rgba(10, 10, 10, 0.95);
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    z-index: 100;
  }

  .countdown-number {
    font-size: 120px;
    font-weight: 700;
    color: #ff9500;
  }

  .results-overlay h2 {
    font-size: 36px;
    margin-bottom: 24px;
    color: #ff9500;
  }

  .results {
    display: flex;
    flex-direction: column;
    gap: 12px;
    margin-bottom: 32px;
    width: 300px;
  }

  .result-item {
    display: flex;
    justify-content: space-between;
    padding: 12px 16px;
    background: rgba(255,255,255,0.02);
    border: 1px solid rgba(255,255,255,0.06);
    border-radius: 8px;
  }

  .result-item.mine {
    border-color: #ff9500;
  }

  .balance {
    font-weight: 600;
    font-variant-numeric: tabular-nums;
    color: #ff9500;
  }

  .players-grid {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(350px, 1fr));
    gap: 16px;
    margin-bottom: 20px;
  }

  .info-row {
    display: flex;
    gap: 12px;
    margin-bottom: 16px;
    flex-wrap: wrap;
  }

  .controls {
    display: flex;
    gap: 24px;
    align-items: center;
  }

  .trade-controls {
    display: flex;
    gap: 12px;
  }

  .btn {
    padding: 10px 20px;
    border: none;
    border-radius: 8px;
    font-size: 14px;
    font-weight: 600;
    cursor: pointer;
    transition: all 0.2s;
    font-family: 'SF Mono', 'Monaco', 'Inconsolata', 'Fira Mono', monospace;
  }

  .btn:disabled {
    opacity: 0.5;
    cursor: not-allowed;
  }

  .btn-primary {
    background: #ff9500;
    color: #0a0a0a;
  }

  .btn-primary:hover:not(:disabled) {
    background: #e68600;
  }

  .btn-bid {
    background: #00ff88;
    color: #0a0a0a;
  }

  .btn-bid:hover:not(:disabled) {
    background: #00dd77;
  }

  .btn-ask {
    background: #ff4466;
    color: #0a0a0a;
  }

  .btn-ask:hover:not(:disabled) {
    background: #e63950;
  }
</style>
