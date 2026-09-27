<script lang="ts">
  // The first thing a new library shows (PLAN.md F8): how to add music
  // (the Music folder suggested, or any other, or a folder dropped on the
  // window), what the online sources that are on send and where, and the
  // first scan's progress.
  import { onMount } from "svelte";
  import { homeDir, join } from "@tauri-apps/api/path";
  import { metadata, type MetadataSettings, type SourceId } from "$lib/api";
  import { t, type MessageKey } from "$lib/i18n";
  import { library } from "$lib/state/library.svelte";
  import { appSettings } from "$lib/state/settings.svelte";
  import { ui } from "$lib/state/ui.svelte";
  import Icon from "./Icon.svelte";

  let music = $state<string | null>(null);
  let sources = $state.raw<MetadataSettings | null>(null);

  onMount(async () => {
    music = await homeDir()
      .then((home) => join(home, "Music"))
      .catch(() => null);
    sources = await metadata.settings().catch(() => null);
  });

  /** What each online source is sent. */
  const SENDS: Partial<Record<SourceId, MessageKey>> = {
    musicbrainz: "welcome.sends.musicbrainz",
    "cover-art-archive": "welcome.sends.coverArtArchive",
    wikipedia: "welcome.sends.wikipedia",
    discogs: "welcome.sends.discogs",
  };

  const on = $derived(
    sources && sources.settings.online
      ? sources.sources.filter(
          (source) => source.online && sources?.settings.sources.find((s) => s.id === source.id)?.enabled,
        )
      : [],
  );
</script>

<section class="welcome">
  <div class="box">
    <h2>{t("welcome.title")}</h2>
    <p class="lead">{t("welcome.lead")}</p>

    {#if library.scanning || library.background}
      <div class="progress" role="status">
        <p>{t("welcome.scanning")}</p>
        {#if library.scanProgress && library.scanProgress.toRead > 0}
          <progress max={library.scanProgress.toRead} value={library.scanProgress.read}></progress>
          <p class="muted small">
            {t("sidebar.scanProgress", { read: library.scanProgress.read, count: library.scanProgress.toRead })}
          </p>
        {:else}
          <progress></progress>
        {/if}
      </div>
    {:else}
      <div class="actions">
        {#if music}
          <button class="primary" onclick={() => library.addFolder(music ?? undefined)}>
            <Icon name="folder" />
            {t("welcome.addMusic")}
          </button>
        {/if}
        <button onclick={() => library.addFolder()}>{t("welcome.addOther")}</button>
      </div>
      <p class="muted small">{t("welcome.drop")}</p>
    {/if}

    <h3><Icon name="cloud" size="1.1rem" /> {t("welcome.onlineTitle")}</h3>
    {#if !sources}
      <p class="muted">…</p>
    {:else if on.length === 0}
      <p>{t("welcome.offline")}</p>
    {:else}
      <p>{t("welcome.onlineLead")}</p>
      <ul>
        {#each on as source (source.id)}
          <li>
            <strong>{source.name}</strong>
            {#if SENDS[source.id]}— {t(SENDS[source.id] as MessageKey)}{/if}
          </li>
        {/each}
      </ul>
      {#if appSettings.current.features.listenbrainz}
        <p>{t("welcome.listenbrainz")}</p>
      {/if}
      <p class="muted small">{t("welcome.never")}</p>
    {/if}
    <button class="link" onclick={() => ui.showSettings("sources")}>{t("welcome.changeSources")}</button>
  </div>
</section>

<style>
  .welcome {
    height: 100%;
    overflow-y: auto;
    display: grid;
    place-items: center;
    padding: 1.5rem;
  }

  .box {
    max-width: 36rem;
  }

  h2 {
    font-size: 1.6rem;
    margin: 0 0 0.5rem;
  }

  .lead {
    color: var(--text-muted);
    margin: 0 0 1.25rem;
  }

  .actions {
    display: flex;
    flex-wrap: wrap;
    gap: 0.5rem;
    margin-bottom: 0.5rem;
  }

  h3 {
    display: flex;
    align-items: center;
    gap: 0.4rem;
    font-size: 1rem;
    margin: 2rem 0 0.5rem;
  }

  ul {
    padding-left: 1.2rem;
  }

  li {
    margin: 0.3rem 0;
  }

  .small {
    font-size: 0.85rem;
  }

  progress {
    width: 100%;
  }
</style>
