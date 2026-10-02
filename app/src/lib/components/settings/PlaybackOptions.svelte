<script lang="ts">
  // Where the sound goes (the output device and its buffer size, with what
  // the device is doing now) and ReplayGain (off, by track or by album, the
  // preamp, the gain for untagged tracks, clipping). A new device is opened
  // before the setting is saved; if it won't open, the one before plays on.
  import { t } from "$lib/i18n";
  import { onMount } from "svelte";
  import { on, settings as api, type OutputStatus, type ReplayGainMode } from "$lib/api";
  import { appSettings } from "$lib/state/settings.svelte";
  import { attempt } from "$lib/state/toasts.svelte";

  const MODES: { id: ReplayGainMode; name: string; about: string }[] = [
    { id: "off", name: t("playback.rgOff"), about: t("playback.rgOffAbout") },
    { id: "track", name: t("playback.rgTrack"), about: t("playback.rgTrackAbout") },
    { id: "album", name: t("playback.rgAlbum"), about: t("playback.rgAlbumAbout") },
  ];

  let status = $state.raw<OutputStatus | null>(null);

  const output = $derived(appSettings.current.output);
  const playback = $derived(appSettings.current.playback);
  const current = $derived(status?.current ?? null);
  /** Buffer sizes to offer: the device's, and the saved one if the device doesn't have it. */
  const sizes = $derived.by(() => {
    const offered = current?.bufferSizes ?? [];
    const saved = output.bufferSize;
    return saved !== null && !offered.includes(saved) ? [...offered, saved].sort((a, b) => a - b) : offered;
  });

  const refresh = () =>
    attempt(async () => {
      status = await api.outputStatus();
    });

  onMount(() => {
    refresh();
    const listener = on("audio-device-changed", () => refresh());
    return () => void listener.then((stop) => stop());
  });

  async function setOutput(edit: (next: typeof output) => void) {
    await appSettings.save((next) => edit(next.output));
    await refresh();
  }

  const ms = (samples: number) =>
    current && current.sampleRate > 0
      ? ` ${t("playback.ms", { ms: ((samples / current.sampleRate) * 1000).toFixed(1) })}`
      : "";

  const db = (value: number) => `${value > 0 ? "+" : value < 0 ? "−" : ""}${Math.abs(value).toFixed(1)} dB`;
</script>

