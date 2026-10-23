<script lang="ts">
  // Ways into the library besides browsing it, each shown while its
  // feature is on, in this order: artists outside the library like the
  // ones the user plays (X5), albums released on this day in earlier
  // years (O17), recently played with runs from one album together
  // (O16), the history's highlights (O8: what was playing a year ago
  // today, favourites not played for a year), recently added grouped by
  // this week, this month and earlier (O15), albums never played (O8),
  // and albums like what the user plays, not played lately (X4). The
  // recommendations fold away, closed until opened. Each section can be
  // hidden in Settings › Display (`hiddenOnHome`), and isn't fetched then.
  import { count, t } from "$lib/i18n";
  import { untrack } from "svelte";
  import {
    features as api,
    outside,
    queue,
    type AlbumCard,
    type Highlights,
    type HomeSection,
    type RecentEntry,
    type OutsideArtist,
    type SimilarAlbum,
  } from "$lib/api";
  import { FOLDER_SHORT, unavailableEntryState, unavailableState } from "$lib/folders";
  import { HOME_SECTIONS, homeSectionAvailable } from "$lib/home";
  import { reasonsText } from "$lib/similar";
  import { features } from "$lib/state/features.svelte";
  import { library } from "$lib/state/library.svelte";
  import { appSettings } from "$lib/state/settings.svelte";
  import { attempt } from "$lib/state/toasts.svelte";
  import { ui } from "$lib/state/ui.svelte";
  import AlbumCards from "./AlbumCards.svelte";
  import Fold from "./Fold.svelte";
  import Icon from "./Icon.svelte";
  import OutsideArtists from "./OutsideArtists.svelte";

  const f = $derived(features.on);
  const hidden = $derived(appSettings.display.hiddenOnHome);

  /** Whether a section shows: its features are on and the user hasn't hidden it. */
  const shows = (section: HomeSection) => homeSectionAvailable(section, f) && !hidden.includes(section);

  // What to fetch, as booleans, so saving an unrelated setting fetches nothing.
  const wantOnThisDay = $derived(shows("onThisDay"));
  const wantRecent = $derived(shows("recentlyPlayed"));
  const wantAdded = $derived(shows("addedWeek") || shows("addedMonth") || shows("addedEarlier"));
  const wantHighlights = $derived(shows("yearAgo") || shows("forgotten") || shows("neverPlayed"));
  const wantForYou = $derived(shows("forYou"));
  const wantOutside = $derived(shows("outside"));

  let onThisDay = $state.raw<AlbumCard[] | null>(null);
  let recent = $state.raw<RecentEntry[] | null>(null);
  let added = $state.raw<AlbumCard[] | null>(null);
  let highlights = $state.raw<Highlights | null>(null);
  let forYou = $state.raw<SimilarAlbum[] | null>(null);
  let outsideArtists = $state.raw<OutsideArtist[]>([]);

  const quiet = <T,>(promise: Promise<T>) => promise.catch(() => null);

  $effect(() => {
    // Folders coming and going change which albums are suggested (H22b).
    void [
      library.version,
      library.unreadableKey,
      features.historyVersion,
      wantOnThisDay,
      wantRecent,
      wantAdded,
      wantHighlights,
      wantForYou,
    ];
    untrack(async () => {
      [onThisDay, recent, added, highlights, forYou] = await Promise.all([
        wantOnThisDay ? quiet(api.onThisDay(new Date())) : null,
        wantRecent ? quiet(api.recentlyPlayed(12)) : null,
        wantAdded ? quiet(api.recentlyAdded(120)) : null,
        wantHighlights ? quiet(api.highlights()) : null,
        wantForYou ? quiet(api.forYou()) : null,
      ]);
    });
  });

  // Apart from the rest: ListenBrainz can take a while to answer.
  $effect(() => {
    void [library.version, features.historyVersion, wantOutside];
    untrack(async () => {
      outsideArtists = wantOutside ? ((await quiet(outside.forYou())) ?? []) : [];
    });
  });

  /** Recently added in groups: this week, this month, earlier. */
  const addedGroups = $derived.by(() => {
    if (!added) return [];
    const now = Date.now() / 1000;
    const groups: { id: HomeSection; name: string; albums: AlbumCard[] }[] = [
      { id: "addedWeek", name: t("home.addedWeek"), albums: [] },
      { id: "addedMonth", name: t("home.addedMonth"), albums: [] },
      { id: "addedEarlier", name: t("home.addedEarlier"), albums: [] },
    ];
    for (const album of added) {
      const age = now - (album.at ?? 0);
      groups[age < 7 * 86400 ? 0 : age < 31 * 86400 ? 1 : 2].albums.push(album);
    }
    return groups.filter((group) => group.albums.length > 0 && shows(group.id));
  });

  const recentAlbums = $derived(
    (recent ?? [])
      .filter((entry) => entry.album !== null)
      .map((entry) => ({
        ...entry.album!,
        note: entry.tracks.length > 1 ? count("count.tracks", entry.tracks.length) : (entry.tracks[0]?.title ?? null),
        unavailable: unavailableEntryState(
          library.unreadable,
          entry.tracks.map((track) => track.folderId),
        ),
      })),
  );
  const recentTracks = $derived((recent ?? []).filter((entry) => entry.album === null));

  const forYouCards = $derived(
    (forYou ?? []).map(({ album, reasons }) => ({ ...album, note: reasonsText(reasons, t) })),
  );

  const nothing = $derived(!HOME_SECTIONS.some(({ id }) => homeSectionAvailable(id, f)));
  const allHidden = $derived(!nothing && !HOME_SECTIONS.some(({ id }) => shows(id)));
  const chooseSections = () => ui.showSettings("display", "setting-hiddenOnHome");
