<script lang="ts">
  // The graphic equaliser (PLAN.md F15): on or off, ten bands and a
  // preamp, from a preset or by hand, with a second profile for
  // headphones when it follows the output. A change glides in over a few
  // milliseconds, so dragging a slider while music plays doesn't click.
  import type { EqualiserProfile } from "$lib/api";
  import { BANDS, MAX_GAIN, PRESETS, bandLabel, presetOf, safePreamp } from "$lib/equaliser";
  import { t, type MessageKey } from "$lib/i18n";
  import { appSettings } from "$lib/state/settings.svelte";

  const eq = $derived(appSettings.current.equaliser);
  let editing = $state<"speakers" | "headphones">("speakers");
  const profile = $derived(eq.followOutput ? eq[editing] : eq.speakers);
  const key = $derived(eq.followOutput ? editing : "speakers");

  const PRESET_NAMES: Record<string, MessageKey> = {
    flat: "eq.preset.flat",
    bassBoost: "eq.preset.bassBoost",
    bassReduce: "eq.preset.bassReduce",
    trebleBoost: "eq.preset.trebleBoost",
    trebleReduce: "eq.preset.trebleReduce",
    vocal: "eq.preset.vocal",
    loudness: "eq.preset.loudness",
    rock: "eq.preset.rock",
    classical: "eq.preset.classical",
    headphones: "eq.preset.headphones",
  };

  const db = (value: number) => `${value > 0 ? "+" : ""}${value.toFixed(1)} dB`;

  function saveProfile(edit: (profile: EqualiserProfile) => void) {
    const which = key;
    return appSettings.save((next) => {
      edit(next.equaliser[which]);
      next.equaliser[which].preset = presetOf(next.equaliser[which].gains);
    });
  }

  function choosePreset(id: string) {
    const gains = PRESETS[id];
    if (!gains) return;
    saveProfile((next) => {
      next.gains = [...gains];
      next.preamp = safePreamp(gains);
    });
  }

  function setBand(index: number, value: number) {
    saveProfile((next) => (next.gains[index] = Math.max(-MAX_GAIN, Math.min(MAX_GAIN, value))));
  }
</script>

<label class="switch">
  <input
    type="checkbox"
    checked={eq.enabled}
    onchange={(event) => {
      const on = event.currentTarget.checked;
      appSettings.save((next) => (next.equaliser.enabled = on));
    }}
  />
  <span>
    <span class="title">{t("eq.enabled")}</span>
    <span class="hint">{t("eq.enabledHint")}</span>
  </span>
</label>

<label class="switch">
  <input
    type="checkbox"
    checked={eq.followOutput}
    onchange={(event) => {
      const on = event.currentTarget.checked;
      appSettings.save((next) => (next.equaliser.followOutput = on));
    }}
  />
  <span>
    <span class="title">{t("eq.follow")}</span>
    <span class="hint">{t("eq.followHint")}</span>
  </span>
</label>

<fieldset disabled={!eq.enabled || appSettings.saving}>
  {#if eq.followOutput}
    <div class="profiles" role="radiogroup" aria-label={t("eq.profile")}>
      <label><input type="radio" bind:group={editing} value="speakers" /> {t("eq.speakers")}</label>
      <label><input type="radio" bind:group={editing} value="headphones" /> {t("eq.headphones")}</label>
    </div>
  {/if}

  <label class="field">
    <span class="label">{t("eq.presetLabel")}</span>
    <span class="control">
      <select value={profile.preset} onchange={(event) => choosePreset(event.currentTarget.value)}>
        {#each Object.keys(PRESETS) as id (id)}
          <option value={id}>{t(PRESET_NAMES[id])}</option>
        {/each}
        {#if profile.preset === "custom"}<option value="custom">{t("eq.preset.custom")}</option>{/if}
      </select>
    </span>
  </label>

  <label class="field">
    <span class="label">{t("eq.preamp")}</span>
    <span class="control">
      <input
        type="range"
        min={-MAX_GAIN}
        max={MAX_GAIN}
        step="0.5"
        value={profile.preamp}
        onchange={(event) => {
          const value = Number(event.currentTarget.value);
          saveProfile((next) => (next.preamp = value));
        }}
      />
      <span class="value">{db(profile.preamp)}</span>
    </span>
    <span class="hint">{t("eq.preampHint")}</span>
  </label>

  <div class="bands" role="group" aria-label={t("eq.bands")}>
    {#each BANDS as hz, index (hz)}
      <label class="band">
        <span class="gain">{profile.gains[index] > 0 ? "+" : ""}{profile.gains[index]}</span>
        <input
          type="range"
          min={-MAX_GAIN}
          max={MAX_GAIN}
          step="0.5"
          value={profile.gains[index]}
          aria-label={t("eq.band", { hz: bandLabel(hz) })}
          aria-valuetext={db(profile.gains[index])}
          onchange={(event) => setBand(index, Number(event.currentTarget.value))}
        />
        <span class="hz">{bandLabel(hz)}</span>
      </label>
    {/each}
  </div>
</fieldset>

<div class="actions">
  <button onclick={() => appSettings.reset("equaliser")} disabled={appSettings.saving}>{t("eq.reset")}</button>
</div>

<style>
  fieldset {
    border: none;
    margin: 0.5rem 0 0;
    padding: 0;
    min-width: 0;
  }

  fieldset:disabled {
    opacity: 0.55;
  }

  .profiles {
    display: flex;
    gap: 1.25rem;
    margin: 0.5rem 0;
  }

  .bands {
    display: grid;
    grid-template-columns: repeat(10, minmax(2rem, 1fr));
    gap: 0.25rem;
    margin: 1rem 0 0.5rem;
    max-width: 36rem;
  }

  .band {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 0.35rem;
    font-size: 0.75rem;
  }

  .band input {
    writing-mode: vertical-lr;
    direction: rtl;
    height: 9rem;
    width: 1.5rem;
  }

  .gain {
    font-variant-numeric: tabular-nums;
    color: var(--text-muted);
  }

  .hz {
    color: var(--text-muted);
  }
</style>
