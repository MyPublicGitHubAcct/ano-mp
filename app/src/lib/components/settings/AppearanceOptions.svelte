<script lang="ts">
  // The look of the app (PLAN.md X1): pick a built-in or saved theme, or
  // edit one, with the whole app as the live preview. Colours and sliders
  // show as they move and are saved when let go; pairs under WCAG AA are
  // flagged. Themes are saved by name, and export to and import from JSON
  // files (`theme.rs`).
  import { open, save } from "@tauri-apps/plugin-dialog";
  import { onDestroy } from "svelte";
  import { themes as api, type Density, type Font, type Scheme, type Theme, type ThemePalette } from "$lib/api";
  import { count, t, type MessageKey } from "$lib/i18n";
  import {
    BUILT_IN,
    COLOR_TOKENS,
    RADII,
    TEXT_SIZES,
    contrastIssues,
    isDark,
    sameLook,
    type ContrastIssue,
  } from "$lib/theme";
  import { appearance } from "$lib/state/appearance.svelte";
  import { appSettings } from "$lib/state/settings.svelte";
  import { attempt, toasts } from "$lib/state/toasts.svelte";
  import Icon from "../Icon.svelte";

  const SCHEMES: Scheme[] = ["system", "light", "dark"];
  const FONT_CHOICES: Font[] = ["system", "rounded", "serif", "mono", "humanist"];
  const DENSITIES: Density[] = ["compact", "regular", "roomy"];
  const SIDES = ["light", "dark"] as const;
  /** As `theme.rs`'s `MAX_SAVED`. */
  const MAX_SAVED = 50;
  const HEX = /^#[0-9a-f]{6}$/;

  const settings = $derived(appSettings.current.appearance);
  /** The theme on screen: the one being edited, or the saved one. */
  const shown = $derived(appearance.preview ?? settings.theme);
  /** Which palette the colour fields edit, while the theme follows the system. */
  let side = $state<"light" | "dark">(appearance.systemDark ? "dark" : "light");
  const editing = $derived<"light" | "dark">(
    shown.scheme === "system" ? side : isDark(shown, appearance.systemDark) ? "dark" : "light",
  );
  const palette = $derived(shown[editing]);
  const issues = $derived(contrastIssues(palette));
  /** The text size while its slider moves. Unlike the other sliders it isn't previewed: the whole layout,
      the slider too, would resize under the pointer. */
  let sizing = $state<number | null>(null);
  /** The name field: the theme's, until the user types another. */
  let name = $derived(settings.theme.name);

  onDestroy(() => (appearance.preview = null));

  const themeName = (theme: Theme) =>
    theme.preset === "custom" ? theme.name : t(`theme.${theme.preset}` as MessageKey);

  /** Whether `theme` (a built-in or saved one) is the one in use. */
  function isCurrent(theme: Theme, builtIn: boolean) {
    const current = settings.theme;
    if (!sameLook(theme, current)) return false;
    return builtIn ? current.preset === theme.preset : current.preset === "custom" && current.name === theme.name;
  }

  const inList = $derived(
    BUILT_IN.some((theme) => isCurrent(theme, true)) || settings.saved.some((theme) => isCurrent(theme, false)),
  );

  /** The theme on screen with `change` made; a built-in one becomes the user's own. */
  function edited(change: (theme: Theme) => void): Theme {
    const next = structuredClone($state.snapshot(shown)) as Theme;
    if (next.preset !== "custom") {
      next.preset = "custom";
      next.name = t("appearance.myTheme");
    }
    change(next);
    return next;
  }

  /** Shows the change without saving it (while a colour or slider moves). */
  function preview(change: (theme: Theme) => void) {
    appearance.preview = edited(change);
  }

  /** Saves the change; the saved theme replaces the preview. */
  async function commit(change: (theme: Theme) => void) {
    const next = edited(change);
    appearance.preview = next;
    await appSettings.save((all) => (all.appearance.theme = next));
    appearance.preview = null;
  }

  function choose(theme: Theme) {
    appearance.preview = null;
    appSettings.save((all) => (all.appearance.theme = structuredClone($state.snapshot(theme)) as Theme));
  }

  /** `theme` kept under its name in the saved themes, replacing one of the same name. */
  function keep(saved: Theme[], theme: Theme): Theme[] {
    const key = theme.name.trim().toLowerCase();
    return [...saved.filter((other) => other.name.trim().toLowerCase() !== key), theme].slice(-MAX_SAVED);
  }

  async function saveTheme() {
    const title = name.trim();
    if (title === "") return;
    const theme = { ...(structuredClone($state.snapshot(settings.theme)) as Theme), name: title, preset: "custom" };
    const saved = await appSettings.save((all) => {
      all.appearance.theme = theme;
      all.appearance.saved = keep(all.appearance.saved, theme);
    });
    if (saved) toasts.show(t("appearance.saved", { name: title }), "info");
  }

  function forget(theme: Theme) {
    appSettings.save((all) => {
      all.appearance.saved = all.appearance.saved.filter((other) => other.name !== theme.name);
    });
  }

  const exportTheme = () =>
    attempt(async () => {
      const theme = settings.theme;
      const path = await save({
        defaultPath: `${themeName(theme).replace(/[/\\:]/g, "-")}.json`,
        filters: [{ name: t("appearance.fileType"), extensions: ["json"] }],
      });
      if (path === null) return;
      await api.export(path, theme);
      toasts.show(t("appearance.exported", { name: themeName(theme) }), "info");
    });

  const importTheme = () =>
    attempt(async () => {
      const path = await open({ multiple: false, filters: [{ name: t("appearance.fileType"), extensions: ["json"] }] });
      if (path === null) return;
      const theme = await api.import(path);
      theme.preset = "custom";
      await appSettings.save((all) => {
        all.appearance.theme = theme;
        all.appearance.saved = keep(all.appearance.saved, theme);
      });
      toasts.show(t("appearance.imported", { name: theme.name }), "info");
    });

  const flags = (token: keyof ThemePalette): ContrastIssue[] => issues.filter((issue) => issue.fg === token);
  const tokenName = (token: keyof ThemePalette) => t(`appearance.color.${token}` as MessageKey);
  const issueText = (issue: ContrastIssue) =>
    t("appearance.lowContrast", {
      ratio: issue.ratio.toFixed(1),
      background: tokenName(issue.bg),
      minimum: issue.minimum,
    });
