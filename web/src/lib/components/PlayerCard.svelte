<script>
  import PriceChart from './PriceChart.svelte';
  import { computePlayerStats } from '../stores/game.js';

  export let playerId;
  export let playerData;
  export let isCurrentPlayer;
  export let startingBalance;

  $: stats = computePlayerStats(playerData, startingBalance);
  $: displayName = isCurrentPlayer ? 'You' : playerId.slice(0, 8) + '...';
</script>

<div class="player-card" class:current={isCurrentPlayer}>
  <div class="player-header">
    <span class="player-name">{displayName}</span>
    <span class="current-price">${playerData.currentPrice}</span>
  </div>

  <PriceChart priceHistory={playerData.priceHistory} compact={true} />

  <div class="player-stats">
    <div class="stat">
      <span class="label">Balance</span>
      <span class="value">${stats.balance}</span>
    </div>
    <div class="stat">
      <span class="label">Shares</span>
      <span class="value">{stats.shares}</span>
    </div>
    <div class="stat">
      <span class="label">Cost Basis</span>
      <span class="value">{stats.costBasis !== null ? '$' + stats.costBasis : '-'}</span>
    </div>
  </div>
</div>

<style>
  .player-card {
    background: rgba(255,255,255,0.02);
    border: 1px solid rgba(255,255,255,0.06);
    border-radius: 12px;
    padding: 16px;
    display: flex;
    flex-direction: column;
    gap: 12px;
  }

  .player-card.current {
    border-color: #ff9500;
  }

  .player-header {
    display: flex;
    justify-content: space-between;
    align-items: center;
  }

  .player-name {
    font-weight: 600;
    color: #e0e0e0;
  }

  .current .player-name {
    color: #ff9500;
  }

  .current-price {
    font-size: 18px;
    font-weight: 700;
    font-variant-numeric: tabular-nums;
    font-family: 'SF Mono', Monaco, monospace;
    color: #fff;
  }

  .player-stats {
    display: flex;
    gap: 16px;
    justify-content: space-between;
  }

  .stat {
    display: flex;
    flex-direction: column;
    gap: 4px;
  }

  .stat .label {
    font-size: 10px;
    text-transform: uppercase;
    color: #666;
    letter-spacing: 1px;
  }

  .stat .value {
    font-size: 14px;
    font-weight: 600;
    font-variant-numeric: tabular-nums;
    font-family: 'SF Mono', Monaco, monospace;
    color: #e0e0e0;
  }
</style>
