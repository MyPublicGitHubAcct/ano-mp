<script lang="ts">
  // An artist's page: who they are (from MusicBrainz), their biography (from
  // Wikipedia, credited under its licence; unless the settings turn
  // descriptions off), their albums grouped by release
  // type, and the albums of others they appear on. Once matched on
  // MusicBrainz, it links to their releases the library lacks
  // (DiscographyPage). Reloads when the
  // metadata worker names the artist in `metadata-changed`. Click an album
  // to open it in the browser, double-click to play it. The heart marks the
  // artist a favourite (PLAN.md F3).
  import { webLink } from "$lib/links";
  import { openLink } from "$lib/openLink";
  import { untrack } from "svelte";
  import { library as api, marks, metadata, queue, type ArtistAlbum, type ArtistPage } from "$lib/api";
  import { lifeSpan } from "$lib/format";
  import { count, errorText, t } from "$lib/i18n";
  import { collection } from "$lib/state/collection.svelte";
  import { appSettings } from "$lib/state/settings.svelte";
  import { bySection, sectionName } from "$lib/releases";
  import { adHocRule, library } from "$lib/state/library.svelte";
  import { player } from "$lib/state/player.svelte";
  import { attempt, toasts } from "$lib/state/toasts.svelte";
  import { ui, type MenuItem } from "$lib/state/ui.svelte";
  import Art from "./Art.svelte";
  import Heart from "./Heart.svelte";
  import Icon from "./Icon.svelte";

  /** Paragraphs of a biography shown before "Read more". */
  const SHORT_BIOGRAPHY = 2;
  const GENRES_SHOWN = 6;

  let page = $state.raw<ArtistPage | null>(null);
  let missing = $state(false);
  let expanded = $state(false);
  let lookingUp = $state(false);
  let request = 0;
  let shownId: number | null = null;

  const artist = $derived(ui.artist);

  // Another artist: start over. A scan or new details for this one: reload
  // in place.
  $effect(() => {
    const id = artist?.id;
    if (id === undefined) return;
    void [library.version, library.artistVersions.get(id)];
    untrack(() => {
      if (id !== shownId) {
        shownId = id;
        page = null;
        missing = false;
        expanded = false;
      }
      load(id);
    });
  });

  async function load(id: number) {
    const current = ++request;
    try {
      const loaded = await api.artist(id);
      if (current === request) page = loaded;
    } catch (error) {
      if (current !== request) return;
      page = null;
      missing = true;
      toasts.show(errorText(error));
    }
  }

  const lookUp = () =>
    attempt(async () => {
      if (!page) return;
      const id = page.id;
      lookingUp = true;
      try {
        await metadata.updateArtist(id);
      } finally {
        lookingUp = false;
        // Also when nothing changed, to show when it was tried.
        if (artist?.id === id) await load(id);
      }
    });

  const mb = $derived(page?.info.musicbrainz ?? null);
  const biography = $derived(page?.info.biography ?? null);
  const showDescriptions = $derived(appSettings.display.showDescriptions);
  const paragraphs = $derived(
    biography === null ? [] : expanded ? biography.paragraphs : biography.paragraphs.slice(0, SHORT_BIOGRAPHY),
  );
  const busy = $derived(lookingUp || (page !== null && library.lookingUp === page.id));

  // Albums without a type (no match yet) are albums.
  const sections = $derived(bySection(page?.albums ?? []));

  function status() {
    if (!page) return "";
    const { info, name } = page;
    if (busy) return t("artist.lookingUp", { name });
    switch (info.status) {
      case null:
        return t(info.canLookUp ? "artist.notLookedUp" : "artist.onlineOff");
      case "none":
        return t(info.chosenByUser ? "artist.youSaidNone" : "artist.notFound", { name });
      case "review":
        return t("artist.review", { name });
      case "matched":
        return showDescriptions ? t("artist.noBiography") : "";
    }
  }

  const albumRule = adHocRule(["album"]);

  let hearted = $state(false);
  $effect(() => {
    const id = artist?.id;
    void collection.version;
    if (id === undefined) return;
    untrack(async () => {
      hearted = (await marks.favouritesAmong("artist", [id]).catch(() => [])).length > 0;
    });
  });
  async function heart(on: boolean) {
    if (!page) return;
    hearted = on;
    await collection.setFavourite("artist", [page.id], on);
  }

  /** Plays the artist's albums oldest first, or the tracks they appear on. */
  function playAll(shuffle: boolean) {
    const current = page;
    if (!current) return;
    const own = current.albums.length > 0;
    const rule = adHocRule([own ? "albumArtist" : "artist", "album"], "year");
    return attempt(async () => {
      if (shuffle && !player.shuffle) await queue.setShuffle(true);
      await queue.playNode(rule, [current.id], true);
    });
  }

  const playAlbum = (album: ArtistAlbum) => attempt(() => queue.playNode(albumRule, [album.id], true));

  function openAlbum(album: ArtistAlbum) {
    if (!library.showAlbum(album)) playAlbum(album);
  }

  function albumMenu(event: MouseEvent, album: ArtistAlbum) {
    const items: MenuItem[] = [
      { label: t("menu.play"), action: () => playAlbum(album) },
      { label: t("menu.playNext"), action: () => attempt(() => queue.addNode(albumRule, [album.id], true, true)) },
      { label: t("menu.addToQueue"), action: () => attempt(() => queue.addNode(albumRule, [album.id], true, false)) },
      { label: t("menu.openAlbum"), action: () => openAlbum(album) },
    ];
    if (album.albumArtistId !== null && album.albumArtistId !== page?.id && album.albumArtist !== null) {
      const other = { id: album.albumArtistId, name: album.albumArtist };
      items.push({ label: t("artist.goTo", { name: other.name }), action: () => ui.showArtist(other) });
    }
    const ref = { id: album.id, title: album.title };
    items.push(
      { label: t("menu.findDetails"), action: () => (ui.dialog = { kind: "findDetails", album: ref }) },
      { label: t("menu.chooseCover"), action: () => (ui.dialog = { kind: "chooseCover", album: ref }) },
    );
    ui.openMenu(event, items);
  }

  /** Opens "Find artist" to pick the MusicBrainz artist. */
  function findArtist() {
    if (page) ui.dialog = { kind: "findArtist", artist: { id: page.id, name: page.name }, info: page.info };
  }

  /** Opens a web page in the browser rather than the app's window. */
