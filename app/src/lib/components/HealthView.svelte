<script lang="ts">
  // The library health report (O4): files that don't decode or are cut
  // short and suspected transcodes (found by the loudness analysis), albums
  // whose tags disagree, and likely duplicates. Read-only: each file can be
  // revealed in Finder to fix it there.
  import { untrack } from "svelte";
  import { revealItemInDir } from "@tauri-apps/plugin-opener";
  import { features as api, type HealthReport, type HealthTrack } from "$lib/api";
  import { plural } from "$lib/format";
  import { features } from "$lib/state/features.svelte";
  import { library } from "$lib/state/library.svelte";
  import { attempt } from "$lib/state/toasts.svelte";
  import { ui } from "$lib/state/ui.svelte";

  let report = $state.raw<HealthReport | null>(null);
  let loading = $state(false);

  $effect(() => {
    void [library.version, features.analysisVersion, features.on.healthReport];
    untrack(load);
  });

  async function load() {
    if (!features.on.healthReport) return;
    loading = true;
    report = (await attempt(api.health)) ?? report;
    loading = false;
  }

  const reveal = (track: HealthTrack) => attempt(() => revealItemInDir(track.path));

  const problems = $derived(
    report
      ? report.undecodable.length +
          report.truncated.length +
          report.transcodes.length +
          report.albums.length +
          report.duplicates.length
      : 0,
  );
</script>

{#snippet trackList(tracks: HealthTrack[])}
  <ul>
    {#each tracks as track (track.trackId)}
      <li>
        <span class="text">
          <span class="name">{track.title}</span>
          <span class="muted small">{[track.artist, track.album].filter(Boolean).join(" · ")}</span>
          {#if track.detail}<span class="detail small">{track.detail}</span>{/if}
          <span class="path small" title={track.path}>{track.path}</span>
        </span>
        <button onclick={() => reveal(track)}>Reveal in Finder</button>
      </li>
    {/each}
  </ul>
{/snippet}

<section class="health" aria-labelledby="health-heading">
  <header>
    <h1 id="health-heading">Library health</h1>
    <button onclick={load} disabled={loading}>Check again</button>
  </header>

  {#if !features.on.healthReport}
    <p class="muted">
      The health report is off. Turn it on in
      <button class="link" onclick={() => ui.showSettings("features")}>Settings › Features</button>.
    </p>
  {:else if report}
    <p class="muted">
      {problems === 0 ? "No problems found." : `${plural(problems, "problem")} found.`}
      The decoding and transcode checks cover the {report.analysed.toLocaleString()} of
      {plural(report.tracks, "track")} analysed so far{features.on.loudnessAnalysis
        ? ""
        : "; turn on the loudness analysis to check them all"}.
    </p>

    {#if report.undecodable.length > 0}
      <h2>Don’t decode</h2>
      {@render trackList(report.undecodable)}
    {/if}
    {#if report.truncated.length > 0}
      <h2>Cut short</h2>
      <p class="hint">They end sooner than their headers say, often from an interrupted copy or download.</p>
      {@render trackList(report.truncated)}
    {/if}
    {#if report.transcodes.length > 0}
      <h2>Suspected transcodes</h2>
      <p class="hint">
        Lossless files whose sound stops at 15–19.5 kHz for the whole track, as MP3 or AAC encoders leave it. A
        heuristic: some recordings are like that anyway.
      </p>
      {@render trackList(report.transcodes)}
    {/if}
    {#if report.albums.length > 0}
      <h2>Albums whose tracks disagree</h2>
      <ul>
        {#each report.albums as album (album.albumId)}
          <li>
            <span class="text">
              <span class="name">{album.title}</span>
              <span class="muted small">{album.artist ?? "No album artist"}</span>
              {#each album.problems as problem (problem)}<span class="detail small">{problem}</span>{/each}
            </span>
            <button
              onclick={() =>
                library.showAlbum({ id: album.albumId, title: album.title, albumArtist: album.artist, albumArtistId: null })}
              >Show</button
            >
          </li>
        {/each}
      </ul>
    {/if}
    {#if report.duplicates.length > 0}
      <h2>Likely duplicates</h2>
      {#each report.duplicates as group, index (index)}
        <p class="hint">{group.reason}</p>
        {@render trackList(group.tracks)}
      {/each}
    {/if}
  {:else if loading}
    <p class="muted">Checking…</p>
  {/if}
</section>

<style>
  .health {
    height: 100%;
    overflow-y: auto;
    padding: 0.75rem 1.25rem 2rem;
  }

  header {
    display: flex;
    align-items: center;
    justify-content: space-between;
  }

  h1 {
    margin: 0 0 0.5rem;
    font-size: 1.6rem;
  }

  h2 {
    font-size: 1.05rem;
    margin: 1.25rem 0 0.3rem;
  }

  .hint {
    color: var(--text-muted);
    font-size: 0.85rem;
    margin: 0.2rem 0;
  }

  ul {
    list-style: none;
    margin: 0 0 0.5rem;
    padding: 0;
  }

  li {
    display: flex;
    align-items: center;
    gap: 1rem;
    padding: 0.4rem 0;
    border-top: 1px solid var(--border);
  }

  .text {
    flex: 1;
    display: flex;
    flex-direction: column;
    min-width: 0;
  }

  .name,
  .path {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .small {
    font-size: 0.8rem;
  }

  .detail {
    color: var(--danger);
  }

  .path {
    color: var(--text-faint);
  }
</style>
