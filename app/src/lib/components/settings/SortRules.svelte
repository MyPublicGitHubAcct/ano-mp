<script lang="ts">
  // The sort and grouping rules (the sidebar's library views): pick one to
  // edit its name, what it groups by, how albums and tracks are ordered;
  // add, save, remove; and the leading words sorting skips. A rule's edits
  // are saved together with Save; the rest are saved at once.
  import { t } from "$lib/i18n";
  import { onMount } from "svelte";
  import { ask } from "@tauri-apps/plugin-dialog";
  import {
    library as api,
    type AlbumOrder,
    type Level,
    type SortRule,
    type SortSettings,
    type TrackKey,
  } from "$lib/api";
  import { library } from "$lib/state/library.svelte";
  import { attempt } from "$lib/state/toasts.svelte";
  import Icon from "../Icon.svelte";
  import OrderedChoices from "./OrderedChoices.svelte";

  const LEVELS: { id: Level; name: string }[] = [
    { id: "albumArtist", name: t("column.albumArtist") },
    { id: "artist", name: t("column.artist") },
    { id: "album", name: t("column.album") },
    { id: "genre", name: t("column.genre") },
    { id: "year", name: t("column.year") },
    { id: "folder", name: t("sort.folderAlone") },
    { id: "composer", name: t("column.composer") },
    { id: "work", name: t("sort.work") },
  ];

  const TRACK_KEYS: { id: TrackKey; name: string }[] = [
    { id: "albumArtist", name: t("column.albumArtist") },
    { id: "artist", name: t("column.artist") },
    { id: "album", name: t("column.album") },
    { id: "year", name: t("column.year") },
    { id: "discNumber", name: t("sort.discNumber") },
    { id: "trackNumber", name: t("column.trackNumber") },
    { id: "title", name: t("album.field.title") },
    { id: "path", name: t("sort.path") },
    { id: "dateAdded", name: t("sort.dateAddedNewest") },
    { id: "movement", name: t("sort.movement") },
  ];

  let data = $state.raw<SortSettings | null>(null);
  let selectedId = $state<string | null>(null);
  /** The rule being edited: a copy of the selected one, or a new one not saved yet. */
  let draft = $state<SortRule | null>(null);
  let isNew = $state(false);
  let articles = $state("");
  let busy = $state(false);

  const saved = $derived(data?.rules.find((rule) => rule.id === selectedId) ?? null);
  const dirty = $derived(draft !== null && (isNew || JSON.stringify(draft) !== JSON.stringify(saved)));
  const savedArticles = $derived(data?.ignoredArticles.join(" ") ?? "");
  /** Folder only goes on its own, so it's offered only to a rule without levels, and nothing is offered after it. */
  const levelOptions = $derived(
    draft === null || draft.levels.length === 0
      ? LEVELS
      : draft.levels.includes("folder")
        ? LEVELS.filter((level) => level.id === "folder")
        : LEVELS.filter((level) => level.id !== "folder"),
  );

  onMount(() => {
    attempt(async () => take(await api.sortSettings(), library.ruleId));
  });

  /** Shows `settings`, editing rule `id` (else the first). */
  function take(settings: SortSettings, id: string | null) {
    data = settings;
    const rule = settings.rules.find((candidate) => candidate.id === id) ?? settings.rules[0];
    selectedId = rule.id;
    draft = structuredClone(rule);
    isNew = false;
    articles = settings.ignoredArticles.join(" ");
    library.setRules(settings.rules);
  }

  /** Whether unsaved edits may be dropped. */
  async function mayDiscard() {
    if (!dirty) return true;
    return ask(t("sort.discardConfirm"), {
      title: t("sort.discardTitle"),
      kind: "warning",
      okLabel: t("sort.discard"),
    });
  }

  async function select(rule: SortRule) {
    if (rule.id === selectedId && !isNew) return;
    if (!(await mayDiscard())) return;
    selectedId = rule.id;
    draft = structuredClone(rule);
    isNew = false;
  }

  async function add() {
    if (!(await mayDiscard()) || !data) return;
    const ids = new Set(data.rules.map((rule) => rule.id));
    let n = 1;
    while (ids.has(`view-${n}`)) n++;
    selectedId = null;
    isNew = true;
    draft = {
      id: `view-${n}`,
      name: t("sort.newView"),
      levels: ["albumArtist", "album"],
      trackOrder: ["discNumber", "trackNumber", "title", "path"],
      albumOrder: "title",
    };
  }

  async function run(action: () => Promise<SortSettings>, id: string | null) {
    busy = true;
    try {
      const settings = await attempt(action);
      if (settings) take(settings, id);
    } finally {
      busy = false;
    }
  }

  function save() {
    const rule = draft;
    if (rule) run(() => api.saveSortRule($state.snapshot(rule)), rule.id);
  }

  async function remove() {
    if (!saved) return;
    const confirmed = await ask(t("sort.removeConfirm", { name: saved.name }), {
      title: t("sort.removeView"),
      kind: "warning",
      okLabel: t("services.remove"),
    });
    if (confirmed) run(() => api.removeSortRule(saved.id), null);
  }

  function revert() {
    if (isNew) {
      isNew = false;
      selectedId = data?.rules[0]?.id ?? null;
    }
    const rule = data?.rules.find((candidate) => candidate.id === selectedId);
    draft = rule ? structuredClone(rule) : null;
  }

  function saveArticles(event: SubmitEvent) {
    event.preventDefault();
    const words = articles.split(/[\s,]+/).filter(Boolean);
    run(() => api.setIgnoredArticles(words), selectedId);
  }

  async function resetAll() {
    const confirmed = await ask(t("sort.resetConfirm"), {
      title: t("sort.resetTitle"),
      kind: "warning",
      okLabel: t("services.reset"),
    });
    if (confirmed) run(() => api.resetSortSettings(), library.ruleId);
  }

  const nameOf = <T extends string>(list: { id: T; name: string }[], id: T) =>
    id === "folder" ? t("sort.folder") : (list.find((item) => item.id === id)?.name ?? id);

  const describe = (rule: SortRule) =>
    (rule.levels.length > 0 ? rule.levels.map((level) => nameOf(LEVELS, level)).join(" → ") : t("sort.allTracks")) +
    (rule.trackOrder.length > 0
      ? t("sort.tracksBy", { keys: rule.trackOrder.map((key) => nameOf(TRACK_KEYS, key)).join(", ") })
      : "");
