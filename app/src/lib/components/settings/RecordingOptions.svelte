<script lang="ts">
  // Recording (PLAN.md X6): the folder recordings go in, what they are
  // written as (WAV, AIFF, FLAC, Apple Lossless, AAC or MP3, with a sample
  // size or a bitrate), and whether a cue sheet names the tracks. Shown
  // while the `recording` feature is on; saved as it changes. A format
  // this build can't write (MP3 without LAME) isn't offered.
  import { onMount } from "svelte";
  import type { RecordingKind, RecordingSettings } from "$lib/api";
  import { t, type MessageKey } from "$lib/i18n";
  import { recording } from "$lib/state/recording.svelte";
  import { appSettings } from "$lib/state/settings.svelte";

  const FORMATS: { id: RecordingKind; name: MessageKey }[] = [
    { id: "wav", name: "recording.format.wav" },
    { id: "aiff", name: "recording.format.aiff" },
    { id: "flac", name: "recording.format.flac" },
    { id: "alac", name: "recording.format.alac" },
    { id: "aac", name: "recording.format.aac" },
    { id: "mp3", name: "recording.format.mp3" },
  ];
  const BITRATES = [128, 160, 192, 256, 320];

  const settings = $derived(appSettings.current.recording);
  const lossy = $derived(settings.format === "aac" || settings.format === "mp3");
  /** Float is WAV's alone; the others record 24-bit in its place. */
  const bits = $derived(settings.format !== "wav" && settings.bits === 32 ? 24 : settings.bits);
  const sizes = $derived(settings.format === "wav" ? [16, 24, 32] : [16, 24]);

  onMount(() => void recording.refresh());

  const save = (edit: (next: RecordingSettings) => void) => appSettings.save((next) => edit(next.recording));

  /** Megabytes a minute at 48 kHz, for the hint. */
  const perMinute = $derived(
    lossy ? Math.round((settings.bitrateKbps * 60) / 8 / 1000) : Math.round((48000 * 2 * (bits / 8) * 60) / 1e6),
  );
</script>

<div class="field">
  <span class="label">{t("recording.folder")}</span>
  <span class="control folder">
    <span class="path" title={recording.state.folder ?? ""}>{recording.state.folder ?? t("recording.noFolder")}</span>
    <button onclick={() => recording.chooseFolder()}>{t("recording.chooseFolder")}</button>
  </span>
  <span class="hint">{t("recording.folderHint")}</span>
</div>

<label class="field">
  <span class="label">{t("recording.format")}</span>
  <span class="control">
    <select
      value={settings.format}
      disabled={appSettings.saving || recording.recording}
      onchange={(event) => {
        const format = event.currentTarget.value as RecordingKind;
        void save((next) => (next.format = format));
      }}
    >
      {#each FORMATS as format (format.id)}
        <option value={format.id} disabled={!recording.available(format.id)}>{t(format.name)}</option>
      {/each}
    </select>
  </span>
</label>

{#if lossy}
  <label class="field">
    <span class="label">{t("recording.bitrate")}</span>
    <span class="control">
      <select
        value={settings.bitrateKbps}
        disabled={appSettings.saving || recording.recording}
        onchange={(event) => {
          const kbps = Number(event.currentTarget.value);
          void save((next) => (next.bitrateKbps = kbps));
        }}
      >
        {#each BITRATES as kbps (kbps)}
          <option value={kbps}>{t("recording.kbps", { kbps })}</option>
        {/each}
      </select>
    </span>
  </label>
{:else}
  <label class="field">
    <span class="label">{t("recording.bits")}</span>
    <span class="control">
      <select
        value={bits}
        disabled={appSettings.saving || recording.recording}
        onchange={(event) => {
          const size = Number(event.currentTarget.value);
          void save((next) => (next.bits = size));
        }}
      >
        {#each sizes as size (size)}
          <option value={size}
            >{size === 32 ? t("recording.bitsFloat") : t("recording.bitsInteger", { bits: size })}</option
          >
        {/each}
      </select>
    </span>
  </label>
{/if}
<p class="hint">
  {t("recording.size", { mb: perMinute })}
  {#if settings.format === "mp3"}{t("recording.mp3Rates")}{:else if settings.format === "aac"}{t(
      "recording.aacRates",
    )}{/if}
</p>

<label class="switch">
  <input
    type="checkbox"
    checked={settings.cueSheet}
    disabled={appSettings.saving}
    onchange={(event) => {
      const on = event.currentTarget.checked;
      void save((next) => (next.cueSheet = on));
    }}
  />
  <span>
    <span class="title">{t("recording.cueSheet")}</span>
    <span class="hint">{t("recording.cueSheetHint")}</span>
  </span>
</label>

<p class="hint">{t("recording.hint")}</p>

<style>
  .folder {
    display: flex;
    align-items: center;
    gap: 0.5rem;
    min-width: 0;
  }

  .path {
    flex: 1;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
</style>
