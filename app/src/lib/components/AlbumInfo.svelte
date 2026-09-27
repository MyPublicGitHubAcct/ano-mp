<script lang="ts">
  // The album being browsed: its cover (click to choose another), what the
  // details sources say about it (the facts the display settings choose)
  // with the source named, its match status, and "Find details…" and
  // "Choose cover…"; then its description (from Wikipedia, credited under
  // its licence) unless the settings turn descriptions off. "Details" opens a table of
  // every field with where it comes from: the tags, or a source. Reloads
  // after a scan and when `metadata-changed` names the album. A source that
  // doesn't keep its releases (Discogs) is asked for the release each time
  // the album is shown, and its data carries the credit its terms require.
  import { untrack } from "svelte";
  import { openUrl } from "@tauri-apps/plugin-opener";
  import {
    metadata,
    type AlbumDetails,
    type AlbumFact,
    type AlbumLink,
    type Release,
    type SourceId,
  } from "$lib/api";
  import { formatDate, formatDay, formatLabels, formatMedia, formatTime, percent } from "$lib/format";
  import { count, errorText, t } from "$lib/i18n";
  import { collection } from "$lib/state/collection.svelte";
  import { marks } from "$lib/api";
  import Heart from "./Heart.svelte";
  import { library } from "$lib/state/library.svelte";
  import { appSettings } from "$lib/state/settings.svelte";
  import { attempt } from "$lib/state/toasts.svelte";
  import { loadPreference, savePreference, ui, type AlbumRef } from "$lib/state/ui.svelte";
  import { features } from "$lib/state/features.svelte";
  import AlbumWorks from "./AlbumWorks.svelte";
  import Art from "./Art.svelte";
  import Icon from "./Icon.svelte";
  import MoreInGenre from "./MoreInGenre.svelte";

  let { album }: { album: AlbumRef } = $props();

  const GENRES_SHOWN = 5;
  /** Paragraphs of a description shown before "Read more". */
  const SHORT_DESCRIPTION = 1;

  let details = $state.raw<AlbumDetails | null>(null);
  /** Releases fetched for links whose source doesn't keep them, by source and release id, or why they couldn't be. */
  let fetched = $state.raw<Partial<Record<SourceId, { id: string; release: Release | null; error: string | null }>>>({});
  let open = $state(loadPreference("albumDetailsOpen", false));
  let expanded = $state(false);
  let request = 0;
  let shownId: number | null = null;

  $effect(() => savePreference("albumDetailsOpen", open));

  $effect(() => {
    const id = album.id;
    void [library.version, library.artVersions.get(id)];
    untrack(() => {
      if (id !== shownId) {
        shownId = id;
        expanded = false;
      }
      load(id);
    });
  });

  async function load(id: number) {
    const current = ++request;
    try {
      const loaded = await metadata.album(id);
      if (current !== request) return;
      details = loaded;
      fetchReleases(id, loaded, current);
    } catch {
      // Gone in a rescan; the browser shows that.
      if (current === request) details = null;
    }
  }

  /** Fetches the releases of links whose source doesn't keep them. */
  async function fetchReleases(id: number, loaded: AlbumDetails, current: number) {
    for (const link of loaded.links) {
      if (link.storesDetails || link.externalId === null) continue;
      const releaseId = link.externalId;
      let result: { id: string; release: Release | null; error: string | null };
      try {
        result = { id: releaseId, release: await metadata.releaseDetails(id, link.source), error: null };
      } catch (error) {
        result = { id: releaseId, release: null, error: errorText(error) };
      }
      if (current !== request) return;
      fetched = { ...fetched, [link.source]: result };
    }
  }

  /** What was fetched for `link`, if it's still for the release linked. */
  const fetchedFor = (link: AlbumLink) => {
    const result = fetched[link.source];
    return result && result.id === link.externalId ? result : null;
  };

  /** The album's links, with the releases fetched for those that need it. */
  const links = $derived(
    (details?.links ?? []).map((link) =>
      link.storesDetails ? link : { ...link, release: fetchedFor(link)?.release ?? null },
    ),
  );
  /** The first link with a release that's matched, for the summary. */
  const matched = $derived(links.find((link) => link.status === "matched" && link.release) ?? null);
  const first = $derived(links[0] ?? null);
  const description = $derived(details?.description ?? null);
  const paragraphs = $derived(
    description === null
      ? []
      : expanded
        ? description.paragraphs
        : description.paragraphs.slice(0, SHORT_DESCRIPTION),
  );
  const genres = $derived.by(() => {
    const release = matched?.release;
    const online = release ? [...new Set([...release.genres, ...release.styles])] : [];
    return online.length > 0 ? online : (details?.genres ?? []);
  });

  /** The facts the display settings choose, in their order. */
  const summary = $derived.by(() => {
    const release = matched?.release;
    if (!release) return "";
    const facts: Record<AlbumFact, string | null> = {
      date: release.date ? formatDate(release.date) : null,
      label: formatLabels(release.labels) || null,
      country: release.country,
      format: release.formats.length > 0 ? formatMedia(release.formats) : null,
      type: release.releaseType ? [release.releaseType, ...release.secondaryTypes].join(" · ") : null,
    };
    return appSettings.display.albumFacts
      .map((fact) => facts[fact])
      .filter(Boolean)
      .join(" · ");
  });
  const showDescription = $derived(appSettings.display.showDescriptions);

  function status(link: AlbumLink | null) {
    if (!details) return "";
    if (link === null) return details.canLookUp ? t("album.notLookedUp") : "";
    switch (link.status) {
      case "matched":
        return "";
      case "review":
        return t("album.needsReview", { source: link.sourceName });
      case "none":
        return link.chosenByUser
          ? t("album.youSaidNone", { source: link.sourceName })
          : t("album.notFoundOn", { source: link.sourceName });
    }
  }

  function describeLink(link: AlbumLink) {
    const day = formatDay(link.checkedAt);
    switch (link.status) {
      case "matched":
        return link.chosenByUser
          ? t("album.linkChosen", { day })
          : t("album.linkMatched", { score: percent(link.score), day });
      case "review":
        return t("album.linkReview", { score: percent(link.score), day });
      case "none":
        return link.chosenByUser ? t("album.linkNoneYou") : t("album.linkNotFound", { day });
    }
  }

  /** `href` links the source to the page the value came from, for a source whose terms want that. */
  type Row = { field: string; value: string; source: string; href?: string };

  const rows = $derived.by((): Row[] => {
    if (!details) return [];
    const tags = t("album.fromTags");
    const rows: Row[] = [
      { field: t("album.field.title"), value: details.title, source: tags },
      { field: t("album.field.albumArtist"), value: details.albumArtist ?? "—", source: tags },
      { field: t("album.field.year"), value: details.year === null ? "—" : String(details.year), source: tags },
      { field: t("album.field.genre"), value: details.genres.join(", ") || "—", source: tags },
      {
        field: t("album.field.tracks"),
        value: `${count("count.files", details.tracks.length)}, ${formatTime(details.duration)}`,
        source: tags,
      },
    ];
    if (details.taggedReleaseId)
      rows.push({ field: t("album.field.releaseId"), value: details.taggedReleaseId, source: tags });
    for (const link of links) {
      const source = link.credit ?? link.sourceName;
      const href = link.credit && link.pageUrl ? link.pageUrl : undefined;
      rows.push({ field: t("album.field.match"), value: describeLink(link), source: link.sourceName });
      const release = link.release;
      const error = fetchedFor(link)?.error;
      if (!release && error && link.status !== "none") {
        rows.push({ field: t("album.field.details"), value: t("album.notAvailable", { error }), source: link.sourceName });
      }
      if (!release) continue;
      if (link.status === "review") {
        const when = release.date ? `, ${formatDate(release.date)}` : "";
        rows.push({
          field: t("album.field.candidate"),
          value: t("album.releaseBy", { title: release.title, artist: release.artist }) + when,
          source,
          href,
        });
        continue;
      }
      if (link.status !== "matched") continue;
      const add = (field: string, value: string | null | undefined) => {
        if (value) rows.push({ field, value, source, href });
      };
      add(t("album.field.release"), t("album.releaseBy", { title: release.title, artist: release.artist }));
      add(
        t("album.field.released"),
        [
          release.date ? formatDate(release.date) : null,
          release.firstReleaseDate && release.firstReleaseDate.slice(0, 4) !== release.date?.slice(0, 4)
            ? t("album.firstReleased", { date: formatDate(release.firstReleaseDate) })
            : null,
        ]
          .filter(Boolean)
          .join(", "),
      );
      add(t("album.field.label"), formatLabels(release.labels));
      add(t("album.field.country"), release.country);
      add(t("album.field.format"), release.formats.length > 0 ? formatMedia(release.formats) : null);
      add(
        t("album.field.type"),
        release.releaseType ? [release.releaseType, ...release.secondaryTypes].join(", ") : null,
      );
      add(t("album.field.status"), release.status);
      add(t("album.field.barcode"), release.barcode);
      add(t("album.field.genres"), release.genres.join(", "));
      add(t("album.field.styles"), release.styles.join(", "));
      add(t("album.field.credits"), release.credits.map((credit) => `${credit.role}: ${credit.name}`).join("; "));
    }
    if (details.description) {
      rows.push({
        field: t("album.field.description"),
        value: t("album.article", { title: details.description.title }),
        source: details.description.sourceName,
      });
    }
    if (details.cover) {
      rows.push({
        field: t("album.field.cover"),
        value: details.cover.chosen ? t("album.coverChosen") : t("album.coverFirst"),
        source: details.cover.sourceName,
      });
    }
    return rows;
  });

  function openLink(event: MouseEvent) {
    event.preventDefault();
    const href = (event.currentTarget as HTMLAnchorElement).href;
    attempt(() => openUrl(href));
  }

  /** The album's heart (PLAN.md F3). */
  let hearted = $state(false);
  $effect(() => {
    const id = album.id;
    void collection.version;
    untrack(async () => {
      hearted = (await marks.favouritesAmong("album", [id]).catch(() => [])).length > 0;
    });
  });
  async function heart(on: boolean) {
    hearted = on;
    await collection.setFavourite("album", [album.id], on);
  }

  const findDetails = () => (ui.dialog = { kind: "findDetails", album });
  const chooseCover = () => (ui.dialog = { kind: "chooseCover", album });
