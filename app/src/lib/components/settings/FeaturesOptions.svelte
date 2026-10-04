<script lang="ts">
  // Every optional feature (PLAN.md §4.6, O1–O19) with a switch, and each
  // one's options under it: the loudness analysis's progress, crossfeed's
  // strength, the ListenBrainz token, what outside recommendations send
  // (X5), the remote's pairing code and paired phones. Saved as they change, like the other sections.
  import { onMount } from "svelte";
  import {
    features as api,
    on,
    outside,
    type CrossfeedLevel,
    type FeatureSettings,
    type ListenBrainzStatus,
    type OutsideStatus,
    type RemoteStatus,
  } from "$lib/api";
  import { formatDay } from "$lib/format";
  import { count, t } from "$lib/i18n";
  import { features } from "$lib/state/features.svelte";
  import { appSettings } from "$lib/state/settings.svelte";
  import { attempt, toasts } from "$lib/state/toasts.svelte";
  import { ui } from "$lib/state/ui.svelte";

  type Switch = { key: keyof FeatureSettings; id: string; title: string; about: string };

  const f = $derived(appSettings.current.features);

  const save = (edit: (next: FeatureSettings) => void) => appSettings.save((next) => edit(next.features));
  const toggle = (key: keyof FeatureSettings, on: boolean) =>
    save((next) => ((next as Record<string, unknown>)[key] = on));

  const SOUND: Switch[] = [
    {
      key: "loudnessAnalysis",
      id: "O1",
      title: t("feature.loudnessAnalysis"),
      about: t("feature.loudnessAnalysisAbout"),
    },
    {
      key: "waveformSeekBar",
      id: "O2",
      title: t("feature.waveformSeekBar"),
      about: t("feature.waveformSeekBarAbout"),
    },
    {
      key: "segueShuffle",
      id: "O3",
      title: t("feature.segueShuffle"),
      about: t("feature.segueShuffleAbout"),
    },
    {
      key: "signalPath",
      id: "O10",
      title: t("feature.signalPath"),
      about: t("feature.signalPathAbout"),
    },
    {
      key: "matchSampleRate",
      id: "O10",
      title: t("feature.matchSampleRate"),
      about: t("feature.matchSampleRateAbout"),
    },
    {
      key: "practiceMode",
      id: "O12",
      title: t("feature.practiceMode"),
      about: t("feature.practiceModeAbout"),
    },
    {
      key: "effects",
      id: "X2",
      title: t("feature.effects"),
      about: t("feature.effectsAbout"),
    },
    {
      key: "recording",
      id: "X6",
      title: t("feature.recording"),
      about: t("feature.recordingAbout"),
    },
  ];

  const LIBRARY: Switch[] = [
    {
      key: "healthReport",
      id: "O4",
      title: t("feature.healthReport"),
      about: t("feature.healthReportAbout"),
    },
    {
      key: "cueSheets",
      id: "O5",
      title: t("feature.cueSheets"),
      about: t("feature.cueSheetsAbout"),
    },
    {
      key: "classical",
      id: "O6",
      title: t("feature.classical"),
      about: t("feature.classicalAbout"),
    },
    {
      key: "playbackPreferences",
      id: "O7",
      title: t("feature.playbackPreferences"),
      about: t("feature.playbackPreferencesAbout"),
    },
    {
      key: "lyrics",
      id: "O13",
      title: t("feature.lyrics"),
      about: t("feature.lyricsAbout"),
    },
  ];

  const LISTENING: Switch[] = [
    {
      key: "listeningHistory",
      id: "O8",
      title: t("feature.listeningHistory"),
      about: t("feature.listeningHistoryAbout"),
    },
    {
      key: "recentlyPlayed",
      id: "O16",
      title: t("feature.recentlyPlayed"),
      about: t("feature.recentlyPlayedAbout"),
    },
    {
      key: "topPlayed",
      id: "O19",
      title: t("feature.topPlayed"),
      about: t("feature.topPlayedAbout"),
    },
    {
      key: "libraryRadio",
      id: "O9",
      title: t("feature.libraryRadio"),
      about: t("feature.libraryRadioAbout"),
    },
    {
      key: "radioAfterQueue",
      id: "O9",
      title: t("feature.radioAfterQueue"),
      about: t("feature.radioAfterQueueAbout"),
    },
    {
      key: "recentlyAdded",
      id: "O15",
      title: t("feature.recentlyAdded"),
      about: t("feature.recentlyAddedAbout"),
    },
    {
      key: "onThisDay",
      id: "O17",
      title: t("feature.onThisDay"),
      about: t("feature.onThisDayAbout"),
    },
    {
      key: "moreInGenre",
      id: "O18",
      title: t("feature.moreInGenre"),
      about: t("feature.moreInGenreAbout"),
    },
    {
      key: "recommendations",
      id: "X4",
      title: t("feature.recommendations"),
      about: t("feature.recommendationsAbout"),
    },
    {
      key: "outsideRecommendations",
      id: "X5",
      title: t("feature.outsideRecommendations"),
      about: t("feature.outsideRecommendationsAbout"),
    },
    {
      key: "similarArtists",
      id: "X7",
      title: t("feature.similarArtists"),
      about: t("feature.similarArtistsAbout"),
    },
  ];

  const LOOK: Switch[] = [
    {
      key: "themes",
      id: "X1",
      title: t("feature.themes"),
      about: t("feature.themesAbout"),
    },
  ];

  const CROSSFEED: { id: CrossfeedLevel; name: string }[] = [
    { id: "off", name: t("signal.off") },
    { id: "light", name: t("signal.crossfeedLight") },
    { id: "medium", name: t("signal.crossfeedMedium") },
    { id: "strong", name: t("signal.crossfeedStrong") },
  ];

  let listenBrainz = $state.raw<ListenBrainzStatus | null>(null);
  let outsideStatus = $state.raw<OutsideStatus | null>(null);
  let token = $state("");
  let remote = $state.raw<RemoteStatus | null>(null);
  let port = $state(String(appSettings.current.features.remotePort));

  const refresh = async () => {
    listenBrainz = await api.listenBrainzStatus().catch(() => null);
    outsideStatus = await outside.status().catch(() => null);
    remote = await api.remoteStatus().catch(() => null);
  };

  onMount(() => {
    refresh();
    const stop = on("settings-changed", () => setTimeout(refresh, 300));
    return () => void stop.then((unlisten) => unlisten());
  });

  // A shown pairing code runs out after five minutes.
  $effect(() => {
    if (!remote?.code) return;
    const timer = setInterval(async () => (remote = await api.remoteStatus().catch(() => remote)), 15000);
    return () => clearInterval(timer);
  });

  async function saveToken(value: string | null) {
    const user = await attempt(() => api.setListenBrainzToken(value));
    if (user !== undefined) {
      token = "";
      if (user) toasts.show(t("features.listenBrainzConnected", { user }), "info");
      await refresh();
    }
  }

  async function savePort() {
    const value = Number(port);
    if (!Number.isInteger(value) || value < 1024 || value > 65535) {
      toasts.show(t("features.portRange"));
      port = String(f.remotePort);
      return;
    }
    await save((next) => (next.remotePort = value));
  }

  const analysisText = $derived.by(() => {
    const progress = features.analysis;
    const total = progress.done + progress.remaining;
    if (total === 0) return t("features.nothingToAnalyse");
    const failed = progress.failed > 0 ? ` ${count("features.undecodable", progress.failed)}` : "";
    if (progress.remaining === 0) return `${count("features.allAnalysed", total)}${failed}`;
    const done = t(f.loudnessAnalysis ? "features.analysed" : "features.analysedPaused", {
      done: progress.done,
      tracks: count("count.tracks", total),
    });
    return `${done}${failed}`;
  });
