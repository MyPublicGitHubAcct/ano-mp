<script lang="ts">
  // The album being browsed: its cover (click to choose another), what the
  // details sources say about it with the source named, its match status,
  // and "Find details…" and "Choose cover…"; then its description (from
  // Wikipedia, credited under its licence). "Details" opens a table of
  // every field with where it comes from: the tags, or a source. Reloads
  // after a scan and when `metadata-changed` names the album.
  import { untrack } from "svelte";
  import { openUrl } from "@tauri-apps/plugin-opener";
  import { metadata, type AlbumDetails, type AlbumLink } from "$lib/api";
  import { formatDate, formatDay, formatLabels, formatMedia, formatTime, percent, plural } from "$lib/format";
  import { library } from "$lib/state/library.svelte";
  import { attempt } from "$lib/state/toasts.svelte";
  import { loadPreference, savePreference, ui, type AlbumRef } from "$lib/state/ui.svelte";
  import Art from "./Art.svelte";
  import Icon from "./Icon.svelte";

  let { album }: { album: AlbumRef } = $props();

  const GENRES_SHOWN = 5;
  /** Paragraphs of a description shown before "Read more". */
  const SHORT_DESCRIPTION = 1;

  let details = $state.raw<AlbumDetails | null>(null);
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
      if (current === request) details = loaded;
    } catch {
      // Gone in a rescan; the browser shows that.
      if (current === request) details = null;
    }
  }

  /** The first link with a release that's matched, for the summary. */
  const matched = $derived(details?.links.find((link) => link.status === "matched" && link.release) ?? null);
  const first = $derived(details?.links[0] ?? null);
  const description = $derived(details?.description ?? null);
  const paragraphs = $derived(
    description === null
      ? []
      : expanded
        ? description.paragraphs
        : description.paragraphs.slice(0, SHORT_DESCRIPTION),
  );
  const genres = $derived(
    matched?.release?.genres.length ? matched.release.genres : (details?.genres ?? []),
  );

  const summary = $derived.by(() => {
    const release = matched?.release;
    if (!release) return "";
    return [
      release.date ? formatDate(release.date) : null,
      formatLabels(release.labels) || null,
      release.country,
      release.formats.length > 0 ? formatMedia(release.formats) : null,
      release.releaseType ? [release.releaseType, ...release.secondaryTypes].join(" · ") : null,
    ]
      .filter(Boolean)
      .join(" · ");
  });

  function status(link: AlbumLink | null) {
    if (!details) return "";
    if (link === null) return details.canLookUp ? "Not looked up online yet." : "";
    switch (link.status) {
      case "matched":
        return "";
      case "review":
        return `A possible match on ${link.sourceName} needs your review.`;
      case "none":
        return link.chosenByUser
          ? `You said ${link.sourceName} doesn’t have this album.`
          : `Not found on ${link.sourceName}.`;
    }
  }

  function describeLink(link: AlbumLink) {
    const checked = `checked ${formatDay(link.checkedAt)}`;
    switch (link.status) {
      case "matched":
        return `${link.chosenByUser ? "Chosen by you" : `Matched automatically (${percent(link.score)})`}, ${checked}`;
      case "review":
        return `Needs review (${percent(link.score)}), ${checked}`;
      case "none":
        return link.chosenByUser ? "None of its releases, you said" : `Not found, ${checked}`;
    }
  }

  type Row = { field: string; value: string; source: string };

  const rows = $derived.by((): Row[] => {
    if (!details) return [];
    const tags = "Tags";
    const rows: Row[] = [
      { field: "Title", value: details.title, source: tags },
      { field: "Album artist", value: details.albumArtist ?? "—", source: tags },
      { field: "Year", value: details.year === null ? "—" : String(details.year), source: tags },
      { field: "Genre", value: details.genres.join(", ") || "—", source: tags },
      {
        field: "Tracks",
        value: `${plural(details.tracks.length, "file")}, ${formatTime(details.duration)}`,
        source: tags,
      },
    ];
    if (details.taggedReleaseId) rows.push({ field: "Release id", value: details.taggedReleaseId, source: tags });
    for (const link of details.links) {
      const source = link.sourceName;
      rows.push({ field: "Match", value: describeLink(link), source });
      const release = link.release;
      if (!release) continue;
      if (link.status === "review") {
        const when = release.date ? `, ${formatDate(release.date)}` : "";
        rows.push({ field: "Candidate", value: `${release.title} by ${release.artist}${when}`, source });
        continue;
      }
      if (link.status !== "matched") continue;
      const add = (field: string, value: string | null | undefined) => {
        if (value) rows.push({ field, value, source });
      };
      add("Release", `${release.title} by ${release.artist}`);
      add(
        "Released",
        [
          release.date ? formatDate(release.date) : null,
          release.firstReleaseDate && release.firstReleaseDate.slice(0, 4) !== release.date?.slice(0, 4)
            ? `first ${formatDate(release.firstReleaseDate)}`
            : null,
        ]
          .filter(Boolean)
          .join(", "),
      );
      add("Label", formatLabels(release.labels));
      add("Country", release.country);
      add("Format", release.formats.length > 0 ? formatMedia(release.formats) : null);
      add("Type", release.releaseType ? [release.releaseType, ...release.secondaryTypes].join(", ") : null);
      add("Status", release.status);
      add("Barcode", release.barcode);
      add("Genres", release.genres.join(", "));
    }
    if (details.description) {
      rows.push({
        field: "Description",
        value: `The article “${details.description.title}”`,
        source: details.description.sourceName,
      });
    }
    if (details.cover) {
      rows.push({
        field: "Cover",
        value: details.cover.chosen ? "Chosen by you" : "First found in the source order",
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

  const findDetails = () => (ui.dialog = { kind: "findDetails", album });
  const chooseCover = () => (ui.dialog = { kind: "chooseCover", album });
</script>

<section class="album-info" aria-label="About {album.title}">
  <div class="top">
    <button class="cover" title="Choose a cover" aria-label="Choose a cover for {album.title}" onclick={chooseCover}>
      <Art albumId={album.id} size="100%" />
    </button>
    <div class="facts">
      {#if details}
        <p class="muted small">
          {[details.albumArtist, details.year, plural(details.tracks.length, "track"), formatTime(details.duration)]
            .filter((part) => part !== null)
            .join(" · ")}
        </p>
        {#if summary && matched}
          <p class="summary">
            {summary}
            <span class="source" title="From {matched.sourceName}">{matched.sourceName}</span>
          </p>
        {/if}
        {#if status(first)}
          <p class="status">{status(first)}</p>
        {/if}
        {#if genres.length > 0}
          <ul class="genres" aria-label="Genres">
            {#each genres.slice(0, GENRES_SHOWN) as genre (genre)}<li>{genre}</li>{/each}
          </ul>
        {/if}
      {/if}
      <div class="buttons">
        <button onclick={findDetails}><Icon name="search" /> Find details…</button>
        <button onclick={chooseCover}>Choose cover…</button>
        <button class="link" aria-expanded={open} onclick={() => (open = !open)}>
          {open ? "Hide details" : "Details"}
        </button>
      </div>
    </div>
  </div>
  {#if description}
    <div class="description">
      {#each paragraphs as paragraph, index (index)}<p>{paragraph}</p>{/each}
      {#if description.paragraphs.length > SHORT_DESCRIPTION}
        <button class="link" aria-expanded={expanded} onclick={() => (expanded = !expanded)}>
          {expanded ? "Show less" : "Read more"}
        </button>
      {/if}
      <p class="credit muted small">
        From the {description.sourceName} article
        <a href={description.url} onclick={openLink}>“{description.title}”</a>, under
        <a href={description.licenseUrl} onclick={openLink}>{description.license}</a>.
      </p>
    </div>
  {/if}
  {#if open && rows.length > 0}
    <div class="table">
      <table>
        <thead>
          <tr><th scope="col">Field</th><th scope="col">Value</th><th scope="col">Source</th></tr>
        </thead>
        <tbody>
          {#each rows as row, index (index)}
            <tr>
              <th scope="row">{row.field}</th>
              <td>{row.value}</td>
              <td><span class="source">{row.source}</span></td>
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
