<script lang="ts">
  // A modal dialog over the whole window (the native <dialog>, so focus stays
  // inside and Escape closes it): a title bar, a scrolling body, and an
  // optional row of actions at the bottom.
  import { t } from "$lib/i18n";
  import type { Snippet } from "svelte";
  import Icon from "./Icon.svelte";

  let {
    title,
    onclose,
    children,
    actions,
  }: { title: string; onclose: () => void; children: Snippet; actions?: Snippet } = $props();

  let dialog: HTMLDialogElement;
  const id = `dialog-${Math.random().toString(36).slice(2)}`;

  $effect(() => {
    dialog.showModal();
    return () => {
      if (dialog.open) dialog.close();
    };
  });

  /** A click on the backdrop (outside the box) closes the dialog. */
  function onclick(event: MouseEvent) {
    if (event.target !== dialog) return;
    const box = dialog.getBoundingClientRect();
    const inside =
      event.clientX >= box.left &&
      event.clientX <= box.right &&
      event.clientY >= box.top &&
      event.clientY <= box.bottom;
    if (!inside) dialog.close();
  }
</script>

<dialog bind:this={dialog} aria-labelledby={id} {onclose} {onclick}>
  <header>
    <h2 {id}>{title}</h2>
    <button class="icon" aria-label={t("dialog.close")} onclick={() => dialog.close()}><Icon name="close" /></button>
  </header>
  <div class="body">{@render children()}</div>
  {#if actions}
    <footer>{@render actions()}</footer>
  {/if}
</dialog>

<style>
  dialog {
    width: min(46rem, calc(100vw - 2rem));
    max-height: min(88vh, 56rem);
    padding: 0;
    border: 1px solid var(--border);
    border-radius: 10px;
    background: var(--surface);
    color: var(--text);
    box-shadow: var(--shadow);
    flex-direction: column;
  }

  dialog[open] {
    display: flex;
  }

  dialog::backdrop {
    background: rgb(0 0 0 / 0.4);
  }

  header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 1rem;
    padding: 0.75rem 0.75rem 0.5rem 1.25rem;
  }

  h2 {
    margin: 0;
    font-size: 1.1rem;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .body {
    /* Its content's height, shrinking to scroll within the dialog's limit. */
    flex: 1 1 auto;
    min-height: 0;
    overflow-y: auto;
    padding: 0.25rem 1.25rem 1rem;
  }

  footer {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 0.5rem;
    padding: 0.75rem 1.25rem;
    border-top: 1px solid var(--border);
  }

  @media (max-width: 640px) {
    dialog {
      width: 100vw;
      max-width: 100vw;
      max-height: 100dvh;
      height: 100dvh;
      border-radius: 0;
      border: none;
    }
  }
</style>