</script>

{#snippet switches(list: Switch[])}
  {#each list as item (item.key)}
    <label class="switch">
      <input
        type="checkbox"
        checked={Boolean(f[item.key])}
        disabled={appSettings.saving}
        onchange={(event) => toggle(item.key, event.currentTarget.checked)}
      />
      <span>
        <span class="title">{item.title} <span class="badge">{item.id}</span></span>
        <span class="hint">{item.about}</span>
      </span>
    </label>
    {#if item.key === "loudnessAnalysis"}
      <p class="sub hint" role="status">{analysisText}</p>
    {:else if item.key === "segueShuffle"}
      <label class="switch sub">
        <input
          type="checkbox"
          checked={f.skipSilence}
          disabled={appSettings.saving}
          onchange={(event) => toggle("skipSilence", event.currentTarget.checked)}
        />
        <span>
          <span class="title">{t("features.skipSilence")} <span class="badge">O3</span></span>
          <span class="hint">{t("features.skipSilenceHint")}</span>
        </span>
      </label>
      <label class="field sub">
        <span class="label">{t("features.skipAfter")}</span>
        <span class="control">
          <input
            type="range"
            min="1"
            max="60"
            step="1"
            value={f.skipSilenceAfter}
            disabled={!f.skipSilence || appSettings.saving}
            onchange={(event) => {
              const value = Number(event.currentTarget.value);
              save((next) => (next.skipSilenceAfter = value));
            }}
          />
          <span class="value">{t("features.seconds", { seconds: f.skipSilenceAfter })}</span>
        </span>
      </label>
    {:else if item.key === "outsideRecommendations" && f.outsideRecommendations && outsideStatus}
      <div class="sub">
        <p class="hint">
          {outsideStatus.sent.length > 0
            ? t("features.outsideSent", {
                artists: new Intl.ListFormat(undefined, { type: "conjunction" }).format(
                  outsideStatus.sent.map((artist) => artist.name),
                ),
              })
            : t("features.outsideNothingSent")}
        </p>
        {#if outsideStatus.dismissed > 0}
          <p class="row">
            <button
              onclick={async () => {
                await attempt(outside.forgetDismissed);
                await refresh();
              }}>{count("features.outsideForget", outsideStatus.dismissed)}</button
            >
          </p>
        {/if}
      </div>
    {:else if item.key === "healthReport" && f.healthReport}
      <p class="sub"><button class="link" onclick={() => ui.showView("health")}>{t("features.openHealth")}</button></p>
    {:else if item.key === "listeningHistory"}
      <div class="sub">
        <label class="switch">
          <input
            type="checkbox"
            checked={f.listenbrainz}
            disabled={!f.listeningHistory || appSettings.saving}
            onchange={(event) => toggle("listenbrainz", event.currentTarget.checked)}
          />
          <span>
            <span class="title">{t("features.listenBrainz")} <span class="badge">O8</span></span>
            <span class="hint">{t("features.listenBrainzHint")}</span>
          </span>
        </label>
        {#if f.listenbrainz}
          <div class="row">
            <input
              type="text"
              placeholder={t(listenBrainz?.hasToken ? "features.tokenSaved" : "features.tokenPlaceholder")}
              bind:value={token}
              autocomplete="off"
              spellcheck="false"
            />
            <button disabled={token.trim() === ""} onclick={() => saveToken(token)}>{t("features.saveToken")}</button>
            {#if listenBrainz?.hasToken}<button onclick={() => saveToken(null)}>{t("services.remove")}</button>{/if}
          </div>
          {#if listenBrainz}
            <p class="hint">
              {t(listenBrainz.hasToken ? "features.hasToken" : "features.noToken")}
              {listenBrainz.pending > 0 ? count("features.pending", listenBrainz.pending) : ""}
            </p>
          {/if}
        {/if}
        <p class="row">
          <button onclick={() => ui.showView("history")} disabled={!f.listeningHistory}
            >{t("features.openHistory")}</button
          >
          <button
            onclick={async () => {
              if (!confirm(t("features.clearHistoryConfirm"))) return;
              await attempt(api.clearHistory);
              features.historyVersion++;
            }}>{t("features.clearHistory")}</button
          >
        </p>
      </div>
    {/if}
  {/each}
{/snippet}

<p class="intro hint">
  {t("features.intro")}
</p>

<h3>{t("features.sound")}</h3>
{@render switches(SOUND)}

<div class="crossfeed">
  <label class="field">
    <span class="label">{t("features.crossfeed")} <span class="badge">O11</span></span>
    <span class="control">
      <select
        value={f.crossfeed}
        disabled={appSettings.saving}
        onchange={(event) => {
          const level = event.currentTarget.value as CrossfeedLevel;
          save((next) => (next.crossfeed = level));
        }}
      >
        {#each CROSSFEED as level (level.id)}<option value={level.id}>{level.name}</option>{/each}
      </select>
    </span>
    <span class="hint">{t("features.crossfeedHint")}</span>
  </label>
  <label class="switch sub">
    <input
      type="checkbox"
      checked={f.crossfeedHeadphonesOnly}
      disabled={f.crossfeed === "off" || appSettings.saving}
      onchange={(event) => toggle("crossfeedHeadphonesOnly", event.currentTarget.checked)}
    />
    <span>
      <span class="title">{t("features.headphonesOnly")}</span>
      <span class="hint">{t("features.headphonesOnlyHint")}</span>
    </span>
  </label>
</div>

<h3>{t("features.library")}</h3>
{@render switches(LIBRARY)}

<h3>{t("features.listening")}</h3>
{@render switches(LISTENING)}

<h3>{t("features.look")}</h3>
{@render switches(LOOK)}

<h3>{t("features.remote")} <span class="badge">O14</span></h3>
<label class="switch">
  <input
    type="checkbox"
    checked={f.remoteControl}
    disabled={appSettings.saving}
    onchange={(event) => toggle("remoteControl", event.currentTarget.checked)}
  />
  <span>
    <span class="title">{t("features.remoteSwitch")}</span>
    <span class="hint">{t("features.remoteHint")}</span>
  </span>
</label>
{#if f.remoteControl}
  <div class="sub">
    <label class="field">
      <span class="label">{t("features.port")}</span>
      <span class="control">
        <input type="number" min="1024" max="65535" bind:value={port} onchange={savePort} style="width: 7rem" />
      </span>
    </label>
    {#if remote}
      {#if remote.error}
        <p class="warning">{remote.error}</p>
      {:else if remote.running}
        <p>
          {t("features.openOnPhone")}
          <strong>{remote.url ?? t("features.thisMac", { port: String(f.remotePort) })}</strong>
        </p>
        <p class="row">
          <button onclick={async () => (remote = (await attempt(api.remoteNewCode)) ?? remote)}
            >{t("features.pair")}</button
          >
          {#if remote.code}
            <span
              >{t("features.code")} <strong class="code">{remote.code}</strong>
              <span class="hint">{t("features.codeHint")}</span></span
            >
          {/if}
        </p>
      {/if}
      {#if remote.devices.length > 0}
        <ul class="devices">
          {#each remote.devices as device (device.id)}
            <li>
              <span>
                {device.name}
                <span class="hint">
                  {device.lastSeen
                    ? t("features.pairedUsed", { paired: formatDay(device.pairedAt), used: formatDay(device.lastSeen) })
                    : t("features.paired", { paired: formatDay(device.pairedAt) })}
                </span>
              </span>
              <button onclick={async () => (remote = (await attempt(() => api.remoteForget(device.id))) ?? remote)}>
                {t("features.forget")}
              </button>
            </li>
          {/each}
        </ul>
      {/if}
    {/if}
  </div>
{/if}

<div class="actions">
  <button onclick={() => appSettings.reset("features")} disabled={appSettings.saving}>{t("features.reset")}</button>
</div>

<style>
  .intro {
    margin-bottom: 0.5rem;
  }

  .badge {
    font-size: 0.7rem;
    font-weight: 600;
    color: var(--text-muted);
    border: 1px solid var(--border);
    border-radius: var(--radius-sm);
    padding: 0 0.3rem;
    margin-left: 0.25rem;
    vertical-align: 1px;
  }

  .sub {
    margin-left: 1.6rem;
  }

  .row {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 0.5rem;
    margin: 0.35rem 0;
  }

  .row input[type="text"] {
    flex: 1 1 14rem;
  }

  .crossfeed {
    margin-top: 0.25rem;
  }

  .code {
    font-size: 1.2rem;
    letter-spacing: 0.15em;
    font-variant-numeric: tabular-nums;
  }

  .devices {
    list-style: none;
    padding: 0;
    margin: 0.5rem 0 0;
  }

  .devices li {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 1rem;
    padding: 0.35rem 0;
    border-top: 1px solid var(--border);
  }

  .warning {
    color: var(--danger);
  }
</style>
