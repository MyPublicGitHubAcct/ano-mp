<script lang="ts">
  // Breadcrumbs for the browser, the search box, and on narrow windows the
  // sidebar button.
  import { t } from "$lib/i18n";
  import { library } from "$lib/state/library.svelte";
  import { ui } from "$lib/state/ui.svelte";
  import Icon from "./Icon.svelte";
</script>

<header class="header">
  <button
    class="icon menu"
    title={t("library.title")}
    aria-label={t("header.showLibrary")}
    onclick={() => (ui.sidebarOpen = true)}
  >
    <Icon name="menu" />
  </button>
  <nav class="crumbs" aria-label={t("header.location")}>
    {#if library.query.trim() !== ""}
      <span class="current">{t("search.placeholder")}</span>
    {:else if ui.queueInMain}
      <span class="current">{t("queue.title")}</span>
    {:else if ui.nowPlayingInMain}
      <span class="current">{t("sidebar.nowPlaying")}</span>
    {:else if ui.visualizerInMain}
      <span class="current">{t("sidebar.visualizer")}</span>
    {:else if ui.artistInMain}
      <button class="crumb" onclick={() => ui.back()}>{t("header.back")}</button>
      <span class="separator" aria-hidden="true">›</span>
      <span class="current">{ui.artist?.name}</span>
    {:else if ui.discographyInMain}
      <button class="crumb" onclick={() => ui.back()}>{t("header.back")}</button>
      <span class="separator" aria-hidden="true">›</span>
      <span class="current">{t("header.notInLibrary", { name: ui.artist?.name ?? "" })}</span>
    {:else if ui.playlistInMain || ["home", "favourites", "history", "health", "settings"].includes(ui.mainView)}
      <button class="crumb" onclick={() => ui.back()}>{t("header.back")}</button>
    {:else}
      <button class="crumb" disabled={library.crumbs.length === 0} onclick={() => library.goUp(0)}>
        {library.rule?.name ?? t("library.title")}
      </button>
      {#each library.crumbs as crumb, depth (depth)}
        <span class="separator" aria-hidden="true">›</span>
        <button class="crumb" disabled={depth === library.crumbs.length - 1} onclick={() => library.goUp(depth + 1)}>
          {crumb.name}
        </button>
      {/each}
    {/if}
  </nav>
  <input
    class="search"
    type="search"
    placeholder={t("search.placeholder")}
    aria-label={t("search.label")}
    title={t("search.filtersHint")}
    bind:value={library.query}
    bind:this={ui.searchInput}
    onkeydown={(event) => {
      if (event.key === "Escape") {
        library.query = "";
        event.currentTarget.blur();
      }
    }}
  />
</header>

<style>
  .header {
    display: flex;
    align-items: center;
    gap: 0.75rem;
    padding: 0.5rem 1rem;
    min-width: 0;
  }

  .menu {
    display: none;
  }

  .crumbs {
    display: flex;
    align-items: center;
    gap: 0.25rem;
    flex: 1;
    min-width: 0;
    overflow-x: auto;
    white-space: nowrap;
    scrollbar-width: none;
  }

  .crumb {
    border: none;
    background: none;
    padding: 0.2rem 0.3rem;
    border-radius: 4px;
    color: var(--text-muted);
    max-width: 16rem;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .crumb:hover:not(:disabled) {
    background: var(--hover);
    color: var(--text);
  }

  .crumb:disabled,
  .current {
    color: var(--text);
    font-weight: 600;
    opacity: 1;
  }

  .separator {
    color: var(--text-faint);
  }

  .search {
    flex: 0 1 16rem;
    min-width: 6rem;
  }

  @media (max-width: 640px) {
    .menu {
      display: inline-flex;
    }
  }
</style>
