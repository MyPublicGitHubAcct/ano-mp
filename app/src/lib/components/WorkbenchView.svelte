<script lang="ts">
  // The effects workbench (PLAN.md X8): one file, chosen with the open
  // dialog or dropped from the Finder, played through the effects and
  // recorded as one take. The file plays through the queue, after the
  // current item; the transport, A–B loop and seek bar act on it while it
  // is current. The effects' and recording's sections are Settings', and
  // their switches stay the user's: the page never turns either on.
  import { formatTime } from "$lib/format";
  import { t } from "$lib/i18n";
  import { takeSpan } from "$lib/workbench";
  import { features } from "$lib/state/features.svelte";
  import { player } from "$lib/state/player.svelte";
  import { recording } from "$lib/state/recording.svelte";
  import { appSettings } from "$lib/state/settings.svelte";
  import { ui } from "$lib/state/ui.svelte";
  import { workbench } from "$lib/state/workbench.svelte";
  import Icon from "./Icon.svelte";
  import LoopControls from "./LoopControls.svelte";
  import SeekBar from "./SeekBar.svelte";
  import EffectsOptions from "./settings/EffectsOptions.svelte";
  import Options from "./settings/Options.svelte";
  import RecordingOptions from "./settings/RecordingOptions.svelte";

  let loop = $state.raw<[number, number] | null>(null);
  const file = $derived(workbench.file);
  const playing = $derived(workbench.current && player.playing);
  const span = $derived(takeSpan(file?.duration ?? 0, loop));
  const action = $derived(workbench.recordAction);

  /** "FLAC · 44.1 kHz · stereo · 900 kbps", as far as the file says. */
  const details = $derived.by(() => {
    if (!file) return "";
    const parts = [file.format];
    if (file.sampleRate > 0) parts.push(t("workbench.kilohertz", { rate: file.sampleRate / 1000 }));
    if (file.channels === 1) parts.push(t("workbench.mono"));
    else if (file.channels === 2) parts.push(t("workbench.stereo"));
    else if (file.channels > 2) parts.push(t("workbench.channels", { count: file.channels }));
    if (file.bitrateKbps) parts.push(t("recording.kbps", { kbps: file.bitrateKbps }));
    parts.push(formatTime(file.duration));
    return parts.filter(Boolean).join(" · ");
  });

  function setRecording(on: boolean) {
    void appSettings.save((next) => (next.features.recording = on));
  }
</script>

