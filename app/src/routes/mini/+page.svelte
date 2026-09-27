<script lang="ts">
  // The mini player window (PLAN.md F7): the playing bar alone. It follows
  // the queue and the player through their events, as the main window does,
  // and its capability allows only the commands the bar uses.
  import NowPlayingBar from "$lib/components/NowPlayingBar.svelte";
  import Toasts from "$lib/components/Toasts.svelte";
  import { player } from "$lib/state/player.svelte";
  import { appSettings } from "$lib/state/settings.svelte";

  $effect(() => {
    const stopPlayer = player.connect();
    const stopSettings = appSettings.connect();
    return () => {
      stopPlayer();
      stopSettings();
    };
  });

  function onkeydown(event: KeyboardEvent) {
    if (event.key === " " && !(event.target instanceof HTMLInputElement)) {
      event.preventDefault();
      player.toggle();
    }
  }
</script>

<svelte:window {onkeydown} />
<svelte:head><title>ano-mp</title></svelte:head>

<main class="mini">
  <NowPlayingBar mini />
</main>
<Toasts />

<style>
  .mini {
    height: 100vh;
    height: 100dvh;
    background: var(--surface);
    display: flex;
    align-items: stretch;
  }

  .mini > :global(*) {
    flex: 1;
  }
</style>