</script>

<h3>{t("appearance.themes")}</h3>
<div class="themes" id="setting-theme" role="radiogroup" aria-label={t("appearance.themes")}>
  {#each BUILT_IN as theme (theme.preset)}
    {@render card(theme, true)}
  {/each}
  {#each settings.saved as theme (theme.name)}
    {@render card(theme, false)}
  {/each}
</div>
{#if !inList}
  <p class="hint">{t("appearance.unsaved")}</p>
{/if}

{#snippet card(theme: Theme, builtIn: boolean)}
  {@const colors = theme[isDark(theme, appearance.systemDark) ? "dark" : "light"]}
  <div class="theme" class:current={isCurrent(theme, builtIn)}>
    <button
      class="pick"
      role="radio"
      aria-checked={isCurrent(theme, builtIn)}
      disabled={appSettings.saving}
      onclick={() => choose(theme)}
    >
      <span
        class="swatch"
        style:background={colors.bg}
        style:border-color={colors.border}
        style:border-radius="{theme.radius}px"
      >
        <span class="bar" style:background={colors.surface}>
          <span class="line" style:background={colors.text}></span>
          <span class="line short" style:background={colors.textMuted}></span>
        </span>
        <span class="dot" style:background={colors.accent}></span>
      </span>
      <span class="name">{themeName(theme)}</span>
    </button>
    {#if !builtIn}
      <button
        class="icon forget"
        aria-label={t("appearance.forget", { name: theme.name })}
        title={t("appearance.forgetShort")}
        disabled={appSettings.saving}
        onclick={() => forget(theme)}
      >
        <Icon name="close" size="0.8rem" />
      </button>
    {/if}
  </div>
{/snippet}

<div class="field">
  <label class="label" for="theme-name">{t("appearance.name")}</label>
  <span class="control">
    <input id="theme-name" type="text" maxlength="64" bind:value={name} />
    <button onclick={saveTheme} disabled={appSettings.saving || name.trim() === ""}>{t("appearance.save")}</button>
    <button onclick={exportTheme}><Icon name="export" /> {t("appearance.export")}</button>
    <button onclick={importTheme}><Icon name="import" /> {t("appearance.import")}</button>
  </span>
  <p class="hint">{t("appearance.nameHint")}</p>
</div>

<h3>{t("appearance.colours")}</h3>
<div class="field">
  <label class="label" for="theme-scheme">{t("appearance.scheme")}</label>
  <span class="control">
    <select
      id="theme-scheme"
      value={shown.scheme}
      disabled={appSettings.saving}
      onchange={(event) => {
        const scheme = event.currentTarget.value as Scheme;
        commit((theme) => (theme.scheme = scheme));
      }}
    >
      {#each SCHEMES as scheme (scheme)}
        <option value={scheme}>{t(`appearance.scheme.${scheme}`)}</option>
      {/each}
    </select>
    {#if shown.scheme === "system"}
      <span class="sides" role="group" aria-label={t("appearance.editing")}>
        {#each SIDES as option (option)}
          <button class:on={side === option} aria-pressed={side === option} onclick={() => (side = option)}>
            {t(`appearance.edit.${option}`)}
          </button>
        {/each}
      </span>
    {/if}
  </span>
</div>

<div class="editor">
  <div class="colors">
    {#each COLOR_TOKENS as token (token)}
      {@const problems = flags(token)}
      <div class="color">
        <input
          type="color"
          id="theme-{token}"
          value={palette[token]}
          disabled={appSettings.saving}
          oninput={(event) => {
            const value = event.currentTarget.value.toLowerCase();
            preview((theme) => (theme[editing][token] = value));
          }}
          onchange={(event) => {
            const value = event.currentTarget.value.toLowerCase();
            commit((theme) => (theme[editing][token] = value));
          }}
        />
        <label for="theme-{token}">{tokenName(token)}</label>
        <input
          class="hex"
          type="text"
          aria-label={t("appearance.hex", { name: tokenName(token) })}
          value={palette[token]}
          maxlength="7"
          spellcheck="false"
          onchange={(event) => {
            const value = event.currentTarget.value.trim().toLowerCase();
            if (HEX.test(value)) commit((theme) => (theme[editing][token] = value));
            else event.currentTarget.value = palette[token];
          }}
        />
        {#if problems.length > 0}
          <span class="flag" title={problems.map(issueText).join("\n")}>
            <Icon name="warning" size="0.9rem" />
            <span>{issueText(problems[0])}</span>
          </span>
        {/if}
      </div>
    {/each}
  </div>

  <div
    class="sample"
    aria-label={t("appearance.preview")}
    role="img"
    style:--bg={palette.bg}
    style:--surface={palette.surface}
    style:--surface-2={palette.surface2}
    style:--text={palette.text}
    style:--text-muted={palette.textMuted}
    style:--text-faint={palette.textFaint}
    style:--border={palette.border}
    style:--accent={palette.accent}
    style:--accent-text={palette.accentText}
    style:--danger={palette.danger}
    style:--heart={palette.heart}
    style:--star={palette.star}
  >
    <div class="sample-card">
      <div class="sample-row playing">
        <span class="sample-title">{t("appearance.sample.title")}</span>
        <span class="sample-heart"><Icon name="heart" size="0.9rem" /></span>
      </div>
      <div class="sample-row">
        <span class="sample-text">{t("appearance.sample.track")}</span>
        <span class="sample-muted">{t("appearance.sample.artist")}</span>
        <span class="sample-star"><Icon name="star" size="0.9rem" /></span>
      </div>
      <div class="sample-row raised">
        <span class="sample-muted">{t("appearance.sample.muted")}</span>
        <span class="sample-faint"><Icon name="more" size="0.9rem" /></span>
      </div>
      <div class="sample-row">
        <span class="sample-link">{t("appearance.sample.link")}</span>
        <span class="sample-danger">{t("appearance.sample.danger")}</span>
      </div>
      <span class="sample-button">{t("appearance.sample.button")}</span>
    </div>
  </div>
</div>
<p class="hint contrast" class:bad={issues.length > 0} role="status">
  {issues.length === 0 ? t("appearance.contrastOk") : count("appearance.contrastIssues", issues.length)}
</p>

<label class="switch">
  <input
    id="setting-accentFromCover"
    type="checkbox"
    checked={shown.accentFromCover}
    disabled={appSettings.saving}
    onchange={(event) => {
      const on = event.currentTarget.checked;
      commit((theme) => (theme.accentFromCover = on));
    }}
  />
  <span>
    <span class="title">{t("appearance.accentFromCover")}</span>
    <span class="hint">{t("appearance.accentFromCoverHint")}</span>
  </span>
</label>
<label class="switch">
  <input
    id="setting-systemContrast"
    type="checkbox"
    checked={settings.followSystemContrast}
    disabled={appSettings.saving}
    onchange={(event) => {
      const on = event.currentTarget.checked;
      appSettings.save((all) => (all.appearance.followSystemContrast = on));
    }}
  />
  <span>
    <span class="title">{t("appearance.systemContrast")}</span>
    <span class="hint">{t("appearance.systemContrastHint")}</span>
  </span>
</label>

<h3>{t("appearance.textAndShape")}</h3>
<div class="field">
  <label class="label" for="theme-font">{t("appearance.font")}</label>
  <span class="control">
    <select
      id="theme-font"
      value={shown.font}
      disabled={appSettings.saving}
      onchange={(event) => {
        const font = event.currentTarget.value as Font;
        commit((theme) => (theme.font = font));
      }}
    >
      {#each FONT_CHOICES as font (font)}
        <option value={font}>{t(`appearance.font.${font}`)}</option>
      {/each}
    </select>
  </span>
</div>
<div class="field">
  <label class="label" for="theme-size">{t("appearance.textSize")}</label>
  <span class="control">
    <input
      id="theme-size"
      type="range"
      min={TEXT_SIZES.min}
      max={TEXT_SIZES.max}
      step="1"
      value={sizing ?? shown.textSize}
      disabled={appSettings.saving}
      oninput={(event) => (sizing = Number(event.currentTarget.value))}
      onchange={async (event) => {
        const size = Number(event.currentTarget.value);
        await commit((theme) => (theme.textSize = size));
        sizing = null;
      }}
    />
    <span class="value">{t("appearance.px", { value: sizing ?? shown.textSize })}</span>
  </span>
  <p class="hint">{t("appearance.textSizeHint")}</p>
</div>
<div class="field">
  <label class="label" for="theme-density">{t("appearance.density")}</label>
  <span class="control">
    <select
      id="theme-density"
      value={shown.density}
      disabled={appSettings.saving}
      onchange={(event) => {
        const density = event.currentTarget.value as Density;
        commit((theme) => (theme.density = density));
      }}
    >
      {#each DENSITIES as density (density)}
        <option value={density}>{t(`appearance.density.${density}`)}</option>
      {/each}
    </select>
  </span>
</div>
<div class="field">
  <label class="label" for="theme-radius">{t("appearance.radius")}</label>
  <span class="control">
    <input
      id="theme-radius"
      type="range"
      min={RADII.min}
      max={RADII.max}
      step="1"
      value={shown.radius}
      disabled={appSettings.saving}
      oninput={(event) => {
        const radius = Number(event.currentTarget.value);
        preview((theme) => (theme.radius = radius));
      }}
      onchange={(event) => {
        const radius = Number(event.currentTarget.value);
        commit((theme) => (theme.radius = radius));
      }}
    />
    <span class="value">{t("appearance.px", { value: shown.radius })}</span>
  </span>
</div>

<div class="actions">
  <button onclick={() => choose(BUILT_IN[0])} disabled={appSettings.saving}>{t("appearance.useDefault")}</button>
</div>

<style>
  .themes {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(7.5rem, 1fr));
    gap: 0.6rem;
    max-width: 44rem;
  }

  .theme {
    position: relative;
  }

  .pick {
    flex-direction: column;
    align-items: stretch;
    gap: 0.35rem;
    width: 100%;
    padding: 0.4rem;
    background: var(--surface);
  }

  .theme.current .pick {
    border-color: var(--accent);
    box-shadow: 0 0 0 1px var(--accent);
  }

  .swatch {
    position: relative;
    display: block;
    height: 3.2rem;
    border: 1px solid;
    overflow: hidden;
  }

  .bar {
    position: absolute;
    inset: 0.45rem 1.8rem 0.45rem 0.45rem;
    display: flex;
    flex-direction: column;
    justify-content: center;
    gap: 0.3rem;
    padding: 0 0.4rem;
    border-radius: 3px;
  }

  .line {
    display: block;
    height: 0.25rem;
    border-radius: 999px;
  }

  .line.short {
    width: 60%;
  }

  .dot {
    position: absolute;
    right: 0.45rem;
    top: calc(50% - 0.5rem);
    width: 1rem;
    height: 1rem;
    border-radius: 50%;
  }

  .name {
    font-size: 0.85rem;
    text-align: center;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .forget {
    position: absolute;
    top: 0.1rem;
    right: 0.1rem;
    padding: 0.15rem;
    background: var(--surface);
  }

  .sides {
    display: inline-flex;
  }

  .sides button {
    border-radius: 0;
  }

  .sides button:first-child {
    border-radius: var(--radius) 0 0 var(--radius);
  }

  .sides button:last-child {
    border-radius: 0 var(--radius) var(--radius) 0;
    margin-left: -1px;
  }

  .sides button.on {
    background: var(--selected);
    color: var(--accent);
  }

  .editor {
    display: flex;
    flex-wrap: wrap;
    gap: 1rem 1.5rem;
    align-items: flex-start;
  }

  .colors {
    display: flex;
    flex-direction: column;
    gap: 0.25rem;
    flex: 1 1 22rem;
  }

  .color {
    display: grid;
    grid-template-columns: 2rem 9rem 5.5rem;
    align-items: center;
    gap: 0.25rem 0.6rem;
  }

  .color input[type="color"] {
    width: 2rem;
    height: 1.6rem;
    padding: 0;
    border: 1px solid var(--border);
    border-radius: var(--radius-sm);
    background: none;
    cursor: pointer;
  }

  .hex {
    width: 5.5rem;
    font-family: ui-monospace, monospace;
    font-size: 0.85rem;
  }

  .flag {
    grid-column: 2 / -1;
    display: flex;
    align-items: center;
    gap: 0.3rem;
    color: var(--danger);
    font-size: 0.8rem;
  }

  .sample {
    flex: 0 1 17rem;
    padding: 0.75rem;
    border: 1px solid var(--border);
    border-radius: var(--radius-lg);
    background: var(--bg);
    color: var(--text);
  }

  .sample-card {
    display: flex;
    flex-direction: column;
    gap: 0.35rem;
    padding: 0.6rem;
    border: 1px solid var(--border);
    border-radius: var(--radius);
    background: var(--surface);
  }

  .sample-row {
    display: flex;
    align-items: center;
    gap: 0.5rem;
    padding: 0.25rem 0.4rem;
    border-radius: var(--radius-sm);
    font-size: 0.9rem;
  }

  .sample-row.raised {
    background: var(--surface-2);
  }

  .sample-title {
    flex: 1;
    color: var(--accent);
    font-weight: 600;
  }

  .sample-text {
    flex: 1;
  }

  .sample-muted {
    color: var(--text-muted);
  }

  .sample-row.raised .sample-muted {
    flex: 1;
  }

  .sample-faint {
    color: var(--text-faint);
    display: inline-flex;
  }

  .sample-heart {
    color: var(--heart);
    display: inline-flex;
  }

  .sample-star {
    color: var(--star);
    display: inline-flex;
  }

  .sample-link {
    flex: 1;
    color: var(--accent);
    text-decoration: underline;
  }

  .sample-danger {
    color: var(--danger);
  }

  .sample-button {
    align-self: flex-start;
    margin-top: 0.25rem;
    padding: 0.3rem 0.8rem;
    border-radius: var(--radius);
    background: var(--accent);
    color: var(--accent-text);
    font-size: 0.9rem;
  }

  .contrast {
    margin-top: 0.5rem;
  }

  .contrast.bad {
    color: var(--danger);
  }
</style>
