<script lang="ts">
  // The effects (PLAN.md X2): a preset, then each effect in the order it
  // runs, with its switch, mix and parameters, whose ranges come from the
  // core. A slider is heard as it moves (previewed, not saved) and saved
  // when let go, so dragging never floods the settings; the spectral
  // freeze has its Hold button, which reflects a new track letting go.
  // Its first switch is the `effects` feature's (also in Features): off,
  // the rest stays in view, greyed out.
  import { onMount } from "svelte";
  import { effects as api, type EffectInfo, type EffectParam, type EffectsSettings } from "$lib/api";
  import { PRESETS, STEPS, applyPreset, display, fromPosition, toPosition, type EffectId } from "$lib/effects";
  import { has, t, type MessageKey } from "$lib/i18n";
  import { effects as freeze } from "$lib/state/effects.svelte";
  import { appSettings } from "$lib/state/settings.svelte";

  type Range = Pick<EffectParam, "min" | "max" | "logarithmic" | "unit">;

  const MIX: Range = { min: 0, max: 1, logarithmic: false, unit: "ratio" };
  const UNITS: Record<ReturnType<typeof display>["key"], MessageKey> = {
    percent: "effects.unit.percent",
    hertz: "effects.unit.hertz",
    kilohertz: "effects.unit.kilohertz",
    milliseconds: "effects.unit.milliseconds",
    seconds: "effects.unit.seconds",
    bits: "effects.unit.bits",
  };

  let catalog = $state.raw<EffectInfo[]>([]);
  const ordered = $derived([...catalog].sort((a, b) => a.position - b.position));
  const ranges = $derived(Object.fromEntries(catalog.map((info) => [info.id, info.params])));

  // What the controls show: what is being previewed, else the saved settings.
  let previewing = $state.raw<EffectsSettings | null>(null);
  const draft = $derived(previewing ?? appSettings.current.effects);

  onMount(() => {
    api.catalog().then(
      (list) => (catalog = list),
      () => {},
    );
    void freeze.refresh();
  });

  /** The settings shown with `change` made to a copy. */
  function edited(change: (next: EffectsSettings) => void) {
    const next = structuredClone(draft);
    change(next);
    return next;
  }

  // At most one preview a frame while a slider moves.
  let previewQueued = false;
  function preview(change: (next: EffectsSettings) => void) {
    previewing = edited(change);
    if (previewQueued) return;
    previewQueued = true;
    requestAnimationFrame(() => {
      previewQueued = false;
      if (previewing) void api.preview(previewing).catch(() => {});
    });
  }

  /** Saves `next` (the settings shown), which stay shown until the saved ones replace them. */
  async function save(next: EffectsSettings = draft) {
    previewing = next;
    const saved = await appSettings.save((settings) => (settings.effects = next));
    previewing = null;
    // Refused, it may still be playing as previewed: back to the saved settings.
    if (!saved) void api.preview(appSettings.current.effects).catch(() => {});
  }

  function choosePreset(id: string) {
    const defaults = appSettings.defaults?.effects;
    if (defaults && id in PRESETS) void save(applyPreset(id, defaults, ranges));
  }

  /** A message whose key is built from an id, or the id if there is none. */
  const text = (key: string) => (has(key) ? t(key) : key);

  function shown(value: number, range: Range) {
    const { key, value: number } = display(value, range.unit);
    return t(UNITS[key], { value: number });
  }
</script>

<label class="switch">
  <input
    type="checkbox"
    checked={appSettings.current.features.effects}
    disabled={appSettings.saving}
    onchange={(event) => {
      const on = event.currentTarget.checked;
      appSettings.save((next) => (next.features.effects = on));
    }}
  />
  <span>
    <span class="title">{t("effects.enabled")}</span>
    <span class="hint">{t("effects.enabledHint")}</span>
  </span>
</label>

