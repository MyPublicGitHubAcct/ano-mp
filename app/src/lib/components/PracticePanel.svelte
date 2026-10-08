<script lang="ts">
  // Practice mode (O12): an A–B loop (`LoopControls`), and the tempo
  // (50–150%) and pitch (±12 semitones), each independent of the other.
  import { t } from "$lib/i18n";
  import { onMount } from "svelte";
  import { player as api } from "$lib/api";
  import { attempt } from "$lib/state/toasts.svelte";
  import LoopControls from "./LoopControls.svelte";
  import Popover from "./Popover.svelte";

  let { onclose }: { onclose: () => void } = $props();

  let tempo = $state(1);
  let semitones = $state(0);

  onMount(async () => {
    const practice = await api.practice().catch(() => null);
    if (practice) {
      tempo = practice.tempo;
      semitones = practice.semitones;
    }
  });

  const applyTempo = () => attempt(() => api.setTempo(tempo, semitones));

  function reset() {
    tempo = 1;
    semitones = 0;
    applyTempo();
  }
</script>

<Popover title={t("bar.practice")} {onclose}>
  <LoopControls />

  <label class="slider">
    <span class="label">{t("practice.speed")}</span>
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
    <span class="label">{t("practice.pitch")}</span>
    <input
      type="range"
      min="-12"
      max="12"
      step="1"
      bind:value={semitones}
      onchange={applyTempo}
      onkeydown={(event) => event.stopPropagation()}
    />
    <span class="value">{t("practice.semitones", { value: `${semitones > 0 ? "+" : ""}${semitones}` })}</span>
  </label>
  <p class="row">
    <button onclick={reset} disabled={tempo === 1 && semitones === 0}>{t("practice.reset")}</button>
  </p>
</Popover>

<style>
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
</style>
