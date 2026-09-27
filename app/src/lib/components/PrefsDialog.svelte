<script lang="ts">
  // Playback preferences (O7) for a track and/or its album: skip in album
  // and shuffle play, never shuffle the album, a gain offset, and trims.
  // A track's own value takes over from its album's; "Album's" leaves it to
  // the album. Kept in the library, never written to the files.
  import { t } from "$lib/i18n";
  import { onMount } from "svelte";
  import { features as api, type AlbumPrefs, type TrackPrefs } from "$lib/api";
  import { attempt } from "$lib/state/toasts.svelte";
  import type { AlbumRef } from "$lib/state/ui.svelte";
  import Dialog from "./Dialog.svelte";

  let {
    track,
    album,
    onclose,
  }: { track: { id: number; title: string } | null; album: AlbumRef | null; onclose: () => void } = $props();

  let trackPrefs = $state<TrackPrefs>({});
  let albumPrefs = $state<AlbumPrefs>({});
  let loaded = $state(false);

  onMount(async () => {
    const prefs = await attempt(() => api.prefs({ trackId: track?.id, albumId: album?.id }));
    if (prefs) {
      trackPrefs = prefs.track ?? {};
      albumPrefs = prefs.album ?? {};
    }
    loaded = true;
  });

  /** A tri-state: yes, no, or left to the album (null). */
  const choice = (value: boolean | null | undefined) => (value === true ? "yes" : value === false ? "no" : "");
  const fromChoice = (value: string) => (value === "yes" ? true : value === "no" ? false : null);
  const number = (value: string) => (value.trim() === "" ? null : Number(value));
  const clean = <T extends object>(prefs: T) =>
    Object.fromEntries(Object.entries(prefs).map(([key, value]) => [key, value ?? null])) as T;

  async function save() {
    let ok = true;
    if (track) ok = (await attempt(() => api.setTrackPrefs(track.id, clean(trackPrefs)))) !== undefined && ok;
    if (album) ok = (await attempt(() => api.setAlbumPrefs(album.id, clean(albumPrefs)))) !== undefined && ok;
    if (ok) onclose();
  }
</script>

<Dialog title={t("prefs.title")} {onclose}>
  {#if loaded}
    {#if track}
      <h3>{t("prefs.track", { title: track.title })}</h3>
      <label class="row">
        <span>{t("prefs.skip")}</span>
        <select
          value={choice(trackPrefs.skip)}
          onchange={(event) => (trackPrefs.skip = fromChoice(event.currentTarget.value))}
        >
          <option value="">{t("prefs.asAlbum")}</option>
          <option value="yes">{t("prefs.skipOption")}</option>
          <option value="no">{t("prefs.playOption")}</option>
        </select>
      </label>
      <p class="hint">{t("prefs.skipHint")}</p>
      <label class="row">
        <span>{t("prefs.gain")}</span>
        <input
          type="number"
          step="0.5"
          min="-15"
          max="15"
          placeholder={t("prefs.asAlbum")}
          value={trackPrefs.gainOffset ?? ""}
          onchange={(event) => (trackPrefs.gainOffset = number(event.currentTarget.value))}
        />
      </label>
      <label class="row">
        <span>{t("prefs.trimStart")}</span>
        <input
          type="number"
          step="0.1"
          min="0"
          placeholder="0"
          value={trackPrefs.trimStart ?? ""}
          onchange={(event) => (trackPrefs.trimStart = number(event.currentTarget.value))}
        />
      </label>
      <label class="row">
        <span>{t("prefs.trimEnd")}</span>
        <input
          type="number"
          step="0.1"
          min="0"
          placeholder="0"
          value={trackPrefs.trimEnd ?? ""}
          onchange={(event) => (trackPrefs.trimEnd = number(event.currentTarget.value))}
        />
      </label>
    {/if}
    {#if album}
      <h3>{t("prefs.album", { title: album.title })}</h3>
      <label class="check">
        <input
          type="checkbox"
          checked={albumPrefs.neverShuffle === true}
          onchange={(event) => (albumPrefs.neverShuffle = event.currentTarget.checked || null)}
        />
        {t("prefs.neverShuffle")}
      </label>
      <label class="check">
        <input
          type="checkbox"
          checked={albumPrefs.skip === true}
          onchange={(event) => (albumPrefs.skip = event.currentTarget.checked || null)}
        />
        {t("prefs.albumSkip")}
      </label>
      <label class="row">
        <span>{t("prefs.gain")}</span>
        <input
          type="number"
          step="0.5"
          min="-15"
          max="15"
          placeholder="0"
          value={albumPrefs.gainOffset ?? ""}
          onchange={(event) => (albumPrefs.gainOffset = number(event.currentTarget.value))}
        />
      </label>
    {/if}
    <p class="hint">{t("prefs.hint")}</p>
  {/if}
  {#snippet actions()}
    <button
      onclick={() => {
        trackPrefs = {};
        albumPrefs = {};
      }}>{t("prefs.clearAll")}</button
    >
    <button onclick={onclose}>{t("dialog.cancel")}</button>
    <button class="primary" onclick={save} disabled={!loaded}>{t("dialog.save")}</button>
  {/snippet}
</Dialog>

<style>
  h3 {
    font-size: 0.95rem;
    margin: 0.75rem 0 0.4rem;
  }

  h3:first-child {
    margin-top: 0;
  }

  .row {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 1rem;
    padding: 0.3rem 0;
  }

  .row input {
    width: 8rem;
  }

  .check {
    display: flex;
    align-items: center;
    gap: 0.5rem;
    padding: 0.3rem 0;
  }

  .hint {
    color: var(--text-muted);
    font-size: 0.8rem;
    margin: 0.2rem 0;
  }
</style>
