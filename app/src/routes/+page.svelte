<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";
  import { listen } from "@tauri-apps/api/event";

  let coreVersion = $state<string | null>(null);
  let deviceName = $state<string | null>(null);
  let playing = $state(false);
  let frequency = $state(440);
  let deviceEvents = $state<string[]>([]);
  let error = $state<string | null>(null);

  async function refreshDevice() {
    deviceName = await invoke<string | null>("audio_device_name");
  }

  async function run(action: () => Promise<unknown>) {
    try {
      error = null;
      await action();
    } catch (e) {
      error = String(e);
    }
  }

  const play = () =>
    run(async () => {
      await invoke("play_test_tone", { frequency });
      playing = true;
    });

  const stop = () =>
    run(async () => {
      await invoke("stop_test_tone");
      playing = false;
    });

  $effect(() => {
    run(async () => {
      coreVersion = await invoke<string>("core_version");
      await refreshDevice();
    });

    const unlisten = listen("audio-device-changed", () => {
      deviceEvents = [`${new Date().toLocaleTimeString()} device changed`, ...deviceEvents];
      run(refreshDevice);
    });

    return () => {
      unlisten.then((stopListening) => stopListening());
    };
  });
</script>

<main>
  <h1>ano-mp</h1>
  <p>Audio core v{coreVersion ?? "…"}</p>
  <p>Output device: {deviceName ?? "none"}</p>

  <div class="row">
    <label>
      Frequency (Hz)
      <input type="number" min="20" max="20000" bind:value={frequency} />
    </label>
    {#if playing}
      <button onclick={stop}>Stop tone</button>
    {:else}
      <button onclick={play}>Play test tone</button>
    {/if}
  </div>

  {#if error}
    <p class="error">{error}</p>
  {/if}

  {#if deviceEvents.length > 0}
    <h2>Device events</h2>
    <ul>
      {#each deviceEvents as event}
        <li>{event}</li>
      {/each}
    </ul>
  {/if}
</main>

<style>
  :root {
    font-family: system-ui, sans-serif;
    color: #0f0f0f;
    background-color: #f6f6f6;
  }

  @media (prefers-color-scheme: dark) {
    :root {
      color: #f6f6f6;
      background-color: #1e1e1e;
    }
  }

  main {
    padding: 2rem;
  }

  .row {
    display: flex;
    flex-wrap: wrap;
    gap: 1rem;
    align-items: end;
  }

  input {
    width: 6rem;
    margin-left: 0.5rem;
  }

  h2 {
    font-size: 1rem;
    margin-top: 2rem;
  }

  .error {
    color: #c62828;
  }
</style>
