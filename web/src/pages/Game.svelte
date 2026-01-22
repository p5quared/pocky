<script>
  import { gameStore, profitLoss } from '../lib/stores/game.js';
  import { matchmakingStore } from '../lib/stores/matchmaking.js';
  import { send } from '../lib/websocket/client.js';
  import { placeBid, placeAsk, cancelBid, cancelAsk } from '../lib/websocket/messages.js';
  import InfoBox from '../lib/components/InfoBox.svelte';
  import PriceChart from '../lib/components/PriceChart.svelte';

  let hoverPrice = null;

  $: myId = $matchmakingStore.playerId;
  $: isEnded = $gameStore.phase === 'ended';
  $: isCountdown = $gameStore.phase === 'countdown';
  $: myResult = $gameStore.finalBalances.find(b => b.playerId === myId);
  $: otherResults = $gameStore.finalBalances.filter(b => b.playerId !== myId);

  // Aggregate orders by price level for orderbook display
  function aggregateOrders(orders, myId) {
    const grouped = {};
    orders.forEach(o => {
      if (!grouped[o.value]) {
        grouped[o.value] = { count: 0, hasMine: false };
      }
      grouped[o.value].count += 1;
      if (o.playerId === myId) {
        grouped[o.value].hasMine = true;
      }
    });
    return Object.entries(grouped)
      .map(([price, data]) => ({ price: Number(price), quantity: data.count, hasMine: data.hasMine }))
      .sort((a, b) => b.price - a.price);
  }

  $: orderBook = {
    bids: aggregateOrders($gameStore.openOrders.filter(o => o.type === 'bid'), myId),
    asks: aggregateOrders($gameStore.openOrders.filter(o => o.type === 'ask'), myId)
  };

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

  function handleCancelBid(price) {
    const gameId = gameStore.getGameId();
    if (gameId) {
      send(cancelBid(gameId, price));
    }
  }

  function handleCancelAsk(price) {
    const gameId = gameStore.getGameId();
    if (gameId) {
      send(cancelAsk(gameId, price));
    }
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

  <div class="main-content">
    <div class="chart-panel">
      <div class="panel-header">PRICE HISTORY</div>
      <PriceChart
        priceHistory={$gameStore.priceHistory}
        bind:hoverPrice
      />
    </div>

    <div class="orderbook-panel">
      <div class="orderbook-header">ORDER BOOK</div>
      <div class="orderbook-columns">
        <span>BID</span>
        <span>PRICE</span>
        <span>ASK</span>
      </div>
      <div class="orderbook-body">
        {#each orderBook.asks as ask}
          <div class="orderbook-row ask-row">
            <span class="bid-qty"></span>
            <span class="price ask-price">{ask.price}</span>
            <span class="ask-qty">
              {ask.quantity}
              {#if ask.hasMine}
                <button class="cancel-btn" on:click={() => handleCancelAsk(ask.price)}>×</button>
              {/if}
            </span>
          </div>
        {/each}
        {#each orderBook.bids as bid}
          <div class="orderbook-row bid-row">
            <span class="bid-qty">
              {#if bid.hasMine}
                <button class="cancel-btn" on:click={() => handleCancelBid(bid.price)}>×</button>
              {/if}
              {bid.quantity}
            </span>
            <span class="price bid-price">{bid.price}</span>
            <span class="ask-qty"></span>
          </div>
        {/each}
        {#if orderBook.asks.length === 0 && orderBook.bids.length === 0}
          <div class="orderbook-empty">No orders</div>
        {/if}
      </div>
    </div>
  </div>

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

  .main-content {
    display: flex;
    gap: 16px;
    margin-bottom: 20px;
  }

  .chart-panel {
    flex: 1;
    min-width: 0;
  }

  .panel-header {
    font-size: 12px;
    font-weight: 600;
    color: #666;
    text-transform: uppercase;
    letter-spacing: 2px;
    margin-bottom: 8px;
  }

  .orderbook-panel {
    width: 200px;
    background: rgba(255,255,255,0.02);
    border: 1px solid rgba(255,255,255,0.06);
    border-radius: 12px;
    display: flex;
    flex-direction: column;
  }

  .orderbook-header {
    padding: 12px;
    font-size: 12px;
    font-weight: 600;
    color: #666;
    text-align: center;
    border-bottom: 1px solid rgba(255,255,255,0.06);
    letter-spacing: 2px;
    text-transform: uppercase;
  }

  .orderbook-columns {
    display: grid;
    grid-template-columns: 1fr 1fr 1fr;
    padding: 8px 12px;
    font-size: 10px;
    color: #888888;
    text-transform: uppercase;
    border-bottom: 1px solid rgba(255,255,255,0.06);
    text-align: center;
  }

  .orderbook-body {
    flex: 1;
    overflow-y: auto;
    font-family: 'SF Mono', 'Monaco', 'Inconsolata', 'Fira Mono', monospace;
  }

  .orderbook-row {
    display: grid;
    grid-template-columns: 1fr 1fr 1fr;
    padding: 4px 12px;
    font-size: 13px;
    text-align: center;
  }

  .orderbook-row:hover {
    background: rgba(255,255,255,0.03);
  }

  .bid-qty {
    color: #00ff88;
  }

  .ask-qty {
    color: #ff4466;
  }

  .bid-price {
    color: #00ff88;
  }

  .ask-price {
    color: #ff4466;
  }

  .orderbook-empty {
    padding: 20px;
    text-align: center;
    color: #888888;
    font-size: 12px;
  }

  .cancel-btn {
    background: none;
    border: none;
    color: #888;
    cursor: pointer;
    font-size: 14px;
    padding: 0 4px;
    opacity: 0;
    transition: opacity 0.15s, color 0.15s;
  }

  .orderbook-row:hover .cancel-btn {
    opacity: 1;
  }

  .cancel-btn:hover {
    color: #ff4466;
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