</script>

<section class="album-info" aria-label={t("album.about", { title: album.title })}>
  <div class="top">
    <button
      class="cover"
      title={t("album.chooseACover")}
      aria-label={t("album.chooseCoverFor", { title: album.title })}
      onclick={chooseCover}
    >
      <Art albumId={album.id} size="100%" />
    </button>
    <div class="facts">
      {#if details}
        <p class="muted small">
          {[details.albumArtist, details.year, count("count.tracks", details.tracks.length), formatTime(details.duration)]
            .filter((part) => part !== null)
            .join(" · ")}
        </p>
        {#if summary && matched}
          <p class="summary">
            {summary}
            {#if matched.credit && matched.pageUrl}
              <a class="source" href={matched.pageUrl} onclick={openLink}>{matched.credit}</a>
            {:else}
              <span class="source" title={t("album.from", { source: matched.sourceName })}>{matched.sourceName}</span>
            {/if}
          </p>
        {/if}
        {#if status(first)}
          <p class="status">{status(first)}</p>
        {/if}
        {#if genres.length > 0}
          <ul class="genres" aria-label={t("album.field.genres")}>
            {#each genres.slice(0, GENRES_SHOWN) as genre (genre)}<li>{genre}</li>{/each}
          </ul>
        {/if}
      {/if}
      <div class="buttons">
        <Heart on={hearted} label={album.title} onchange={heart} size="1.2rem" />
        <button onclick={findDetails}><Icon name="search" /> {t("menu.findDetails")}</button>
        <button onclick={chooseCover}>{t("menu.chooseCover")}</button>
        {#if features.on.playbackPreferences}
          <button onclick={() => (ui.dialog = { kind: "prefs", track: null, album })}>{t("album.playback")}</button>
        {/if}
        <button class="link" aria-expanded={open} onclick={() => (open = !open)}>
          {open ? t("album.hideDetails") : t("album.details")}
        </button>
      </div>
    </div>
  </div>
  {#if details && features.on.classical}
    <AlbumWorks tracks={details.tracks} />
  {/if}
  {#if description && showDescription}
    <div class="description">
      {#each paragraphs as paragraph, index (index)}<p>{paragraph}</p>{/each}
      {#if description.paragraphs.length > SHORT_DESCRIPTION}
        <button class="link" aria-expanded={expanded} onclick={() => (expanded = !expanded)}>
          {expanded ? t("album.showLess") : t("album.readMore")}
        </button>
      {/if}
      <p class="credit muted small">
        {t("album.creditFrom", { source: description.sourceName })}
        <a href={description.url} onclick={openLink}>“{description.title}”</a>{t("album.creditUnder")}
        <a href={description.licenseUrl} onclick={openLink}>{description.license}</a>.
      </p>
    </div>
  {/if}
  {#if details && features.on.moreInGenre && details.genres.length > 0}
    {#key details.id}<MoreInGenre albumId={details.id} genres={details.genres} />{/key}
  {/if}
  {#if open && rows.length > 0}
    <div class="table">
      <table>
        <thead>
          <tr>
            <th scope="col">{t("album.col.field")}</th><th scope="col">{t("album.col.value")}</th><th scope="col"
              >{t("album.col.source")}</th
            >
          </tr>
        </thead>
        <tbody>
          {#each rows as row, index (index)}
            <tr>
              <th scope="row">{row.field}</th>
              <td>{row.value}</td>
              <td>
                {#if row.href}
                  <a class="source" href={row.href} onclick={openLink}>{row.source}</a>
                {:else}
                  <span class="source">{row.source}</span>
                {/if}
              </td>
            </tr>
          {/each}
        </tbody>
      </table>
    </div>
  {/if}
</section>

<style>
  .album-info {
    padding: 0 1rem 0.75rem;
    border-bottom: 1px solid var(--border);
  }

  .top {
    display: flex;
    gap: 1rem;
    align-items: flex-start;
  }

  .cover {
    flex: none;
    width: 6.5rem;
    height: 6.5rem;
    padding: 0;
    border: none;
    background: none;
    border-radius: 4px;
    overflow: hidden;
  }

  .cover:hover:not(:disabled) {
    background: none;
    filter: brightness(0.92);
  }

  .facts {
    display: flex;
    flex-direction: column;
    gap: 0.25rem;
    min-width: 0;
  }

  p {
    margin: 0;
  }

  .small {
    font-size: 0.8rem;
  }

  .summary {
    font-size: 0.9rem;
  }

  .status {
    font-size: 0.9rem;
    color: var(--text-muted);
  }

  a.source {
    color: var(--accent);
  }

  .source {
    display: inline-block;
    font-size: 0.7rem;
    padding: 0.05rem 0.45rem;
    border-radius: 999px;
    background: var(--surface-2);
    color: var(--text-muted);
    white-space: nowrap;
    vertical-align: 0.05rem;
  }

  .genres {
    display: flex;
    flex-wrap: wrap;
    gap: 0.3rem;
    list-style: none;
    margin: 0.1rem 0 0;
    padding: 0;
  }

  .genres li {
    font-size: 0.75rem;
    padding: 0.05rem 0.5rem;
    border-radius: 999px;
    border: 1px solid var(--border);
    color: var(--text-muted);
  }

  .buttons {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 0.4rem 0.6rem;
    margin-top: 0.35rem;
  }

  .description {
    display: flex;
    flex-direction: column;
    align-items: flex-start;
    gap: 0.5rem;
    max-width: 46rem;
    margin-top: 0.75rem;
    font-size: 0.9rem;
    line-height: 1.5;
  }

  .description .credit {
    margin-top: 0.1rem;
  }

  a {
    color: var(--accent);
    text-decoration: none;
  }

  a:hover {
    text-decoration: underline;
  }

  .table {
    max-height: 16rem;
    overflow-y: auto;
    margin-top: 0.75rem;
  }

  table {
    width: 100%;
    max-width: 46rem;
    border-collapse: collapse;
    font-size: 0.85rem;
  }

  thead th {
    text-align: left;
    font-weight: 500;
    color: var(--text-muted);
    border-bottom: 1px solid var(--border);
    padding: 0.25rem 0.5rem;
    position: sticky;
    top: 0;
    background: var(--bg);
  }

  tbody th {
    text-align: left;
    font-weight: 500;
    white-space: nowrap;
    color: var(--text-muted);
  }

  tbody th,
  td {
    padding: 0.2rem 0.5rem;
    vertical-align: top;
  }

  td {
    overflow-wrap: anywhere;
  }

  @media (max-width: 640px) {
    .cover {
      width: 4.5rem;
      height: 4.5rem;
    }
  }
</style>
