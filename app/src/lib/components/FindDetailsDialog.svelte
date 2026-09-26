<script lang="ts">
  // "Find details": the releases each album-details source offers for an
  // album, best first, to pick the right one, reject them all, or go back to
  // automatic matching. The search starts with the album's own title and
  // artist (the automatic match's search, usually cached); either can be
  // edited, and a MusicBrainz release link or id is looked up directly.
  import { onMount } from "svelte";
  import { openUrl } from "@tauri-apps/plugin-opener";
  import {
    metadata,
    type AlbumDetails,
    type AlbumLink,
    type Release,
    type ReleaseCandidate,
    type SourceCandidates,
    type SourceId,
  } from "$lib/api";
  import { formatDate, formatLabels, formatMedia, formatTime, percent, plural } from "$lib/format";
  import { attempt, toasts } from "$lib/state/toasts.svelte";
  import type { AlbumRef } from "$lib/state/ui.svelte";
  import Dialog from "./Dialog.svelte";
  import Icon from "./Icon.svelte";

  let { album, onclose }: { album: AlbumRef; onclose: () => void } = $props();

  /** Track lengths within this many seconds agree (as the matcher counts them). */
  const TOLERANCE = 3;

  let details = $state.raw<AlbumDetails | null>(null);
  let sources = $state.raw<SourceCandidates<ReleaseCandidate>[] | null>(null);
  let title = $state("");
  let artist = $state("");
  let searching = $state(false);
  let busy = $state(false);
  let selected = $state.raw<{ source: SourceId; id: string } | null>(null);
  let request = 0;

  onMount(() => {
    attempt(async () => {
      const loaded = await metadata.album(album.id);
      details = loaded;
      title = loaded.title;
      artist = loaded.albumArtist ?? "";
    });
    search(false);
  });

  /** The album's own search, or the one typed in. */
  async function search(typed: boolean) {
    const current = ++request;
    searching = true;
    try {
      const found = await metadata.releaseCandidates(album.id, typed ? title : null, typed ? artist : null);
      if (current !== request) return;
      sources = found;
      // The current release, if offered.
      selected = null;
      for (const source of found) {
        const id = linkOf(source.source)?.externalId;
        if (id && source.candidates.some((c) => c.release.id === id)) selected = { source: source.source, id };
      }
    } catch (error) {
      if (current === request) {
        sources = [];
        toasts.show(String(error));
      }
    } finally {
      if (current === request) searching = false;
    }
  }

  const linkOf = (source: SourceId): AlbumLink | undefined => details?.links.find((link) => link.source === source);

  function describeLink(link: AlbumLink | undefined, sourceName: string) {
    if (!link) return `Not looked up on ${sourceName} yet.`;
    switch (link.status) {
      case "none":
        return link.chosenByUser
          ? `You said none of ${sourceName}’s releases is this album.`
          : `Nothing found on ${sourceName} when it was last searched.`;
      case "review":
        return `${sourceName} has a possible match (${percent(link.score)}) that needs your review.`;
      case "matched":
        return link.chosenByUser
          ? `Matched to the ${sourceName} release you chose.`
          : `Matched automatically on ${sourceName} (${percent(link.score)}).`;
    }
  }

  /** "26 December 2007 · GB · CD · XL Recordings (XLCD324) · 10 tracks" */
  function describe(release: Release) {
    return [
      release.date ? formatDate(release.date) : null,
      release.country,
      release.formats.length > 0 ? formatMedia(release.formats) : null,
      formatLabels(release.labels) || null,
      plural(release.trackCount, "track"),
    ]
      .filter(Boolean)
      .join(" · ");
  }

  const isSelected = (source: SourceId, release: Release) =>
    selected?.source === source && selected.id === release.id;

  const selectedCandidate = $derived.by(() => {
    const source = sources?.find((s) => s.source === selected?.source);
    return source?.candidates.find((c) => c.release.id === selected?.id) ?? null;
  });

  /** The release's tracks next to the album's, in order. */
  const comparison = $derived.by(() => {
    const release = selectedCandidate?.release;
    if (!release || release.tracks.length === 0 || !details) return [];
    const rows = Math.max(release.tracks.length, details.tracks.length);
    return Array.from({ length: rows }, (_, i) => {
      const theirs = release.tracks[i];
      const ours = details!.tracks[i];
      const length = theirs?.lengthMs != null ? theirs.lengthMs / 1000 : null;
      return {
        position: theirs ? `${release.formats.length > 1 ? `${theirs.disc}.` : ""}${theirs.position}` : "",
        title: theirs?.title ?? "—",
        length,
        ours: ours?.duration ?? null,
        agrees: length !== null && ours !== undefined && Math.abs(length - ours.duration) <= TOLERANCE,
      };
    });
  });

  const current = $derived(selected ? linkOf(selected.source) : undefined);
  const alreadyChosen = $derived(
    !!selected && current?.chosenByUser === true && current.status === "matched" && current.externalId === selected.id,
  );
  /** The first source with a link, for "None of these" and "Use automatic". */
  const primary = $derived(sources?.[0]?.source ?? null);

  async function act(action: () => Promise<unknown>, done: string) {
    busy = true;
    try {
      await action();
      toasts.show(done, "info", 3000);
      onclose();
    } catch (error) {
      toasts.show(String(error));
    } finally {
      busy = false;
    }
  }

  const choose = () => {
    const pick = selected;
    if (pick) act(() => metadata.chooseRelease(album.id, pick.source, pick.id), `Details for ${album.title} updated`);
  };
  const reject = (source: SourceId) =>
    act(() => metadata.rejectRelease(album.id, source), `${album.title} won’t be matched automatically`);
  const automatic = (source: SourceId) =>
    act(() => metadata.useAutomaticRelease(album.id, source), `${album.title} was matched again`);

  function openLink(event: MouseEvent) {
    event.preventDefault();
    const href = (event.currentTarget as HTMLAnchorElement).href;
    attempt(() => openUrl(href));
  }
