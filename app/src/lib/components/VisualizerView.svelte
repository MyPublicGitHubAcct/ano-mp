<script lang="ts">
  // The visualizer: a visualization filling the main area (or, full screen,
  // the window), with the picker, the cover wall's year/artist switch, full
  // screen and close along the top, and the track along the bottom. The
  // controls fade out while the pointer rests. If the settings say so, it
  // moves on to the next visualization every so often.
  //
  // The first time it opens, a note about flashing light comes first
  // (PLAN.md F18), and nothing moves until it's read; calm mode can be
  // turned on from there or from the controls.
  import { t } from "$lib/i18n";
  import { player } from "$lib/state/player.svelte";
  import { appSettings } from "$lib/state/settings.svelte";
  import { ui } from "$lib/state/ui.svelte";
  import { visualizer } from "$lib/state/visualizer.svelte";
  import { VISUALIZATIONS, visualization } from "$lib/visualizer";
  import Icon from "./Icon.svelte";
  import Visualizer from "./Visualizer.svelte";

  const IDLE_MS = 2500;

  const item = $derived(player.currentItem);
  const chosen = $derived(visualization(visualizer.choice));
  let idle = $state(false);
  let timer: ReturnType<typeof setTimeout> | undefined;

  function wake() {
    idle = false;
    clearTimeout(timer);
    timer = setTimeout(() => (idle = true), IDLE_MS);
  }

  $effect(() => {
    wake();
    return () => clearTimeout(timer);
  });

  // The next visualization every `cycleSeconds`, counted from the last change.
  $effect(() => {
    const seconds = appSettings.visualizer.cycleSeconds;
    void visualizer.choice;
    if (seconds <= 0) return;
    const timer = setTimeout(() => visualizer.cycle(1), seconds * 1000);
    return () => clearTimeout(timer);
  });

  // Leaving the visualizer leaves full screen too.
  $effect(() => () => {
    if (visualizer.fullscreen) void visualizer.setFullscreen(false);
  });
</script>

<section
  class="visualizer"
  class:idle
  aria-label={t("bar.visualizer")}
  onpointermove={wake}
  onpointerdown={wake}
  onfocusin={wake}
  ondblclick={() => visualizer.setFullscreen(!visualizer.fullscreen)}
