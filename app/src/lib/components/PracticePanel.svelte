<script lang="ts">
  // Practice mode (O12): an A–B loop set from the current position (the
  // engine jumps back on the exact sample), and the tempo (50–150%) and
  // pitch (±12 semitones), each independent of the other.
  import { onMount } from "svelte";
  import { player as api } from "$lib/api";
  import { formatTime } from "$lib/format";
  import { playback } from "$lib/state/position.svelte";
  import { player } from "$lib/state/player.svelte";
  import { attempt } from "$lib/state/toasts.svelte";
  import Popover from "./Popover.svelte";

  let { onclose }: { onclose: () => void } = $props();

  let loop = $state.raw<[number, number] | null>(null);
  /** A set, waiting for B. */
  let pendingStart = $state<number | null>(null);
  let tempo = $state(1);
  let semitones = $state(0);

  onMount(async () => {
    const practice = await api.practice().catch(() => null);
    if (practice) {
      loop = practice.loop;
      tempo = practice.tempo;
      semitones = practice.semitones;
    }
  });

  function setA() {
    pendingStart = player.position;
    loop = null;
  }

  async function setB() {
    const start = pendingStart ?? loop?.[0] ?? 0;
    const end = player.position;
    if (end - start < 0.25) return;
    const set = await attempt(() => api.setLoop(start, end));
    if (set !== undefined) {
      loop = set;
      pendingStart = null;
    }
  }

  async function clearLoop() {
    pendingStart = null;
    if ((await attempt(() => api.setLoop(null, null))) !== undefined) loop = null;
  }

  const applyTempo = () => attempt(() => api.setTempo(tempo, semitones));

  function reset() {
    tempo = 1;
    semitones = 0;
    applyTempo();
  }
</script>

<Popover title="Practice" {onclose}>
  <div class="loop">
    <span class="label">Loop</span>
    <button onclick={setA} disabled={!player.loaded}>Set A</button>
    <button onclick={setB} disabled={!player.loaded || (pendingStart === null && loop === null)}>Set B</button>
    <button onclick={clearLoop} disabled={loop === null && pendingStart === null}>Clear</button>
  </div>
  <p class="muted small" role="status">
    {#if loop}
      Looping {formatTime(loop[0])} – {formatTime(loop[1])}
    {:else if pendingStart !== null}
      A at {formatTime(pendingStart)}; play on and set B.
    {:else}
      Set A where the loop starts and B where it ends, as it plays. Now {formatTime(playback.position)}.
    {/if}
  </p>

  <label class="slider">
    <span class="label">Speed</span>
    <input
      type="range"
      min="0.5"
      max="1.5"
      step="0.05"
      bind:value={tempo}
      onchange={applyTempo}
      onkeydown={(event) => event.stopPropagation()}
    />
    <span class="value">{Math.round(tempo * 100)}%</span>
  </label>
  <label class="slider">
    <span class="label">Pitch</span>
    <input
      type="range"
      min="-12"
      max="12"
      step="1"
      bind:value={semitones}
      onchange={applyTempo}
      onkeydown={(event) => event.stopPropagation()}
    />
    <span class="value">{semitones > 0 ? "+" : ""}{semitones} st</span>
  </label>
  <p class="row">
    <button onclick={reset} disabled={tempo === 1 && semitones === 0}>Normal speed and pitch</button>
  </p>
</Popover>

<style>
  .loop,
  .slider,
  .row {
    display: flex;
    align-items: center;
    gap: 0.5rem;
    margin: 0.35rem 0;
  }

  .label {
    width: 3.5rem;
    font-weight: 600;
    font-size: 0.88rem;
  }

  .slider input {
    flex: 1;
  }

  .value {
    width: 3.5rem;
    text-align: right;
    font-variant-numeric: tabular-nums;
    font-size: 0.88rem;
  }

  .small {
    font-size: 0.8rem;
    margin: 0 0 0.5rem;
  }
</style>