</script>

<Dialog title="Find details for {album.title}" {onclose}>
  <form
    class="search"
    onsubmit={(event) => {
      event.preventDefault();
      search(true);
    }}
  >
    <label>
      <span class="muted small">Album, or a MusicBrainz release link</span>
      <input type="text" bind:value={title} placeholder="Title" />
    </label>
    <label>
      <span class="muted small">Artist</span>
      <input type="text" bind:value={artist} placeholder="Any artist" />
    </label>
    <button type="submit" disabled={searching || title.trim() === ""}><Icon name="search" /> Search</button>
  </form>

  {#if details && !details.canLookUp}
    <p class="note">Album details sources are turned off in Online sources.</p>
  {/if}

  {#if sources === null}
    <p class="muted" aria-live="polite">Searching…</p>
  {:else}
    {#each sources as source (source.source)}
      <section aria-label={source.sourceName}>
        <h3>{source.sourceName}</h3>
        <p class="muted small">{describeLink(linkOf(source.source), source.sourceName)}</p>
        {#if source.note}
          <p class="note">{source.note}</p>
        {:else if source.candidates.length === 0}
          <p class="muted">{searching ? "Searching…" : "No releases found. Try another title or artist."}</p>
        {/if}
        <ul class="candidates" aria-busy={searching}>
          {#each source.candidates as candidate (candidate.release.id)}
            {@const release = candidate.release}
            {@const link = linkOf(source.source)}
            <li>
              <button
                class="candidate"
                class:selected={isSelected(source.source, release)}
                aria-pressed={isSelected(source.source, release)}
                onclick={() => (selected = { source: source.source, id: release.id })}
              >
                <span class="line">
                  <span class="name">
                    {release.title}{#if release.disambiguation}{" "}<span class="muted">({release.disambiguation})</span>{/if}
                  </span>
                  <span class="score" title={candidate.full ? "Including track lengths" : "From the search result alone"}>
                    {candidate.full ? "" : "~"}{percent(candidate.score)}
                  </span>
                </span>
                <span class="muted small">{release.artist}</span>
                <span class="muted small">{describe(release)}</span>
                <span class="badges">
                  {#if link?.externalId === release.id && link.status === "matched"}
                    <span class="badge current">{link.chosenByUser ? "Your choice" : "Current match"}</span>
                  {:else if link?.externalId === release.id && link.status === "review"}
                    <span class="badge">Possible match</span>
                  {/if}
                  {#if release.status && release.status !== "Official"}<span class="badge">{release.status}</span>{/if}
                  {#if release.releaseType}<span class="badge">{[release.releaseType, ...release.secondaryTypes].join(" · ")}</span>{/if}
                </span>
              </button>
              {#if isSelected(source.source, release)}
                <div class="expanded">
                  {#if comparison.length > 0}
                    <table>
                      <thead>
                        <tr>
                          <th scope="col">#</th>
                          <th scope="col">{source.sourceName}</th>
                          <th scope="col" class="time">Length</th>
                          <th scope="col" class="time">Your file</th>
                        </tr>
                      </thead>
                      <tbody>
                        {#each comparison as row, index (index)}
                          <tr class:differs={!row.agrees}>
                            <td class="muted">{row.position}</td>
                            <td>{row.title}</td>
                            <td class="time">{row.length === null ? "—" : formatTime(row.length)}</td>
                            <td class="time">
                              {#if row.ours === null}—{:else}{formatTime(row.ours)}{/if}
                              {#if row.agrees}<span class="agrees" title="Lengths agree"><Icon name="check" size="0.9rem" /></span>{/if}
                            </td>
                          </tr>
                        {/each}
                      </tbody>
                    </table>
                  {:else}
                    <p class="muted small">This search result has no track list; its score leaves out track lengths.</p>
                  {/if}
                  {#if source.source === "musicbrainz"}
                    <a class="small" href="https://musicbrainz.org/release/{release.id}" onclick={openLink}>
                      Open on MusicBrainz
                    </a>
                  {/if}
                </div>
              {/if}
            </li>
          {/each}
        </ul>
      </section>
    {/each}
  {/if}

  {#snippet actions()}
    {#if primary}
      {@const link = linkOf(primary)}
      {#if link}
        <button onclick={() => automatic(primary)} disabled={busy} title="Forget this album’s match and match it again now">
          Use automatic
        </button>
      {/if}
      {#if !(link?.status === "none" && link.chosenByUser)}
        <button onclick={() => reject(primary)} disabled={busy}>None of these</button>
      {/if}
    {/if}
    <span class="spacer"></span>
    <button onclick={onclose} disabled={busy}>Cancel</button>
    <button class="primary" onclick={choose} disabled={busy || !selected || alreadyChosen}>Use this release</button>
  {/snippet}
</Dialog>

<style>
  p {
    margin: 0.25rem 0;
  }

  .small {
    font-size: 0.8rem;
  }

  .search {
    display: flex;
    flex-wrap: wrap;
    align-items: flex-end;
    gap: 0.5rem;
    margin-bottom: 0.75rem;
  }

  .search label {
    display: flex;
    flex-direction: column;
    gap: 0.2rem;
    flex: 1 1 12rem;
    min-width: 0;
  }

  .note {
    padding: 0.5rem 0.75rem;
    border-radius: 6px;
    background: var(--surface-2);
  }

  h3 {
    margin: 0.75rem 0 0;
    font-size: 0.95rem;
  }

  .candidates {
    list-style: none;
    margin: 0.5rem 0 0;
    padding: 0;
    display: flex;
    flex-direction: column;
    gap: 0.4rem;
  }

  .candidates[aria-busy="true"] {
    opacity: 0.6;
  }

  .candidate {
    display: flex;
    flex-direction: column;
    align-items: stretch;
    gap: 0.1rem;
    width: 100%;
    padding: 0.5rem 0.75rem;
    text-align: left;
  }

  .candidate.selected {
    border-color: var(--accent);
    background: var(--selected);
  }

  .line {
    display: flex;
    align-items: baseline;
    justify-content: space-between;
    gap: 1rem;
  }

  .name {
    font-weight: 600;
    min-width: 0;
    overflow-wrap: anywhere;
  }

  .score {
    flex: none;
    font-variant-numeric: tabular-nums;
    color: var(--text-muted);
  }

  .badges {
    display: flex;
    flex-wrap: wrap;
    gap: 0.3rem;
    margin-top: 0.2rem;
  }

  .badges:empty {
    display: none;
  }

  .badge {
    font-size: 0.72rem;
    padding: 0.05rem 0.45rem;
    border-radius: 999px;
    background: var(--surface-2);
    color: var(--text-muted);
  }

  .badge.current {
    background: var(--accent);
    color: var(--accent-text);
  }

  .expanded {
    padding: 0.5rem 0.75rem 0.25rem;
  }

  table {
    width: 100%;
    border-collapse: collapse;
    font-size: 0.85rem;
    margin-bottom: 0.4rem;
  }

  th {
    text-align: left;
    font-weight: 500;
    color: var(--text-muted);
    border-bottom: 1px solid var(--border);
    padding: 0.2rem 0.4rem;
  }

  td {
    padding: 0.15rem 0.4rem;
    vertical-align: top;
  }

  .time {
    text-align: right;
    white-space: nowrap;
    font-variant-numeric: tabular-nums;
  }

  .differs .time:last-child {
    color: var(--danger);
  }

  .agrees {
    display: inline-flex;
    vertical-align: -0.15rem;
    color: var(--accent);
  }

  a {
    color: var(--accent);
    text-decoration: none;
  }

  a:hover {
    text-decoration: underline;
  }

  .spacer {
    flex: 1;
  }
</style>
