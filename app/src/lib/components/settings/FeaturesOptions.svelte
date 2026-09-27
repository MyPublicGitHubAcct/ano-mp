<script lang="ts">
  // Every optional feature (PLAN.md §4.6, O1–O19) with a switch, and each
  // one's options under it: the loudness analysis's progress, crossfeed's
  // strength, the ListenBrainz token, the remote's pairing code and paired
  // phones. Saved as they change, like the other sections.
  import { onMount } from "svelte";
  import {
    features as api,
    on,
    type CrossfeedLevel,
    type FeatureSettings,
    type ListenBrainzStatus,
    type RemoteStatus,
  } from "$lib/api";
  import { formatDay, plural } from "$lib/format";
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
      title: "Loudness analysis",
      about:
        "Measures every track’s loudness in the background (EBU R128), so ReplayGain evens out files without ReplayGain tags too. Your files aren’t changed. A large library takes hours of processor time, once.",
    },
    {
      key: "waveformSeekBar",
      id: "O2",
      title: "Waveform seek bar",
      about: "Draws the track’s waveform in the seek bar, showing quiet intros, drops and hidden tracks. The playing track is measured for it even with the analysis off.",
    },
    {
      key: "segueShuffle",
      id: "O3",
      title: "Keep segues together in shuffle",
      about: "Tracks that run into each other (live albums, DJ mixes, concept albums) shuffle as one, in order. Needs the analysis.",
    },
    {
      key: "signalPath",
      id: "O10",
      title: "Signal path",
      about: "Clicking the format in the playing bar shows every step from the file to the speakers.",
    },
    {
      key: "matchSampleRate",
      id: "O10",
      title: "Match the device’s sample rate",
      about: "Switches the output device to each track’s sample rate when it offers it, so nothing is resampled. Output pauses briefly at such a switch, and other apps using the device change rate too.",
    },
    {
      key: "practiceMode",
      id: "O12",
      title: "Practice mode",
      about: "A–B loops, and playing slower or faster without changing the pitch (or changing the pitch alone), from a button in the playing bar.",
    },
  ];

  const LIBRARY: Switch[] = [
    {
      key: "healthReport",
      id: "O4",
      title: "Library health report",
      about: "Finds files that don’t decode or are cut short, suspected lossy transcodes, albums whose tags disagree, and likely duplicates.",
    },
    {
      key: "cueSheets",
      id: "O5",
      title: "Cue sheets and chapters as tracks",
      about: "A single-file album with a cue sheet, or a file with chapters, becomes a track per cue or chapter. Changing this reads every file again.",
    },
    {
      key: "classical",
      id: "O6",
      title: "Classical works",
      about: "Groups movements under their work on album pages, adds Composer and Work to browsing, and shuffles a work as one.",
    },
    {
      key: "playbackPreferences",
      id: "O7",
      title: "Playback preferences",
      about: "Your own rules per track or album, from its menu: skip it in album and shuffle play, never shuffle an album, a gain offset, trims.",
    },
    {
      key: "lyrics",
      id: "O13",
      title: "Lyrics",
      about: "Shows lyrics from the tags or a .lrc file next to the track in the Now Playing view; synced lines follow the music and seek when clicked.",
    },
  ];

  const LISTENING: Switch[] = [
    {
      key: "listeningHistory",
      id: "O8",
      title: "Listening history",
      about: "Remembers what you play (once half a track, or four minutes, has played), on this computer only.",
    },
    {
      key: "recentlyPlayed",
      id: "O16",
      title: "Recently played",
      about: "What you played last, with plays from one album together. Needs the history.",
    },
    {
      key: "topPlayed",
      id: "O19",
      title: "Top 20 of a year or month",
      about: "Your most played tracks, albums and artists, on the History page. Needs the history.",
    },
    {
      key: "libraryRadio",
      id: "O9",
      title: "Library radio",
      about: "“Start radio” in a track’s menu plays tracks like it from your library (genre, era, label, related artists), each saying why it was picked.",
    },
    {
      key: "radioAfterQueue",
      id: "O9",
      title: "Keep playing when the queue ends",
      about: "Any queue carries on as radio from its last track.",
    },
    { key: "recentlyAdded", id: "O15", title: "Recently added", about: "The albums that came into the library last, on the Home page." },
    {
      key: "onThisDay",
      id: "O17",
      title: "Released on this day",
      about: "Albums first released on today’s date in earlier years, on the Home page.",
    },
    {
      key: "moreInGenre",
      id: "O18",
      title: "More in this genre",
      about: "Five random albums sharing a genre with the album you’re looking at.",
    },
  ];

  const CROSSFEED: { id: CrossfeedLevel; name: string }[] = [
    { id: "off", name: "Off" },
    { id: "light", name: "Light" },
    { id: "medium", name: "Medium" },
    { id: "strong", name: "Strong" },
  ];

  let listenBrainz = $state.raw<ListenBrainzStatus | null>(null);
  let token = $state("");
  let remote = $state.raw<RemoteStatus | null>(null);
  let port = $state(String(appSettings.current.features.remotePort));

  const refresh = async () => {
    listenBrainz = await api.listenBrainzStatus().catch(() => null);
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
      if (user) toasts.show(`Connected to ListenBrainz as ${user}.`, "info");
      await refresh();
    }
  }

  async function savePort() {
    const value = Number(port);
    if (!Number.isInteger(value) || value < 1024 || value > 65535) {
      toasts.show("The port must be a number from 1024 to 65535.");
      port = String(f.remotePort);
      return;
    }
    await save((next) => (next.remotePort = value));
  }

  const analysisText = $derived.by(() => {
    const progress = features.analysis;
    const total = progress.done + progress.remaining;
    if (total === 0) return "Nothing to analyse yet.";
    const failed = progress.failed > 0 ? ` ${plural(progress.failed, "file")} couldn’t be decoded.` : "";
    if (progress.remaining === 0) return `All ${plural(total, "track")} analysed.${failed}`;
    return `${progress.done.toLocaleString()} of ${plural(total, "track")} analysed${f.loudnessAnalysis ? "" : " (paused)"}.${failed}`;
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
          <span class="title">Skip long silences <span class="badge">O3</span></span>
          <span class="hint">
            Jumps over a long silence inside a track (the gap before a hidden track), and ends a track in silence,
            after a few seconds of it. Needs the analysis.
          </span>
        </span>
      </label>
      <label class="field sub">
        <span class="label">Skip after</span>
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
          <span class="value">{f.skipSilenceAfter} s</span>
        </span>
      </label>
    {:else if item.key === "healthReport" && f.healthReport}
      <p class="sub"><button class="link" onclick={() => ui.showView("health")}>Open the health report</button></p>
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
            <span class="title">Send listens to ListenBrainz <span class="badge">O8</span></span>
            <span class="hint">
              Each play that counts is sent to your ListenBrainz account (a MetaBrainz service), with its MusicBrainz
              ids. Listens wait while you’re offline. Get your user token at listenbrainz.org › Settings.
            </span>
          </span>
        </label>
        {#if f.listenbrainz}
          <div class="row">
            <input
              type="text"
              placeholder={listenBrainz?.hasToken ? "Token saved in the keychain" : "ListenBrainz user token"}
              bind:value={token}
              autocomplete="off"
              spellcheck="false"
            />
            <button disabled={token.trim() === ""} onclick={() => saveToken(token)}>Save token</button>
            {#if listenBrainz?.hasToken}<button onclick={() => saveToken(null)}>Remove</button>{/if}
          </div>
          {#if listenBrainz}
            <p class="hint">
              {listenBrainz.hasToken ? "A token is saved." : "No token yet."}
              {listenBrainz.pending > 0 ? `${plural(listenBrainz.pending, "listen")} waiting to be sent.` : ""}
            </p>
          {/if}
        {/if}
        <p class="row">
          <button onclick={() => ui.showView("history")} disabled={!f.listeningHistory}>Open history</button>
          <button
            onclick={async () => {
              if (!confirm("Forget every play in the listening history? This can’t be undone.")) return;
              await attempt(api.clearHistory);
              features.historyVersion++;
            }}>Clear history…</button
          >
        </p>
      </div>
    {/if}
  {/each}
{/snippet}

<p class="intro hint">
  Features beyond the usual. Each can be turned off; online ones and ones that listen on the network are off until you
  turn them on.
</p>

<h3>Sound and playback</h3>
{@render switches(SOUND)}

<div class="crossfeed">
  <label class="field">
    <span class="label">Headphone crossfeed <span class="badge">O11</span></span>
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
    <span class="hint">
      Blends a little of each channel into the other, as speakers do, so hard-panned stereo is easier on headphones.
    </span>
  </label>
  <label class="switch sub">
    <input
      type="checkbox"
      checked={f.crossfeedHeadphonesOnly}
      disabled={f.crossfeed === "off" || appSettings.saving}
      onchange={(event) => toggle("crossfeedHeadphonesOnly", event.currentTarget.checked)}
    />
    <span>
      <span class="title">Only with headphones</span>
      <span class="hint">While macOS says headphones are plugged into this Mac. Bluetooth headphones can’t be told apart from speakers, so they need this off.</span>
    </span>
  </label>
</div>

<h3>Library</h3>
{@render switches(LIBRARY)}

<h3>Listening and discovery</h3>
{@render switches(LISTENING)}

<h3>Remote control <span class="badge">O14</span></h3>
<label class="switch">
  <input
    type="checkbox"
    checked={f.remoteControl}
    disabled={appSettings.saving}
    onchange={(event) => toggle("remoteControl", event.currentTarget.checked)}
  />
  <span>
    <span class="title">Control playback from a phone</span>
    <span class="hint">
      Serves a remote page to phones on this network (only local addresses are answered). A phone pairs once with a
      code shown here; forget it below at any time. macOS may ask to let the app use the local network.
    </span>
  </span>
</label>
{#if f.remoteControl}
  <div class="sub">
    <label class="field">
      <span class="label">Port</span>
      <span class="control">
        <input type="number" min="1024" max="65535" bind:value={port} onchange={savePort} style="width: 7rem" />
      </span>
    </label>
    {#if remote}
      {#if remote.error}
        <p class="warning">{remote.error}</p>
      {:else if remote.running}
        <p>
          On your phone, open <strong>{remote.url ?? `this Mac’s address, port ${f.remotePort}`}</strong>
        </p>
        <p class="row">
          <button onclick={async () => (remote = (await attempt(api.remoteNewCode)) ?? remote)}>Pair a phone</button>
          {#if remote.code}
            <span>Code: <strong class="code">{remote.code}</strong> <span class="hint">(valid for five minutes, once)</span></span>
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
                  paired {formatDay(device.pairedAt)}{device.lastSeen ? `, last used ${formatDay(device.lastSeen)}` : ""}
                </span>
              </span>
              <button onclick={async () => (remote = (await attempt(() => api.remoteForget(device.id))) ?? remote)}>
                Forget
              </button>
            </li>
          {/each}
        </ul>
      {/if}
    {/if}
  </div>
{/if}

<div class="actions">
  <button onclick={() => appSettings.reset("features")} disabled={appSettings.saving}>Reset features</button>
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
    border-radius: 4px;
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
