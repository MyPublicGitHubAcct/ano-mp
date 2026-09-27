<script lang="ts">
  // Along the bottom: the current track (click it for the now-playing view),
  // the transport, the seek bar, volume, shuffle, repeat and the queue toggle.
  import { untrack } from "svelte";
  import { player as playerApi } from "$lib/api";
  import { features } from "$lib/state/features.svelte";
  import { library } from "$lib/state/library.svelte";
  import { player } from "$lib/state/player.svelte";
  import { ui } from "$lib/state/ui.svelte";
  import Art from "./Art.svelte";
  import Icon from "./Icon.svelte";
  import PracticePanel from "./PracticePanel.svelte";
  import SeekBar from "./SeekBar.svelte";
  import SignalPathPanel from "./SignalPathPanel.svelte";

  /** The panel open above the bar, if any. */
  let panel = $state<"signal" | "practice" | null>(null);
  /** "FLAC 16/44.1", for the signal path's button. */
  let format = $state("");

  $effect(() => {
    void [player.currentItem?.uid, player.loaded, features.on.signalPath];
    untrack(async () => {
      if (!features.on.signalPath || !player.loaded) {
        format = "";
        return;
      }
      const path = await playerApi.signalPath().catch(() => null);
      const p = path?.path;
      format =
        p && p.loaded
          ? `${p.codec.toUpperCase()} ${p.bitsPerSample ? `${p.bitsPerSample}/` : ""}${(p.fileSampleRate / 1000).toLocaleString(undefined, { maximumFractionDigits: 1 })}`
          : "";
    });
  });

  const togglePanel = (which: "signal" | "practice") => (panel = panel === which ? null : which);

  const item = $derived(player.currentItem);
  const repeatLabel = $derived({ off: "Repeat off", all: "Repeat all", one: "Repeat one" }[player.repeat]);
  let lastVolume = 1;

  function toggleNowPlaying() {
    if (ui.nowPlayingInMain && library.query.trim() === "") {
      ui.back();
    } else {
      library.query = "";
      ui.showNowPlaying();
    }
  }

  function toggleVisualizer() {
    if (ui.visualizerInMain && library.query === "") {
      ui.back();
    } else {
      library.query = "";
      ui.showVisualizer();
    }
  }

  function toggleMute() {
    if (player.volume > 0) {
      lastVolume = player.volume;
      player.setVolume(0);
    } else {
      player.setVolume(lastVolume || 1);
    }
  }
</script>