</script>

<section class="home" aria-labelledby="home-heading">
  <header>
    <h1 id="home-heading">{t("home.title")}</h1>
    <button onclick={chooseSections}><Icon name="sliders" /> {t("home.chooseSections")}</button>
  </header>

  {#if nothing}
    <p class="muted">
      {t("home.nothingBefore")}
      <button class="link" onclick={() => ui.showSettings("features")}>{t("health.settingsFeatures")}</button>
      {t("home.nothingAfter")}
    </p>
  {:else if allHidden}
    <p class="muted">
      {t("home.allHidden")}
      <button class="link" onclick={chooseSections}>{t("home.chooseSections")}</button>
    </p>
  {/if}

  {#if wantOutside && outsideArtists.length > 0}
    <Fold key="home.outside" heading={t("home.outside")}>
      <OutsideArtists
        artists={outsideArtists}
        label={t("home.outsideLabel")}
        ondismissed={(mbid) => (outsideArtists = outsideArtists.filter((artist) => artist.mbid !== mbid))}
      />
    </Fold>
  {/if}

  {#if wantOnThisDay && onThisDay && onThisDay.length > 0}
    <h2>{t("home.onThisDay")}</h2>
    <AlbumCards albums={onThisDay} label={t("home.onThisDay")} />
  {/if}

  {#if wantRecent && recent && recent.length > 0}
    <h2>{t("history.recent")}</h2>
    {#if recentAlbums.length > 0}<AlbumCards albums={recentAlbums} label={t("history.recent")} />{/if}
    {#if recentTracks.length > 0}
      <ul class="tracks">
        {#each recentTracks as entry (entry.playedAt)}
          {@const track = entry.tracks[0]}
          {@const unavailable = unavailableState(library.unreadable, track.folderId)}
          <li class:unavailable title={unavailable ? t(FOLDER_SHORT[unavailable]) : undefined}>
            <button class="link" onclick={() => attempt(() => queue.play([track.trackId], 0))}>
              <Icon name="play" size="0.9rem" />
              {track.title}
            </button>
            <span class="muted">{track.artist ?? ""}</span>
          </li>
        {/each}
      </ul>
    {/if}
  {/if}

  {#if shows("yearAgo") && highlights && highlights.yearAgo.length > 0}
    <h2>{t("home.yearAgo")}</h2>
    <AlbumCards albums={highlights.yearAgo} label={t("home.yearAgoLabel")} />
  {/if}

  {#if shows("forgotten") && highlights && highlights.forgotten.length > 0}
    <h2>{t("home.forgotten")}</h2>
    <AlbumCards albums={highlights.forgotten} label={t("home.forgottenLabel")} />
  {/if}

  {#each addedGroups as group (group.name)}
    <h2>{group.name}</h2>
    <AlbumCards albums={group.albums} label={group.name} />
  {/each}

  {#if shows("neverPlayed") && highlights && highlights.neverPlayed.length > 0}
    <h2>{t("home.neverPlayed")}</h2>
    <AlbumCards albums={highlights.neverPlayed} label={t("home.neverPlayed")} />
  {/if}

  {#if wantForYou && forYouCards.length > 0}
    <Fold key="home.forYou" heading={t("home.forYou")}>
      <AlbumCards albums={forYouCards} label={t("home.forYouLabel")} />
    </Fold>
  {/if}
</section>

<style>
  .unavailable {
    opacity: 0.45;
  }

  .home {
    height: 100%;
    overflow-y: auto;
    padding: 0.75rem 1.25rem 2rem;
  }

  header {
    display: flex;
    align-items: center;
    justify-content: space-between;
  }

  h1 {
    margin: 0 0 0.5rem;
    font-size: 1.6rem;
  }

  h2 {
    font-size: 1.05rem;
    margin: 1rem 0 0.4rem;
  }

  .tracks {
    list-style: none;
    padding: 0;
    margin: 0;
  }

  .tracks li {
    display: flex;
    gap: 0.75rem;
    padding: 0.2rem 0;
  }
</style>
