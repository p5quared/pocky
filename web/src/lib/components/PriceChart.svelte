<script>
  import { onMount, onDestroy } from 'svelte';

  export let priceHistory = [];
  export let hoverPrice = null;

  let containerEl;
  let width = 800;
  let height = 400;
  let mouseX = null;
  let mouseY = null;
  let resizeObserver;

  const padding = { top: 20, right: 60, bottom: 40, left: 20 };
  const WINDOW_SIZE = 120; // 60 seconds at 500ms tick interval

  // Sliding window of visible data
  $: visibleHistory = priceHistory.length > WINDOW_SIZE
    ? priceHistory.slice(-WINDOW_SIZE)
    : priceHistory;

  $: prices = visibleHistory.map(p => p.value);
  $: minPrice = prices.length > 0 ? Math.min(...prices) : 0;
  $: maxPrice = prices.length > 0 ? Math.max(...prices) : 100;
  $: priceRange = maxPrice - minPrice || 1;
  $: paddedMin = minPrice - priceRange * 0.1;
  $: paddedMax = maxPrice + priceRange * 0.1;

  $: priceChange = visibleHistory.length >= 2
    ? visibleHistory[visibleHistory.length - 1].value - visibleHistory[0].value
    : 0;
  $: lineColor = priceChange >= 0 ? '#00ff88' : '#ff4466';

  $: chartWidth = width - padding.left - padding.right;
  $: chartHeight = height - padding.top - padding.bottom;

  function scaleX(index) {
    // Fixed scale based on window size, not data length
    return padding.left + (index / (WINDOW_SIZE - 1)) * chartWidth;
  }

  function scaleY(value) {
    const range = paddedMax - paddedMin || 1;
    return padding.top + (1 - (value - paddedMin) / range) * chartHeight;
  }

  function priceFromY(y) {
    const range = paddedMax - paddedMin || 1;
    return paddedMin + (1 - (y - padding.top) / chartHeight) * range;
  }

  $: linePath = visibleHistory.map((p, i) =>
    `${i === 0 ? 'M' : 'L'} ${scaleX(i)} ${scaleY(p.value)}`
  ).join(' ');

  $: areaPath = visibleHistory.length > 0
    ? linePath + ` L ${scaleX(visibleHistory.length - 1)} ${padding.top + chartHeight} L ${scaleX(0)} ${padding.top + chartHeight} Z`
    : '';

  $: lastPoint = visibleHistory.length > 0
    ? { x: scaleX(visibleHistory.length - 1), y: scaleY(visibleHistory[visibleHistory.length - 1].value) }
    : null;

  $: gridLines = (() => {
    const lines = [];
    const numLines = 5;
    for (let i = 0; i <= numLines; i++) {
      const y = padding.top + (i / numLines) * chartHeight;
      const price = priceFromY(y);
      lines.push({ y, price: Math.round(price) });
    }
    return lines;
  })();

  function handleMouseMove(event) {
    if (!containerEl) return;
    const rect = containerEl.getBoundingClientRect();
    mouseX = event.clientX - rect.left;
    mouseY = event.clientY - rect.top;

    if (mouseX >= padding.left && mouseX <= width - padding.right &&
        mouseY >= padding.top && mouseY <= height - padding.bottom) {
      const rawPrice = priceFromY(mouseY);
      hoverPrice = Math.round(rawPrice / 10) * 10;
    } else {
      hoverPrice = null;
    }
  }

  function handleMouseLeave() {
    mouseX = null;
    mouseY = null;
    hoverPrice = null;
  }

  onMount(() => {
    if (containerEl) {
      width = containerEl.clientWidth;
      resizeObserver = new ResizeObserver((entries) => {
        for (const entry of entries) {
          width = entry.contentRect.width;
        }
      });
      resizeObserver.observe(containerEl);
    }
  });

  onDestroy(() => {
    if (resizeObserver) {
      resizeObserver.disconnect();
    }
  });
</script>

<div
  class="chart-wrapper"
  bind:this={containerEl}
  on:mousemove={handleMouseMove}
  on:mouseleave={handleMouseLeave}
  role="img"
  aria-label="Price chart"
>
  <svg viewBox="0 0 {width} {height}" preserveAspectRatio="none">
    <defs>
      <linearGradient id="chartGradient" x1="0%" y1="0%" x2="0%" y2="100%">
        <stop offset="0%" stop-color={lineColor} stop-opacity="0.3" />
        <stop offset="100%" stop-color={lineColor} stop-opacity="0" />
      </linearGradient>
      <filter id="glow" x="-50%" y="-50%" width="200%" height="200%">
        <feGaussianBlur stdDeviation="3" result="blur"/>
        <feMerge>
          <feMergeNode in="blur"/>
          <feMergeNode in="SourceGraphic"/>
        </feMerge>
      </filter>
    </defs>

    <!-- Grid lines -->
    {#each gridLines as line}
      <line
        x1={padding.left}
        y1={line.y}
        x2={width - padding.right}
        y2={line.y}
        stroke="rgba(255,255,255,0.05)"
        stroke-dasharray="4,4"
      />
      <text
        x={width - padding.right + 8}
        y={line.y + 4}
        fill="#666"
        font-size="11"
        font-family="SF Mono, Monaco, monospace"
      >
        {line.price}
      </text>
    {/each}

    {#if priceHistory.length > 0}
      <!-- Area fill -->
      <path d={areaPath} fill="url(#chartGradient)" />

      <!-- Price line with glow -->
      <path
        d={linePath}
        fill="none"
        stroke={lineColor}
        stroke-width="2"
        filter="url(#glow)"
      />

      <!-- Current price dot -->
      {#if lastPoint}
        <circle
          cx={lastPoint.x}
          cy={lastPoint.y}
          r="5"
          fill={lineColor}
          filter="url(#glow)"
        />
      {/if}
    {/if}

    <!-- Hover indicator -->
    {#if hoverPrice !== null && mouseX !== null && mouseY !== null}
      <!-- Horizontal line -->
      <line
        x1={padding.left}
        y1={scaleY(hoverPrice)}
        x2={width - padding.right}
        y2={scaleY(hoverPrice)}
        stroke="#666"
        stroke-dasharray="2,2"
      />
      <!-- Price label -->
      <rect
        x={width - padding.right + 2}
        y={scaleY(hoverPrice) - 10}
        width="50"
        height="20"
        fill="#ff9500"
        rx="3"
      />
      <text
        x={width - padding.right + 27}
        y={scaleY(hoverPrice) + 4}
        fill="#0a0a0a"
        font-size="12"
        font-weight="600"
        font-family="SF Mono, Monaco, monospace"
        text-anchor="middle"
      >
        ${hoverPrice}
      </text>
    {/if}
  </svg>

  <div class="time-labels">
    <span>60s ago</span>
    <span>45s</span>
    <span>30s</span>
    <span>15s</span>
    <span>Now</span>
  </div>
</div>

<style>
  .chart-wrapper {
    width: 100%;
    height: 400px;
    position: relative;
    background: rgba(255,255,255,0.02);
    border: 1px solid rgba(255,255,255,0.06);
    border-radius: 12px;
    overflow: hidden;
  }

  svg {
    width: 100%;
    height: calc(100% - 30px);
    display: block;
  }

  .time-labels {
    position: absolute;
    bottom: 0;
    left: 20px;
    right: 60px;
    height: 30px;
    display: flex;
    justify-content: space-between;
    align-items: center;
    font-size: 11px;
    color: #666;
    font-family: 'SF Mono', Monaco, monospace;
    padding: 0 4px;
  }
</style>
