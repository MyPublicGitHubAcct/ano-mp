<script lang="ts">
  // The visualizer: which visualization, the cover wall's albums, how often
  // it analyses and how strongly it reacts, whether it takes its colours
  // from the cover, and whether it moves on to the next one by itself.
  import { t } from "$lib/i18n";
  import { appSettings } from "$lib/state/settings.svelte";
  import { VISUALIZATIONS, visualization } from "$lib/visualizer";

  const CYCLES: [number, string][] = [
    [0, t("vizOptions.never")],
    [30, t("vizOptions.everySeconds", { count: 30 })],
    [60, t("vizOptions.everyMinute")],
    [120, t("vizOptions.everyMinutes", { count: 2 })],
    [300, t("vizOptions.everyMinutes", { count: 5 })],
    [600, t("vizOptions.everyMinutes", { count: 10 })],
  ];

  const settings = $derived(appSettings.visualizer);
  const chosen = $derived(visualization(settings.visualization));
  const cycles = $derived(
    CYCLES.some(([seconds]) => seconds === settings.cycleSeconds)
      ? CYCLES
      : [...CYCLES, [settings.cycleSeconds, t("vizOptions.everySeconds", { count: settings.cycleSeconds })] as [number, string]],
  );

  const save = appSettings.save.bind(appSettings);
</script>

<label class="field">
  <span class="label">{t("vizOptions.visualization")}</span>
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
  <span class="hint">{chosen.description} {t("vizOptions.visualizationHint")}</span>
</label>

<label class="field">
  <span class="label">{t("vizOptions.cycle")}</span>
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
  <span class="hint">{t("vizOptions.cycleHint")}</span>
</label>

<div class="field">
  <span class="label">{t("vizOptions.coverWall")}</span>
  <span class="control" role="radiogroup" aria-label={t("vizOptions.coverWallShows")}>
    {#each [["year", t("vizOptions.sameYear")], ["artist", t("vizOptions.sameArtist")]] as const as [basis, name] (basis)}
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

<h3>{t("vizOptions.look")}</h3>
<label class="field">
  <span class="label">{t("vizOptions.frameRate")}</span>
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
        <option value={String(rate)}>{t(rate === 30 ? "vizOptions.rateLowPower" : "vizOptions.rate", { rate })}</option>
      {/each}
    </select>
  </span>
  <span class="hint">{t("vizOptions.frameRateHint")}</span>
</label>

<label class="field">
  <span class="label">{t("vizOptions.sensitivity")}</span>
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
  <span class="hint">{t("vizOptions.sensitivityHint")}</span>
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
    <span class="title">{t("vizOptions.coverColours")}</span>
    <span class="hint">{t("vizOptions.coverColoursHint")}</span>
  </span>
</label>

<label class="switch">
  <input
    type="checkbox"
    checked={settings.calm}
    disabled={appSettings.saving}
    onchange={(event) => {
      const on = event.currentTarget.checked;
      save((next) => (next.visualizer.calm = on));
    }}
  />
  <span>
    <span class="title">{t("vizOptions.calm")}</span>
    <span class="hint">{t("vizOptions.calmHint")}</span>
  </span>
</label>

<div class="actions">
  <button onclick={() => appSettings.reset("visualizer")} disabled={appSettings.saving}>{t("settings.reset")}</button>
</div>
