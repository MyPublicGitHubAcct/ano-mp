<script lang="ts">
  // Online sources: the switch for every online service, automatic lookups,
  // each source (on or off, what it supplies and relies on, a key if it
  // takes one, whether it can be reached now, and any notice its terms
  // require), and for each kind of data the order the sources are tried in.
  // Every change is saved at once; keys go to the keychain, never into the
  // settings. A section of the settings page (SettingsPage).
  import { onMount } from "svelte";
  import { ask } from "@tauri-apps/plugin-dialog";
  import { openUrl } from "@tauri-apps/plugin-opener";
  import {
    metadata,
    type MetadataKind,
    type MetadataSettings,
    type ServiceSettings,
    type SourceId,
    type SourceInfo,
  } from "$lib/api";
  import { library } from "$lib/state/library.svelte";
  import { metadataStatus } from "$lib/state/metadata.svelte";
  import { attempt } from "$lib/state/toasts.svelte";
  import Icon from "./Icon.svelte";

  const KINDS: { kind: MetadataKind; name: string; about: string }[] = [
    { kind: "release", name: "Album details", about: "Release dates, labels, formats and genres." },
    { kind: "albumArt", name: "Album art", about: "The first source with a picture for an album is shown." },
    { kind: "artistInfo", name: "Artist biographies", about: "The first source with a biography is shown." },
    { kind: "albumInfo", name: "Album descriptions", about: "The first source with a description is shown." },
  ];

  let data = $state.raw<MetadataSettings | null>(null);
  let saving = $state(false);
  /** Keys as typed, until saved. */
  let keys = $state<Partial<Record<SourceId, string>>>({});

  onMount(() => {
    attempt(async () => {
      data = await metadata.settings();
    });
  });

  const settings = $derived(data?.settings ?? null);
  const infoOf = (id: SourceId) => data?.sources.find((source) => source.id === id);
  const sourceSettings = (id: SourceId) => settings?.sources.find((source) => source.id === id);

  /** Stores `next`; art may come from other sources now, so it's reloaded. */
  async function save(next: ServiceSettings) {
    saving = true;
    try {
      await attempt(async () => {
        data = await metadata.saveSettings(next);
        library.version++;
      });
    } finally {
      saving = false;
    }
  }

  function change(edit: (next: ServiceSettings) => void) {
    if (!settings) return;
    const next: ServiceSettings = structuredClone($state.snapshot(settings));
    edit(next);
    save(next);
  }

  const setEnabled = (id: SourceId, enabled: boolean) =>
    change((next) => {
      const source = next.sources.find((s) => s.id === id);
      if (source) source.enabled = enabled;
    });

  /** Saves the key typed for `id` (null removes it) in the keychain. */
  async function setKey(id: SourceId, key: string | null) {
    saving = true;
    try {
      await attempt(async () => {
        data = await metadata.setKey(id, key);
        keys[id] = "";
        library.version++;
      });
    } finally {
      saving = false;
    }
  }

  async function removeKey(info: SourceInfo) {
    const confirmed = await ask(`Remove the ${info.keyName?.toLowerCase() ?? "key"} for ${info.name}? ${info.name} won’t be used until you add one again.`, {
      title: `Remove ${info.name} key`,
      kind: "warning",
      okLabel: "Remove",
    });
    if (confirmed) setKey(info.id, null);
  }

  const move = (kind: MetadataKind, index: number, by: number) =>
    change((next) => {
      const order = next.order[kind];
      const [source] = order.splice(index, 1);
      order.splice(index + by, 0, source);
    });

  async function reset() {
    const confirmed = await ask("Restore the default sources and order? Saved keys are kept.", {
      title: "Reset online sources",
      kind: "warning",
      okLabel: "Reset",
    });
    if (!confirmed) return;
    await attempt(async () => {
      data = await metadata.resetSettings();
      library.version++;
    });
  }

  /** Whether `id` may be used now, as the backend decides it. */
  function usable(id: SourceId): boolean {
    const info = infoOf(id);
    const source = sourceSettings(id);
    if (!info || !source || !settings) return false;
    return (
      source.enabled &&
      (!info.needsKey || source.hasKey) &&
      (!info.online || settings.online) &&
      (info.requires === null || usable(info.requires))
    );
  }

  /** What a source is doing, and whether that's a problem. */
  function status(info: SourceInfo): { text: string; tone: "on" | "off" | "problem" } {
    const source = sourceSettings(info.id);
    if (!source?.enabled) return { text: "Off", tone: "off" };
    if (info.online && !settings?.online) return { text: "Online services off", tone: "off" };
    if (info.requires !== null && !usable(info.requires))
      return { text: `Needs ${infoOf(info.requires)?.name ?? info.requires}`, tone: "problem" };
    if (info.needsKey && !source.hasKey)
      return { text: `Needs a ${info.keyName?.toLowerCase() ?? "key"}`, tone: "problem" };
    if (info.hosts.some((host) => metadataStatus.progress.unreachable.includes(host)))
      return { text: "Can’t be reached", tone: "problem" };
    return { text: "In use", tone: "on" };
  }

  const kindNames = (info: SourceInfo) =>
    info.kinds.map((kind) => KINDS.find((k) => k.kind === kind)?.name ?? kind).join(", ");

  function openLink(event: MouseEvent) {
    event.preventDefault();
    const href = (event.currentTarget as HTMLAnchorElement).href;
    attempt(() => openUrl(href));
  }
