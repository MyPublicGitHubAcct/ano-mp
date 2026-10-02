<script lang="ts">
  // What track lists show besides the title (as columns when wide, under
  // the title when narrow), what an album's summary line says, and whether
  // Wikipedia's descriptions and biographies show.
  import { t } from "$lib/i18n";
  import { ALBUM_FACTS, TRACK_COLUMNS, columnAvailable, columnText, textColumns } from "$lib/columns";
  import type { Track } from "$lib/api";
  import { appSettings } from "$lib/state/settings.svelte";
  import OrderedChoices from "./OrderedChoices.svelte";

  const display = $derived(appSettings.display);

  /** A made-up track for the preview. */
  const SAMPLE: Track = {
    id: 0,
    path: "/Music/Nina Simone/Pastel Blues/05 Sinnerman.flac",
    title: "Sinnerman",
    artist: "Nina Simone",
    artistId: null,
    album: "Pastel Blues",
    albumId: null,
    albumArtist: "Nina Simone",
    genre: "Jazz",
    year: 1965,
    discNumber: 1,
    trackNumber: 5,
    duration: 622,
    bitrateKbps: 1024,
    sampleRate: 44100,
    addedAt: 1790000000,
    playCount: 12,
    lastPlayed: 1790400000,
    hasPrefs: false,
    composer: "Nina Simone",
    work: null,
    movementName: null,
    movementNumber: null,
    rangeStart: 0,
    favourite: true,
    rating: 4,
    folderId: 0,
  };

  const preview = $derived(
    [
      display.trackColumns.includes("trackNumber") ? columnText("trackNumber", SAMPLE) : null,
      SAMPLE.title,
      ...textColumns(display.trackColumns).map((column) => columnText(column, SAMPLE)),
      display.trackColumns.includes("duration") ? columnText("duration", SAMPLE) : null,
    ].filter(Boolean),
  );
</script>

<h3>{t("display.trackLists")}</h3>
<div class="field stacked">
  <OrderedChoices
    options={TRACK_COLUMNS.filter((column) => columnAvailable(column.id, appSettings.current.features))}
    value={display.trackColumns}
    onchange={(columns) => appSettings.save((next) => (next.display.trackColumns = columns))}
    label={t("display.columns")}
    addLabel={t("display.addColumn")}
    empty={t("display.onlyTitle")}
    disabled={appSettings.saving}
  />
  <p class="hint">{t("display.columnsHint")}</p>
  <p class="preview card" aria-label={t("display.preview")}>
    {#each preview as text, index (index)}<span class:title={text === SAMPLE.title}>{text}</span>{/each}
  </p>
</div>

<h3>{t("display.albumPages")}</h3>
<div class="field stacked">
  <span class="label">{t("display.releaseFacts")}</span>
  <OrderedChoices
    options={ALBUM_FACTS}
    value={display.albumFacts}
    onchange={(facts) => appSettings.save((next) => (next.display.albumFacts = facts))}
    label={t("display.albumFacts")}
    addLabel={t("display.addFact")}
    empty={t("display.none")}
    disabled={appSettings.saving}
  />
  <p class="hint">{t("display.factsHint")}</p>
</div>
<label class="switch">
  <input
    type="checkbox"
    checked={display.showDescriptions}
    disabled={appSettings.saving}
    onchange={(event) => {
      const show = event.currentTarget.checked;
      appSettings.save((next) => (next.display.showDescriptions = show));
    }}
  />
  <span>
    <span class="title">{t("display.descriptions")}</span>
    <span class="hint">{t("display.descriptionsHint")}</span>
  </span>
</label>

<div class="actions">
  <button onclick={() => appSettings.reset("display")} disabled={appSettings.saving}>{t("settings.reset")}</button>
</div>

<style>
  .preview {
    display: flex;
    flex-wrap: wrap;
    gap: 0.25rem 1rem;
    margin-top: 0.5rem;
    padding: 0.5rem 0.8rem;
    color: var(--text-muted);
    font-size: 0.9rem;
    max-width: 40rem;
  }

  .preview .title {
    color: var(--text);
    font-weight: 500;
  }
</style>
