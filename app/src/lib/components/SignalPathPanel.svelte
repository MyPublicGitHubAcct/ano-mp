<script lang="ts">
  // The signal path (O10): every step from the file to the speakers, as the
  // engine has it now — the file's codec, bit depth, rate and bit rate, the
  // gain applied, practice mode's stretching, resampling (or not), the
  // effects (X2), the equaliser, crossfeed, crossfading into the next
  // track, a recording in progress (X6, which takes the sound before the
  // volume), the volume, and the device.
  import { onMount } from "svelte";
  import { player as api, type SignalPathPayload } from "$lib/api";
  import { has, t, type MessageKey } from "$lib/i18n";
  import { features } from "$lib/state/features.svelte";
  import { player } from "$lib/state/player.svelte";
  import Popover from "./Popover.svelte";

  let { onclose }: { onclose: () => void } = $props();
  let path = $state.raw<SignalPathPayload | null>(null);

  const refresh = async () => (path = await api.signalPath().catch(() => path));

  onMount(() => {
    refresh();
    const timer = setInterval(refresh, 1000);
    return () => clearInterval(timer);
  });

  const khz = (hz: number) =>
    t("info.kHz", { rate: (hz / 1000).toLocaleString(undefined, { maximumFractionDigits: 1 }) });
  const db = (linear: number) => {
    if (linear <= 0) return "−∞ dB";
    const value = 20 * Math.log10(linear);
    return `${value > 0.05 ? "+" : value < -0.05 ? "−" : ""}${Math.abs(value).toFixed(1)} dB`;
  };
  const effectName = (id: string) => {
    const key = `effects.${id}`;
    return has(key) ? t(key) : id;
  };
  const CROSSFEED: MessageKey[] = [
    "signal.off",
    "signal.crossfeedLight",
    "signal.crossfeedMedium",
    "signal.crossfeedStrong",
  ];
</script>

<Popover title={t("signal.title")} {onclose}>
  {#if !path || !path.path.loaded}
    <p class="muted">{t(player.currentItem ? "signal.startPlaying" : "signal.nothing")}</p>
  {:else}
    {@const p = path.path}
    <ol>
      <li>
        <span class="step">{t("signal.file")}</span>
        <span>
          {p.codec.toUpperCase()}
          {#if p.bitsPerSample}· {t("signal.bits", { bits: p.bitsPerSample })}{/if}
          · {khz(p.fileSampleRate)} · {p.fileChannels === 1
            ? t("info.mono")
            : p.fileChannels === 2
              ? t("info.stereo")
              : t("info.channels", { count: p.fileChannels })}
          {#if p.bitrateKbps}· {t("info.kbps", { rate: p.bitrateKbps })}{/if}
          <span class="muted">{t(p.lossless ? "info.lossless" : "signal.lossy")}</span>
        </span>
      </li>
      <li>
        <span class="step">{t("signal.gain")}</span>
        <span>{db(p.trackGain)} <span class="muted">{t("signal.gainHint")}</span></span>
      </li>
      {#if p.tempo !== 1 || p.semitones !== 0}
        <li>
          <span class="step">{t("signal.practice")}</span>
          <span>
            {t("signal.speed", { percent: Math.round(p.tempo * 100) })}{p.semitones !== 0
              ? `, ${t("signal.semitones", { count: p.semitones, signed: `${p.semitones > 0 ? "+" : ""}${p.semitones}` })}`
              : ""}
          </span>
        </li>
      {/if}
      <li>
        <span class="step">{t("signal.rate")}</span>
        <span>
          {#if p.resampling}
            {t("signal.resampled", { from: khz(p.fileSampleRate), to: khz(p.deviceSampleRate) })}
          {:else}
            {t("signal.notResampled")}
          {/if}
          {#if p.resampling && !features.on.matchSampleRate}
            <span class="muted">{t("signal.matchRateHint")}</span>
          {/if}
        </span>
      </li>
      {#if p.effects.length > 0 || features.on.effects}
        <li>
          <span class="step">{t("signal.effects")}</span>
          <span>
            {p.effects.length > 0 ? p.effects.map(effectName).join(" · ") : t("signal.off")}
            {#if p.freezeHeld}<span class="muted">{t("signal.freezeHeld")}</span>{/if}
          </span>
        </li>
      {/if}
      <li>
        <span class="step">{t("signal.equaliser")}</span>
        <span>{t(p.equaliser ? "signal.on" : "signal.off")}</span>
      </li>
      <li>
        <span class="step">{t("signal.crossfeed")}</span>
        <span>
          {t(CROSSFEED[p.crossfeed] ?? "signal.off")}
          {#if path.headphones !== null}<span class="muted"
              >{t(path.headphones ? "signal.headphones" : "signal.notHeadphones")}</span
            >{/if}
        </span>
      </li>
      <li>
        <span class="step">{t("signal.crossfade")}</span>
        <span>{p.crossfade > 0 ? t("signal.crossfadeSeconds", { seconds: p.crossfade }) : t("signal.off")}</span>
      </li>
      {#if p.recording}
        <li><span class="step">{t("signal.recording")}</span><span>{t("signal.recordingOn")}</span></li>
      {/if}
      <li><span class="step">{t("signal.volume")}</span><span>{db(p.volume)}</span></li>
      <li>
        <span class="step">{t("signal.device")}</span>
        <span>
          {#if path.device}
            {path.device.name} · {khz(path.device.sampleRate)} · {t("signal.buffer", { count: path.device.bufferSize })}
          {:else}
            {t("signal.noDevice")}
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
