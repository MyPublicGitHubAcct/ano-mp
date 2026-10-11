<script lang="ts">
  // Music servers (PLAN.md Phases 7e and 7f): each server signed in to (a
  // Jellyfin server, or a Plex server's music library), with its state, sync
  // and sign out; adding a Jellyfin server (an address typed or found on the
  // network, then a password or Quick Connect) or signing in to Plex
  // (`PlexSignIn`); what is told to servers and fetched from them; and the
  // downloaded music. Shown while the `jellyfin` or `plex` feature is on.
  import { ask } from "@tauri-apps/plugin-dialog";
  import { onDestroy } from "svelte";
  import {
    servers as api,
    type DiscoveredServer,
    type FolderState,
    type ServerCheck,
    type ServerSettings,
    type ServerSummary,
  } from "$lib/api";
  import { formatDay, formatSize } from "$lib/format";
  import { count, errorText, t, type MessageKey } from "$lib/i18n";
  import { library } from "$lib/state/library.svelte";
  import { servers, sourceName } from "$lib/state/servers.svelte";
  import { appSettings } from "$lib/state/settings.svelte";
  import Icon from "../Icon.svelte";
  import PlexSignIn from "./PlexSignIn.svelte";

  const AWAY = [0, 128, 192, 256, 320];
  const CACHE_MB = [512, 1024, 2048, 5120, 10240, 20480, 51200];

  /** Why a server's tracks can't all play now. */
  const STATE: Partial<Record<FolderState, MessageKey>> = {
    unreachable: "folderState.unreachable",
    signedOut: "folderState.signedOut",
    off: "folderState.off",
    empty: "folderState.mostlyGone",
    mostlyGone: "folderState.mostlyGone",
  };

  const settings = $derived(appSettings.current.servers);
  const features = $derived(appSettings.current.features);

  // Signing in to Plex, which `PlexSignIn` carries on.
  let plexSignIn = $state<PlexSignIn | undefined>();
  let plexActive = $state(false);
  const save = (edit: (next: ServerSettings) => void) => appSettings.save((next) => edit(next.servers));

  // Adding a server: the address, then the server found there, then signing in.
  let adding = $state(false);
  let address = $state("");
  let found = $state<DiscoveredServer[] | null>(null);
  let finding = $state(false);
  let checked = $state<ServerCheck | null>(null);
  let user = $state("");
  let password = $state("");
  let busy = $state(false);
  let error = $state<string | null>(null);
  let code = $state<string | null>(null);
  let polling: ReturnType<typeof setTimeout> | undefined;

  onDestroy(() => clearTimeout(polling));

  function reset() {
    clearTimeout(polling);
    adding = false;
    address = "";
    found = null;
    checked = null;
    user = "";
    password = "";
    error = null;
    code = null;
    busy = false;
  }

  async function run(action: () => Promise<void>) {
    busy = true;
    error = null;
    try {
      await action();
    } catch (failure) {
      error = errorText(failure);
    } finally {
      busy = false;
    }
  }

  async function find() {
    finding = true;
    error = null;
    try {
      found = await api.discover();
    } catch (failure) {
      error = errorText(failure);
    } finally {
      finding = false;
    }
  }

  const check = (at = address) =>
    run(async () => {
      checked = await api.check(at);
      address = checked.address;
    });

  const signIn = () =>
    run(async () => {
      if (!checked) return;
      await api.signIn(checked.address, user, password);
      password = "";
      reset();
      await servers.refresh();
      await library.refresh();
    });

  const quickConnect = () =>
    run(async () => {
      if (!checked) return;
      const started = await api.quickConnectStart(checked.address);
      code = started.code;
      const at = checked.address;
      const poll = async () => {
        try {
          const source = await api.quickConnectFinish(at, started.secret);
          if (source) {
            reset();
            await servers.refresh();
            await library.refresh();
            return;
          }
          polling = setTimeout(poll, 3000);
        } catch (failure) {
          code = null;
          error = errorText(failure);
        }
      };
      polling = setTimeout(poll, 3000);
    });

  async function signOut(server: ServerSummary) {
    const confirmed = await ask(t("servers.signOutConfirm", { name: sourceName(server) }), {
      title: t("servers.signOutTitle"),
      kind: "warning",
      okLabel: t("servers.signOut"),
    });
    if (confirmed) {
      await servers.signOut(server);
      await library.refresh();
    }
  }

  function line(server: ServerSummary) {
    if (server.syncing) return t("servers.syncing");
    if (server.syncedAt === null) return t("servers.notSynced");
    return t("servers.synced", { tracks: count("count.tracks", server.trackCount), day: formatDay(server.syncedAt) });
  }

  /** A server whose sync kept tracks it didn't find: the user can say they're gone (H22). */
  function removeMissing(server: ServerSummary) {
    const folder = library.folders.find((candidate) => candidate.id === server.folderId);
    if (folder) void library.removeMissing({ ...folder, path: sourceName(server) });
  }

  const held = (server: ServerSummary) => server.status.state === "empty" || server.status.state === "mostlyGone";
