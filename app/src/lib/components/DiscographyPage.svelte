<script lang="ts">
  // The releases MusicBrainz lists for an artist that the library doesn't
  // have, grouped by release type like the artist page, oldest first. Each
  // links to its MusicBrainz page. Fetched through the metadata worker
  // (cached for a week; "Check again" asks anew), or only from the cache
  // while online services are off. Reloads after a scan, and when the
  // metadata worker names the artist in `metadata-changed`.
  import { openLink } from "$lib/openLink";
  import { untrack } from "svelte";
  import { metadata, type Discography, type ReleaseGroupEntry } from "$lib/api";
  import { formatDate } from "$lib/format";
  import { count, errorText, t } from "$lib/i18n";
  import { bySection, sectionName } from "$lib/releases";
  import { library } from "$lib/state/library.svelte";
  import { loadPreference, savePreference, ui } from "$lib/state/ui.svelte";
  import Icon from "./Icon.svelte";

  let discography = $state.raw<Discography | null>(null);
  let error = $state<string | null>(null);
  let loading = $state(false);
  let collapsed = $state<string[]>(loadPreference("discography.collapsed", ["Singles", "Other releases"]));
  let request = 0;
  let shownId: number | null = null;

  const artist = $derived(ui.artist);

  // Another artist: start over. A scan or a new match for this one: reload
  // in place.
  $effect(() => {
    const id = artist?.id;
    if (id === undefined) return;
    void [library.version, library.artistVersions.get(id)];
    untrack(() => {
      if (id !== shownId) {
        shownId = id;
        discography = null;
        error = null;
      }
      load(id, false);
    });
  });

  async function load(id: number, refresh: boolean) {
    const current = ++request;
    loading = true;
    try {
      const loaded = await metadata.artistDiscography(id, refresh);
      if (current !== request) return;
      discography = loaded;
      error = null;
    } catch (failure) {
      if (current !== request) return;
      // Keep what was shown if checking again failed.
      if (!refresh) discography = null;
      error = errorText(failure);
    } finally {
      if (current === request) loading = false;
    }
  }

  const sections = $derived(bySection(discography?.missing ?? [], "Other releases"));

  function summary(found: Discography) {
    const missing = found.missing.length;
    if (missing === 0) return count("discography.hasAll", found.listed);
    const lacks = t("discography.lacks", { missing, listed: found.listed, count: missing });
    return found.inLibrary > 0 ? t("discography.lacksSome", { lacks, count: found.inLibrary }) : `${lacks}.`;
  }

  function toggle(section: string) {
    collapsed = collapsed.includes(section) ? collapsed.filter((name) => name !== section) : [...collapsed, section];
    savePreference("discography.collapsed", collapsed);
  }

  const year = (entry: ReleaseGroupEntry) => entry.firstReleaseDate?.slice(0, 4) ?? "—";

  /** The types each section's name already says. */
  const IMPLIED: Record<string, string[]> = {
    Albums: ["Album"],
    EPs: ["EP"],
    Singles: ["Single"],
    "Live albums": ["Album", "Live"],
    Compilations: ["Album", "Compilation"],
    Soundtracks: ["Album", "Soundtrack"],
    "Other releases": ["Other"],
  };

  /** Types beyond what the section says, e.g. "Remix", or "EP" among live albums. */
  function tags(entry: ReleaseGroupEntry, section: string) {
    const implied = IMPLIED[section] ?? [];
    return [entry.releaseType, ...entry.secondaryTypes].filter(
      (type): type is string => type !== null && !implied.includes(type),
    );
  }

  /** Opens a web page in the browser rather than the app's window. */
</script>