<div class="bar">
  <button
    class="info"
    title={ui.nowPlayingInMain ? "Close the now-playing view" : "Show the now-playing view"}
    aria-pressed={ui.nowPlayingInMain}
    onclick={toggleNowPlaying}
  >
    <Art albumId={item?.albumId ?? null} trackId={item?.trackId ?? null} size="3.25rem" />
    <span class="text">
      {#if item}
        <span class="title">{item.title}</span>
        <span class="muted small">{[item.artist, item.album].filter(Boolean).join(" · ")}</span>
      {:else}
        <span class="muted">Not playing</span>
      {/if}
    </span>
  </button>

  <div class="transport">
    <div class="buttons">
      <button
        class="icon toggle"
        class:on={player.shuffle}
        title={player.shuffle ? "Shuffle on" : "Shuffle off"}
        aria-label="Shuffle"
        aria-pressed={player.shuffle}
        onclick={player.toggleShuffle}><Icon name="shuffle" /></button
      >
      <button class="icon" title="Previous" aria-label="Previous" disabled={!item} onclick={player.previous}>
        <Icon name="previous" />
      </button>
      <button
        class="icon play"
        title={player.playing ? "Pause" : "Play"}
        aria-label={player.playing ? "Pause" : "Play"}
        disabled={!item}
        onclick={player.toggle}
      >
        <Icon name={player.playing ? "pause" : "play"} size="1.5rem" />
      </button>
      <button class="icon" title="Next" aria-label="Next" disabled={!player.hasNext} onclick={player.next}>
        <Icon name="next" />
      </button>
      <button
        class="icon toggle repeat"
        class:on={player.repeat !== "off"}
        title={repeatLabel}
        aria-label={repeatLabel}
        onclick={player.cycleRepeat}
      >
        <Icon name="repeat" />
        {#if player.repeat === "one"}<span class="one">1</span>{/if}
      </button>
    </div>
    <SeekBar />
  </div>

  <div class="extra">
    {#if features.on.signalPath && format}
      <button
        class="format"
        data-popover-toggle
        title="Signal path"
        aria-expanded={panel === "signal"}
        onclick={() => togglePanel("signal")}>{format}</button
      >
    {/if}
    {#if features.on.practiceMode}
      <button
        class="icon toggle"
        class:on={panel === "practice"}
        data-popover-toggle
        title="Practice: loop, speed and pitch"
        aria-label="Practice"
        aria-expanded={panel === "practice"}
        onclick={() => togglePanel("practice")}><Icon name="sliders" /></button
      >
    {/if}
    {#if panel === "signal"}
      <SignalPathPanel onclose={() => (panel = null)} />
    {:else if panel === "practice"}
      <PracticePanel onclose={() => (panel = null)} />
    {/if}
    <button class="icon" title={player.volume > 0 ? "Mute" : "Unmute"} aria-label="Mute" onclick={toggleMute}>
      <Icon name={player.volume > 0 ? "volume" : "mute"} />
    </button>
    <input
      class="volume"
      type="range"
      min="0"
      max="1"
      step="0.01"
      value={player.volume}
      aria-label="Volume"
      oninput={(event) => player.setVolume(Number(event.currentTarget.value))}
      onkeydown={(event) => event.stopPropagation()}
    />
    <button
      class="icon toggle"
      class:on={ui.visualizerInMain}
      title={ui.visualizerInMain ? "Close the visualizer" : "Show the visualizer"}
      aria-label="Visualizer"
      aria-pressed={ui.visualizerInMain}
      onclick={toggleVisualizer}><Icon name="wave" /></button
    >
    <button
      class="icon toggle"
      class:on={ui.queueInMain || (ui.queueOpen && !ui.nowPlayingInMain)}
      title={ui.queueInMain
        ? "Back to the library"
        : ui.nowPlayingInMain || !ui.queueOpen
          ? "Show the queue"
          : "Hide the queue"}
      aria-label="Queue"
      aria-pressed={ui.queueInMain || (ui.queueOpen && !ui.nowPlayingInMain)}
      onclick={() => {
        if (ui.queueInMain) ui.showLibrary();
        else if (ui.nowPlayingInMain) ui.showQueue();
        else ui.queueOpen = !ui.queueOpen;
      }}><Icon name="queue" /></button
    >
  </div>
</div>

<style>
  .bar {
    display: grid;
    grid-template-columns: minmax(0, 1fr) minmax(0, 2fr) minmax(0, 1fr);
    grid-template-areas: "info transport extra";
    align-items: center;
    gap: 0.5rem 1rem;
    padding: 0.5rem 1rem;
    padding-bottom: max(0.5rem, env(safe-area-inset-bottom));
  }

  .info {
    grid-area: info;
    display: flex;
    align-items: center;
    gap: 0.75rem;
    min-width: 0;
    margin: -0.25rem;
    padding: 0.25rem;
    border: none;
    background: none;
    text-align: left;
  }

  .info:hover:not(:disabled) {
    background: var(--hover);
  }

  .text {
    display: flex;
    flex-direction: column;
    min-width: 0;
  }

  .title,
  .small {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .title {
    font-weight: 600;
  }

  .small {
    font-size: 0.8rem;
  }

  .transport {
    grid-area: transport;
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 0.15rem;
    min-width: 0;
  }

  .buttons {
    display: flex;
    align-items: center;
    gap: 0.25rem;
  }

  .play {
    width: 2.5rem;
    height: 2.5rem;
    border-radius: 50%;
    background: var(--text);
    color: var(--bg);
  }

  .play:hover:not(:disabled) {
    background: var(--text);
    opacity: 0.85;
  }

  .toggle {
    color: var(--text-muted);
  }

  .toggle.on {
    color: var(--accent);
  }

  .repeat {
    position: relative;
  }

  .one {
    position: absolute;
    right: 0.1rem;
    bottom: 0.1rem;
    font-size: 0.6rem;
    font-weight: 700;
  }

  .format {
    font-size: 0.7rem;
    font-weight: 600;
    letter-spacing: 0.03em;
    padding: 0.15rem 0.4rem;
    color: var(--text-muted);
    white-space: nowrap;
  }

  .extra {
    position: relative;
    grid-area: extra;
    display: flex;
    align-items: center;
    justify-content: flex-end;
    gap: 0.25rem;
    min-width: 0;
  }

  .volume {
    width: 100%;
    max-width: 7rem;
    min-width: 3rem;
  }

  @media (max-width: 640px) {
    .bar {
      grid-template-columns: minmax(0, 1fr) auto;
      grid-template-areas:
        "info extra"
        "transport transport";
    }

    .volume {
      display: none;
    }
  }
</style>
