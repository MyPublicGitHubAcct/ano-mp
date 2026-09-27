<script lang="ts">
  // The current track alone: its title, artist and album on the left and its
  // cover, as large as the area allows, on the right. Narrow windows put the
  // cover on top.
  import { t } from "$lib/i18n";
  import { features } from "$lib/state/features.svelte";
  import { player } from "$lib/state/player.svelte";
  import { ui } from "$lib/state/ui.svelte";
  import Art from "./Art.svelte";
  import Icon from "./Icon.svelte";
  import LyricsPanel from "./LyricsPanel.svelte";

  const item = $derived(player.currentItem);
</script>

<section class="now-playing" aria-label={t("nowPlaying.label")}>
  <div class="details">
    {#if item}
      <p class="state muted small">{t(player.playing ? "nowPlaying.label" : "nowPlaying.paused")}</p>
      <h1 title={item.title}>{item.title}</h1>
      {#if item.artist && item.artistId !== null}
        {@const artist = { id: item.artistId, name: item.artist }}
        <p class="artist">
          <button class="link" title={t("nowPlaying.showArtist", { name: artist.name })} onclick={() => ui.showArtist(artist)}>{artist.name}</button>
        </p>
      {:else if item.artist}
        <p class="artist">{item.artist}</p>
      {/if}
      {#if item.album}<p class="album muted">{item.album}</p>{/if}
      {#if item.reason}<p class="reason small">{t("queue.radioReason", { reason: item.reason })}</p>{/if}
      {#if player.current !== null && player.items.length > 1}
        <p class="position muted small">
          {t("nowPlaying.position", { position: player.current + 1, count: player.items.length })}
        </p>
      {/if}
    {:else}
      <h1 class="muted">{t("bar.notPlaying")}</h1>
      <p class="muted">{t("nowPlaying.hint")}</p>
    {/if}
    {#if item && features.on.lyrics}
      {#key item.trackId}<LyricsPanel trackId={item.trackId} />{/key}
    {/if}
    <div class="actions">
      <button class="link" onclick={() => ui.showVisualizer()}>
        <Icon name="wave" size="1rem" /> {t("bar.visualizer")}
      </button>
      <button class="link" onclick={() => ui.back()}>
        <Icon name="close" size="1rem" /> {t("dialog.close")}
      </button>
    </div>
  </div>
  <div class="cover">
    <div class="frame">
      <Art albumId={item?.albumId ?? null} trackId={item?.trackId ?? null} size="100%" />
    </div>
  </div>
</section>

<style>
  .now-playing {
    height: 100%;
    display: grid;
    grid-template-columns: minmax(12rem, 1fr) minmax(0, 1.6fr);
    gap: 2rem;
    padding: 1.5rem 2rem 2rem;
    overflow: auto;
  }

  .details {
    display: flex;
    flex-direction: column;
    justify-content: center;
    gap: 0.35rem;
    min-width: 0;
  }

  p {
    margin: 0;
  }

  h1 {
    margin: 0 0 0.25rem;
    font-size: clamp(1.5rem, 3vw, 2.4rem);
    line-height: 1.15;
    overflow-wrap: anywhere;
  }

  .state {
    text-transform: uppercase;
    letter-spacing: 0.06em;
  }

  .artist {
    font-size: 1.2rem;
    font-weight: 600;
  }

  .artist .link {
    font-weight: inherit;
    text-align: left;
  }

  .artist .link:hover {
    text-decoration: underline;
  }

  .album {
    font-size: 1.05rem;
  }

  .small {
    font-size: 0.8rem;
  }

  .position {
    margin-top: 0.75rem;
  }

  .reason {
    color: var(--accent);
  }

  .actions {
    align-self: flex-start;
    margin-top: 1.25rem;
    display: flex;
    gap: 1.25rem;
  }

  .actions .link {
    display: inline-flex;
    align-items: center;
    gap: 0.3rem;
  }

  /* The cover: a square as large as the pane's width and height allow. */
  .cover {
    min-width: 0;
    min-height: 0;
    container-type: size;
  }

  .frame {
    width: min(100cqw, 100cqh);
    aspect-ratio: 1;
    margin-left: auto;
    border-radius: 6px;
    overflow: hidden;
    box-shadow: var(--shadow);
  }

  .frame :global(.art) {
    border-radius: 0;
  }

  @media (max-width: 640px) {
    .now-playing {
      grid-template-columns: minmax(0, 1fr);
      grid-template-rows: minmax(12rem, 1fr) auto;
      gap: 1.25rem;
      padding: 1rem;
    }

    .cover {
      order: -1;
    }

    .frame {
      margin: 0 auto;
    }

    .details {
      justify-content: flex-start;
      text-align: center;
      align-items: center;
    }

    .actions {
      align-self: center;
    }
  }
</style>
