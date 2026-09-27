<script lang="ts">
  // The visualizer: which visualization, the cover wall's albums, how often
  // it analyses and how strongly it reacts, whether it takes its colours
  // from the cover, and whether it moves on to the next one by itself.
  import { appSettings } from "$lib/state/settings.svelte";
  import { VISUALIZATIONS, visualization } from "$lib/visualizer";

  const CYCLES: [number, string][] = [
    [0, "Never"],
    [30, "Every 30 seconds"],
    [60, "Every minute"],
    [120, "Every 2 minutes"],
    [300, "Every 5 minutes"],
    [600, "Every 10 minutes"],
  ];

  const settings = $derived(appSettings.visualizer);
  const chosen = $derived(visualization(settings.visualization));
  const cycles = $derived(
    CYCLES.some(([seconds]) => seconds === settings.cycleSeconds)
      ? CYCLES
      : [...CYCLES, [settings.cycleSeconds, `Every ${settings.cycleSeconds} seconds`] as [number, string]],
  );

  const save = appSettings.save.bind(appSettings);
</script>

<label class="field">
  <span class="label">Visualization</span>
  <span class="control">
    <select
      value={chosen.id}
      disabled={appSettings.saving}
      onchange={(event) => {
        const id = event.currentTarget.value;
        save((next) => (next.visualizer.visualization = id));
      }}
    >
      {#each VISUALIZATIONS as option (option.id)}
        <option value={option.id}>{option.name}</option>
      {/each}
    </select>
  </span>
  <span class="hint">{chosen.description} In the visualizer, V changes it.</span>
</label>

<label class="field">
  <span class="label">Change by itself</span>
  <span class="control">
    <select
      value={String(settings.cycleSeconds)}
      disabled={appSettings.saving}
      onchange={(event) => {
        const seconds = Number(event.currentTarget.value);
        save((next) => (next.visualizer.cycleSeconds = seconds));
      }}
    >
      {#each cycles as [seconds, name] (seconds)}
        <option value={String(seconds)}>{name}</option>
      {/each}
    </select>
  </span>
  <span class="hint">Moves on to the next visualization while the visualizer shows.</span>
</label>

<div class="field">
  <span class="label">Cover wall</span>
  <span class="control" role="radiogroup" aria-label="Cover wall shows">
    {#each [["year", "Albums from the same year"], ["artist", "Albums by the same artist"]] as const as [basis, name] (basis)}
      <label class="switch">
        <input
          type="radio"
          name="cover-basis"
          checked={settings.coverBasis === basis}
          disabled={appSettings.saving}
          onchange={() => save((next) => (next.visualizer.coverBasis = basis))}
        />
        <span>{name}</span>
      </label>
    {/each}
  </span>
</div>

<h3>Look and feel</h3>
<label class="field">
  <span class="label">Frame rate</span>
  <span class="control">
    <select
      value={String(settings.frameRate)}
      disabled={appSettings.saving}
      onchange={(event) => {
        const rate = Number(event.currentTarget.value);
        save((next) => (next.visualizer.frameRate = rate));
      }}
    >
      {#each [30, 60].includes(settings.frameRate) ? [30, 60] : [30, 60, settings.frameRate] as rate (rate)}
        <option value={String(rate)}>{rate} a second{rate === 30 ? " (uses less power)" : ""}</option>
      {/each}
    </select>
  </span>
  <span class="hint">How often the sound is analysed.</span>
</label>

<label class="field">
  <span class="label">Sensitivity</span>
  <span class="control">
    <input
      type="range"
      min="0.25"
      max="4"
      step="0.05"
      value={settings.sensitivity}
      disabled={appSettings.saving}
      onchange={(event) => {
        const value = Number(event.currentTarget.value);
        save((next) => (next.visualizer.sensitivity = value));
      }}
    />
    <span class="value">{Math.round(settings.sensitivity * 100)}%</span>
  </span>
  <span class="hint">Scales the spectrum: higher for quiet music, lower if the bars sit at the top.</span>
</label>

<label class="switch">
  <input
    type="checkbox"
    checked={settings.colorsFromCover}
    disabled={appSettings.saving}
    onchange={(event) => {
      const on = event.currentTarget.checked;
      save((next) => (next.visualizer.colorsFromCover = on));
    }}
  />
  <span>
    <span class="title">Colours from the album cover</span>
    <span class="hint">Otherwise every album gets the same colours.</span>
  </span>
</label>

<div class="actions">
  <button onclick={() => appSettings.reset("visualizer")} disabled={appSettings.saving}>Reset to defaults</button>
</div>
