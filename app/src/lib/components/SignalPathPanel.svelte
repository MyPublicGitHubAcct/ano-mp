<script lang="ts">
  // The signal path (O10): every step from the file to the speakers, as the
  // engine has it now — the file's codec, bit depth, rate and bit rate, the
  // gain applied, practice mode's stretching, resampling (or not),
  // crossfeed, the volume, and the device.
  import { onMount } from "svelte";
  import { player as api, type SignalPath } from "$lib/api";
  import { features } from "$lib/state/features.svelte";
  import { player } from "$lib/state/player.svelte";
  import Popover from "./Popover.svelte";

  let { onclose }: { onclose: () => void } = $props();
  let path = $state.raw<SignalPath | null>(null);

  const refresh = async () => (path = await api.signalPath().catch(() => path));

  onMount(() => {
    refresh();
    const timer = setInterval(refresh, 1000);
    return () => clearInterval(timer);
  });

  const khz = (hz: number) => `${(hz / 1000).toLocaleString(undefined, { maximumFractionDigits: 1 })} kHz`;
  const db = (linear: number) => {
    if (linear <= 0) return "−∞ dB";
    const value = 20 * Math.log10(linear);
    return `${value > 0.05 ? "+" : value < -0.05 ? "−" : ""}${Math.abs(value).toFixed(1)} dB`;
  };
  const CROSSFEED = ["Off", "Light", "Medium", "Strong"];
</script>

<Popover title="Signal path" {onclose}>
  {#if !path || !path.path.loaded}
    <p class="muted">{player.currentItem ? "Start playing to see the path." : "Nothing is playing."}</p>
  {:else}
    {@const p = path.path}
    <ol>
      <li>
        <span class="step">File</span>
        <span>
          {p.codec.toUpperCase()}
          {#if p.bitsPerSample}· {p.bitsPerSample}-bit{/if}
          · {khz(p.fileSampleRate)} · {p.fileChannels === 1 ? "mono" : p.fileChannels === 2 ? "stereo" : `${p.fileChannels} channels`}
          {#if p.bitrateKbps}· {p.bitrateKbps} kbps{/if}
          <span class="muted">{p.lossless ? "(lossless)" : "(lossy)"}</span>
        </span>
      </li>
      <li>
        <span class="step">Gain</span>
        <span>{db(p.trackGain)} <span class="muted">(ReplayGain and your offsets)</span></span>
      </li>
      {#if p.tempo !== 1 || p.semitones !== 0}
        <li>
          <span class="step">Practice</span>
          <span>{Math.round(p.tempo * 100)}% speed{p.semitones !== 0 ? `, ${p.semitones > 0 ? "+" : ""}${p.semitones} semitones` : ""}</span>
        </li>
      {/if}
      <li>
        <span class="step">Rate</span>
        <span>
          {#if p.resampling}
            Resampled {khz(p.fileSampleRate)} → {khz(p.deviceSampleRate)}
          {:else}
            Not resampled
          {/if}
          {#if p.resampling && !features.on.matchSampleRate}
            <span class="muted">(“Match the device’s sample rate” in Features avoids this)</span>
          {/if}
        </span>
      </li>
      <li>
        <span class="step">Crossfeed</span>
        <span>
          {CROSSFEED[p.crossfeed] ?? "Off"}
          {#if path.headphones !== null}<span class="muted">({path.headphones ? "headphones" : "not headphones"})</span>{/if}
        </span>
      </li>
      <li><span class="step">Volume</span><span>{db(p.volume)}</span></li>
      <li>
        <span class="step">Device</span>
        <span>
          {#if path.device}
            {path.device.name} · {khz(path.device.sampleRate)} · {path.device.bufferSize} samples
          {:else}
            None open
          {/if}
        </span>
      </li>
    </ol>
  {/if}
</Popover>

<style>
  ol {
    list-style: none;
    margin: 0;
    padding: 0;
    counter-reset: step;
  }

  li {
    display: grid;
    grid-template-columns: 5.5rem minmax(0, 1fr);
    gap: 0.5rem;
    padding: 0.3rem 0;
    border-top: 1px solid var(--border);
    font-size: 0.88rem;
  }

  li:first-child {
    border-top: none;
  }

  .step {
    font-weight: 600;
    color: var(--text-muted);
  }
</style>
