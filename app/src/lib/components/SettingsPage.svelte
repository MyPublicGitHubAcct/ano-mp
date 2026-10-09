<script lang="ts">
  // The settings (⌘,): the library's folders, its sort and grouping rules,
  // what lists and pages show, playback and the output device, the
  // visualizer, the theme, recording, and the online sources. Sections are listed down the side
  // (along the top when narrow); every change is saved as it's made. The
  // section components share the styles of `Options` (`.field`, `.switch`,
  // `.hint`…). A found feature (PLAN.md X9) opens a section at a control,
  // by the element id `ui.settingsControl` names (`lib/find.ts`).
  import { sectionTab } from "$lib/find";
  import { t, type MessageKey } from "$lib/i18n";
  import { landWhenDrawn } from "$lib/landOn";
  import { appSettings } from "$lib/state/settings.svelte";
  import { ui, type SettingsSection } from "$lib/state/ui.svelte";
  import Icon from "./Icon.svelte";
  import ServicesPanel from "./ServicesPanel.svelte";
  import AboutOptions from "./settings/AboutOptions.svelte";
  import AppearanceOptions from "./settings/AppearanceOptions.svelte";
  import DisplayOptions from "./settings/DisplayOptions.svelte";
  import EffectsOptions from "./settings/EffectsOptions.svelte";
  import EqualiserOptions from "./settings/EqualiserOptions.svelte";
  import GeneralOptions from "./settings/GeneralOptions.svelte";
  import FeaturesOptions from "./settings/FeaturesOptions.svelte";
  import LibraryFolders from "./settings/LibraryFolders.svelte";
  import PlaybackOptions from "./settings/PlaybackOptions.svelte";
  import RecordingOptions from "./settings/RecordingOptions.svelte";
  import SortRules from "./settings/SortRules.svelte";
  import Options from "./settings/Options.svelte";
  import VisualizerOptions from "./settings/VisualizerOptions.svelte";

  const ALL_SECTIONS: { id: SettingsSection; name: MessageKey; about: MessageKey }[] = [
    { id: "general", name: "settings.general", about: "settings.generalAbout" },
    { id: "library", name: "settings.library", about: "settings.libraryAbout" },
    { id: "sorting", name: "settings.sorting", about: "settings.sortingAbout" },
    { id: "display", name: "settings.display", about: "settings.displayAbout" },
    { id: "appearance", name: "settings.appearance", about: "settings.appearanceAbout" },
    { id: "playback", name: "settings.playback", about: "settings.playbackAbout" },
    { id: "equaliser", name: "settings.equaliser", about: "settings.equaliserAbout" },
    { id: "effects", name: "settings.effects", about: "settings.effectsAbout" },
    { id: "recording", name: "settings.recording", about: "settings.recordingAbout" },
    { id: "visualizer", name: "settings.visualizer", about: "settings.visualizerAbout" },
    { id: "sources", name: "settings.sources", about: "settings.sourcesAbout" },
    { id: "features", name: "settings.features", about: "settings.featuresAbout" },
    { id: "about", name: "settings.about", about: "settings.aboutAbout" },
  ];

  /** Appearance only while themes are on (PLAN.md X1), Recording while recording is (X6). */
  const SECTIONS = $derived(
    ALL_SECTIONS.filter(
      (candidate) =>
        (candidate.id !== "appearance" || appSettings.current.features.themes) &&
        (candidate.id !== "recording" || appSettings.current.features.recording),
    ),
  );

  const section = $derived(SECTIONS.find((candidate) => candidate.id === ui.settingsSection) ?? SECTIONS[0]);

  // A found feature (PLAN.md X9) lands on its control once the section has drawn it, else on the section's tab.
  $effect(() => {
    const control = ui.settingsControl;
    if (control === null) return;
    return landWhenDrawn(control, sectionTab(section.id), () => (ui.settingsControl = null));
  });

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
      <Options>
        <h2>{t(section.name)}</h2>
        <p class="muted about">{t(section.about)}</p>
        {#key section.id}
          {#if section.id === "library"}
            <LibraryFolders />
          {:else if section.id === "sorting"}
            <SortRules />
          {:else if section.id === "display"}
            <DisplayOptions />
          {:else if section.id === "appearance"}
            <AppearanceOptions />
          {:else if section.id === "playback"}
            <PlaybackOptions />
          {:else if section.id === "equaliser"}
            <EqualiserOptions />
          {:else if section.id === "effects"}
            <EffectsOptions />
          {:else if section.id === "recording"}
            <RecordingOptions />
          {:else if section.id === "general"}
            <GeneralOptions />
          {:else if section.id === "visualizer"}
            <VisualizerOptions />
          {:else if section.id === "features"}
            <FeaturesOptions />
          {:else if section.id === "about"}
            <AboutOptions />
          {:else}
            <ServicesPanel />
          {/if}
        {/key}
      </Options>
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
