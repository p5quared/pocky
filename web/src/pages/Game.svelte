<script>
  import { onMount, onDestroy } from 'svelte';
  import { createChart } from 'lightweight-charts';
  import { gameStore, profitLoss } from '../lib/stores/game.js';
  import { matchmakingStore } from '../lib/stores/matchmaking.js';
  import { send } from '../lib/websocket/client.js';
  import { placeBid, placeAsk } from '../lib/websocket/messages.js';
  import InfoBox from '../lib/components/InfoBox.svelte';

  let chartContainer;
  let chart;
  let lineSeries;
  let balanceSeries;
  let hoverPrice = null;

  $: myId = $matchmakingStore.playerId;
  $: isEnded = $gameStore.phase === 'ended';
  $: isCountdown = $gameStore.phase === 'countdown';
  $: myResult = $gameStore.finalBalances.find(b => b.playerId === myId);
  $: otherResults = $gameStore.finalBalances.filter(b => b.playerId !== myId);

  onMount(() => {
    chart = createChart(chartContainer, {
      width: chartContainer.clientWidth,
      height: 400,
      layout: {
        background: { color: '#1a1a2e' },
        textColor: '#888'
      },
      grid: {
        vertLines: { color: '#16213e' },
        horzLines: { color: '#16213e' }
      },
      timeScale: {
        timeVisible: false,
        secondsVisible: false
      },
      rightPriceScale: {
        borderColor: '#0f3460'
      },
      leftPriceScale: {
        borderColor: '#0f3460',
        visible: true
      }
    });

    lineSeries = chart.addLineSeries({
      color: '#e94560',
      lineWidth: 2
    });

    balanceSeries = chart.addLineSeries({
      color: '#4ade80',
      lineWidth: 2,
      priceScaleId: 'left'
    });

    chart.subscribeCrosshairMove((param) => {
      if (!param.point || param.point.x < 0 || param.point.y < 0) {
        hoverPrice = null;
        return;
      }
      const price = lineSeries.coordinateToPrice(param.point.y);
      if (price !== null) {
        hoverPrice = Math.round(price);
      }
    });

    const resizeObserver = new ResizeObserver(() => {
      if (chart && chartContainer) {
        chart.applyOptions({ width: chartContainer.clientWidth });
      }
    });
    resizeObserver.observe(chartContainer);

    return () => {
      resizeObserver.disconnect();
      if (chart) chart.remove();
    };
  });

  // Update chart when price history changes
  $: if (lineSeries && $gameStore.priceHistory.length > 0) {
    const data = $gameStore.priceHistory.map(p => ({
      time: p.time,
      value: p.value
    }));
    lineSeries.setData(data);
  }

  // Update chart when balance history changes
  $: if (balanceSeries && $gameStore.balanceHistory.length > 0) {
    balanceSeries.setData($gameStore.balanceHistory);
  }

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
    if (gameId && hoverPrice !== null && hoverPrice > 0) {
      send(placeBid(gameId, hoverPrice));
    }
  }

  function handleAsk() {
    const gameId = gameStore.getGameId();
    if (gameId && hoverPrice !== null && hoverPrice > 0) {
      send(placeAsk(gameId, hoverPrice));
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

  <div class="chart-container" bind:this={chartContainer}></div>

  <div class="info-row">
    <InfoBox label="Price" value={$gameStore.currentPrice} />
    <InfoBox label="Cursor" value={hoverPrice ?? '-'} highlight />
    <InfoBox label="Balance" value={`$${$gameStore.balance}`} />
    <InfoBox label="Shares" value={$gameStore.shares} />
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

  <div class="orders">
    <h3>Open Orders</h3>
    {#if $gameStore.openOrders.length === 0}
      <p class="no-orders">No open orders</p>
    {:else}
      <ul>
        {#each $gameStore.openOrders as order}
          <li class:mine={order.playerId === myId}>
            <span class="order-type" class:bid={order.type === 'bid'} class:ask={order.type === 'ask'}>
              {order.type.toUpperCase()}
            </span>
            <span class="order-value">${order.value}</span>
            {#if order.playerId === myId}
              <span class="order-owner">(You)</span>
            {/if}
          </li>
        {/each}
      </ul>
    {/if}
  </div>
</div>

<style>
  .game {
    padding: 20px;
    max-width: 1200px;
    margin: 0 auto;
    position: relative;
  }

  .countdown-overlay,
  .results-overlay {
    position: fixed;
    top: 0;
    left: 0;
    right: 0;
    bottom: 0;
    background: rgba(26, 26, 46, 0.95);
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    z-index: 100;
  }

  .countdown-number {
    font-size: 120px;
    font-weight: 700;
    color: #e94560;
  }

  .results-overlay h2 {
    font-size: 36px;
    margin-bottom: 24px;
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
    background: #16213e;
    border-radius: 8px;
  }

  .result-item.mine {
    border: 2px solid #e94560;
  }

  .balance {
    font-weight: 600;
    font-variant-numeric: tabular-nums;
  }

  .chart-container {
    width: 100%;
    height: 400px;
    margin-bottom: 20px;
    border-radius: 8px;
    overflow: hidden;
  }

  .info-row {
    display: flex;
    gap: 16px;
    margin-bottom: 20px;
    flex-wrap: wrap;
  }

  .controls {
    display: flex;
    gap: 24px;
    align-items: center;
    margin-bottom: 20px;
  }

  .trade-controls {
    display: flex;
    gap: 12px;
  }

  .btn {
    padding: 12px 24px;
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

  .btn-bid {
    background: #4ade80;
    color: #1a1a2e;
  }

  .btn-bid:hover:not(:disabled) {
    background: #22c55e;
  }

  .btn-ask {
    background: #f87171;
    color: #1a1a2e;
  }

  .btn-ask:hover:not(:disabled) {
    background: #ef4444;
  }

  .orders {
    background: #16213e;
    border-radius: 8px;
    padding: 16px;
  }

  .orders h3 {
    margin-bottom: 12px;
    font-size: 14px;
    color: #888;
    text-transform: uppercase;
  }

  .no-orders {
    color: #666;
    font-style: italic;
  }

  .orders ul {
    list-style: none;
    display: flex;
    flex-direction: column;
    gap: 8px;
  }

  .orders li {
    display: flex;
    align-items: center;
    gap: 12px;
    padding: 8px 12px;
    background: #1a1a2e;
    border-radius: 4px;
  }

  .orders li.mine {
    border-left: 3px solid #e94560;
  }

  .order-type {
    font-size: 12px;
    font-weight: 600;
    padding: 2px 8px;
    border-radius: 4px;
  }

  .order-type.bid {
    background: #4ade80;
    color: #1a1a2e;
  }

  .order-type.ask {
    background: #f87171;
    color: #1a1a2e;
  }

  .order-value {
    font-variant-numeric: tabular-nums;
  }

  .order-owner {
    font-size: 12px;
    color: #888;
  }
</style>
