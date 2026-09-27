<script lang="ts">
  // A smart playlist's rules (PLAN.md F2): its name, whether every
  // condition must hold or any one, the conditions (genre, year, format,
  // date added, favourite, rating, plays, not played lately, artist), the
  // order and an optional limit. A new one is made on Save; an existing one
  // takes the new rules.
  import { untrack } from "svelte";
  import type { Playlist, SmartCondition, SmartOrder, SmartRules } from "$lib/api";
  import { playlists as api } from "$lib/api";
  import { t, type MessageKey } from "$lib/i18n";
  import { collection } from "$lib/state/collection.svelte";
  import { attempt } from "$lib/state/toasts.svelte";
  import Dialog from "./Dialog.svelte";
  import Icon from "./Icon.svelte";

  let { playlist, onclose }: { playlist: Playlist | null; onclose: () => void } = $props();

  type Field = SmartCondition["field"];

  const FIELDS: { field: Field; label: MessageKey }[] = [
    { field: "genre", label: "smart.field.genre" },
    { field: "artist", label: "smart.field.artist" },
    { field: "year", label: "smart.field.year" },
    { field: "format", label: "smart.field.format" },
    { field: "addedWithin", label: "smart.field.addedWithin" },
    { field: "favourite", label: "smart.field.favourite" },
    { field: "rating", label: "smart.field.rating" },
    { field: "playCount", label: "smart.field.playCount" },
    { field: "notPlayedFor", label: "smart.field.notPlayedFor" },
  ];

  const ORDERS: { order: SmartOrder; label: MessageKey }[] = [
    { order: "random", label: "smart.order.random" },
    { order: "dateAdded", label: "smart.order.dateAdded" },
    { order: "mostPlayed", label: "smart.order.mostPlayed" },
    { order: "lastPlayed", label: "smart.order.lastPlayed" },
    { order: "rating", label: "smart.order.rating" },
    { order: "album", label: "smart.order.album" },
    { order: "title", label: "smart.order.title" },
  ];

  function blank(field: Field): SmartCondition {
    switch (field) {
      case "genre":
      case "artist":
        return { field, value: "" };
      case "format":
        return { field, value: "flac" };
      case "year":
        return { field, from: 1990, to: 1999 };
      case "addedWithin":
        return { field, days: 30 };
      case "notPlayedFor":
        return { field, days: 90 };
      case "favourite":
        return { field, value: true };
      case "rating":
        return { field, atLeast: 4 };
      case "playCount":
        return { field, atLeast: 1, atMost: null };
    }
  }

  // The dialog edits a copy of the rules it opened with.
  const opened = untrack(() => playlist);
  const initial: SmartRules = opened?.rules ?? {
    matchAll: true,
    conditions: [blank("genre")],
    order: "random",
    limit: 100,
    seed: Math.floor(Math.random() * 1_000_000),
  };

  let name = $state(opened?.name ?? t("smart.newName"));
  let rules = $state<SmartRules>(structuredClone(initial));
  let limited = $state(initial.limit !== null);
  let matching = $state<number | null>(null);
  let saving = $state(false);

  const number = (value: string) => (value.trim() === "" ? null : Math.max(0, Math.round(Number(value))));

  function setField(index: number, field: Field) {
    rules.conditions[index] = blank(field);
  }

  const finished = $derived<SmartRules>({ ...rules, limit: limited ? (rules.limit ?? 100) : null });

  async function save() {
    saving = true;
    try {
      if (playlist) {
        const renamed = name.trim() !== playlist.name && (await attempt(() => api.rename(playlist.id, name)));
        const saved = await attempt(() => api.setRules(playlist.id, finished));
        if (saved || renamed) await collection.refresh();
        if (saved) onclose();
      } else if (await collection.createSmart(name, finished)) {
        onclose();
      }
    } finally {
      saving = false;
    }
  }

  // How many match, as the rules change.
  $effect(() => {
    const snapshot = $state.snapshot(finished);
    const timer = setTimeout(async () => {
      matching = await api.preview(snapshot).catch(() => null);
    }, 300);
    return () => clearTimeout(timer);
  });
</script>

