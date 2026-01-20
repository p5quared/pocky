<script>
  export let label;
  export let value;
  export let highlight = false;
</script>

<div class="info-box" class:highlight>
  <span class="label">{label}</span>
  <span class="value">{value}</span>
</div>

<style>
  .info-box {
    background: #16213e;
    border: 1px solid #0f3460;
    border-radius: 8px;
    padding: 12px 16px;
    display: flex;
    flex-direction: column;
    gap: 4px;
    min-width: 100px;
  }

  .info-box.highlight {
    border-color: #e94560;
    background: #1a1a3e;
  }

  .label {
    font-size: 12px;
    color: #888;
    text-transform: uppercase;
    letter-spacing: 0.5px;
  }

  .value {
    font-size: 24px;
    font-weight: 600;
    color: #fff;
    font-variant-numeric: tabular-nums;
  }
</style>
