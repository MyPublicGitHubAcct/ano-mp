<script lang="ts">
  // The settings (⌘,): the library's folders, its sort and grouping rules,
  // what lists and pages show, playback and the output device, the
  // visualizer, and the online sources. Sections are listed down the side
  // (along the top when narrow); every change is saved as it's made. The
  // section components use the shared styles below (`.field`, `.switch`,
  // `.hint`…).
  import { t, type MessageKey } from "$lib/i18n";
  import { ui, type SettingsSection } from "$lib/state/ui.svelte";
  import Icon from "./Icon.svelte";
  import ServicesPanel from "./ServicesPanel.svelte";
  import DisplayOptions from "./settings/DisplayOptions.svelte";
  import EqualiserOptions from "./settings/EqualiserOptions.svelte";
  import GeneralOptions from "./settings/GeneralOptions.svelte";
  import FeaturesOptions from "./settings/FeaturesOptions.svelte";
  import LibraryFolders from "./settings/LibraryFolders.svelte";
  import PlaybackOptions from "./settings/PlaybackOptions.svelte";
  import SortRules from "./settings/SortRules.svelte";
  import VisualizerOptions from "./settings/VisualizerOptions.svelte";

  const SECTIONS: { id: SettingsSection; name: MessageKey; about: MessageKey }[] = [
    { id: "general", name: "settings.general", about: "settings.generalAbout" },
    { id: "library", name: "settings.library", about: "settings.libraryAbout" },
    { id: "sorting", name: "settings.sorting", about: "settings.sortingAbout" },
    { id: "display", name: "settings.display", about: "settings.displayAbout" },
    { id: "playback", name: "settings.playback", about: "settings.playbackAbout" },
    { id: "equaliser", name: "settings.equaliser", about: "settings.equaliserAbout" },
    { id: "visualizer", name: "settings.visualizer", about: "settings.visualizerAbout" },
    { id: "sources", name: "settings.sources", about: "settings.sourcesAbout" },
    { id: "features", name: "settings.features", about: "settings.featuresAbout" },
  ];

  const section = $derived(SECTIONS.find((candidate) => candidate.id === ui.settingsSection) ?? SECTIONS[0]);

  function onkeydown(event: KeyboardEvent) {
    if (event.key !== "ArrowDown" && event.key !== "ArrowUp" && event.key !== "ArrowLeft" && event.key !== "ArrowRight")
      return;
    event.preventDefault();
    event.stopPropagation();
    const step = event.key === "ArrowDown" || event.key === "ArrowRight" ? 1 : -1;
    const index = SECTIONS.indexOf(section);
    const next = SECTIONS[(index + step + SECTIONS.length) % SECTIONS.length];
    ui.settingsSection = next.id;
    document.getElementById(`settings-tab-${next.id}`)?.focus();
  }
</script>

