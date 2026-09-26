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

  type Folder = { id: number; path: string; trackCount: number; lastScanAt: number | null };
  type Track = {
    id: number;
    path: string;
    title: string | null;
    artist: string | null;
    album: string | null;
    albumArtist: string | null;
    discNumber: number | null;
    trackNumber: number | null;
    duration: number;
  };
  type ScanReport = {
    folderId: number;
    added: number;
    updated: number;
    removed: number;
    unchanged: number;
    failed: { path: string; error: string }[];
  };

  // Dev UI only: the real library browser (Phase 3) will page its queries.
  const shownTracks = 500;

  let folders = $state<Folder[]>([]);
  let tracks = $state<Track[]>([]);
  let scanning = $state(false);
  let scanProgress = $state<{ read: number; toRead: number } | null>(null);

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

  const fileName = (path: string) => path.split(/[\\/]/).pop() ?? path;

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

  async function refreshLibrary() {
    folders = await invoke<Folder[]>("library_folders");
    tracks = await invoke<Track[]>("library_tracks");
  }

  /** Scans one folder, or all of them when `folderId` is null. */
  async function scan(folderId: number | null) {
    scanning = true;
    try {
      const reports = await invoke<ScanReport[]>("library_scan", { folderId });
      for (const r of reports) {
        const path = folders.find((f) => f.id === r.folderId)?.path ?? `folder ${r.folderId}`;
        log(`scanned ${path}: ${r.added} added, ${r.updated} updated, ${r.removed} removed, ` +
          `${r.unchanged} unchanged, ${r.failed.length} failed`);
        for (const failure of r.failed.slice(0, 10)) log(`  ${failure.path}: ${failure.error}`);
      }
    } finally {
      scanning = false;
      scanProgress = null;
      await refreshLibrary();
    }
  }

  const addFolder = () =>
    run(async () => {
      const path = await open({ multiple: false, directory: true });
      if (path === null) return;
      const folder = await invoke<Folder>("library_add_folder", { path });
      folders = [...folders, folder];
      await scan(folder.id);
    });

  const rescan = (folderId: number | null) => run(() => scan(folderId));

  const removeFolder = (folderId: number) =>
    run(async () => {
      await invoke("library_remove_folder", { folderId });
      await refreshLibrary();
    });

  const playTrack = (track: Track) =>
    run(async () => {
      trackPath = track.path;
      await invoke("player_load", { path: track.path });
      nextQueued = false;
      await invoke("player_play");
      await refreshPlayer();
    });

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
      await refreshLibrary();
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
      listen<{ folderId: number; read: number; toRead: number }>("library-scan-progress", ({ payload }) => {
        scanProgress = payload;
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

  <h2>Library</h2>
  <div class="row">
    <button onclick={addFolder} disabled={scanning}>Add folder…</button>
    <button onclick={() => rescan(null)} disabled={scanning || folders.length === 0}>Rescan all</button>
    {#if scanning}
      <span>Scanning{scanProgress ? `: read ${scanProgress.read} of ${scanProgress.toRead} new or changed files` : "…"}</span>
    {/if}
  </div>
  {#if folders.length > 0}
    <ul class="folders">
      {#each folders as folder (folder.id)}
        <li>
          <span class="folder-path">{folder.path}</span>
          <span>
            {folder.trackCount} tracks{folder.lastScanAt === null ? ", not scanned" : ""}
          </span>
          <button onclick={() => rescan(folder.id)} disabled={scanning}>Rescan</button>
          <button onclick={() => removeFolder(folder.id)} disabled={scanning}>Remove</button>
        </li>
      {/each}
    </ul>
  {/if}
  {#if tracks.length > 0}
    <table class="tracks">
      <thead>
        <tr><th>#</th><th>Title</th><th>Artist</th><th>Album</th><th>Time</th></tr>
      </thead>
      <tbody>
        {#each tracks.slice(0, shownTracks) as track (track.id)}
          <tr ondblclick={() => playTrack(track)} title="Double-click to play">
            <td>{track.trackNumber ?? ""}</td>
            <td>{track.title ?? fileName(track.path)}</td>
            <td>{track.artist ?? ""}</td>
            <td>{track.album ?? ""}</td>
            <td>{formatTime(track.duration)}</td>
          </tr>
        {/each}
      </tbody>
    </table>
    {#if tracks.length > shownTracks}
      <p>First {shownTracks} of {tracks.length} tracks.</p>
    {/if}
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

  .folders {
    padding: 0;
    list-style: none;
  }

  .folders li {
    display: flex;
    flex-wrap: wrap;
    gap: 1rem;
    align-items: center;
    margin-top: 0.5rem;
  }

  .folder-path {
    overflow-wrap: anywhere;
  }

  .tracks {
    width: 100%;
    margin-top: 1rem;
    border-collapse: collapse;
    font-size: 0.9rem;
  }

  .tracks th {
    text-align: left;
  }

  .tracks td,
  .tracks th {
    padding: 0.2rem 0.5rem;
  }

  .tracks tbody tr {
    cursor: default;
  }

  .tracks tbody tr:hover {
    background-color: rgb(128 128 128 / 0.15);
  }
</style>
