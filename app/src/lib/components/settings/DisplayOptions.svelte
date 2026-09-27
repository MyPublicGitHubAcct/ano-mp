<script lang="ts">
  // What track lists show besides the title (as columns when wide, under
  // the title when narrow), what an album's summary line says, and whether
  // Wikipedia's descriptions and biographies show.
  import { ALBUM_FACTS, TRACK_COLUMNS, columnText, textColumns } from "$lib/columns";
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

<h3>Track lists</h3>
<div class="field stacked">
  <OrderedChoices
    options={TRACK_COLUMNS}
    value={display.trackColumns}
    onchange={(columns) => appSettings.save((next) => (next.display.trackColumns = columns))}
    label="Track list columns"
    addLabel="Add a column…"
    empty="Only the title"
    disabled={appSettings.saving}
  />
  <p class="hint">
    Beside the title, in this order, when the list is wide enough; under it when it isn’t. The track number
    leads the row and the length ends it, wherever they are in the list.
  </p>
  <p class="preview card" aria-label="Preview">
    {#each preview as text, index (index)}<span class:title={text === SAMPLE.title}>{text}</span>{/each}
  </p>
</div>

<h3>Album pages</h3>
<div class="field stacked">
  <span class="label">Release facts</span>
  <OrderedChoices
    options={ALBUM_FACTS}
    value={display.albumFacts}
    onchange={(facts) => appSettings.save((next) => (next.display.albumFacts = facts))}
    label="Album facts"
    addLabel="Add a fact…"
    empty="None"
    disabled={appSettings.saving}
  />
  <p class="hint">The line under an album’s title, from the album details source it’s matched on.</p>
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
    <span class="title">Show album descriptions and artist biographies</span>
    <span class="hint">From Wikipedia, when the online sources find them.</span>
  </span>
</label>

<div class="actions">
  <button onclick={() => appSettings.reset("display")} disabled={appSettings.saving}>Reset to defaults</button>
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