<section class="discography-page" aria-label={t("discography.label", { name: artist?.name ?? t("column.artist") })}>
  <header class="hero">
    <div class="identity">
      <p class="kicker muted small">{t("discography.kicker")}</p>
      <h1>{artist?.name ?? ""}</h1>
      <p class="muted small" aria-live="polite">
        {#if loading}
          {t(discography ? "discography.checking" : "discography.fetching")}
        {:else if discography}
          {summary(discography)}
        {/if}
      </p>
    </div>
    <div class="actions">
      <button onclick={() => artist && load(artist.id, true)} disabled={loading || !artist}>
        <Icon name="refresh" />
        {t("discography.checkAgain")}
      </button>
      <button class="icon" title={t("header.back")} aria-label={t("header.back")} onclick={() => ui.back()}
        ><Icon name="close" /></button
      >
    </div>
  </header>

  {#if error}
    <p class="error muted" role="status">{error}</p>
  {/if}

  {#if discography}
    {#each sections as section (section.name)}
      {@const open = !collapsed.includes(section.name)}
      <section class="section" aria-labelledby="section-{section.name}">
        <h2 id="section-{section.name}">
          <button class="toggle" aria-expanded={open} onclick={() => toggle(section.name)}>
            <span class="chevron" class:open><Icon name="down" size="1.1em" /></span>
            {sectionName(section.name)} <span class="muted">{section.releases.length}</span>
          </button>
        </h2>
        {#if open}
          <ul class="releases">
            {#each section.releases as entry (entry.id)}
              <li>
                <span
                  class="year muted"
                  title={entry.firstReleaseDate ? formatDate(entry.firstReleaseDate) : t("discography.undated")}
                >
                  {year(entry)}
                </span>
                <span class="what">
                  <a href="https://musicbrainz.org/release-group/{entry.id}" onclick={openLink}>{entry.title}</a>
                  {#if entry.disambiguation}<span class="muted"> ({entry.disambiguation})</span>{/if}
                  {#if entry.artist !== discography.musicbrainzName}
                    <span class="credit muted small">{entry.artist}</span>
                  {/if}
                </span>
                <span class="tags">
                  {#each tags(entry, section.name) as tag (tag)}<span class="tag">{tag}</span>{/each}
                </span>
              </li>
            {/each}
          </ul>
        {/if}
      </section>
    {/each}

    <p class="source muted small">
      {t("discography.from")}
      <a href="https://musicbrainz.org/artist/{discography.musicbrainzId}" onclick={openLink}>MusicBrainz</a>{t(
        "discography.official",
      )}
      {#if discography.listed < discography.total}
        {t("discography.firstOnly", { listed: discography.listed, total: discography.total })}
      {/if}
    </p>
  {/if}
</section>

<style>
  .discography-page {
    height: 100%;
    overflow-y: auto;
    padding: 1rem 1.5rem 2rem;
  }

  .hero {
    display: flex;
    flex-wrap: wrap;
    align-items: flex-start;
    justify-content: space-between;
    gap: 1rem;
    margin-bottom: 1.5rem;
  }

  .identity {
    display: flex;
    flex-direction: column;
    gap: 0.3rem;
    min-width: 0;
    flex: 1 1 18rem;
  }

  p {
    margin: 0;
  }

  h1 {
    margin: 0;
    font-size: clamp(1.6rem, 3.2vw, 2.5rem);
    line-height: 1.15;
    overflow-wrap: anywhere;
  }

  h2 {
    font-size: 1rem;
    margin: 0 0 0.4rem;
  }

  .kicker {
    text-transform: uppercase;
    letter-spacing: 0.06em;
  }

  .small {
    font-size: 0.8rem;
  }

  .actions {
    display: flex;
    align-items: center;
    gap: 0.5rem;
  }

  .error {
    margin-bottom: 1rem;
  }

  .section {
    margin-bottom: 1.25rem;
    max-width: 52rem;
  }

  .toggle {
    display: inline-flex;
    align-items: center;
    gap: 0.35rem;
    padding: 0.2rem 0.3rem 0.2rem 0;
    border: none;
    background: none;
    font: inherit;
    font-weight: 600;
  }

  .toggle:hover:not(:disabled) {
    background: none;
    color: var(--accent);
  }

  .chevron {
    display: inline-flex;
    transform: rotate(-90deg);
    transition: transform 0.15s;
  }

  .chevron.open {
    transform: none;
  }

  .releases {
    list-style: none;
    margin: 0;
    padding: 0;
  }

  .releases li {
    display: grid;
    grid-template-columns: 3rem minmax(0, 1fr) auto;
    align-items: baseline;
    gap: 0.75rem;
    padding: 0.4rem 0;
    border-bottom: 1px solid var(--border);
  }

  .releases li:last-child {
    border-bottom: none;
  }

  .year {
    font-variant-numeric: tabular-nums;
  }

  .what {
    min-width: 0;
    overflow-wrap: anywhere;
  }

  .credit {
    display: block;
  }

  .tags {
    display: flex;
    flex-wrap: wrap;
    justify-content: flex-end;
    gap: 0.3rem;
  }

  .tag {
    font-size: 0.75rem;
    padding: 0.05rem 0.5rem;
    border-radius: 999px;
    background: var(--surface-2);
    color: var(--text-muted);
  }

  a {
    color: var(--accent);
    text-decoration: none;
  }

  a:hover {
    text-decoration: underline;
  }

  .source {
    margin-top: 1.5rem;
    max-width: 52rem;
  }

  @media (max-width: 640px) {
    .discography-page {
      padding: 1rem;
    }

    .releases li {
      grid-template-columns: 2.75rem minmax(0, 1fr);
    }

    .tags {
      grid-column: 2;
      justify-content: flex-start;
    }
  }
</style>
