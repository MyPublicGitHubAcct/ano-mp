<script lang="ts">
  // Where the sound goes (the output device and its buffer size, with what
  // the device is doing now) and ReplayGain (off, by track or by album, the
  // preamp, the gain for untagged tracks, clipping). A new device is opened
  // before the setting is saved; if it won't open, the one before plays on.
  import { onMount } from "svelte";
  import { on, settings as api, type OutputStatus, type ReplayGainMode } from "$lib/api";
  import { appSettings } from "$lib/state/settings.svelte";
  import { attempt } from "$lib/state/toasts.svelte";

  const MODES: { id: ReplayGainMode; name: string; about: string }[] = [
    { id: "off", name: "Off", about: "Every track plays as it was mastered." },
    { id: "track", name: "By track", about: "Every track at about the same loudness; best for shuffle." },
    {
      id: "album",
      name: "By album",
      about: "Every album at about the same loudness, keeping the quiet and loud tracks within it.",
    },
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
    current && current.sampleRate > 0 ? ` (${((samples / current.sampleRate) * 1000).toFixed(1)} ms)` : "";

  const db = (value: number) => `${value > 0 ? "+" : value < 0 ? "−" : ""}${Math.abs(value).toFixed(1)} dB`;
</script>

<h3>Output</h3>
<label class="field">
  <span class="label">Device</span>
  <span class="control">
    <select
      value={output.device ?? ""}
      disabled={appSettings.saving || status === null}
      onchange={(event) => {
        const device = event.currentTarget.value || null;
        setOutput((next) => (next.device = device));
      }}
    >
      <option value="">System default</option>
      {#each status?.devices ?? [] as device (device)}
        <option value={device}>{device}</option>
      {/each}
      {#if output.device !== null && status?.chosenMissing}
        <option value={output.device}>{output.device} (not connected)</option>
      {/if}
    </select>
  </span>
  {#if status?.chosenMissing}
    <span class="hint warning">
      {output.device} isn’t connected, so the default device plays; it takes over again when it’s back.
    </span>
  {/if}
</label>

<label class="field">
  <span class="label">Buffer size</span>
  <span class="control">
    <select
      value={output.bufferSize === null ? "" : String(output.bufferSize)}
      disabled={appSettings.saving || current === null}
      onchange={(event) => {
        const value = event.currentTarget.value;
        setOutput((next) => (next.bufferSize = value === "" ? null : Number(value)));
      }}
    >
      <option value="">Device default{current ? `: ${current.defaultBufferSize} samples${ms(current.defaultBufferSize)}` : ""}</option>
      {#each sizes as size (size)}
        <option value={String(size)}>{size} samples{ms(size)}</option>
      {/each}
    </select>
  </span>
  <span class="hint">
    Smaller buffers respond sooner (pause, seek, the visualizer) but may crackle on a busy computer; larger ones are
    safer.
  </span>
</label>

<p class="now muted" role="status">
  {#if status === null}
    Looking for devices…
  {:else if current}
    Playing through {current.name} at {(current.sampleRate / 1000).toLocaleString()} kHz, {current.bufferSize} samples
    a block, about {Math.round(current.outputLatency * 1000)} ms to the speakers.
  {:else}
    No output device is open.
  {/if}
</p>

<h3>ReplayGain</h3>
<div class="modes" role="radiogroup" aria-label="ReplayGain">
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
<p class="hint">
  Uses the ReplayGain (or Opus R128) tags that a loudness scanner such as foobar2000, beets or loudgain wrote into
  your files; they’re read when folders are scanned. A track with only one kind of gain uses that one in either
  mode.
</p>

<fieldset disabled={playback.replayGain === "off" || appSettings.saving}>
  <label class="field">
    <span class="label">Preamp</span>
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
    <span class="hint">Added to every tagged track’s gain. ReplayGain aims a little quieter than most modern releases.</span>
  </label>
  <label class="field">
    <span class="label">Untagged tracks</span>
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
    <span class="hint">For tracks without ReplayGain tags, so they aren’t much louder than tagged ones.</span>
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
      <span class="title">Prevent clipping</span>
      <span class="hint">Turns a track up only as far as its tagged peak allows.</span>
    </span>
  </label>
</fieldset>

<div class="actions">
  <button onclick={() => appSettings.reset("playback")} disabled={appSettings.saving}>Reset ReplayGain</button>
  <button
    onclick={async () => {
      await appSettings.reset("output");
      await refresh();
    }}
    disabled={appSettings.saving}>Use the default device</button
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
