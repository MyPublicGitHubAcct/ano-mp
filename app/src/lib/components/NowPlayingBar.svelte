<script lang="ts">
  // Along the bottom: the current track (click it for the now-playing view)
  // with its heart (PLAN.md F3), the transport, the seek bar, volume,
  // shuffle, repeat, the sleep timer (F13), the spectral freeze's Hold
  // while the freeze is on (X2), Record with the time recorded while
  // recording is on (X6) and the queue toggle. In the mini
  // player (F7, `mini`) the same bar stands alone: the track brings back the
  // main window, and the view toggles are left out.
  import { untrack } from "svelte";
  import { marks, player as playerApi, shell } from "$lib/api";
  import { t } from "$lib/i18n";
  import { collection } from "$lib/state/collection.svelte";
  import { effects } from "$lib/state/effects.svelte";
  import { recording } from "$lib/state/recording.svelte";
  import { formatTime } from "$lib/format";
  import { attempt } from "$lib/state/toasts.svelte";
  import { features } from "$lib/state/features.svelte";
  import { library } from "$lib/state/library.svelte";
  import { player } from "$lib/state/player.svelte";
  import { ui } from "$lib/state/ui.svelte";
  import Art from "./Art.svelte";
  import Heart from "./Heart.svelte";
  import Icon from "./Icon.svelte";
  import PracticePanel from "./PracticePanel.svelte";
  import SeekBar from "./SeekBar.svelte";
  import SignalPathPanel from "./SignalPathPanel.svelte";
  import SleepTimerPanel from "./SleepTimerPanel.svelte";

  let { mini = false }: { mini?: boolean } = $props();

  /** The panel open above the bar, if any. */
  let panel = $state<"signal" | "practice" | "sleep" | null>(null);
  /** Whether the current track is hearted. */
  let hearted = $state(false);

  $effect(() => {
    const trackId = player.currentItem?.external ? null : (player.currentItem?.trackId ?? null);
    void collection.version;
    untrack(async () => {
      hearted = trackId === null ? false : (await marks.favouritesAmong("track", [trackId]).catch(() => [])).length > 0;
    });
  });

  async function heart(on: boolean) {
    const trackId = player.currentItem?.trackId;
    if (trackId === undefined) return;
    hearted = on;
    if (mini) await attempt(() => marks.setFavourite("track", [trackId], on));
    else await collection.setFavourite("track", [trackId], on);
  }
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

  // A new track lets go of a held freeze: ask again as it changes.
  $effect(() => {
    void [player.currentItem?.uid, player.loaded, effects.freezeOn];
    if (effects.freezeOn) untrack(() => void effects.refresh());
  });

  const togglePanel = (which: "signal" | "practice" | "sleep") => (panel = panel === which ? null : which);

  /** Says the track is opening once that has taken a while (most open
      within milliseconds, and a flash at every change would distract). */
  let slowOpen = $state(false);
  $effect(() => {
    if (!player.loading) {
      slowOpen = false;
      return;
    }
    const timer = setTimeout(() => (slowOpen = true), 300);
    return () => clearTimeout(timer);
  });

  const item = $derived(player.currentItem);
  const repeatLabel = $derived(
    { off: t("bar.repeatOff"), all: t("bar.repeatAll"), one: t("bar.repeatOne") }[player.repeat],
  );
  const sleeping = $derived(player.sleep !== null || player.stopAfter !== null);
  let lastVolume = 1;

  function toggleNowPlaying() {
    if (mini) {
      attempt(shell.showMain);
      return;
    }
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

<div class="bar" class:mini>
  <div class="now">
    <button
      class="info"
      title={mini ? t("bar.showMain") : ui.nowPlayingInMain ? t("bar.closeNowPlaying") : t("bar.showNowPlaying")}
      aria-pressed={mini ? undefined : ui.nowPlayingInMain}
      onclick={toggleNowPlaying}
    >
      <Art albumId={item?.albumId ?? null} trackId={item?.trackId ?? null} size={mini ? "4rem" : "3.25rem"} />
      <span class="text">
        {#if item}
          <span class="title">{item.title}</span>
          {#if slowOpen}
            <span class="muted small" role="status">
              {player.downloading ? t("bar.downloading") : t("bar.opening")}
            </span>
          {:else}
            <span class="muted small">{[item.artist, item.album].filter(Boolean).join(" · ")}</span>
          {/if}
        {:else}
          <span class="muted">{t("bar.notPlaying")}</span>
        {/if}
      </span>
    </button>
    {#if item && !item.external}
      <Heart on={hearted} label={item.title} onchange={heart} />
    {/if}
  </div>

  <div class="transport">
    <div class="buttons">
      <button
        class="icon toggle"
        class:on={player.shuffle}
        title={player.shuffle ? t("bar.shuffleOn") : t("bar.shuffleOff")}
        aria-label={t("bar.shuffle")}
        aria-pressed={player.shuffle}
        onclick={player.toggleShuffle}><Icon name="shuffle" /></button
      >
      <button
        class="icon"
        title={t("bar.previous")}
        aria-label={t("bar.previous")}
        disabled={!item}
        onclick={player.previous}
      >
        <Icon name="previous" />
      </button>
      <button
        class="icon play"
        title={player.playing ? t("bar.pause") : t("bar.play")}
        aria-label={player.playing ? t("bar.pause") : t("bar.play")}
        disabled={!item}
        onclick={player.toggle}
      >
        <Icon name={player.playing ? "pause" : "play"} size="1.5rem" />
      </button>
      <button
        class="icon"
        title={t("bar.next")}
        aria-label={t("bar.next")}
        disabled={!player.hasNext}
        onclick={player.next}
      >
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
    {#if !mini && features.on.signalPath && format}
      <button
        class="format"
        data-popover-toggle
        title={t("bar.signalPath")}
        aria-expanded={panel === "signal"}
        onclick={() => togglePanel("signal")}>{format}</button
      >
    {/if}
    {#if !mini && features.on.practiceMode}
      <button
        class="icon toggle"
        class:on={panel === "practice"}
        data-popover-toggle
        title={t("bar.practiceTitle")}
        aria-label={t("bar.practice")}
        aria-expanded={panel === "practice"}
        onclick={() => togglePanel("practice")}><Icon name="sliders" /></button
      >
    {/if}
    {#if !mini && effects.freezeOn}
      <button
        class="icon toggle"
        class:on={effects.held}
        title={effects.held ? t("bar.freezeRelease") : t("bar.freezeHold")}
        aria-label={t("bar.freeze")}
        aria-pressed={effects.held}
        disabled={!item && !effects.held}
        onclick={() => effects.toggleHold()}><Icon name="snowflake" /></button
      >
    {/if}
    {#if !mini && (features.on.recording || recording.recording)}
      <button
        class="record"
        class:on={recording.recording}
        title={recording.recording ? t("bar.recordStop") : t("bar.recordStart")}
        aria-label={t("bar.record")}
        aria-pressed={recording.recording}
        disabled={recording.busy}
        onclick={() => recording.toggle()}
      >
        <Icon name="record" />
        {#if recording.recording}
          <span class="elapsed" role="timer" aria-label={t("bar.recorded")}>{formatTime(recording.state.seconds)}</span>
        {/if}
      </button>
    {/if}
    {#if !mini}
      <button
        class="icon toggle"
        class:on={sleeping}
        data-popover-toggle
        title={sleeping ? t("bar.sleepOn") : t("bar.sleep")}
        aria-label={t("bar.sleep")}
        aria-expanded={panel === "sleep"}
        onclick={() => togglePanel("sleep")}><Icon name="moon" /></button
      >
    {/if}
    {#if panel === "signal"}
      <SignalPathPanel onclose={() => (panel = null)} />
    {:else if panel === "practice"}
      <PracticePanel onclose={() => (panel = null)} />
    {:else if panel === "sleep"}
      <SleepTimerPanel onclose={() => (panel = null)} />
    {/if}
    <button
      class="icon"
      title={player.volume > 0 ? t("bar.mute") : t("bar.unmute")}
      aria-label={t("bar.mute")}
      aria-pressed={player.volume === 0}
      onclick={toggleMute}
    >
      <Icon name={player.volume > 0 ? "volume" : "mute"} />
    </button>
    <input
      class="volume"
      type="range"
      min="0"
      max="1"
      step="0.01"
      value={player.volume}
      aria-label={t("bar.volume")}
      aria-valuetext={t("bar.volumePercent", { percent: Math.round(player.volume * 100) })}
      oninput={(event) => player.setVolume(Number(event.currentTarget.value))}
      onkeydown={(event) => event.stopPropagation()}
    />
    {#if !mini}
      <button
        class="icon toggle"
        class:on={ui.visualizerInMain}
        title={ui.visualizerInMain ? t("bar.closeVisualizer") : t("bar.showVisualizer")}
        aria-label={t("bar.visualizer")}
        aria-pressed={ui.visualizerInMain}
        onclick={toggleVisualizer}><Icon name="wave" /></button
      >
      <button
        class="icon toggle"
        class:on={ui.queueInMain || (ui.queueOpen && !ui.nowPlayingInMain)}
        title={ui.queueInMain
          ? t("bar.backToLibrary")
          : ui.nowPlayingInMain || !ui.queueOpen
            ? t("bar.showQueue")
            : t("bar.hideQueue")}
        aria-label={t("queue.title")}
        aria-pressed={ui.queueInMain || (ui.queueOpen && !ui.nowPlayingInMain)}
        onclick={() => {
          if (ui.queueInMain) ui.showLibrary();
          else if (ui.nowPlayingInMain) ui.showQueue();
          else ui.queueOpen = !ui.queueOpen;
        }}><Icon name="queue" /></button
      >
    {/if}
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

  .now {
    grid-area: info;
    display: flex;
    align-items: center;
    gap: 0.25rem;
    min-width: 0;
  }

  .info {
    flex: 1;
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

  .record {
    display: flex;
    align-items: center;
    gap: 0.25rem;
    padding: 0.25rem 0.4rem;
    border: none;
    background: none;
    color: var(--text-muted);
    font-variant-numeric: tabular-nums;
  }

  .record.on {
    color: var(--danger);
  }

  .record .elapsed {
    font-size: 0.8rem;
    color: var(--text);
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

  /* The mini player: the track above the transport, filling the window. */
  .bar.mini {
    height: 100%;
    grid-template-columns: minmax(0, 1fr) auto;
    grid-template-areas:
      "info extra"
      "transport transport";
    padding: 0.5rem 0.75rem;
  }

  .mini .volume {
    max-width: 5rem;
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
