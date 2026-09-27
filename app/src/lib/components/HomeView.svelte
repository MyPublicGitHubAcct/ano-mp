<script lang="ts">
  // Ways into the library besides browsing it, each shown while its
  // feature is on: albums released on this day in earlier years (O17),
  // recently played with runs from one album together (O16), recently
  // added grouped by this week, this month and earlier (O15), and the
  // history's highlights (O8): favourites not played for a year, what was
  // playing a year ago today, and albums never played.
  import { count, t } from "$lib/i18n";
  import { untrack } from "svelte";
  import { features as api, queue, type AlbumCard, type Highlights, type RecentEntry } from "$lib/api";
  import { features } from "$lib/state/features.svelte";
  import { library } from "$lib/state/library.svelte";
  import { attempt } from "$lib/state/toasts.svelte";
  import { ui } from "$lib/state/ui.svelte";
  import AlbumCards from "./AlbumCards.svelte";
  import Icon from "./Icon.svelte";

  const f = $derived(features.on);

  let onThisDay = $state.raw<AlbumCard[] | null>(null);
  let recent = $state.raw<RecentEntry[] | null>(null);
  let added = $state.raw<AlbumCard[] | null>(null);
  let highlights = $state.raw<Highlights | null>(null);

  const quiet = <T,>(promise: Promise<T>) => promise.catch(() => null);

  $effect(() => {
    void [library.version, features.historyVersion, f.onThisDay, f.recentlyPlayed, f.recentlyAdded, f.listeningHistory];
    untrack(async () => {
      [onThisDay, recent, added, highlights] = await Promise.all([
        f.onThisDay ? quiet(api.onThisDay(new Date())) : null,
        f.listeningHistory && f.recentlyPlayed ? quiet(api.recentlyPlayed(12)) : null,
        f.recentlyAdded ? quiet(api.recentlyAdded(120)) : null,
        f.listeningHistory ? quiet(api.highlights()) : null,
      ]);
    });
  });

  /** Recently added in groups: this week, this month, earlier. */
  const addedGroups = $derived.by(() => {
    if (!added) return [];
    const now = Date.now() / 1000;
    const groups: { name: string; albums: AlbumCard[] }[] = [
      { name: t("home.addedWeek"), albums: [] },
      { name: t("home.addedMonth"), albums: [] },
      { name: t("home.addedEarlier"), albums: [] },
    ];
    for (const album of added) {
      const age = now - (album.at ?? 0);
      groups[age < 7 * 86400 ? 0 : age < 31 * 86400 ? 1 : 2].albums.push(album);
    }
    return groups.filter((group) => group.albums.length > 0);
  });

  const recentAlbums = $derived(
    (recent ?? [])
      .filter((entry) => entry.album !== null)
      .map((entry) => ({
        ...entry.album!,
        note: entry.tracks.length > 1 ? count("count.tracks", entry.tracks.length) : (entry.tracks[0]?.title ?? null),
      })),
  );
  const recentTracks = $derived((recent ?? []).filter((entry) => entry.album === null));

  const nothing = $derived(
    !f.onThisDay && !f.recentlyAdded && !(f.listeningHistory && f.recentlyPlayed) && !f.listeningHistory,
  );
</script>

<section class="home" aria-labelledby="home-heading">
  <header>
    <h1 id="home-heading">{t("home.title")}</h1>
  </header>

  {#if nothing}
    <p class="muted">
      {t("home.nothingBefore")}
      <button class="link" onclick={() => ui.showSettings("features")}>{t("health.settingsFeatures")}</button>
      {t("home.nothingAfter")}
    </p>
  {/if}

  {#if onThisDay && onThisDay.length > 0}
    <h2>{t("home.onThisDay")}</h2>
    <AlbumCards albums={onThisDay} label={t("home.onThisDay")} />
  {/if}

  {#if recent && recent.length > 0}
    <h2>{t("history.recent")}</h2>
    {#if recentAlbums.length > 0}<AlbumCards albums={recentAlbums} label={t("history.recent")} />{/if}
    {#if recentTracks.length > 0}
      <ul class="tracks">
        {#each recentTracks as entry (entry.playedAt)}
          {@const track = entry.tracks[0]}
          <li>
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

  {#if highlights && highlights.yearAgo.length > 0}
    <h2>{t("home.yearAgo")}</h2>
    <AlbumCards albums={highlights.yearAgo} label={t("home.yearAgoLabel")} />
  {/if}

  {#if highlights && highlights.forgotten.length > 0}
    <h2>{t("home.forgotten")}</h2>
    <AlbumCards albums={highlights.forgotten} label={t("home.forgottenLabel")} />
  {/if}

  {#each addedGroups as group (group.name)}
    <h2>{group.name}</h2>
    <AlbumCards albums={group.albums} label={group.name} />
  {/each}

  {#if highlights && highlights.neverPlayed.length > 0}
    <h2>{t("home.neverPlayed")}</h2>
    <AlbumCards albums={highlights.neverPlayed} label={t("home.neverPlayed")} />
  {/if}
</section>

<style>
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
