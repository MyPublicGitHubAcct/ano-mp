<script lang="ts">
  // The Help window (PLAN.md Phase 7c D1): the user guide, bundled with the
  // app from docs/user-guide/ so it works offline, with its contents down
  // the side. Pages are parsed by `guide.ts` and drawn with ordinary
  // elements, never as HTML. A link to another page opens it here (at its
  // heading), a web link opens in the browser, and Back retraces the pages
  // visited. The window's capability allows only reading the settings, for
  // the theme.
  import { tick } from "svelte";
  import { guidePages, INDEX, type Block, type Inline, type LinkTarget } from "$lib/guide";
  import { t } from "$lib/i18n";
  import { openWebLink } from "$lib/openLink";
  import { appSettings } from "$lib/state/settings.svelte";

  const files = import.meta.glob("../../../../docs/user-guide/*.md", {
    query: "?raw",
    import: "default",
    eager: true,
  }) as Record<string, string>;
  const pages = guidePages(
    Object.fromEntries(Object.entries(files).map(([path, text]) => [path.split("/").pop() ?? path, text])),
  );

  let current = $state(INDEX);
  let history = $state<string[]>([]);
  let article = $state<HTMLElement>();

  const page = $derived(pages.find((candidate) => candidate.name === current) ?? pages[0]);

  $effect(() => appSettings.connect());

  async function go(name: string, anchor: string | null, remember = true) {
    if (name !== current) {
      if (remember) history = [...history, current];
      current = name;
    }
    await tick();
    const target = anchor ? document.getElementById(anchor) : null;
    if (target) target.scrollIntoView();
    else article?.scrollTo(0, 0);
    (target ?? article)?.focus({ preventScroll: true });
  }

  function back() {
    const previous = history.at(-1);
    if (previous === undefined) return;
    history = history.slice(0, -1);
    void go(previous, null, false);
  }

  function follow(event: MouseEvent, target: LinkTarget) {
    event.preventDefault();
    if (target.kind === "page") void go(target.page, target.anchor);
    else if (target.kind === "web") openWebLink(target.url);
  }

  function onkeydown(event: KeyboardEvent) {
    if ((event.metaKey && event.key === "[") || (event.key === "Escape" && history.length > 0)) {
      event.preventDefault();
      back();
    }
  }
</script>

<svelte:window {onkeydown} />
<svelte:head><title>{t("help.title")}: {page.title}</title></svelte:head>