>
  {#if appSettings.current.window.visualizerNoteSeen}
    <Visualizer id={chosen.id} />
  {:else}
    <div
      class="note"
      role="alertdialog"
      aria-labelledby="visualizer-note-title"
      aria-describedby="visualizer-note-text"
    >
      <h2 id="visualizer-note-title">{t("visualizer.noteTitle")}</h2>
      <p id="visualizer-note-text">{t("visualizer.noteText")}</p>
      <div class="note-actions">
        <button
          onclick={() =>
            appSettings.save((next) => {
              next.window.visualizerNoteSeen = true;
              next.visualizer.calm = true;
            })}>{t("visualizer.noteCalm")}</button
        >
        <button class="primary" onclick={() => appSettings.save((next) => (next.window.visualizerNoteSeen = true))}>
          {t("visualizer.noteContinue")}
        </button>
      </div>
    </div>
  {/if}

  <div class="controls top">
    <select
      aria-label={t("vizOptions.visualization")}
      title={chosen.description}
      value={chosen.id}
      onchange={(event) => visualizer.choose(event.currentTarget.value)}
      onkeydown={(event) => event.stopPropagation()}
    >
      {#each VISUALIZATIONS as option (option.id)}
        <option value={option.id}>{option.name}</option>
      {/each}
    </select>
    <button
      class="calm"
      aria-pressed={appSettings.visualizer.calm}
      title={t("visualizer.calmHint")}
      onclick={() => appSettings.save((next) => (next.visualizer.calm = !next.visualizer.calm))}
    >
      {t("visualizer.calm")}
    </button>
    {#if chosen.id === "covers"}
      <div class="segmented" role="group" aria-label={t("vizOptions.coverWallShows")}>
        <button aria-pressed={visualizer.coverBasis === "year"} onclick={() => visualizer.setCoverBasis("year")}>
          {t("visualizer.sameYear")}
        </button>
        <button aria-pressed={visualizer.coverBasis === "artist"} onclick={() => visualizer.setCoverBasis("artist")}>
          {t("visualizer.sameArtist")}
        </button>
      </div>
    {/if}
    <span class="spacer"></span>
    <button
      class="icon"
      title={`${t(visualizer.fullscreen ? "visualizer.leaveFullscreen" : "visualizer.fullscreen")} (F)`}
      aria-label={t(visualizer.fullscreen ? "visualizer.leaveFullscreen" : "visualizer.fullscreen")}
      aria-pressed={visualizer.fullscreen}
      onclick={() => visualizer.setFullscreen(!visualizer.fullscreen)}
    >
      <Icon name="expand" />
    </button>
    {#if !visualizer.fullscreen}
      <button class="icon" title={t("dialog.close")} aria-label={t("bar.closeVisualizer")} onclick={() => ui.back()}>
        <Icon name="close" />
      </button>
    {/if}
  </div>

  <div class="controls bottom">
    {#if item}
      <p class="title">{item.title}</p>
      <p class="small">{[item.artist, item.album].filter(Boolean).join(" · ")}</p>
    {:else}
      <p class="title">{t("bar.notPlaying")}</p>
    {/if}
    {#if visualizer.caption}<p class="small caption">{visualizer.caption}</p>{/if}
    {#if visualizer.fullscreen}
      <p class="small hint">{t("visualizer.keys")}</p>
    {/if}
  </div>
</section>

<style>
  .note {
    position: absolute;
    inset: 0;
    display: grid;
    place-content: center;
    gap: 0.75rem;
    padding: 2rem;
    max-width: 34rem;
    margin: auto;
    color: #ececef;
  }

  .note h2 {
    margin: 0;
  }

  .note p {
    margin: 0;
    line-height: 1.5;
  }

  .note-actions {
    display: flex;
    flex-wrap: wrap;
    gap: 0.5rem;
  }

  .calm[aria-pressed="true"] {
    background: var(--accent);
    color: var(--accent-text);
  }

  .visualizer {
    position: relative;
    height: 100%;
    overflow: hidden;
    background: rgb(9 9 12);
    color: #f2f2f5;
  }

  .visualizer.idle {
    cursor: none;
  }

  .controls {
    position: absolute;
    left: 0;
    right: 0;
    padding: 0.75rem 1rem;
    transition: opacity 0.4s;
  }

  .idle .controls {
    opacity: 0;
  }

  .controls:focus-within {
    opacity: 1;
  }

  .top {
    top: 0;
    display: flex;
    align-items: center;
    gap: 0.5rem;
    background: linear-gradient(rgb(0 0 0 / 0.55), transparent);
  }

  .spacer {
    flex: 1;
  }

  .bottom {
    bottom: 0;
    padding-top: 2.5rem;
    background: linear-gradient(transparent, rgb(0 0 0 / 0.6));
    pointer-events: none;
  }

  p {
    margin: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .title {
    font-size: 1.15rem;
    font-weight: 600;
  }

  .small {
    font-size: 0.85rem;
    color: rgb(242 242 245 / 0.7);
  }

  .caption {
    margin-top: 0.3rem;
  }

  .hint {
    margin-top: 0.3rem;
    color: rgb(242 242 245 / 0.45);
  }

  /* Controls on the dark stage, whatever the app's theme. */
  select,
  .top button {
    color: #f2f2f5;
    background: rgb(255 255 255 / 0.08);
    border-color: rgb(255 255 255 / 0.16);
  }

  .top button:hover:not(:disabled) {
    background: rgb(255 255 255 / 0.16);
  }

  select option {
    color: initial;
    background: initial;
  }

  .top button.icon {
    background: none;
  }

  .segmented {
    display: inline-flex;
  }

  .segmented button {
    border-radius: 0;
    padding: 0.3rem 0.7rem;
  }

  .segmented button:first-child {
    border-radius: var(--radius) 0 0 var(--radius);
  }

  .segmented button:last-child {
    border-radius: 0 var(--radius) var(--radius) 0;
    border-left: none;
  }

  .segmented button[aria-pressed="true"] {
    background: rgb(255 255 255 / 0.85);
    color: #111114;
  }

  @media (max-width: 640px) {
    .controls {
      padding: 0.5rem 0.75rem;
    }

    .segmented button {
      padding: 0.3rem 0.5rem;
      font-size: 0.85rem;
    }
  }
</style>
