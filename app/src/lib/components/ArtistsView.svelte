<script lang="ts">
  // Every artist in the library, credited on any track (a duet's artists
  // both), in the library's sort order with its ignored articles. A click
  // opens the artist's page; the menu plays, queues or hearts them, and a
  // row drags their tracks onto a playlist or the queue. The box above
  // narrows the list by name, ignoring case and accents.
  import { untrack } from "svelte";
  import { library as api, queue, type Group } from "$lib/api";
  import { artistFeatureItems } from "$lib/featureMenu";
  import { count, t } from "$lib/i18n";
  import { emptySelection, type Selection } from "$lib/selection";
  import { appearance } from "$lib/state/appearance.svelte";
  import { collection } from "$lib/state/collection.svelte";
  import { adHocRule, library } from "$lib/state/library.svelte";
  import { attempt } from "$lib/state/toasts.svelte";
  import { ui, type MenuItem } from "$lib/state/ui.svelte";
  import { playlistItems } from "$lib/trackMenu";
  import Heart from "./Heart.svelte";
  import Icon from "./Icon.svelte";
  import VirtualList from "./VirtualList.svelte";

  type Artist = { id: number; name: string; trackCount: number; favourite: boolean };

  /** The largest page `library_browse` gives (`MAX_PAGE_SIZE`). */
  const PAGE = 1000;
  const listRule = adHocRule(["artist"]);
  /** Their tracks album by album, oldest first. */
  const playRule = adHocRule(["artist", "album"], "year");

  let artists = $state.raw<Artist[] | null>(null);
  let query = $state("");
  let selection = $state<Selection>(emptySelection);

  $effect(() => {
    void [library.version, collection.version];
    untrack(async () => {
      artists = (await attempt(loadAll)) ?? artists;
    });
  });

  async function loadAll() {
    const groups: Group[] = [];
    for (let total = Infinity; groups.length < total;) {
      const page = await api.browse(listRule, [], groups.length, PAGE);
      total = page.total;
      if (page.groups.length === 0) break;
      groups.push(...page.groups);
    }
    // "Unknown artist" has no page to open.
    return groups.flatMap((group) =>
      typeof group.key === "number"
        ? [{ id: group.key, name: group.name, trackCount: group.trackCount, favourite: group.favourite }]
        : [],
    );
  }

  const folded = (text: string) => text.normalize("NFD").replace(/\p{M}/gu, "").toLocaleLowerCase();

  const shown = $derived.by(() => {
    const words = folded(query).split(/\s+/).filter(Boolean);
    if (!artists || words.length === 0) return artists ?? [];
    return artists.filter((artist) => {
      const name = folded(artist.name);
      return words.every((word) => name.includes(word));
    });
  });

  $effect(() => {
    void shown;
    selection = emptySelection;
  });

  const open = (artist: Artist) => ui.showArtist({ id: artist.id, name: artist.name });

  const play = (artist: Artist, shuffle = false) =>
    attempt(async () => {
      if (shuffle) await queue.setShuffle(true);
      await queue.playNode(playRule, [artist.id], true);
    });

  function menu(index: number, event: MouseEvent) {
    const artist = shown[index];
    if (!artist) return;
    const ref = { id: artist.id, name: artist.name };
    const ids = () => api.nodeTrackIds(playRule, [artist.id], true);
    const items: MenuItem[] = [
      { label: t("menu.play"), action: () => play(artist) },
      { label: t("library.shuffle"), action: () => play(artist, true) },
      { label: t("menu.playNext"), action: () => attempt(() => queue.addNode(playRule, [artist.id], true, true)) },
      { label: t("menu.addToQueue"), action: () => attempt(() => queue.addNode(playRule, [artist.id], true, false)) },
      { label: t("menu.addToPlaylist"), items: playlistItems(ids) },
      { separator: true },
      {
        label: artist.favourite ? t("heart.removeShort") : t("heart.addShort"),
        action: () => collection.setFavourite("artist", [artist.id], !artist.favourite),
      },
      { label: t("menu.goToArtist"), action: () => open(artist) },
      ...artistFeatureItems(ref),
    ];
    ui.openMenu(event, items);
  }

  function dragRows(rows: number[]) {
    const artist = shown[rows[0]];
    if (!artist) return null;
    return {
      payload: { kind: "tracks" as const, trackIds: () => api.nodeTrackIds(playRule, [artist.id], true) },
      label: artist.name,
    };
  }
