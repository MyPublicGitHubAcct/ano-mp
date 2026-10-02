<script lang="ts">
  // The listening history (O8): the top 20 tracks, albums or artists of a
  // year or a month, stepping back and forward, with "Play these" (O19);
  // and everything played recently, newest first, runs from one album
  // together, each playable again (O16).
  import { untrack } from "svelte";
  import { features as api, queue, type RecentEntry, type TopKind, type TopPlayed } from "$lib/api";
  import { FOLDER_SHORT, unavailableEntryState } from "$lib/folders";
  import { formatDay } from "$lib/format";
  import { count, t } from "$lib/i18n";
  import { features } from "$lib/state/features.svelte";
  import { adHocRule, library } from "$lib/state/library.svelte";
  import { attempt } from "$lib/state/toasts.svelte";
  import { ui } from "$lib/state/ui.svelte";
  import Icon from "./Icon.svelte";

  const f = $derived(features.on);
  const KINDS: { id: TopKind; name: string }[] = [
    { id: "tracks", name: t("history.tracks") },
    { id: "albums", name: t("history.albums") },
    { id: "artists", name: t("history.artists") },
  ];

  const today = new Date();
  let kind = $state<TopKind>("tracks");
  let year = $state(today.getFullYear());
  /** 1–12, or null for the whole year. */
  let month = $state<number | null>(null);
  let top = $state.raw<TopPlayed | null>(null);
  let recent = $state.raw<RecentEntry[] | null>(null);

  $effect(() => {
    void [kind, year, month, features.historyVersion, library.version, f.topPlayed, f.recentlyPlayed];
    untrack(async () => {
      top = f.topPlayed ? await api.topPlayed(kind, year, month).catch(() => null) : null;
      recent = f.recentlyPlayed ? await api.recentlyPlayed(100).catch(() => null) : null;
    });
  });

  const periodName = $derived(
    month === null
      ? String(year)
      : new Date(year, month - 1, 1).toLocaleDateString(undefined, { month: "long", year: "numeric" }),
  );

  /** The period `by` steps from this one. */
  function stepped(by: number): { year: number; month: number | null } {
    if (month === null) return { year: year + by, month: null };
    const index = year * 12 + (month - 1) + by;
    return { year: Math.floor(index / 12), month: (index % 12) + 1 };
  }

  const step = (by: number) => ({ year, month } = stepped(by));

  /** Only as far as there are plays, and this year. */
  const canStep = (by: number) => {
    const first = Math.min(top?.years?.[0] ?? today.getFullYear(), today.getFullYear());
    const target = stepped(by);
    return target.year >= first && target.year <= today.getFullYear();
  };

  const playThese = () => {
    const ids = (top?.entries ?? []).flatMap((entry) => entry.trackIds);
    if (ids.length > 0) attempt(() => queue.play(ids, 0));
  };

  const albumRule = adHocRule(["album"]);
  const playEntry = (entry: RecentEntry) =>
    attempt(() =>
      entry.album
        ? queue.playNode(albumRule, [entry.album.id], true)
        : queue.play(
            entry.tracks.map((track) => track.trackId),
            0,
          ),
    );

  const time = (seconds: number) =>
    new Date(seconds * 1000).toLocaleTimeString(undefined, { hour: "numeric", minute: "2-digit" });
</script>

