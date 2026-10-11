<script lang="ts">
  // Signing in to Plex (PLAN.md Phase 7f): a pin the user approves on plex.tv in their browser (no
  // password is typed here), then the servers the account reaches with their music libraries, of
  // which those picked are added, each a source of its own. Shown while the `plex` feature is on;
  // the Servers panel's "Sign In with Plex" calls `start`, and hides while `onactive` says so.
  import { onDestroy } from "svelte";
  import { servers as api, type PlexAccount, type PlexPin, type PlexServer } from "$lib/api";
  import { errorText, t } from "$lib/i18n";
  import { openWebLink } from "$lib/openLink";
  import { library } from "$lib/state/library.svelte";
  import { servers } from "$lib/state/servers.svelte";

  /** How often to ask whether the pin is approved. */
  const POLL_MS = 2000;

  /** Told whether a sign-in is under way, so the panel hides its button meanwhile. */
  let { onactive }: { onactive: (active: boolean) => void } = $props();

  let step = $state<"idle" | "starting" | "waiting" | "loading" | "choosing">("idle");

  $effect(() => onactive(step !== "idle"));
  let pin = $state<PlexPin | null>(null);
  let account = $state<PlexAccount | null>(null);
  let found = $state<PlexServer[]>([]);
  /** The libraries picked, by server id. */
  let picked = $state<Record<string, string[]>>({});
  let adding = $state<string | null>(null);
  let error = $state<string | null>(null);
  let polling: ReturnType<typeof setTimeout> | undefined;

  onDestroy(() => {
    clearTimeout(polling);
    forget();
  });

  /** Forgets the account's token unless one of its libraries was added. */
  function forget() {
    if (account) void api.plexSignInCancel(account.id).catch(() => undefined);
  }

  function reset() {
    clearTimeout(polling);
    forget();
    step = "idle";
    pin = null;
    account = null;
    found = [];
    picked = {};
    adding = null;
    error = null;
  }

  /** Starts signing in: makes a pin and opens plex.tv's page for it. */
  export async function start() {
    step = "starting";
    error = null;
    try {
      pin = await api.plexSignInStart();
      step = "waiting";
      openWebLink(pin.url);
      polling = setTimeout(poll, POLL_MS);
    } catch (failure) {
      step = "idle";
      error = errorText(failure);
    }
  }

  async function poll() {
    if (!pin) return;
    try {
      const signedIn = await api.plexSignInFinish(pin.id);
      if (!signedIn) {
        polling = setTimeout(poll, POLL_MS);
        return;
      }
      account = signedIn;
      await load();
    } catch (failure) {
      step = "idle";
      pin = null;
      error = errorText(failure);
    }
  }

  async function load() {
    if (!account) return;
    step = "loading";
    error = null;
    try {
      found = await api.plexServers(account.id);
      picked = Object.fromEntries(found.map((server) => [server.id, []]));
      step = "choosing";
    } catch (failure) {
      step = "idle";
      error = errorText(failure);
    }
  }

  function toggle(server: PlexServer, key: string, on: boolean) {
    const keys = picked[server.id] ?? [];
    picked[server.id] = on ? [...keys, key] : keys.filter((candidate) => candidate !== key);
  }

  async function add(server: PlexServer) {
    if (!account) return;
    adding = server.id;
    error = null;
    try {
      await api.plexAdd(account.id, server.id, picked[server.id] ?? []);
      await servers.refresh();
      await library.refresh();
      await load();
    } catch (failure) {
      error = errorText(failure);
    } finally {
      adding = null;
    }
  }
</script>

{#if step !== "idle"}
  <div class="add card">
    <h3>{t("servers.plexSignIn")}</h3>
    {#if step === "starting"}
      <p class="hint" role="status">{t("servers.plexStarting")}</p>
    {:else if step === "waiting" && pin}
      <p>{t("servers.plexApprove")}</p>
      <p class="code">{pin.code}</p>
      <p class="hint" role="status">{t("servers.plexWaiting")}</p>
      <div class="actions">
        <button onclick={() => pin && openWebLink(pin.url)}>{t("servers.plexOpenAgain")}</button>
        <button onclick={reset}>{t("servers.cancel")}</button>
      </div>
    {:else if step === "loading"}
      <p class="hint" role="status">{t("servers.plexLoading")}</p>
    {:else if account}
      {#if found.length === 0}
        <p>{t("servers.plexNoServers")}</p>
      {:else}
        <p>{t("servers.plexChoose", { account: account.name })}</p>
        <ul class="plex-servers">
          {#each found as server (server.id)}
            <li>
              <span class="name">
                {server.name}
                {#if !server.owned}<span class="muted small">· {t("servers.plexShared")}</span>{/if}
              </span>
              {#if server.error}
                <span class="error small">{server.error}</span>
              {:else if server.libraries.length === 0}
                <span class="muted small">{t("servers.plexNoLibraries")}</span>
              {:else}
                {#each server.libraries as plexLibrary (plexLibrary.key)}
                  <label class="check">
                    <input
                      type="checkbox"
                      checked={plexLibrary.added || (picked[server.id] ?? []).includes(plexLibrary.key)}
                      disabled={plexLibrary.added || adding !== null}
                      onchange={(event) => toggle(server, plexLibrary.key, event.currentTarget.checked)}
                    />
                    <span>
                      {plexLibrary.name}
                      {#if plexLibrary.added}<span class="muted small">· {t("servers.plexAdded")}</span>{/if}
                    </span>
                  </label>
                {/each}
                <div class="actions">
                  <button
                    class="primary"
                    disabled={adding !== null || (picked[server.id] ?? []).length === 0}
                    onclick={() => add(server)}>{t("servers.plexAdd")}</button
                  >
                </div>
              {/if}
            </li>
          {/each}
        </ul>
      {/if}
      <div class="actions"><button onclick={reset}>{t("servers.plexDone")}</button></div>
    {/if}
  </div>
{/if}
{#if error}
  <p class="error" role="alert">{error}</p>
{/if}

<style>
  .add {
    margin-top: 0.75rem;
    padding: 0.75rem 1rem;
  }

  .code {
    font-size: 1.2rem;
    font-weight: 600;
    letter-spacing: 0.1em;
    overflow-wrap: anywhere;
  }

  .plex-servers {
    list-style: none;
    margin: 0;
    padding: 0;
  }

  .plex-servers li {
    display: flex;
    flex-direction: column;
    gap: 0.35rem;
    padding: 0.6rem 0;
  }

  .plex-servers li + li {
    border-top: 1px solid var(--border);
  }

  .name {
    font-weight: 500;
  }

  .check {
    display: flex;
    align-items: center;
    gap: 0.5rem;
  }

  .small {
    font-size: 0.8rem;
  }

  .error {
    color: var(--danger);
  }
</style>