</script>

<div class="services">

  <div class="status" role="status">
    <Icon name="cloud" />
    <span>{metadataStatus.summary}</span>
    {#if metadataStatus.progress.unreachable.length > 0}
      <button onclick={() => attempt(metadata.retryNow)}><Icon name="refresh" /> Try now</button>
    {/if}
  </div>

  {#if settings && data}
    <section aria-labelledby="general-heading">
      <h3 id="general-heading">General</h3>
      <label class="switch">
        <input
          type="checkbox"
          checked={settings.online}
          disabled={saving}
          onchange={(event) => change((next) => (next.online = event.currentTarget.checked))}
        />
        <span>
          <span class="title">Use online services</span>
          <span class="muted small">
            When off, nothing is fetched; details and covers already fetched keep showing.
          </span>
        </span>
      </label>
      <label class="switch">
        <input
          type="checkbox"
          checked={settings.autoMatch}
          disabled={saving}
          onchange={(event) => change((next) => (next.autoMatch = event.currentTarget.checked))}
        />
        <span>
          <span class="title">Look up albums and artists automatically</span>
          <span class="muted small">
            In the background after a scan, and the album playing. When off, only what you ask for is looked up.
          </span>
        </span>
      </label>
    </section>

    <section aria-labelledby="sources-heading">
      <h3 id="sources-heading">Sources</h3>
      <ul class="sources">
        {#each data.sources as info (info.id)}
          {@const source = sourceSettings(info.id)}
          {@const state = status(info)}
          <li>
            <label class="switch">
              <input
                type="checkbox"
                checked={source?.enabled ?? false}
                disabled={saving}
                onchange={(event) => setEnabled(info.id, event.currentTarget.checked)}
              />
              <span>
                <span class="title">
                  {info.name}
                  {#if info.homepage}
                    <a class="small" href={info.homepage} onclick={openLink}>{new URL(info.homepage).host}</a>
                  {/if}
                </span>
                <span class="muted small">
                  {kindNames(info)}{info.requires ? ` · uses ${infoOf(info.requires)?.name} matches` : ""}{info.online
                    ? ""
                    : " · on this computer"}{info.storesDetails ? "" : " · fetched when shown, never stored"}
                </span>
              </span>
            </label>
            <span class="pill {state.tone}">{state.text}</span>
            {#if info.needsKey}
              {@const keyName = info.keyName ?? "API key"}
              {#if source?.hasKey}
                <div class="key">
                  <span class="muted small">{keyName} saved in the keychain</span>
                  <button class="link small" disabled={saving} onclick={() => removeKey(info)}>Remove</button>
                </div>
              {:else}
                <form
                  class="key"
                  onsubmit={(event) => {
                    event.preventDefault();
                    const key = keys[info.id]?.trim();
                    if (key) setKey(info.id, key);
                  }}
                >
                  <label>
                    <span class="muted small">{keyName}</span>
                    <input type="password" autocomplete="off" spellcheck="false" bind:value={keys[info.id]} />
                  </label>
                  <button type="submit" disabled={saving || !keys[info.id]?.trim()}>Save</button>
                  {#if info.keyUrl}
                    <a class="small" href={info.keyUrl} onclick={openLink}>Get one</a>
                  {/if}
                </form>
              {/if}
            {/if}
            {#if info.notice}
              <p class="notice muted small">{info.notice}</p>
            {/if}
          </li>
        {/each}
      </ul>
    </section>

    <section aria-labelledby="order-heading">
      <h3 id="order-heading">Order</h3>
      <p class="muted small">For each kind of data, sources are tried from the top. A cover you choose for an album comes first.</p>
      <div class="orders">
        {#each KINDS as { kind, name, about } (kind)}
          <div class="order">
            <h4>{name}</h4>
            <p class="muted small">{about}</p>
            <ol>
              {#each settings.order[kind] as id, index (id)}
                <li class:unusable={!usable(id)}>
                  <span class="position muted">{index + 1}</span>
                  <span class="name">{infoOf(id)?.name ?? id}</span>
                  <button
                    class="icon"
                    title="Move up"
                    aria-label="Move {infoOf(id)?.name} up"
                    disabled={saving || index === 0}
                    onclick={() => move(kind, index, -1)}><Icon name="up" size="1rem" /></button
                  >
                  <button
                    class="icon"
                    title="Move down"
                    aria-label="Move {infoOf(id)?.name} down"
                    disabled={saving || index === settings.order[kind].length - 1}
                    onclick={() => move(kind, index, 1)}><Icon name="down" size="1rem" /></button
                  >
                </li>
              {/each}
            </ol>
          </div>
        {/each}
      </div>
    </section>

    <section class="reset">
      <button onclick={reset} disabled={saving}>Reset to defaults</button>
    </section>
  {:else}
    <p class="muted">Loading…</p>
  {/if}
</div>

<style>
  h3 {
    font-size: 1rem;
    margin: 1.5rem 0 0.5rem;
  }

  h4 {
    font-size: 0.9rem;
    margin: 0;
  }

  p {
    margin: 0;
  }

  .small {
    font-size: 0.8rem;
  }

  .status {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 0.5rem 0.75rem;
    padding: 0.6rem 0.8rem;
    border-radius: 8px;
    background: var(--surface);
    border: 1px solid var(--border);
  }

  .status span {
    flex: 1;
    min-width: 0;
  }

  .switch {
    display: flex;
    align-items: flex-start;
    gap: 0.6rem;
    padding: 0.35rem 0;
    cursor: pointer;
  }

  .switch > span {
    display: flex;
    flex-direction: column;
    gap: 0.1rem;
    min-width: 0;
  }

  input[type="checkbox"] {
    margin: 0.2rem 0 0;
    accent-color: var(--accent);
    width: 1rem;
    height: 1rem;
    flex: none;
  }

  .title {
    font-weight: 500;
  }

  .title a {
    font-weight: normal;
    margin-left: 0.4rem;
  }

  a {
    color: var(--accent);
    text-decoration: none;
  }

  a:hover {
    text-decoration: underline;
  }

  .sources {
    list-style: none;
    margin: 0;
    padding: 0;
    border: 1px solid var(--border);
    border-radius: 8px;
    background: var(--surface);
  }

  .sources li {
    display: flex;
    flex-wrap: wrap;
    align-items: flex-start;
    justify-content: space-between;
    gap: 0.25rem 1rem;
    padding: 0.4rem 0.8rem;
  }

  .sources li + li {
    border-top: 1px solid var(--border);
  }

  .pill {
    flex: none;
    margin-top: 0.35rem;
    font-size: 0.75rem;
    padding: 0.05rem 0.5rem;
    border-radius: 999px;
    background: var(--surface-2);
    color: var(--text-muted);
  }

  .pill.on {
    background: var(--selected);
    color: var(--accent);
  }

  .pill.problem {
    color: var(--danger);
  }

  .key {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 0.5rem;
    flex-basis: 100%;
    padding-left: 1.6rem;
  }

  .key label {
    display: flex;
    align-items: center;
    gap: 0.5rem;
    flex: 1 1 16rem;
    max-width: 30rem;
    min-width: 0;
  }

  .key input {
    flex: 1;
    min-width: 0;
    font-family: ui-monospace, monospace;
  }

  .notice {
    flex-basis: 100%;
    padding-left: 1.6rem;
    margin-bottom: 0.2rem;
  }

  .orders {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(14rem, 1fr));
    gap: 1rem;
    margin-top: 0.75rem;
  }

  .order {
    display: flex;
    flex-direction: column;
    gap: 0.25rem;
  }

  ol {
    list-style: none;
    margin: 0.35rem 0 0;
    padding: 0;
    border: 1px solid var(--border);
    border-radius: 8px;
    background: var(--surface);
  }

  ol li {
    display: flex;
    align-items: center;
    gap: 0.4rem;
    padding: 0.2rem 0.3rem 0.2rem 0.7rem;
  }

  ol li + li {
    border-top: 1px solid var(--border);
  }

  .position {
    width: 1.2ch;
    font-variant-numeric: tabular-nums;
  }

  .name {
    flex: 1;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .unusable .name {
    color: var(--text-faint);
    text-decoration: line-through;
  }

  .reset {
    margin-top: 1.5rem;
  }

</style>