{#snippet inlines(items: Inline[])}
  {#each items as item, index (index)}
    {#if item.kind === "text"}{item.text}{:else if item.kind === "strong"}<strong
        >{@render inlines(item.children)}</strong
      >{:else if item.kind === "em"}<em>{@render inlines(item.children)}</em>{:else if item.kind === "code"}<code
        >{item.text}</code
      >{:else if item.target.kind === "none"}{@render inlines(item.children)}{:else}<a
        href={item.target.kind === "web" ? item.target.url : `#${item.target.anchor ?? ""}`}
        onclick={(event) => follow(event, item.target)}>{@render inlines(item.children)}</a
      >{/if}
  {/each}
{/snippet}

{#snippet blocks(items: Block[])}
  {#each items as block, index (index)}
    {#if block.kind === "heading"}
      <svelte:element this={`h${Math.min(block.level, 4)}`} id={block.id} tabindex="-1"
        >{@render inlines(block.children)}</svelte:element
      >
    {:else if block.kind === "paragraph"}
      <p>{@render inlines(block.children)}</p>
    {:else if block.kind === "list" && block.ordered}
      <ol start={block.start}>
        {#each block.items as item, n (n)}<li>{@render blocks(item)}</li>{/each}
      </ol>
    {:else if block.kind === "list"}
      <ul>
        {#each block.items as item, n (n)}<li>{@render blocks(item)}</li>{/each}
      </ul>
    {:else if block.kind === "table"}
      <div class="table">
        <table>
          <thead>
            <tr
              >{#each block.head as cell, n (n)}<th>{@render inlines(cell)}</th>{/each}</tr
            >
          </thead>
          <tbody>
            {#each block.rows as row, r (r)}
              <tr
                >{#each row as cell, n (n)}<td>{@render inlines(cell)}</td>{/each}</tr
              >
            {/each}
          </tbody>
        </table>
      </div>
    {/if}
  {/each}
{/snippet}

<div class="help">
  <nav aria-label={t("help.contents")}>
    <h2>{t("help.contents")}</h2>
    <ul>
      {#each pages as candidate (candidate.name)}
        <li>
          <button
            class="page"
            class:active={candidate.name === current}
            aria-current={candidate.name === current ? "page" : undefined}
            onclick={() => go(candidate.name, null)}>{candidate.title}</button
          >
        </li>
      {/each}
    </ul>
  </nav>
  <main>
    <header>
      <button onclick={back} disabled={history.length === 0} title={t("help.backHint")}>{t("help.back")}</button>
    </header>
    <article bind:this={article} tabindex="-1">
      {#key page.name}{@render blocks(page.blocks)}{/key}
    </article>
  </main>
</div>

<style>
  .help {
    display: grid;
    grid-template-columns: minmax(12rem, 16rem) 1fr;
    height: 100vh;
    height: 100dvh;
    background: var(--bg);
  }

  nav {
    overflow-y: auto;
    padding: 1rem 0.5rem;
    background: var(--surface);
    border-right: 1px solid var(--border);
  }

  nav h2 {
    font-size: 0.8rem;
    text-transform: uppercase;
    letter-spacing: 0.05em;
    color: var(--text-muted);
    margin: 0 0.5rem 0.5rem;
  }

  nav ul {
    list-style: none;
    margin: 0;
    padding: 0;
  }

  .page {
    display: block;
    width: 100%;
    text-align: left;
    border: none;
    background: none;
    border-radius: var(--radius-sm);
    padding: 0.3rem 0.5rem;
  }

  .page.active {
    background: var(--selected);
  }

  main {
    display: flex;
    flex-direction: column;
    min-width: 0;
  }

  header {
    padding: 0.5rem 1rem;
    border-bottom: 1px solid var(--border);
  }

  article {
    flex: 1;
    overflow-y: auto;
    padding: 0.5rem 2rem 3rem;
    line-height: 1.55;
    outline: none;
  }

  article > :global(*) {
    max-width: 46rem;
  }

  article :global(h1) {
    font-size: 1.7rem;
  }

  article :global(h2) {
    font-size: 1.3rem;
    margin-top: 2rem;
  }

  article :global(h3) {
    font-size: 1.1rem;
    margin-top: 1.5rem;
  }

  article :global(:is(h1, h2, h3, h4)) {
    outline: none;
    scroll-margin-top: 1rem;
  }

  article :global(a) {
    color: var(--accent);
  }

  article :global(code) {
    font-size: 0.95em;
    padding: 0.05rem 0.3rem;
    border-radius: var(--radius-sm);
    background: var(--surface-2);
  }

  article :global(li + li) {
    margin-top: 0.25rem;
  }

  article :global(li > p) {
    margin: 0.25rem 0;
  }

  article :global(.table) {
    overflow-x: auto;
    margin: 0.75rem 0;
  }

  article :global(table) {
    border-collapse: collapse;
    width: 100%;
  }

  article :global(:is(th, td)) {
    text-align: left;
    vertical-align: top;
    padding: 0.35rem 0.6rem;
    border-bottom: 1px solid var(--border);
  }

  article :global(th) {
    background: var(--surface);
  }

  @media (max-width: 40rem) {
    .help {
      grid-template-columns: 1fr;
      grid-template-rows: auto 1fr;
    }

    nav {
      max-height: 30vh;
      border-right: none;
      border-bottom: 1px solid var(--border);
    }
  }
</style>
