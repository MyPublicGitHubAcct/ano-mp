<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";

  let coreVersion = $state<string | null>(null);
  let error = $state<string | null>(null);

  $effect(() => {
    invoke<string>("core_version")
      .then((v) => (coreVersion = v))
      .catch((e) => (error = String(e)));
  });
</script>

<main>
  <h1>ano-mp</h1>
  {#if error}
    <p class="error">Core unavailable: {error}</p>
  {:else if coreVersion}
    <p>Audio core v{coreVersion}</p>
  {:else}
    <p>Loading core…</p>
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

  .error {
    color: #c62828;
  }
</style>
