<script lang="ts">
  // A row (or grid) of albums with their covers, as the Home and History
  // views and "More in this genre" list them: click to open the album,
  // the play button to play it, right-click for more.
  import { t } from "$lib/i18n";
  import { queue, type AlbumCard, type FolderState } from "$lib/api";
  import { FOLDER_SHORT } from "$lib/folders";
  import { adHocRule, library } from "$lib/state/library.svelte";
  import { attempt } from "$lib/state/toasts.svelte";
  import { ui } from "$lib/state/ui.svelte";
  import Art from "./Art.svelte";
  import Icon from "./Icon.svelte";

  /** An album none of whose tracks can be opened now, with its folder's state (H22b), is dimmed. */
  type Card = AlbumCard & { unavailable?: FolderState | null };

  let { albums, wrap = false, label }: { albums: Card[]; wrap?: boolean; label: string } = $props();

  const albumRule = adHocRule(["album"]);

  const play = (album: AlbumCard) => attempt(() => queue.playNode(albumRule, [album.id], true));

  function open(album: AlbumCard) {
    const shown = library.showAlbum({
      id: album.id,
      title: album.title,
      albumArtist: album.artist,
      albumArtistId: album.artistId,
    });
    if (!shown) play(album);
  }

  function menu(event: MouseEvent, album: AlbumCard) {
    ui.openMenu(event, [
      { label: t("menu.play"), action: () => play(album) },
      { label: t("menu.playNext"), action: () => attempt(() => queue.addNode(albumRule, [album.id], true, true)) },
      { label: t("menu.addToQueue"), action: () => attempt(() => queue.addNode(albumRule, [album.id], true, false)) },
      { label: t("menu.openAlbum"), action: () => open(album) },
      {
        label: t("menu.goToArtist"),
        disabled: album.artistId === null,
        action: () => {
          if (album.artistId !== null) ui.showArtist({ id: album.artistId, name: album.artist ?? "" });
        },
      },
    ]);
  }
</script>

<ul class="cards" class:wrap aria-label={label}>
  {#each albums as album (album.id)}
    <li>
      <div
        class="card"
        class:unavailable={album.unavailable}
        role="group"
        aria-label={album.title}
        title={album.unavailable ? t(FOLDER_SHORT[album.unavailable]) : undefined}
        oncontextmenu={(event) => menu(event, album)}
      >
        <div class="art">
          <button class="cover" title={t("library.openName", { name: album.title })} onclick={() => open(album)}>
            <Art albumId={album.id} size="100%" quality="header" />
          </button>
          <button
            class="icon play"
            title={t("library.playName", { name: album.title })}
            aria-label={t("library.playName", { name: album.title })}
            onclick={() => play(album)}><Icon name="play" /></button
          >
        </div>
        <span class="title" title={album.title}>{album.title}</span>
        <span class="muted small">{[album.artist, album.year].filter((part) => part !== null).join(" · ")}</span>
        {#if album.note}<span class="note small">{album.note}</span>{/if}
      </div>
    </li>
  {/each}
</ul>

<style>
  .cards {
    list-style: none;
    margin: 0;
    padding: 0.25rem 0 0.75rem;
    display: grid;
    grid-auto-flow: column;
    grid-auto-columns: 9.5rem;
    gap: 1rem;
    overflow-x: auto;
  }

  .cards.wrap {
    grid-auto-flow: row;
    grid-template-columns: repeat(auto-fill, minmax(9rem, 1fr));
    overflow-x: visible;
  }

  .card.unavailable {
    opacity: 0.45;
  }

  .card {
    display: flex;
    flex-direction: column;
    min-width: 0;
    gap: 0.1rem;
  }

  .art {
    position: relative;
    margin-bottom: 0.3rem;
  }

  .cover {
    display: block;
    padding: 0;
    border: none;
    background: none;
    aspect-ratio: 1;
    width: 100%;
    border-radius: var(--radius);
    overflow: hidden;
  }

  .play {
    position: absolute;
    right: 0.4rem;
    bottom: 0.4rem;
    background: var(--surface) !important;
    box-shadow: var(--shadow);
    border-radius: 50%;
    opacity: 0;
  }

  .card:hover .play,
  .play:focus-visible {
    opacity: 1;
  }

  .title,
  .small {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .title {
    font-weight: 600;
    font-size: 0.9rem;
  }

  .small {
    font-size: 0.78rem;
  }

  .note {
    color: var(--accent);
  }

  @media (hover: none) {
    .play {
      opacity: 1;
    }
  }
</style>