<div class="chain" class:off={!appSettings.current.features.effects} inert={!appSettings.current.features.effects}>
  <p class="hint">{t("effects.hint")}</p>

  <label class="field">
    <span class="label">{t("effects.preset")}</span>
    <span class="control">
      <select
        value=""
        disabled={appSettings.saving || catalog.length === 0}
        onchange={(event) => {
          choosePreset(event.currentTarget.value);
          event.currentTarget.value = "";
        }}
      >
        <option value="" disabled>{t("effects.preset.choose")}</option>
        {#each Object.keys(PRESETS) as id (id)}
          <option value={id}>{text(`effects.preset.${id}`)}</option>
        {/each}
      </select>
    </span>
    <span class="hint">{t("effects.presetHint")}</span>
  </label>

  {#snippet slider(
    label: string,
    value: number,
    range: Range,
    set: (next: EffectsSettings, value: number) => void,
    hint?: string,
  )}
    <label class="field">
      <span class="label">{label}</span>
      <span class="control">
        <input
          type="range"
          min="0"
          max={STEPS}
          step="1"
          value={toPosition(value, range)}
          aria-valuetext={shown(value, range)}
          oninput={(event) => {
            const position = Number(event.currentTarget.value);
            preview((next) => set(next, fromPosition(position, range)));
          }}
          onchange={() => void save()}
        />
        <span class="value">{shown(value, range)}</span>
      </span>
      {#if hint}<span class="hint">{hint}</span>{/if}
    </label>
  {/snippet}

  {#each ordered as info (info.id)}
    {@const id = info.id as EffectId}
    {#if draft[id]}
      {@const effect = draft[id]}
      <section class="effect card" aria-labelledby="effect-{id}">
        <label class="switch">
          <input
            type="checkbox"
            checked={effect.enabled}
            disabled={appSettings.saving}
            onchange={(event) => {
              const on = event.currentTarget.checked;
              void save(edited((next) => (next[id].enabled = on)));
            }}
          />
          <span>
            <span class="title" id="effect-{id}">{text(`effects.${id}`)}</span>
            <span class="hint">{text(`effects.${id}About`)}</span>
          </span>
        </label>

        <fieldset disabled={!effect.enabled}>
          {#if id === "freeze"}
            <div class="hold">
              <button class:held={freeze.held} aria-pressed={freeze.held} onclick={() => freeze.toggleHold()}>
                {freeze.held ? t("effects.release") : t("effects.hold")}
              </button>
              <span class="hint" aria-live="polite">
                {effect.enabled
                  ? freeze.held
                    ? t("effects.freezeHeld")
                    : t("effects.holdHint")
                  : t("effects.freezeOff")}
              </span>
            </div>
          {/if}
          {@render slider(
            t("effects.mix"),
            effect.mix,
            MIX,
            (next, value) => (next[id].mix = value),
            t("effects.mixHint"),
          )}
          {#each info.params as param (param.id)}
            {@render slider(
              text(`effects.param.${param.id}`),
              effect.params[param.id] ?? param.defaultValue,
              param,
              (next, value) => (next[id].params[param.id] = value),
            )}
          {/each}
        </fieldset>
      </section>
    {/if}
  {/each}

  <div class="actions">
    <button onclick={() => appSettings.reset("effects")} disabled={appSettings.saving}>{t("effects.reset")}</button>
  </div>
</div>

<style>
  .chain.off {
    opacity: 0.55;
  }

  .effect {
    margin: 0.75rem 0;
    padding: 0.5rem 0.9rem 0.6rem;
  }

  fieldset {
    border: none;
    margin: 0;
    padding: 0 0 0 1.6rem;
    min-width: 0;
  }

  fieldset:disabled {
    opacity: 0.55;
  }

  .hold {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 0.75rem;
    padding: 0.4rem 0;
  }

  .hold button.held {
    background: var(--accent);
    color: var(--accent-text);
    border-color: var(--accent);
  }
</style>