</script>

{#if servers.list.length === 0 && !adding}
  <p class="muted">{t("servers.none")}</p>
{:else if servers.list.length > 0}
  <ul class="servers card">
    {#each servers.list as server (server.folderId)}
      {@const state = STATE[server.status.state]}
      <li class:missing={state !== undefined}>
        <span class="glyph"><Icon name={state ? "warning" : "folder"} /></span>
        <span class="text">
          <span class="name">{sourceName(server)}</span>
          <span class="muted small">
            {t("servers.signedInAs", { user: server.userName })} ·
            {t(server.kind === "plex" ? "servers.plexVersion" : "servers.jellyfinVersion", {
              version: server.version ?? t("servers.unknownVersion"),
            })}
          </span>
          <span class="muted small">{state ? t(state) : line(server)}</span>
          {#if server.viaRelay && !state}<span class="muted small">{t("servers.viaRelay")}</span>{/if}
        </span>
        {#if held(server)}
          <button disabled={server.syncing} onclick={() => removeMissing(server)}>{t("servers.removeMissing")}</button>
        {/if}
        <button disabled={server.syncing} onclick={() => servers.sync(server)}>
          <Icon name="refresh" size="1rem" />
          {t("servers.sync")}
        </button>
        <button disabled={server.syncing} title={t("servers.syncAllHint")} onclick={() => servers.sync(server, true)}>
          {t("servers.syncAll")}
        </button>
        <button disabled={server.syncing} onclick={() => signOut(server)}>{t("servers.signOut")}</button>
      </li>
    {/each}
  </ul>
  <p class="hint">{t("servers.syncAllHint")}</p>
{/if}

{#if !adding}
  {#if features.jellyfin}
    <div class="actions">
      <button class="primary" id="setting-addServer" onclick={() => (adding = true)}>
        <Icon name="plus" />
        {t("servers.add")}
      </button>
    </div>
  {/if}
  {#if features.plex && !plexActive}
    <div class="actions">
      <button id="setting-plexSignIn" onclick={() => void plexSignIn?.start()}>
        <Icon name="plus" />
        {t("servers.plexSignIn")}
      </button>
    </div>
  {/if}
{:else}
  <div class="add card">
    <h3>{t("servers.add")}</h3>
    {#if checked === null}
      <label class="field">
        <span class="label">{t("servers.address")}</span>
        <span class="control">
          <input
            type="url"
            bind:value={address}
            placeholder="http://192.168.1.10:8096"
            autocomplete="url"
            onkeydown={(event) => {
              if (event.key === "Enter" && address.trim()) void check();
            }}
          />
        </span>
        <span class="hint">{t("servers.addressHint")}</span>
      </label>
      <div class="actions">
        <button class="primary" disabled={busy || !address.trim()} onclick={() => check()}
          >{t("servers.continue")}</button
        >
        <button disabled={finding} onclick={find}>{finding ? t("servers.finding") : t("servers.find")}</button>
        <button onclick={reset}>{t("servers.cancel")}</button>
      </div>
      {#if found !== null}
        {#if found.length === 0}
          <p class="hint">{t("servers.foundNone")}</p>
        {:else}
          <ul class="found">
            {#each found as server (server.id)}
              <li>
                <button onclick={() => check(server.address)}>
                  {server.name} <span class="muted">{server.address}</span>
                </button>
              </li>
            {/each}
          </ul>
        {/if}
      {/if}
    {:else}
      <p>
        {t("servers.about", { name: checked.name, version: checked.version ?? t("servers.unknownVersion") })}
        <span class="muted">{checked.address}</span>
      </p>
      {#if code !== null}
        <p>{t("servers.quickConnectCode")}</p>
        <p class="code">{code}</p>
        <p class="hint" role="status">{t("servers.quickConnectWaiting")}</p>
        <div class="actions"><button onclick={reset}>{t("servers.cancel")}</button></div>
      {:else}
        <label class="field">
          <span class="label">{t("servers.user")}</span>
          <span class="control"><input bind:value={user} autocomplete="username" /></span>
        </label>
        <label class="field">
          <span class="label">{t("servers.password")}</span>
          <span class="control">
            <input
              type="password"
              bind:value={password}
              autocomplete="current-password"
              onkeydown={(event) => {
                if (event.key === "Enter" && user.trim()) void signIn();
              }}
            />
          </span>
          <span class="hint">{t("servers.passwordHint")}</span>
        </label>
        <div class="actions">
          <button class="primary" disabled={busy || !user.trim()} onclick={signIn}>
            {busy ? t("servers.signingIn") : t("servers.signIn")}
          </button>
          {#if checked.quickConnect}
            <button disabled={busy} onclick={quickConnect}>{t("servers.quickConnect")}</button>
          {/if}
          <button onclick={reset}>{t("servers.cancel")}</button>
        </div>
      {/if}
    {/if}
    {#if error}
      <p class="error" role="alert">{error}</p>
    {/if}
  </div>
{/if}

{#if features.plex}
  <PlexSignIn bind:this={plexSignIn} onactive={(active) => (plexActive = active)} />
{/if}

<h3>{t("servers.playing")}</h3>
<label class="switch">
  <input
    id="setting-reportPlays"
    type="checkbox"
    checked={settings.reportPlays}
    onchange={(event) => {
      const on = event.currentTarget.checked;
      void save((next) => (next.reportPlays = on));
    }}
  />
  <span>
    <span class="title">{t("servers.reportPlays")}</span>
    <span class="hint">{t("servers.reportPlaysHint")}</span>
  </span>
</label>
{#if features.plex}
  <label class="switch">
    <input
      id="setting-plexRelay"
      type="checkbox"
      checked={settings.plexRelay}
      onchange={(event) => {
        const on = event.currentTarget.checked;
        void save((next) => (next.plexRelay = on));
      }}
    />
    <span>
      <span class="title">{t("servers.plexRelay")}</span>
      <span class="hint">{t("servers.plexRelayHint")}</span>
    </span>
  </label>
{/if}
<label class="field">
  <span class="label">{t("servers.awayKbps")}</span>
  <span class="control">
    <select
      id="setting-awayKbps"
      value={settings.awayKbps}
      disabled={appSettings.saving}
      onchange={(event) => {
        const kbps = Number(event.currentTarget.value);
        void save((next) => (next.awayKbps = kbps));
      }}
    >
      {#each AWAY as kbps (kbps)}
        <option value={kbps}>{kbps === 0 ? t("servers.awayOriginal") : t("servers.kbps", { kbps })}</option>
      {/each}
    </select>
  </span>
  <span class="hint">{t("servers.awayKbpsHint")}</span>
</label>

<h3>{t("servers.cache")}</h3>
<label class="field">
  <span class="label">{t("servers.cacheSize")}</span>
  <span class="control">
    <select
      id="setting-cacheMb"
      value={settings.cacheMb}
      disabled={appSettings.saving}
      onchange={(event) => {
        const mb = Number(event.currentTarget.value);
        void save((next) => (next.cacheMb = mb));
      }}
    >
      {#each CACHE_MB.includes(settings.cacheMb) ? CACHE_MB : [...CACHE_MB, settings.cacheMb].sort((a, b) => a - b) as mb (mb)}
        <option value={mb}>{formatSize(mb * 2 ** 20)}</option>
      {/each}
    </select>
  </span>
  <span class="hint">{t("servers.cacheHint")}</span>
</label>
{#if servers.offline}
  <p class="muted">
    {t("servers.cacheUsed", {
      used: formatSize(servers.offline.cacheBytes),
      budget: formatSize(servers.offline.cacheBudget),
    })}
    {#if servers.offline.tracks > 0}
      · {t("servers.offlineCount", { downloaded: servers.offline.downloaded, tracks: servers.offline.tracks })}
    {/if}
  </p>
{/if}
<div class="actions">
  <button onclick={() => servers.clearCache()}>{t("servers.clearCache")}</button>
</div>

<style>
  .servers,
  .found {
    list-style: none;
    margin: 0;
    padding: 0;
  }

  .servers li {
    display: flex;
    align-items: center;
    gap: 0.75rem;
    padding: 0.6rem 0.5rem 0.6rem 0.8rem;
  }

  .servers li + li {
    border-top: 1px solid var(--border);
  }

  .glyph {
    color: var(--text-faint);
    display: flex;
  }

  .missing .glyph {
    color: var(--danger);
  }

  .text {
    display: flex;
    flex-direction: column;
    flex: 1;
    min-width: 0;
  }

  .name {
    font-weight: 500;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .small {
    font-size: 0.8rem;
  }

  .add {
    margin-top: 0.75rem;
    padding: 0.75rem 1rem;
  }

  .code {
    font-size: 1.6rem;
    font-weight: 600;
    letter-spacing: 0.2em;
  }

  .error {
    color: var(--danger);
  }

  .hint {
    margin-top: 0.75rem;
  }

  @media (max-width: 480px) {
    .servers li {
      flex-wrap: wrap;
    }

    .text {
      flex-basis: calc(100% - 2.5rem);
    }
  }
</style>