<section class="settings" aria-labelledby="settings-heading">
  <header class="hero">
    <h1 id="settings-heading">{t("settings.title")}</h1>
    <button class="icon" title={t("settings.closeEsc")} aria-label={t("settings.close")} onclick={() => ui.back()}>
      <Icon name="close" />
    </button>
  </header>

  <div class="layout">
    <div
      class="tabs"
      role="tablist"
      aria-label={t("settings.sections")}
      aria-orientation="vertical"
      tabindex="-1"
      {onkeydown}
    >
      {#each SECTIONS as candidate (candidate.id)}
        <button
          id="settings-tab-{candidate.id}"
          role="tab"
          aria-selected={candidate.id === section.id}
          aria-controls="settings-panel"
          tabindex={candidate.id === section.id ? 0 : -1}
          onclick={() => (ui.settingsSection = candidate.id)}
        >
          {t(candidate.name)}
        </button>
      {/each}
    </div>

    <div class="panel" id="settings-panel" role="tabpanel" aria-labelledby="settings-tab-{section.id}">
      <h2>{t(section.name)}</h2>
      <p class="muted about">{t(section.about)}</p>
      {#key section.id}
        {#if section.id === "library"}
          <LibraryFolders />
        {:else if section.id === "sorting"}
          <SortRules />
        {:else if section.id === "display"}
          <DisplayOptions />
        {:else if section.id === "playback"}
          <PlaybackOptions />
        {:else if section.id === "equaliser"}
          <EqualiserOptions />
        {:else if section.id === "general"}
          <GeneralOptions />
        {:else if section.id === "visualizer"}
          <VisualizerOptions />
        {:else if section.id === "features"}
          <FeaturesOptions />
        {:else}
          <ServicesPanel />
        {/if}
      {/key}
    </div>
  </div>
</section>

<style>
  .settings {
    display: flex;
    flex-direction: column;
    height: 100%;
    min-height: 0;
  }

  .hero {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 1rem;
    padding: 1rem 1.5rem 0.5rem;
  }

  h1 {
    margin: 0;
    font-size: clamp(1.4rem, 2.6vw, 2rem);
  }

  .layout {
    display: grid;
    grid-template-columns: 11rem minmax(0, 1fr);
    flex: 1;
    min-height: 0;
  }

  .tabs {
    display: flex;
    flex-direction: column;
    gap: 0.1rem;
    padding: 0.5rem 0.5rem 1rem 1rem;
    overflow-y: auto;
  }

  /* Focused only by a click between tabs; the tabs show focus themselves. */
  .tabs:focus {
    outline: none;
  }

  .tabs button {
    justify-content: flex-start;
    border: none;
    background: none;
    padding: 0.4rem 0.6rem;
    text-align: left;
  }

  .tabs button:hover:not(:disabled) {
    background: var(--hover);
  }

  .tabs button[aria-selected="true"] {
    background: var(--selected);
    color: var(--accent);
    font-weight: 600;
  }

  .panel {
    overflow-y: auto;
    padding: 0.5rem 1.5rem 2.5rem;
  }

  .panel > :global(*) {
    max-width: 50rem;
  }

  h2 {
    margin: 0.2rem 0 0.25rem;
    font-size: 1.25rem;
  }

  .about {
    margin: 0 0 1rem;
  }

  /* Shared by the sections. */

  .panel :global(h3) {
    font-size: 1rem;
    margin: 1.5rem 0 0.5rem;
  }

  .panel :global(h3:first-child) {
    margin-top: 0.5rem;
  }

  .panel :global(p) {
    margin: 0;
  }

  .panel :global(.hint) {
    color: var(--text-muted);
    font-size: 0.8rem;
  }

  .panel :global(.field) {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 0.35rem 0.75rem;
    padding: 0.4rem 0;
  }

  .panel :global(.field > .label) {
    flex: 0 0 11rem;
    font-weight: 500;
  }

  .panel :global(.field > .control) {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 0.5rem;
    flex: 1 1 16rem;
    min-width: 0;
  }

  .panel :global(.field select) {
    max-width: 100%;
  }

  .panel :global(.field .hint) {
    flex-basis: 100%;
  }

  .panel :global(.stacked) {
    flex-direction: column;
    align-items: stretch;
  }

  .panel :global(.stacked > .label) {
    flex-basis: auto;
  }

  .panel :global(.switch) {
    display: flex;
    align-items: flex-start;
    gap: 0.6rem;
    padding: 0.4rem 0;
    cursor: pointer;
  }

  .panel :global(.switch > span) {
    display: flex;
    flex-direction: column;
    gap: 0.1rem;
    min-width: 0;
  }

  .panel :global(.switch .title) {
    font-weight: 500;
  }

  .panel :global(input[type="checkbox"]),
  .panel :global(input[type="radio"]) {
    margin: 0.2rem 0 0;
    accent-color: var(--accent);
    width: 1rem;
    height: 1rem;
    flex: none;
  }

  .panel :global(input[type="range"]) {
    flex: 1 1 10rem;
    max-width: 18rem;
  }

  .panel :global(.value) {
    min-width: 4.5rem;
    font-variant-numeric: tabular-nums;
  }

  .panel :global(.actions) {
    display: flex;
    flex-wrap: wrap;
    gap: 0.5rem;
    margin-top: 1.5rem;
  }

  .panel :global(.card) {
    border: 1px solid var(--border);
    border-radius: 8px;
    background: var(--surface);
  }

  @media (max-width: 640px) {
    .hero {
      padding: 1rem 1rem 0.25rem;
    }

    .layout {
      grid-template-columns: minmax(0, 1fr);
      grid-template-rows: auto minmax(0, 1fr);
    }

    .tabs {
      flex-direction: row;
      overflow-x: auto;
      padding: 0.25rem 1rem 0.5rem;
    }

    .tabs button {
      flex: none;
    }

    .panel {
      padding: 0.5rem 1rem 2rem;
    }

    .panel :global(.field > .label) {
      flex-basis: 100%;
    }
  }
</style>