</script>

{#if data && draft}
  <div class="editor">
    <div class="rules">
      <ul class="card" id="setting-libraryViews" aria-label={t("sort.views")}>
        {#each data.rules as rule (rule.id)}
          <li>
            <button class:selected={!isNew && rule.id === selectedId} onclick={() => select(rule)}>
              <span class="rule-name">{rule.name}</span>
              <span class="muted small">{describe(rule)}</span>
            </button>
          </li>
        {/each}
        {#if isNew}
          <li><button class="selected"><span class="rule-name">{draft.name || t("sort.newView")}</span></button></li>
        {/if}
      </ul>
      <button onclick={add} disabled={busy}><Icon name="plus" /> {t("sort.newView")}</button>
    </div>

    <form
      class="rule card"
      aria-label={t("sort.edit", { name: draft.name })}
      onsubmit={(event) => {
        event.preventDefault();
        save();
      }}
    >
      <label class="field stacked">
        <span class="label">{t("sort.name")}</span>
        <input type="text" bind:value={draft.name} maxlength="60" required />
      </label>

      <div class="field stacked">
        <span class="label">{t("sort.groupBy")}</span>
        <OrderedChoices
          options={levelOptions}
          value={draft.levels}
          onchange={(levels) => draft && (draft.levels = levels)}
          label={t("sort.levels")}
          addLabel={t("sort.addLevel")}
          empty={t("sort.noLevels")}
          disabled={busy}
        />
      </div>

      {#if draft.levels.includes("album")}
        <label class="field">
          <span class="label">{t("sort.albumsBy")}</span>
          <select bind:value={draft.albumOrder}>
            {#each [["title", t("album.field.title")], ["year", t("sort.yearOldest")], ["dateAdded", t("sort.dateAddedNewest")]] as [value, name] (value)}
              <option value={value as AlbumOrder}>{name}</option>
            {/each}
          </select>
        </label>
      {/if}

      <div class="field stacked">
        <span class="label">{t("sort.tracksSortedBy")}</span>
        <OrderedChoices
          options={TRACK_KEYS}
          value={draft.trackOrder}
          onchange={(order) => draft && (draft.trackOrder = order)}
          label={t("sort.trackOrder")}
          addLabel={t("sort.addKey")}
          empty={t("sort.noKeys")}
          disabled={busy}
        />
      </div>

      <div class="buttons">
        <button type="submit" class="primary" disabled={busy || !dirty || draft.name.trim() === ""}>
          {isNew ? t("sort.addView") : t("dialog.save")}
        </button>
        {#if dirty}<button type="button" onclick={revert} disabled={busy}
            >{isNew ? t("dialog.cancel") : t("sort.revert")}</button
          >{/if}
        {#if !isNew}
          <span class="spacer"></span>
          <button type="button" onclick={remove} disabled={busy || data.rules.length <= 1}
            >{t("sort.removeView")}</button
          >
        {/if}
      </div>
    </form>
  </div>

  <h3>{t("sort.skipped")}</h3>
  <form class="field articles" onsubmit={saveArticles}>
    <span class="control">
      <input
        id="setting-skippedWords"
        type="text"
        bind:value={articles}
        placeholder={t("sort.skippedPlaceholder")}
        aria-label={t("sort.skippedLabel")}
      />
      <button
        type="submit"
        disabled={busy ||
          articles
            .split(/[\s,]+/)
            .filter(Boolean)
            .join(" ") === savedArticles}
      >
        {t("dialog.save")}
      </button>
    </span>
    <span class="hint">
      {t("sort.skippedHint")}
    </span>
  </form>

  <div class="actions">
    <button onclick={resetAll} disabled={busy}>{t("settings.reset")}</button>
  </div>
{:else}
  <p class="muted">{t("common.loading")}</p>
{/if}

<style>
  .editor {
    display: grid;
    grid-template-columns: minmax(12rem, 16rem) minmax(0, 1fr);
    gap: 1rem;
    align-items: start;
  }

  .rules {
    display: flex;
    flex-direction: column;
    align-items: flex-start;
    gap: 0.5rem;
  }

  .rules ul {
    list-style: none;
    margin: 0;
    padding: 0.25rem;
    width: 100%;
  }

  .rules li button {
    display: flex;
    flex-direction: column;
    align-items: flex-start;
    width: 100%;
    border: none;
    background: none;
    text-align: left;
    padding: 0.35rem 0.5rem;
  }

  .rules li button:hover:not(:disabled) {
    background: var(--hover);
  }

  .rules li button.selected {
    background: var(--selected);
  }

  .rule-name {
    font-weight: 500;
  }

  .small {
    font-size: 0.78rem;
  }

  .rule {
    display: flex;
    flex-direction: column;
    gap: 0.25rem;
    padding: 0.75rem 1rem 1rem;
  }

  .rule input[type="text"] {
    max-width: 24rem;
  }

  .buttons {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 0.5rem;
    margin-top: 0.75rem;
  }

  .spacer {
    flex: 1;
  }

  .articles input[type="text"] {
    flex: 1 1 12rem;
    max-width: 20rem;
  }

  @media (max-width: 760px) {
    .editor {
      grid-template-columns: minmax(0, 1fr);
    }
  }
</style>
