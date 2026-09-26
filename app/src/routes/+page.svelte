<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";
  import { listen } from "@tauri-apps/api/event";
  import { open } from "@tauri-apps/plugin-dialog";

  let coreVersion = $state<string | null>(null);
  let deviceName = $state<string | null>(null);
  let playing = $state(false);
  let frequency = $state(440);
  let deviceEvents = $state<string[]>([]);
  let error = $state<string | null>(null);

  type PlayerState = "empty" | "stopped" | "playing" | "paused";
  type PlayerStatus = { state: PlayerState; position: number; duration: number; volume: number };

  let trackPath = $state("");
  let nextPath = $state("");
  let nextQueued = $state(false);
  let playerState = $state<PlayerState>("empty");
  let position = $state(0);
  let duration = $state(0);
  let volume = $state(1);
  let seeking = $state(false);

  /** Accepts paths copied from Terminal: surrounding quotes, or backslash
      escapes as in `My\ Music/Spock\'s\ Beard`. */
  function cleanPath(text: string) {
    const path = text.trim();
    const quoted = /^(['"])(.*)\1$/.exec(path);
    return quoted ? quoted[2] : path.replace(/\\(.)/g, "$1");
  }

  // Dev UI only: the formats in PLAN.md §4.3. The core has the final say.
  const audioExtensions = ["mp3", "flac", "wav", "aif", "aiff", "ogg", "oga", "opus", "m4a", "mp4", "aac", "wma"];

  /** Shows an open dialog; returns the chosen path, or null if cancelled. */
  const pickFile = () =>
    open({ multiple: false, directory: false, filters: [{ name: "Audio", extensions: audioExtensions }] });

  const browseTrack = () =>
    run(async () => {
      const path = await pickFile();
      if (path === null) return;
      trackPath = path;
      await loadTrack();
    });

  const browseNext = () =>
    run(async () => {
      const path = await pickFile();
      if (path === null) return;
      nextPath = path;
      await queueNext();
    });

  const formatTime = (seconds: number) =>
    `${Math.floor(seconds / 60)}:${Math.floor(seconds % 60).toString().padStart(2, "0")}`;

  function log(message: string) {
    deviceEvents = [`${new Date().toLocaleTimeString()} ${message}`, ...deviceEvents];
  }

  async function refreshPlayer() {
    const status = await invoke<PlayerStatus>("player_status");
    ({ state: playerState, position, duration, volume } = status);
  }

  const loadTrack = () =>
    run(async () => {
      await invoke("player_load", { path: cleanPath(trackPath) });
      nextQueued = false;
      await refreshPlayer();
    });

  const queueNext = () =>
    run(async () => {
      const path = cleanPath(nextPath);
      await invoke("player_set_next", { path: path || null });
      nextQueued = path !== "";
    });

  const togglePlay = () =>
    run(() => invoke(playerState === "playing" ? "player_pause" : "player_play"));

  const stopPlayer = () => run(() => invoke("player_stop"));

  const seekTo = () =>
    run(async () => {
      await invoke("player_seek", { seconds: position });
      seeking = false;
    });

  const setVolume = () => run(() => invoke("player_set_volume", { volume }));

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
      await refreshPlayer();
    });

    const listeners = [
      listen("audio-device-changed", () => {
        log("device changed");
        run(refreshDevice);
      }),
      listen<PlayerState>("player-state", ({ payload }) => {
        playerState = payload;
        log(`player ${payload}`);
      }),
      listen<{ position: number; duration: number }>("player-position", ({ payload }) => {
        if (!seeking) position = payload.position;
        duration = payload.duration;
      }),
      listen<{ advanced: boolean }>("player-track-ended", ({ payload }) => {
        log(payload.advanced ? "track ended, next track playing" : "track ended, stopped");
        if (payload.advanced) nextQueued = false;
      }),
    ];

    return () => {
      for (const unlisten of listeners) unlisten.then((stopListening) => stopListening());
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

  <h2>Player</h2>
  <div class="row">
    <label class="path">
      Track
      <input placeholder="/absolute/path/to/file.flac" bind:value={trackPath} />
    </label>
    <button onclick={browseTrack}>Browse…</button>
    <button onclick={loadTrack} disabled={trackPath === ""}>Load</button>
  </div>
  <div class="row">
    <label class="path">
      Next
      <input placeholder="plays gaplessly after the track" bind:value={nextPath} />
    </label>
    <button onclick={browseNext} disabled={playerState === "empty"}>Browse…</button>
    <button onclick={queueNext} disabled={playerState === "empty"}>
      {nextPath === "" ? "Clear next" : "Set next"}
    </button>
    {#if nextQueued}<span>queued</span>{/if}
  </div>
  <div class="row">
    <button onclick={togglePlay} disabled={playerState === "empty"}>
      {playerState === "playing" ? "Pause" : "Play"}
    </button>
    <button onclick={stopPlayer} disabled={playerState === "empty"}>Stop</button>
    <input
      class="seek"
      type="range"
      min="0"
      max={duration}
      step="0.01"
      bind:value={position}
      oninput={() => (seeking = true)}
      onchange={seekTo}
      disabled={playerState === "empty"}
    />
    <span>{formatTime(position)} / {formatTime(duration)}</span>
    <label>
      Volume
      <input type="range" min="0" max="1" step="0.01" bind:value={volume} oninput={setVolume} />
    </label>
  </div>

  {#if error}
    <p class="error">{error}</p>
  {/if}

  {#if deviceEvents.length > 0}
    <h2>Events</h2>
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

  .row + .row {
    margin-top: 0.75rem;
  }

  input {
    width: 6rem;
    margin-left: 0.5rem;
  }

  .path {
    flex: 1 1 20rem;
    display: flex;
    align-items: center;
  }

  .path input {
    flex: 1;
    min-width: 0;
  }

  .seek {
    flex: 1 1 12rem;
    width: auto;
  }

  h2 {
    font-size: 1rem;
    margin-top: 2rem;
  }

  .error {
    color: #c62828;
  }
</style>