</script>

{#snippet albumGrid(albums: ArtistAlbum[], showArtist: boolean)}
  <ul class="albums">
    {#each albums as album (album.id)}
      <li>
        <button
          class="album"
          onclick={() => openAlbum(album)}
          ondblclick={() => playAlbum(album)}
          oncontextmenu={(event) => albumMenu(event, album)}
        >
          <Art albumId={album.id} size="100%" />
          <span class="name" title={album.title}>{album.title}</span>
          <span class="muted small">
            {[
              showArtist ? album.albumArtist : null,
              album.year,
              showArtist ? null : count("count.tracks", album.trackCount),
            ]
              .filter((part) => part !== null)
              .join(" · ")}
          </span>
        </button>
      </li>
    {/each}
  </ul>
{/snippet}

<section class="artist-page" aria-label={artist?.name ?? t("column.artist")}>
  <header class="hero">
    <div class="identity">
      {#if mb && (mb.type || mb.area)}
        <p class="kicker muted small">{[mb.type, mb.area].filter(Boolean).join(" · ")}</p>
      {/if}
      <h1>{page?.name ?? artist?.name ?? ""}</h1>
      {#if mb?.disambiguation}<p class="muted">{mb.disambiguation}</p>{/if}
      {#if mb && lifeSpan(mb)}<p class="life">{lifeSpan(mb)}</p>{/if}
      {#if page}
        <p class="muted small">
          {[
            page.albums.length > 0 ? count("count.albums", page.albums.length) : null,
            page.appearsOn.length > 0 ? count("artist.onOtherAlbums", page.appearsOn.length) : null,
            count("count.tracks", page.trackCount),
          ]
            .filter(Boolean)
            .join(" · ")}
        </p>
      {/if}
      {#if mb && mb.genres.length > 0}
        <ul class="genres" aria-label={t("album.field.genres")}>
          {#each mb.genres.slice(0, GENRES_SHOWN) as genre (genre)}<li>{genre}</li>{/each}
        </ul>
      {/if}
    </div>
    <div class="actions">
      {#if page}<Heart on={hearted} label={page.name} onchange={heart} size="1.2rem" />{/if}
      <button class="primary" onclick={() => playAll(false)} disabled={!page || page.trackCount === 0}>
        <Icon name="play" />
        {t("menu.play")}
      </button>
      <button onclick={() => playAll(true)} disabled={!page || page.trackCount === 0}>
        <Icon name="shuffle" />
        {t("bar.shuffle")}
      </button>
      <button class="icon" title={t("header.back")} aria-label={t("header.back")} onclick={() => ui.back()}
        ><Icon name="close" /></button
      >
    </div>
  </header>

  {#if missing}
    <p class="muted">{t("artist.gone")}</p>
  {:else if page === null}
    <p class="muted">{t("common.loading")}</p>
  {:else}
    <section class="about" aria-labelledby="about-heading">
      <h2 id="about-heading">{t("artist.about")}</h2>
      {#if biography && showDescriptions}
        <div class="biography">
          {#each paragraphs as paragraph, index (index)}<p>{paragraph}</p>{/each}
        </div>
        {#if biography.paragraphs.length > SHORT_BIOGRAPHY}
          <button class="link" onclick={() => (expanded = !expanded)}
            >{t(expanded ? "album.showLess" : "album.readMore")}</button
          >
        {/if}
        <p class="credit muted small">
          {t("album.creditFrom", { source: biography.sourceName })}
          <a href={webLink(biography.url)} onclick={openLink}>“{biography.title}”</a>{t("album.creditUnder")}
          <a href={webLink(biography.licenseUrl)} onclick={openLink}>{biography.license}</a>.
        </p>
      {:else if status()}
        <p class="status muted" aria-live="polite">{status()}</p>
      {/if}
      <div class="links">
        {#if page.info.canLookUp && page.info.status === "review"}
          <button class="primary" onclick={findArtist} disabled={busy}
            ><Icon name="search" /> {t("artist.choose")}</button
          >
        {/if}
        {#if page.info.canLookUp && !biography && page.info.status !== "review"}
          <button onclick={lookUp} disabled={busy}>
            <Icon name="refresh" />
            {t(page.info.status === null ? "artist.lookUp" : "artist.lookUpAgain")}
          </button>
        {/if}
        {#if mb}
          <button class="link" onclick={() => page && ui.showDiscography({ id: page.id, name: page.name })}>
            {t("artist.missingReleases")}
          </button>
          <a href="https://musicbrainz.org/artist/{mb.id}" onclick={openLink}>MusicBrainz</a>
          {#if mb.homepage}<a href={webLink(mb.homepage)} onclick={openLink}>{t("artist.website")}</a>{/if}
        {/if}
        {#if page.info.canLookUp && page.info.status !== "review"}
          <button class="link" onclick={findArtist}>
            {t(page.info.status === "matched" ? "artist.wrong" : "artist.search")}
          </button>
        {/if}
      </div>
    </section>

    {#each sections as section (section.name)}
      <section class="discography" aria-label={sectionName(section.name)}>
        <h2>{sectionName(section.name)} <span class="muted">{section.releases.length}</span></h2>
        {@render albumGrid(section.releases, false)}
      </section>
    {/each}

    {#if page.appearsOn.length > 0}
      <section class="discography" aria-label={t("artist.appearsOn")}>
        <h2>{t("artist.appearsOn")} <span class="muted">{page.appearsOn.length}</span></h2>
        {@render albumGrid(page.appearsOn, true)}
      </section>
    {/if}

    {#if page.albums.length === 0 && page.appearsOn.length === 0}
      <p class="muted">{t("artist.noAlbums")}</p>
    {/if}
  {/if}
</section>

<style>
  .artist-page {
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
    margin: 0 0 0.6rem;
  }

  .kicker {
    text-transform: uppercase;
    letter-spacing: 0.06em;
  }

  .life {
    font-weight: 500;
  }

  .small {
    font-size: 0.8rem;
  }

  .genres {
    display: flex;
    flex-wrap: wrap;
    gap: 0.35rem;
    list-style: none;
    margin: 0.35rem 0 0;
    padding: 0;
  }

  .genres li {
    font-size: 0.8rem;
    padding: 0.1rem 0.55rem;
    border-radius: 999px;
    background: var(--surface-2);
    color: var(--text-muted);
  }

  .actions {
    display: flex;
    align-items: center;
    gap: 0.5rem;
  }

  .about {
    max-width: 46rem;
    margin-bottom: 2rem;
  }

  .biography {
    display: flex;
    flex-direction: column;
    gap: 0.7rem;
    line-height: 1.55;
  }

  .about > .link {
    margin-top: 0.5rem;
  }

  .credit {
    margin-top: 0.75rem;
  }

  a {
    color: var(--accent);
    text-decoration: none;
  }

  a:hover {
    text-decoration: underline;
  }

  .links {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 0.5rem 1rem;
    margin-top: 0.75rem;
  }

  .links:empty {
    display: none;
  }

  .discography {
    margin-bottom: 1.75rem;
  }

  .albums {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(8.5rem, 1fr));
    gap: 1rem;
    list-style: none;
    margin: 0;
    padding: 0;
  }

  .album {
    display: flex;
    flex-direction: column;
    align-items: stretch;
    gap: 0.25rem;
    width: 100%;
    padding: 0;
    border: none;
    background: none;
    text-align: left;
  }

  .album:hover:not(:disabled) {
    background: none;
  }

  .album:hover .name {
    color: var(--accent);
  }

  .album :global(.art) {
    aspect-ratio: 1;
    height: auto !important;
  }

  .name,
  .album .small {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    max-width: 100%;
  }

  @media (max-width: 640px) {
    .artist-page {
      padding: 1rem;
    }

    .albums {
      grid-template-columns: repeat(auto-fill, minmax(7rem, 1fr));
      gap: 0.75rem;
    }
  }
</style>
