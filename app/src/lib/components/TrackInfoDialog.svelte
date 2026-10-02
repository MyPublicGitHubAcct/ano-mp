<script lang="ts">
  // Get Info for a track (PLAN.md F16), read only: its title and rating,
  // the format as the decoder sees it (the facts the signal path shows),
  // where the file is (with Show in Finder), its MusicBrainz links, every
  // embedded picture, and every tag field TagLib reads.
  import { onMount } from "svelte";
  import { revealItemInDir } from "@tauri-apps/plugin-opener";
  import { openWebLink } from "$lib/openLink";
  import { library, type TrackDetails } from "$lib/api";
  import { count, errorText, t } from "$lib/i18n";
  import { fileName, formatDay, formatTime } from "$lib/format";
  import { collection } from "$lib/state/collection.svelte";
  import { attempt } from "$lib/state/toasts.svelte";
  import Dialog from "./Dialog.svelte";
  import Heart from "./Heart.svelte";
  import Icon from "./Icon.svelte";
  import Stars from "./Stars.svelte";

  let { trackId, onclose }: { trackId: number; onclose: () => void } = $props();

  let details = $state.raw<TrackDetails | null>(null);
  let failed = $state<string | null>(null);

  async function load() {
    try {
      details = await library.trackDetails(trackId);
    } catch (error) {
      failed = errorText(error);
    }
  }

  onMount(load);

  const track = $derived(details?.track ?? null);
  const path = $derived(details?.track.path ?? "");
  const numbers = new Intl.NumberFormat(undefined, { maximumFractionDigits: 1 });

  const size = (bytes: number) =>
    bytes >= 1 << 20
      ? t("info.megabytes", { size: numbers.format(bytes / (1 << 20)) })
      : t("info.kilobytes", { size: numbers.format(bytes / 1024) });

  const channels = (n: number) =>
    n === 1 ? t("info.mono") : n === 2 ? t("info.stereo") : t("info.channels", { count: n });

  const MUSICBRAINZ = "https://musicbrainz.org";
  const links = $derived.by(() => {
    const ids = details?.musicbrainz;
    if (!ids) return [];
    const result: { label: string; url: string }[] = [];
    if (ids.recording) result.push({ label: t("info.mbRecording"), url: `${MUSICBRAINZ}/recording/${ids.recording}` });
    if (ids.release) result.push({ label: t("info.mbRelease"), url: `${MUSICBRAINZ}/release/${ids.release}` });
    if (ids.releaseGroup)
      result.push({ label: t("info.mbReleaseGroup"), url: `${MUSICBRAINZ}/release-group/${ids.releaseGroup}` });
    for (const id of ids.artists) result.push({ label: t("info.mbArtist"), url: `${MUSICBRAINZ}/artist/${id}` });
    for (const id of ids.albumArtists)
      if (!ids.artists.includes(id))
        result.push({ label: t("info.mbAlbumArtist"), url: `${MUSICBRAINZ}/artist/${id}` });
    if (ids.work) result.push({ label: t("info.mbWork"), url: `${MUSICBRAINZ}/work/${ids.work}` });
    return result;
  });

  async function heart(on: boolean) {
    if (!track) return;
    await collection.setFavourite("track", [track.id], on);
    await load();
  }

  async function rate(rating: number | null) {
    if (!track) return;
    await collection.setRating([track.id], rating);
    await load();
  }
</script>

