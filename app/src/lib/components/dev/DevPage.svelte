<script lang="ts">
  // Developer tools from the Phase 0–2 dev UI: the core version and output
  // device, a test tone, and loading files by path straight into the engine
  // (which detaches the queue until it next starts a track), with an event
  // log. Library folders are managed in the player's sidebar.
  import { open } from "@tauri-apps/plugin-dialog";
  import { dev, on, onAll, type PlayerState } from "$lib/api";
  import { formatTime } from "$lib/format";

  let coreVersion = $state<string | null>(null);
  let deviceName = $state<string | null>(null);
  let tonePlaying = $state(false);
  let frequency = $state(440);
  let trackPath = $state("");
  let nextPath = $state("");
  let playerState = $state<PlayerState>("empty");
  let position = $state(0);
  let duration = $state(0);
  let events = $state<string[]>([]);
  let error = $state<string | null>(null);

  // The formats in PLAN.md §4.3; the core has the final say.
  const audioExtensions = ["mp3", "flac", "wav", "aif", "aiff", "ogg", "oga", "opus", "m4a", "mp4", "aac", "wma"];

  function log(message: string) {
    events = [`${new Date().toLocaleTimeString()} ${message}`, ...events.slice(0, 199)];
  }

  async function run(action: () => Promise<unknown>) {
    try {
      error = null;
      await action();
    } catch (e) {
      error = String(e);
    }
  }

  /** Accepts paths copied from Terminal: surrounding quotes, or backslash
      escapes as in `My\ Music/Spock\'s\ Beard`. */
  function cleanPath(text: string) {
    const path = text.trim();
    const quoted = /^(['"])(.*)\1$/.exec(path);
    return quoted ? quoted[2] : path.replace(/\\(.)/g, "$1");
  }

  const pickFile = () =>
    open({ multiple: false, directory: false, filters: [{ name: "Audio", extensions: audioExtensions }] });

  const load = () => run(() => dev.load(cleanPath(trackPath)));
  const setNext = () => run(() => dev.setNext(cleanPath(nextPath) || null));

  const browse = (which: "track" | "next") =>
    run(async () => {
      const path = await pickFile();
      if (path === null) return;
      if (which === "track") {
        trackPath = path;
        await dev.load(path);
      } else {
        nextPath = path;
        await dev.setNext(path);
      }
    });

  $effect(() => {
    run(async () => {
      coreVersion = await dev.coreVersion();
      deviceName = await dev.deviceName();
    });
    return onAll([
      on("audio-device-changed", () => {
        log("device changed");
        run(async () => (deviceName = await dev.deviceName()));
      }),
      on("player-state", (state) => {
        playerState = state;
        log(`player ${state}`);
      }),
      on("player-position", (payload) => ({ position, duration } = payload)),
      on("player-track-ended", ({ advanced }) =>
        log(advanced ? "track ended, next track playing" : "track ended, stopped"),
      ),
      on("queue-changed", (state) =>
        log(
          `queue r${state.revision}: ${state.length} items, current ${state.current ?? "none"}${state.loaded ? "" : " (not loaded)"}`,
        ),
      ),
    ]);
  });
</script>

<main>
  <p><a href="/">← Back to the player</a></p>
  <h1>Developer tools</h1>
  <p>Audio core v{coreVersion ?? "…"} · Output device: {deviceName ?? "none"}</p>

  <h2>Test tone</h2>
  <div class="row">
    <label>Frequency (Hz) <input type="number" min="20" max="20000" bind:value={frequency} /></label>
    {#if tonePlaying}
      <button
        onclick={() =>
          run(async () => {
            await dev.stopTestTone();
            tonePlaying = false;
          })}>Stop tone</button
      >
    {:else}
      <button
        onclick={() =>
          run(async () => {
            await dev.playTestTone(frequency);
            tonePlaying = true;
          })}>Play test tone</button
      >
    {/if}
  </div>

  <h2>Engine</h2>
  <p class="muted">Loads files directly; the queue stops following the engine until it next starts a track.</p>
  <div class="row">
    <label class="path"
      >Track <input type="text" placeholder="/absolute/path/to/file.flac" bind:value={trackPath} /></label
    >
    <button onclick={() => browse("track")}>Browse…</button>
    <button onclick={load} disabled={trackPath === ""}>Load</button>
  </div>
  <div class="row">
    <label class="path"
      >Next <input type="text" placeholder="plays gaplessly after the track" bind:value={nextPath} /></label
    >
    <button onclick={() => browse("next")} disabled={playerState === "empty"}>Browse…</button>
    <button onclick={setNext} disabled={playerState === "empty"}>{nextPath === "" ? "Clear next" : "Set next"}</button>
  </div>
  <div class="row">
    <button onclick={() => run(playerState === "playing" ? dev.pause : dev.play)} disabled={playerState === "empty"}>
      {playerState === "playing" ? "Pause" : "Play"}
    </button>
    <button onclick={() => run(dev.stop)} disabled={playerState === "empty"}>Stop</button>
    <span>{playerState} · {formatTime(position)} / {formatTime(duration)}</span>
  </div>

  {#if error}<p class="error">{error}</p>{/if}

  <h2>Events</h2>
  <ul class="events">
    {#each events as event, i (i)}
      <li>{event}</li>
    {/each}
  </ul>
</main>

<style>
  main {
    height: 100vh;
    overflow-y: auto;
    padding: 1.5rem 2rem;
  }

  a {
    color: var(--accent);
  }

  h2 {
    font-size: 1rem;
    margin-top: 1.75rem;
  }

  .row {
    display: flex;
    flex-wrap: wrap;
    gap: 0.75rem;
    align-items: center;
    margin-top: 0.6rem;
  }

  label {
    display: flex;
    align-items: center;
    gap: 0.5rem;
  }

  input[type="number"] {
    width: 6rem;
  }

  .path {
    flex: 1 1 20rem;
  }

  .path input {
    flex: 1;
    min-width: 0;
  }

  .error {
    color: var(--danger);
  }

  .events {
    font-family: ui-monospace, monospace;
    font-size: 0.8rem;
    padding-left: 1rem;
  }
</style>