</script>

<section class="view">
  <header>
    <h2>{t("artists.title")}</h2>
    {#if artists && artists.length > 0}
      <span class="muted">{count("count.artists", artists.length)}</span>
    {/if}
    <input
      class="filter"
      type="search"
      placeholder={t("artists.filter")}
      aria-label={t("artists.filter")}
      bind:value={query}
      onkeydown={(event) => {
        if (event.key === "Escape" && query !== "") {
          event.stopPropagation();
          query = "";
        }
      }}
    />
  </header>

  {#if artists !== null && artists.length === 0}
    <div class="empty">
      <Icon name="person" size="2.5rem" />
      <p>{t("artists.empty")}</p>
    </div>
  {:else if artists !== null && shown.length === 0}
    <p class="muted none">{t("artists.noMatch", { query: query.trim() })}</p>
  {:else if artists !== null}
    <div class="list">
      <VirtualList
        count={shown.length}
        rowHeight={appearance.rowHeight(40)}
        item={(index) => shown[index]}
        label={t("artists.title")}
        bind:selection
        onclick={(index, event) => {
          const artist = shown[index];
          if (artist && (event.target as HTMLElement).closest("button") === null) open(artist);
        }}
        onactivate={(index) => shown[index] && open(shown[index])}
        oncontextmenu={menu}
        ondragrow={dragRows}
      >
        {#snippet row(artist: Artist | undefined, index: number)}
          {#if artist}
            <div class="entry">
              <Icon name="person" />
              <span class="name">{artist.name}</span>
              <span class="muted small">{count("count.tracks", artist.trackCount)}</span>
              <Heart
                on={artist.favourite}
                label={artist.name}
                onchange={(on) => collection.setFavourite("artist", [artist.id], on)}
              />
              <button
                class="icon"
                title={t("library.more")}
                aria-label={t("library.moreFor", { name: artist.name })}
                onclick={(event) => {
                  event.stopPropagation();
                  menu(index, event);
                }}><Icon name="more" /></button
              >
            </div>
          {/if}
        {/snippet}
      </VirtualList>
    </div>
  {/if}
</section>

<style>
  .view {
    display: flex;
    flex-direction: column;
    height: 100%;
    min-height: 0;
  }

  header {
    display: flex;
    align-items: center;
    gap: 0.75rem;
    padding: 0.75rem 1rem;
  }

  h2 {
    margin: 0;
    font-size: 1.35rem;
  }

  .filter {
    margin-left: auto;
    width: min(16rem, 40%);
  }

  .list {
    flex: 1;
    min-height: 0;
    margin: 0 0.5rem 0.5rem;
    border: 1px solid var(--border);
    border-radius: var(--radius-lg);
  }

  .entry {
    display: flex;
    align-items: center;
    gap: 0.75rem;
    width: 100%;
    height: 100%;
    padding: 0 0.5rem 0 1rem;
    min-width: 0;
    cursor: pointer;
  }

  .name {
    flex: 1;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .small {
    font-size: 0.8rem;
    white-space: nowrap;
  }

  .none {
    padding: 0 1rem;
  }

  .empty {
    display: grid;
    place-items: center;
    align-content: center;
    gap: 1rem;
    flex: 1;
    color: var(--text-muted);
    text-align: center;
    padding: 1rem;
  }

  .empty p {
    margin: 0;
    max-width: 28rem;
  }
</style>
