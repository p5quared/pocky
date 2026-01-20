<script>
  import { gameStore } from './lib/stores/game.js';
  import Matchmaking from './pages/Matchmaking.svelte';
  import Game from './pages/Game.svelte';

  $: inGame = $gameStore.phase !== null;
</script>

{#if inGame}
  <Game />
{:else}
  <Matchmaking />
{/if}