<Dialog title={playlist ? t("smart.editTitle") : t("smart.newTitle")} {onclose}>
  <label class="row">
    <span>{t("smart.name")}</span>
    <input type="text" bind:value={name} />
  </label>

  <div class="row">
    <span>{t("smart.match")}</span>
    <select
      value={rules.matchAll ? "all" : "any"}
      onchange={(event) => (rules.matchAll = event.currentTarget.value === "all")}
      aria-label={t("smart.match")}
    >
      <option value="all">{t("smart.matchAll")}</option>
      <option value="any">{t("smart.matchAny")}</option>
    </select>
  </div>

  <ul class="conditions">
    {#each rules.conditions as condition, index (index)}
      <li>
        <select
          value={condition.field}
          aria-label={t("smart.fieldLabel", { n: index + 1 })}
          onchange={(event) => setField(index, event.currentTarget.value as Field)}
        >
          {#each FIELDS as option (option.field)}
            <option value={option.field}>{t(option.label)}</option>
          {/each}
        </select>
        {#if condition.field === "genre" || condition.field === "artist"}
          <span class="muted">{t("smart.is")}</span>
          <input type="text" bind:value={condition.value} aria-label={t("smart.value")} />
        {:else if condition.field === "format"}
          <span class="muted">{t("smart.is")}</span>
          <select bind:value={condition.value} aria-label={t("smart.value")}>
            {#each ["flac", "mp3", "m4a", "ogg", "opus", "wav", "aiff", "wma", "ape", "wv"] as format (format)}
              <option value={format}>{format.toUpperCase()}</option>
            {/each}
          </select>
        {:else if condition.field === "year"}
          <span class="muted">{t("smart.from")}</span>
          <input
            type="number"
            value={condition.from ?? ""}
            aria-label={t("smart.from")}
            oninput={(event) => condition.field === "year" && (condition.from = number(event.currentTarget.value))}
          />
          <span class="muted">{t("smart.to")}</span>
          <input
            type="number"
            value={condition.to ?? ""}
            aria-label={t("smart.to")}
            oninput={(event) => condition.field === "year" && (condition.to = number(event.currentTarget.value))}
          />
        {:else if condition.field === "addedWithin" || condition.field === "notPlayedFor"}
          <input type="number" min="1" bind:value={condition.days} aria-label={t("smart.days")} />
          <span class="muted">{t("smart.days")}</span>
        {:else if condition.field === "favourite"}
          <select
            value={condition.value ? "yes" : "no"}
            aria-label={t("smart.value")}
            onchange={(event) => condition.field === "favourite" && (condition.value = event.currentTarget.value === "yes")}
          >
            <option value="yes">{t("smart.yes")}</option>
            <option value="no">{t("smart.no")}</option>
          </select>
        {:else if condition.field === "rating"}
          <span class="muted">{t("smart.atLeast")}</span>
          <select bind:value={condition.atLeast} aria-label={t("smart.value")}>
            {#each [1, 2, 3, 4, 5] as stars (stars)}
              <option value={stars}>{"★".repeat(stars)}</option>
            {/each}
          </select>
        {:else if condition.field === "playCount"}
          <span class="muted">{t("smart.from")}</span>
          <input
            type="number"
            min="0"
            value={condition.atLeast ?? ""}
            aria-label={t("smart.atLeast")}
            oninput={(event) => condition.field === "playCount" && (condition.atLeast = number(event.currentTarget.value))}
          />
          <span class="muted">{t("smart.to")}</span>
          <input
            type="number"
            min="0"
            value={condition.atMost ?? ""}
            aria-label={t("smart.atMost")}
            oninput={(event) => condition.field === "playCount" && (condition.atMost = number(event.currentTarget.value))}
          />
        {/if}
        <button
          class="icon"
          aria-label={t("smart.removeCondition")}
          title={t("smart.removeCondition")}
          onclick={() => rules.conditions.splice(index, 1)}><Icon name="close" size="1rem" /></button
        >
      </li>
    {/each}
  </ul>
  <button disabled={rules.conditions.length >= 20} onclick={() => rules.conditions.push(blank("genre"))}>
    <Icon name="plus" size="1rem" />
    {t("smart.addCondition")}
  </button>

  <div class="row">
    <span>{t("smart.orderLabel")}</span>
    <select bind:value={rules.order}>
      {#each ORDERS as option (option.order)}
        <option value={option.order}>{t(option.label)}</option>
      {/each}
    </select>
  </div>
  <div class="row">
    <label><input type="checkbox" bind:checked={limited} /> {t("smart.limit")}</label>
    {#if limited}
      <input type="number" min="1" bind:value={rules.limit} aria-label={t("smart.limitCount")} />
      <span class="muted">{t("smart.tracks")}</span>
    {/if}
  </div>
  <p class="muted" role="status">
    {matching === null ? "" : t("smart.matching", { count: matching })}
  </p>

  {#snippet actions()}
    <button onclick={onclose}>{t("dialog.cancel")}</button>
    <button class="primary" disabled={saving || name.trim() === ""} onclick={save}>{t("dialog.save")}</button>
  {/snippet}
</Dialog>

<style>
  .row {
    display: flex;
    align-items: center;
    gap: 0.75rem;
    margin: 0.75rem 0;
  }

  .row > span:first-child {
    min-width: 6rem;
  }

  .row input[type="text"] {
    flex: 1;
  }

  .conditions {
    list-style: none;
    margin: 0 0 0.5rem;
    padding: 0;
    display: flex;
    flex-direction: column;
    gap: 0.4rem;
  }

  .conditions li {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 0.5rem;
  }

  .conditions input[type="number"] {
    width: 6rem;
  }

  .conditions input[type="text"] {
    flex: 1;
    min-width: 8rem;
  }
</style>