<section class="workbench" aria-labelledby="workbench-title">
  <h1 id="workbench-title">{t("workbench.title")}</h1>

  {#if !features.on.effectsWorkbench}
    <p class="muted">
      {t("workbench.off")}
      <button class="link" onclick={() => ui.showSettings("features")}>{t("health.settingsFeatures")}</button>.
    </p>
  {:else}
    <p class="hint">{t("workbench.intro")}</p>

    <div class="file card">
      <div class="about">
        {#if file}
          <strong class="title">{file.title}</strong>
          {#if file.artist}<span>{file.artist}</span>{/if}
          <span class="hint">{details}</span>
        {:else}
          <span class="hint">{t("workbench.noFile")}</span>
        {/if}
      </div>
      <div class="choose">
        <div class="buttons">
          <button class={file ? "" : "primary"} onclick={() => workbench.choose()} disabled={workbench.opening}>
            {t("workbench.choose")}
          </button>
          <button
            onclick={() => workbench.useCurrent()}
            disabled={workbench.opening || !workbench.canUseCurrent}
            title={player.currentItem ? t("workbench.useCurrentHint") : t("workbench.nothingPlaying")}
          >
            {t("workbench.useCurrent")}
          </button>
        </div>
        <span class="hint">{t("workbench.dropHint")}</span>
      </div>
    </div>

    {#if file}
      <div class="transport">
        <button
          class="icon play"
          onclick={() => workbench.play()}
          disabled={workbench.opening}
          aria-label={playing ? t("bar.pause") : t("bar.play")}
          title={playing ? t("bar.pause") : t("bar.play")}
        >
          <Icon name={playing ? "pause" : "play"} size="1.5rem" />
        </button>
        {#if workbench.current}
          <SeekBar />
        {:else}
          <span class="hint">
            {workbench.playAction === "jump" ? t("workbench.elsewhere") : t("workbench.gone")}
          </span>
        {/if}
      </div>

      {#if !features.on.practiceMode}
        <p class="hint">
          {t("workbench.loopOff")}
          <button class="link" onclick={() => ui.showSettings("features")}>{t("health.settingsFeatures")}</button>.
        </p>
      {:else if workbench.current}
        <LoopControls bind:loop />
      {/if}
    {/if}

    <h2>{t("workbench.effects")}</h2>
    <Options>
      <EffectsOptions />
    </Options>

    <h2>{t("workbench.recording")}</h2>
    <Options>
      <label class="switch">
        <input
          type="checkbox"
          checked={appSettings.current.features.recording}
          disabled={appSettings.saving || recording.recording}
          onchange={(event) => setRecording(event.currentTarget.checked)}
        />
        <span>
          <span class="title">{t("feature.recording")}</span>
          <span class="hint">{t("workbench.recordingSwitch")}</span>
        </span>
      </label>

      <div class="record">
        <button
          class:recording={action === "stop"}
          onclick={() => workbench.record()}
          disabled={recording.busy || action === "recordingOff" || action === "notReady"}
        >
          <Icon name="record" />
          {action === "stop" ? t("workbench.stop") : t("workbench.record")}
        </button>
        <span class="hint" role="status">
          {#if action === "stop" && workbench.taking}
            {t("workbench.taking", {
              time: formatTime(recording.state.seconds),
              length: formatTime(span.to - span.from),
            })}
          {:else if action === "stop"}
            {t("workbench.otherRecording")}
          {:else if action === "recordingOff"}
            {t("workbench.recordingOff")}
          {:else if action === "notReady"}
            {t(file ? "workbench.notReady" : "workbench.chooseFirst")}
          {:else if span.loop}
            {t("workbench.takeLoop", { from: formatTime(span.from), to: formatTime(span.to) })}
          {:else}
            {t("workbench.takeWhole")}
          {/if}
        </span>
      </div>
      {#if workbench.saved}
        <p class="hint" role="status">{t("workbench.saved", { file: workbench.saved })}</p>
      {/if}

      {#if appSettings.current.features.recording}
        <RecordingOptions />
      {/if}
    </Options>
  {/if}
</section>

<style>
  .workbench {
    height: 100%;
    overflow-y: auto;
    padding: 0.75rem 1.25rem 2rem;
  }

  .workbench > :global(*) {
    max-width: 50rem;
  }

  h1 {
    margin: 0 0 0.5rem;
    font-size: 1.6rem;
  }

  h2 {
    font-size: 1.05rem;
    margin: 1.5rem 0 0.3rem;
  }

  .hint {
    color: var(--text-muted);
    font-size: 0.85rem;
    margin: 0.2rem 0;
  }

  .file {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    justify-content: space-between;
    gap: 0.75rem 1.5rem;
    margin: 0.75rem 0;
    padding: 0.75rem 1rem;
    border: 1px solid var(--border);
    border-radius: var(--radius-lg);
    background: var(--surface);
  }

  .about {
    display: flex;
    flex-direction: column;
    gap: 0.15rem;
    min-width: 0;
  }

  .title {
    overflow-wrap: anywhere;
  }

  .choose {
    display: flex;
    flex-direction: column;
    align-items: flex-end;
    gap: 0.2rem;
  }

  .buttons {
    display: flex;
    flex-wrap: wrap;
    justify-content: flex-end;
    gap: 0.5rem;
  }

  .transport {
    display: flex;
    align-items: center;
    gap: 0.75rem;
    margin: 0.5rem 0;
  }

  .record {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 0.5rem 0.75rem;
    padding: 0.4rem 0;
  }

  .record button {
    display: inline-flex;
    align-items: center;
    gap: 0.35rem;
  }

  .record button.recording {
    color: var(--danger);
  }

  @media (max-width: 640px) {
    .choose {
      align-items: flex-start;
    }

    .buttons {
      justify-content: flex-start;
    }
  }
</style>