<section class="history" aria-labelledby="history-heading">
  <h1 id="history-heading">{t("history.title")}</h1>

  {#if !f.listeningHistory}
    <p class="muted">
      {t("history.off")}
      <button class="link" onclick={() => ui.showSettings("features")}>{t("health.settingsFeatures")}</button>.
    </p>
  {/if}

  {#if f.listeningHistory && f.topPlayed}
    <div class="top-head">
      <h2>{t("history.top", { period: periodName })}</h2>
      <div class="controls">
        <div class="segmented" role="radiogroup" aria-label={t("history.whatToCount")}>
          {#each KINDS as option (option.id)}
            <button
              role="radio"
              aria-checked={kind === option.id}
              class:on={kind === option.id}
              onclick={() => (kind = option.id)}
            >
              {option.name}
            </button>
          {/each}
        </div>
        <select
          aria-label={t("history.period")}
          value={month === null ? "year" : "month"}
          onchange={(event) => (month = event.currentTarget.value === "year" ? null : today.getMonth() + 1)}
        >
          <option value="year">{t("history.year")}</option>
          <option value="month">{t("history.month")}</option>
        </select>
        <button
          class="icon"
          aria-label={t("history.earlier")}
          title={t("history.earlier")}
          disabled={!canStep(-1)}
          onclick={() => step(-1)}>‹</button
        >
        <button
          class="icon"
          aria-label={t("history.later")}
          title={t("history.later")}
          disabled={!canStep(1)}
          onclick={() => step(1)}>›</button
        >
        <button class="primary" disabled={!top || top.entries.length === 0} onclick={playThese}>
          <Icon name="play" />
          {t("history.playThese")}
        </button>
      </div>
    </div>
    {#if top}
      {#if top.entries.length === 0}
        <p class="muted">{t("history.nothingIn", { period: periodName })}</p>
      {:else}
        <p class="muted small">
          {t("history.playsIn", { plays: count("count.plays", top.plays), period: periodName })}
        </p>
        <ol class="top">
          {#each top.entries as entry, index (entry.id)}
            {@const unavailable = unavailableEntryState(library.unreadable, entry.folderIds)}
            <li class:unavailable title={unavailable ? t(FOLDER_SHORT[unavailable]) : undefined}>
              <span class="rank">{index + 1}</span>
              <span class="text">
                <span class="name">{entry.title}</span>
                {#if entry.subtitle}<span class="muted small">{entry.subtitle}</span>{/if}
              </span>
              <span class="muted small">{count("count.plays", entry.plays)}</span>
              <button
                class="icon"
                title={t("menu.play")}
                aria-label={t("library.playName", { name: entry.title })}
                onclick={() => attempt(() => queue.play(entry.trackIds, 0))}
              >
                <Icon name="play" />
              </button>
            </li>
          {/each}
        </ol>
      {/if}
    {/if}
  {/if}

  {#if f.listeningHistory && f.recentlyPlayed && recent}
    <h2>{t("history.recent")}</h2>
    {#if recent.length === 0}
      <p class="muted">{t("history.nothingYet")}</p>
    {:else}
      <ul class="recent">
        {#each recent as entry (entry.playedAt)}
          {@const unavailable = unavailableEntryState(
            library.unreadable,
            entry.tracks.map((track) => track.folderId),
          )}
          <li class:unavailable title={unavailable ? t(FOLDER_SHORT[unavailable]) : undefined}>
            <span class="when muted small">{formatDay(entry.playedAt)} {time(entry.playedAt)}</span>
            <span class="text">
              {#if entry.album}
                <span class="name">{entry.album.title}</span>
                <span class="muted small">
                  {entry.album.artist ?? ""} · {entry.tracks.length > 1
                    ? count("count.tracks", entry.tracks.length)
                    : entry.tracks[0].title}
                </span>
              {:else}
                <span class="name">{entry.tracks[0].title}</span>
                <span class="muted small">{entry.tracks[0].artist ?? ""}</span>
              {/if}
            </span>
            <button
              class="icon"
              title={t("history.playAgain")}
              aria-label={t("history.playAgain")}
              onclick={() => playEntry(entry)}
            >
              <Icon name="play" />
            </button>
          </li>
        {/each}
      </ul>
    {/if}
  {/if}
</section>

<style>
  .unavailable {
    opacity: 0.45;
  }

  .history {
    height: 100%;
    overflow-y: auto;
    padding: 0.75rem 1.25rem 2rem;
  }

  h1 {
    margin: 0 0 0.5rem;
    font-size: 1.6rem;
  }

  h2 {
    font-size: 1.05rem;
    margin: 1rem 0 0.4rem;
  }

  .top-head {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    justify-content: space-between;
    gap: 0.5rem 1rem;
  }

  .controls {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 0.35rem;
  }

  .segmented {
    display: inline-flex;
  }

  .segmented button {
    border-radius: 0;
  }

  .segmented button:first-child {
    border-radius: 6px 0 0 6px;
  }

  .segmented button:last-child {
    border-radius: 0 6px 6px 0;
  }

  .segmented button.on {
    background: var(--selected);
    color: var(--accent);
  }

  ol,
  ul {
    list-style: none;
    margin: 0;
    padding: 0;
  }

  li {
    display: flex;
    align-items: center;
    gap: 0.75rem;
    padding: 0.35rem 0;
    border-top: 1px solid var(--border);
  }

  .rank {
    width: 1.75rem;
    text-align: right;
    font-weight: 700;
    color: var(--text-muted);
    font-variant-numeric: tabular-nums;
  }

  .text {
    flex: 1;
    display: flex;
    flex-direction: column;
    min-width: 0;
  }

  .name,
  .text .small {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .small {
    font-size: 0.8rem;
  }

  .when {
    width: 9rem;
    flex: none;
  }
</style>
