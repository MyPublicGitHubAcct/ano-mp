<script lang="ts">
  // Artists outside the library (PLAN.md X5), each saying why it is
  // suggested. Its name opens a menu of links out (MusicBrainz,
  // ListenBrainz, and the homepage and Bandcamp page MusicBrainz knows),
  // never anything that plays; "Not Interested" stops suggesting it.
  import { t } from "$lib/i18n";
  import { outside as api, type OutsideArtist } from "$lib/api";
  import { openWebLink } from "$lib/openLink";
  import { outsideReasonsText } from "$lib/similar";
  import { attempt } from "$lib/state/toasts.svelte";
  import { ui, type MenuItem } from "$lib/state/ui.svelte";
  import Icon from "./Icon.svelte";

  let {
    artists,
    label,
    ondismissed,
  }: { artists: OutsideArtist[]; label: string; ondismissed: (mbid: string) => void } = $props();

  async function dismiss(artist: OutsideArtist) {
    const done = await attempt(async () => {
      await api.dismiss(artist.mbid, artist.name);
      return true;
    });
    if (done) ondismissed(artist.mbid);
  }

  async function menu(event: MouseEvent, artist: OutsideArtist) {
    event.preventDefault();
    const links = await attempt(() => api.links(artist.mbid));
    if (!links) return;
    const open = (key: Parameters<typeof t>[0], url: string | null): MenuItem[] =>
      url ? [{ label: t(key), action: () => openWebLink(url) }] : [];
    ui.openMenu(event, [
      ...open("outside.musicbrainz", links.musicbrainz),
      ...open("outside.listenbrainz", links.listenbrainz),
      ...open("outside.homepage", links.homepage),
      ...open("outside.bandcamp", links.bandcamp),
      { separator: true },
      { label: t("outside.dismiss"), action: () => dismiss(artist) },
    ]);
  }
</script>

<ul class="outside" aria-label={label}>
  {#each artists as artist (artist.mbid)}
    <li>
      <span class="text">
        <button
          class="link name"
          aria-haspopup="menu"
          title={t("outside.links", { name: artist.name })}
          onclick={(event) => menu(event, artist)}
          oncontextmenu={(event) => menu(event, artist)}>{artist.name}</button
        >
        {#if artist.disambiguation}<span class="muted small">{artist.disambiguation}</span>{/if}
        <span class="note small">{outsideReasonsText(artist.reasons, t)}</span>
      </span>
      <button
        class="icon"
        title={t("outside.dismissName", { name: artist.name })}
        aria-label={t("outside.dismissName", { name: artist.name })}
        onclick={() => dismiss(artist)}><Icon name="close" size="0.8rem" /></button
      >
    </li>
  {/each}
</ul>

<style>
  .outside {
    list-style: none;
    margin: 0;
    padding: 0;
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(14rem, 1fr));
    gap: 0.5rem;
  }

  li {
    display: flex;
    align-items: flex-start;
    gap: 0.4rem;
    padding: 0.45rem 0.6rem;
    border: 1px solid var(--border);
    border-radius: var(--radius);
    background: var(--surface);
    min-width: 0;
  }

  .text {
    display: flex;
    flex-direction: column;
    min-width: 0;
    flex: 1;
  }

  .name {
    text-align: left;
    font-weight: 600;
  }

  .name,
  .text .muted,
  .note {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .note {
    color: var(--text-muted);
  }
</style>