<h3>{t("playback.output")}</h3>
<label class="field">
  <span class="label">{t("playback.device")}</span>
  <span class="control">
    <select
      value={output.device ?? ""}
      disabled={appSettings.saving || status === null}
      onchange={(event) => {
        const device = event.currentTarget.value || null;
        setOutput((next) => (next.device = device));
      }}
    >
      <option value="">{t("playback.systemDefault")}</option>
      {#each status?.devices ?? [] as device (device)}
        <option value={device}>{device}</option>
      {/each}
      {#if output.device !== null && status?.chosenMissing}
        <option value={output.device}>{t("playback.notConnected", { name: output.device })}</option>
      {/if}
    </select>
  </span>
  {#if status?.chosenMissing}
    <span class="hint warning">{t("playback.missing", { name: output.device ?? "" })}</span>
  {/if}
</label>

<label class="field">
  <span class="label">{t("playback.bufferSize")}</span>
  <span class="control">
    <select
      value={output.bufferSize === null ? "" : String(output.bufferSize)}
      disabled={appSettings.saving || current === null}
      onchange={(event) => {
        const value = event.currentTarget.value;
        setOutput((next) => (next.bufferSize = value === "" ? null : Number(value)));
      }}
    >
      <option value=""
        >{current
          ? t("playback.deviceDefaultSize", { size: current.defaultBufferSize }) + ms(current.defaultBufferSize)
          : t("playback.deviceDefault")}</option
      >
      {#each sizes as size (size)}
        <option value={String(size)}>{t("playback.samples", { count: size })}{ms(size)}</option>
      {/each}
    </select>
  </span>
  <span class="hint">{t("playback.bufferHint")}</span>
</label>

<p class="now muted" role="status">
  {#if status === null}
    {t("playback.looking")}
  {:else if current}
    {t("playback.now", {
      name: current.name,
      rate: (current.sampleRate / 1000).toLocaleString(),
      size: current.bufferSize,
      latency: Math.round(current.outputLatency * 1000),
    })}
  {:else}
    {t("playback.noDevice")}
  {/if}
</p>

<h3>{t("playback.replayGain")}</h3>
<div class="modes" role="radiogroup" aria-label={t("playback.replayGain")}>
  {#each MODES as mode (mode.id)}
    <label class="switch">
      <input
        type="radio"
        name="replay-gain"
        value={mode.id}
        checked={playback.replayGain === mode.id}
        disabled={appSettings.saving}
        onchange={() => appSettings.save((next) => (next.playback.replayGain = mode.id))}
      />
      <span>
        <span class="title">{mode.name}</span>
        <span class="hint">{mode.about}</span>
      </span>
    </label>
  {/each}
</div>
<p class="hint">{t("playback.rgHint")}</p>

<fieldset disabled={playback.replayGain === "off" || appSettings.saving}>
  <label class="field">
    <span class="label">{t("eq.preamp")}</span>
    <span class="control">
      <input
        type="range"
        min="-15"
        max="15"
        step="0.5"
        value={playback.preamp}
        onchange={(event) => {
          const value = Number(event.currentTarget.value);
          appSettings.save((next) => (next.playback.preamp = value));
        }}
      />
      <span class="value">{db(playback.preamp)}</span>
    </span>
    <span class="hint">{t("playback.preampHint")}</span>
  </label>
  <label class="field">
    <span class="label">{t("playback.untagged")}</span>
    <span class="control">
      <input
        type="range"
        min="-15"
        max="15"
        step="0.5"
        value={playback.untaggedGain}
        onchange={(event) => {
          const value = Number(event.currentTarget.value);
          appSettings.save((next) => (next.playback.untaggedGain = value));
        }}
      />
      <span class="value">{db(playback.untaggedGain)}</span>
    </span>
    <span class="hint">{t("playback.untaggedHint")}</span>
  </label>
  <label class="switch">
    <input
      type="checkbox"
      checked={playback.preventClipping}
      onchange={(event) => {
        const on = event.currentTarget.checked;
        appSettings.save((next) => (next.playback.preventClipping = on));
      }}
    />
    <span>
      <span class="title">{t("playback.preventClipping")}</span>
      <span class="hint">{t("playback.preventClippingHint")}</span>
    </span>
  </label>
</fieldset>

<h3>{t("playback.crossfadeTitle")}</h3>
<label class="field">
  <span class="label">{t("playback.crossfade")}</span>
  <span class="control">
    <input
      type="range"
      min="0"
      max="12"
      step="0.5"
      value={playback.crossfade}
      disabled={appSettings.saving}
      aria-valuetext={playback.crossfade === 0
        ? t("playback.crossfadeOff")
        : t("playback.seconds", { count: playback.crossfade })}
      onchange={(event) => {
        const value = Number(event.currentTarget.value);
        appSettings.save((next) => (next.playback.crossfade = value));
      }}
    />
    <span class="value">
      {playback.crossfade === 0 ? t("playback.crossfadeOff") : t("playback.seconds", { count: playback.crossfade })}
    </span>
  </span>
  <span class="hint">{t("playback.crossfadeHint")}</span>
</label>

<div class="actions">
  <button onclick={() => appSettings.reset("playback")} disabled={appSettings.saving}>{t("playback.reset")}</button>
  <button
    onclick={async () => {
      await appSettings.reset("output");
      await refresh();
    }}
    disabled={appSettings.saving}>{t("playback.useDefault")}</button
  >
</div>

<style>
  .now {
    margin-top: 0.5rem;
    font-size: 0.9rem;
  }

  .warning {
    color: var(--danger);
  }

  .modes {
    display: flex;
    flex-direction: column;
  }

  fieldset {
    border: none;
    margin: 0.5rem 0 0;
    padding: 0;
    min-width: 0;
  }

  fieldset:disabled {
    opacity: 0.55;
  }
</style>