<Dialog title={track ? (track.title ?? fileName(track.path)) : t("info.title")} {onclose}>
  {#if failed}
    <p class="error">{failed}</p>
  {:else if !details || !track}
    <p class="muted">{t("info.reading")}</p>
  {:else}
    <div class="summary">
      {#if details.pictures[0]?.dataUrl}
        <img class="cover" src={details.pictures[0].dataUrl} alt={t("info.cover")} />
      {/if}
      <div>
        <p class="artist">{[track.artist, track.album].filter(Boolean).join(" — ")}</p>
        <div class="marks">
          <Heart on={track.favourite} label={track.title ?? ""} onchange={heart} size="1.2rem" />
          <Stars rating={track.rating} label={track.title ?? ""} onchange={rate} size="1.1rem" />
        </div>
        {#if details.error}
          <p class="warning"><Icon name="warning" size="1rem" /> {details.error}</p>
        {/if}
      </div>
    </div>

    <h3>{t("info.format")}</h3>
    <dl>
      <dt>{t("info.codec")}</dt>
      <dd>
        {details.file.codec.toUpperCase() || "—"}
        {details.file.lossless ? t("info.lossless") : ""}
      </dd>
      {#if details.file.bitsPerSample}<dt>{t("info.bits")}</dt>
        <dd>{details.file.bitsPerSample}</dd>{/if}
      <dt>{t("info.sampleRate")}</dt>
      <dd>{t("info.kHz", { rate: numbers.format(details.file.sampleRate / 1000) })}</dd>
      {#if details.file.channels > 0}<dt>{t("info.channelsLabel")}</dt>
        <dd>{channels(details.file.channels)}</dd>{/if}
      {#if details.file.bitrateKbps}<dt>{t("info.bitrate")}</dt>
        <dd>{t("info.kbps", { rate: details.file.bitrateKbps })}</dd>{/if}
      <dt>{t("info.length")}</dt>
      <dd>{formatTime(track.duration)}</dd>
      {#if details.file.fileSize > 0}<dt>{t("info.size")}</dt>
        <dd>{size(details.file.fileSize)}</dd>{/if}
      {#if details.file.tagTypes}<dt>{t("info.tags")}</dt>
        <dd>{details.file.tagTypes}</dd>{/if}
      {#if track.rangeStart > 0 || details.rangeEnd !== null}
        <dt>{t("info.part")}</dt>
        <dd>
          {formatTime(track.rangeStart)} – {details.rangeEnd === null ? t("info.end") : formatTime(details.rangeEnd)}
        </dd>
      {/if}
      <dt>{t("info.added")}</dt>
      <dd>{track.addedAt > 0 ? formatDay(track.addedAt) : "—"}</dd>
      <dt>{t("info.plays")}</dt>
      <dd>{track.playCount > 0 ? count("count.plays", track.playCount) : "—"}</dd>
    </dl>

    <h3>{t("info.file")}</h3>
    <div class="path">
      <code>{path}</code>
      <button onclick={() => attempt(() => revealItemInDir(track.path))}>
        <Icon name="folder" size="1rem" />
        {t("menu.showInFinder")}
      </button>
    </div>

    {#if links.length > 0}
      <h3>{t("info.musicbrainz")}</h3>
      <ul class="links">
        {#each links as link (link.url)}
          <li><button class="link" onclick={() => openWebLink(link.url)}>{link.label}</button></li>
        {/each}
      </ul>
    {/if}

    {#if details.pictures.length > 0}
      <h3>{count("info.pictures", details.pictures.length)}</h3>
      <ul class="pictures">
        {#each details.pictures as picture, index (index)}
          <li>
            {#if picture.dataUrl}<img src={picture.dataUrl} alt={picture.kind || t("info.picture")} />{/if}
            <span class="muted small">
              {[picture.kind, picture.description, picture.mimeType, size(picture.size)].filter(Boolean).join(" · ")}
            </span>
          </li>
        {/each}
      </ul>
    {/if}

    <h3>{t("info.allTags")}</h3>
    {#if details.file.fields.length === 0}
      <p class="muted">{t("info.noTags")}</p>
    {:else}
      <table>
        <tbody>
          {#each details.file.fields as [key, value], index (index)}
            <tr><th scope="row">{key}</th><td>{value}</td></tr>
          {/each}
        </tbody>
      </table>
    {/if}
  {/if}

  {#snippet actions()}
    <button class="primary" onclick={onclose}>{t("dialog.close")}</button>
  {/snippet}
</Dialog>

<style>
  .summary {
    display: flex;
    gap: 1rem;
    align-items: flex-start;
  }

  .cover {
    width: 7rem;
    height: 7rem;
    object-fit: cover;
    border-radius: 6px;
  }

  .artist {
    margin: 0 0 0.5rem;
    color: var(--text-muted);
  }

  .marks {
    display: flex;
    align-items: center;
    gap: 0.5rem;
  }

  .warning,
  .error {
    color: var(--danger);
    display: flex;
    align-items: center;
    gap: 0.35rem;
  }

  h3 {
    font-size: 0.9rem;
    margin: 1.25rem 0 0.5rem;
  }

  dl {
    display: grid;
    grid-template-columns: max-content 1fr;
    gap: 0.25rem 1rem;
    margin: 0;
  }

  dt {
    color: var(--text-muted);
  }

  dd {
    margin: 0;
  }

  .path {
    display: flex;
    align-items: center;
    gap: 0.75rem;
  }

  code {
    flex: 1;
    overflow-wrap: anywhere;
    font-size: 0.85rem;
  }

  .links {
    list-style: none;
    margin: 0;
    padding: 0;
    display: flex;
    flex-wrap: wrap;
    gap: 0.5rem 1rem;
  }

  .pictures {
    list-style: none;
    margin: 0;
    padding: 0;
    display: flex;
    flex-wrap: wrap;
    gap: 1rem;
  }

  .pictures li {
    display: flex;
    flex-direction: column;
    gap: 0.25rem;
    max-width: 10rem;
  }

  .pictures img {
    width: 10rem;
    height: 10rem;
    object-fit: contain;
    background: var(--surface-2);
    border-radius: 6px;
  }

  .small {
    font-size: 0.8rem;
  }

  table {
    width: 100%;
    border-collapse: collapse;
    font-size: 0.85rem;
  }

  th {
    text-align: left;
    font-weight: 600;
    padding: 0.2rem 1rem 0.2rem 0;
    vertical-align: top;
    white-space: nowrap;
    color: var(--text-muted);
  }

  td {
    padding: 0.2rem 0;
    overflow-wrap: anywhere;
    white-space: pre-wrap;
  }

  tr + tr {
    border-top: 1px solid var(--border);
  }
</style>
