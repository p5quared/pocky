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
    background: rgba(255,255,255,0.02);
    border: 1px solid rgba(255,255,255,0.06);
    border-radius: 8px;
    padding: 8px 12px;
    display: flex;
    flex-direction: column;
    gap: 2px;
    min-width: 80px;
  }

  .info-box.highlight {
    border-color: #ff9500;
  }

  .label {
    font-size: 10px;
    color: #888888;
    text-transform: uppercase;
    letter-spacing: 0.5px;
  }

  .value {
    font-size: 18px;
    font-weight: 600;
    color: #ff9500;
    font-variant-numeric: tabular-nums;
    font-family: 'SF Mono', 'Monaco', 'Inconsolata', 'Fira Mono', monospace;
  }
</style>
