<script lang="ts">
  // The position slider and times. The only component reading the position
  // store, so position updates (every 50 ms) re-render just this. With the
  // waveform seek bar on (O2) and the track analysed, the track's waveform
  // is drawn under the slider, the played part in the accent colour;
  // until then it is the plain slider.
  import { t } from "$lib/i18n";
  import { untrack } from "svelte";
  import { formatTime } from "$lib/format";
  import { appearance } from "$lib/state/appearance.svelte";
  import { features } from "$lib/state/features.svelte";
  import { player } from "$lib/state/player.svelte";

  /** The value while the thumb is held; seeking happens on release. */
  let held = $state<number | null>(null);
  const position = $derived(held ?? player.position);
  const duration = $derived(player.duration);
  const disabled = $derived(player.currentItem === null);

  let waveform = $state.raw<number[] | null>(null);
  let canvas = $state<HTMLCanvasElement>();
  let width = $state(0);
  let height = $state(0);
  const trackId = $derived(player.currentItem?.trackId ?? null);
  const shown = $derived(features.on.waveformSeekBar && waveform !== null);

  $effect(() => {
    const id = trackId;
    void [features.analysisVersion, features.on.waveformSeekBar];
    untrack(async () => {
      if (id === null || !features.on.waveformSeekBar) {
        waveform = null;
        return;
      }
      const loaded = await features.waveform(id);
      if (id === trackId) waveform = loaded;
    });
  });

  // Redrawn as the position moves; cheap (one rect per pixel column).
  $effect(() => {
    if (!canvas || !waveform || width === 0 || height === 0) return;
    const ratio = window.devicePixelRatio || 1;
    canvas.width = Math.round(width * ratio);
    canvas.height = Math.round(height * ratio);
    const context = canvas.getContext("2d");
    if (!context) return;
    const { accent: played, "text-faint": rest } = appearance.current.props;
    const points = waveform.length / 2;
    const columns = canvas.width;
    const middle = canvas.height / 2;
    const progress = duration > 0 ? position / duration : 0;
    context.clearRect(0, 0, canvas.width, canvas.height);
    for (let x = 0; x < columns; x++) {
      // The loudest point in the column's slice of the envelope.
      const from = Math.floor((x / columns) * points);
      const to = Math.max(from + 1, Math.floor(((x + 1) / columns) * points));
      let low = 0;
      let high = 0;
      for (let point = from; point < to && point < points; point++) {
        low = Math.min(low, waveform[point * 2]);
        high = Math.max(high, waveform[point * 2 + 1]);
      }
      const top = middle - (high / 127) * middle;
      const bottom = middle - (low / 127) * middle;
      context.fillStyle = x / columns <= progress ? played : rest;
      context.fillRect(x, top, 1, Math.max(ratio, bottom - top));
    }
  });

  function commit() {
    if (held !== null) player.seek(held);
    held = null;
  }
</script>

<div class="seek">
  <span class="time">{formatTime(position)}</span>
  <div class="track" class:waveform={shown} bind:clientWidth={width} bind:clientHeight={height}>
    {#if shown}<canvas bind:this={canvas} aria-hidden="true"></canvas>{/if}
    <input
      type="range"
      min="0"
      max={duration || 1}
      step="0.1"
      value={position}
      {disabled}
      aria-label={t("seek.label")}
      aria-valuetext={t("seek.value", { position: formatTime(position), duration: formatTime(duration) })}
      style:--progress="{duration > 0 ? (position / duration) * 100 : 0}%"
      oninput={(event) => (held = Number(event.currentTarget.value))}
      onchange={commit}
      onkeydown={(event) => event.stopPropagation()}
    />
  </div>
  <span class="time">-{formatTime(Math.max(0, duration - position))}</span>
</div>

<style>
  .seek {
    display: flex;
    align-items: center;
    gap: 0.5rem;
    width: 100%;
  }

  .time {
    font-size: 0.75rem;
    font-variant-numeric: tabular-nums;
    color: var(--text-muted);
    min-width: 3.2em;
    text-align: center;
  }

  .track {
    position: relative;
    flex: 1;
    min-width: 0;
    display: flex;
    align-items: center;
  }

  .track.waveform {
    height: 1.75rem;
  }

  canvas {
    position: absolute;
    inset: 0;
    width: 100%;
    height: 100%;
  }

  input {
    flex: 1;
    min-width: 0;
    position: relative;
  }

  /* Over the waveform: an invisible track, a slim thumb. */
  .waveform input {
    -webkit-appearance: none;
    appearance: none;
    background: transparent;
    height: 100%;
    margin: 0;
  }

  .waveform input::-webkit-slider-runnable-track {
    background: transparent;
    height: 100%;
  }

  .waveform input::-webkit-slider-thumb {
    -webkit-appearance: none;
    width: 3px;
    height: 100%;
    background: var(--text);
    border-radius: 1px;
  }
</style>
