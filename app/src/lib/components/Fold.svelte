<script lang="ts">
  // A section that folds away under its heading, closed until the viewer
  // opens it. The choice is remembered under `key`, and only once the
  // viewer makes it, so a later change of default reaches everyone else.
  import { untrack, type Snippet } from "svelte";
  import { loadPreference, savePreference } from "$lib/state/ui.svelte";
  import Icon from "./Icon.svelte";

  let {
    key,
    heading,
    level = 2,
    children,
  }: { key: string; heading: string; level?: 2 | 3; children: Snippet } = $props();

  const id = $props.id();
  // A fold's key never changes, so it is read once.
  let open = $state(untrack(() => loadPreference(`fold.${key}`, false)));

  function toggle() {
    open = !open;
    savePreference(`fold.${key}`, open);
  }
</script>

<svelte:element this={`h${level}`} class="fold-heading">
  <button class="disclosure" class:open aria-expanded={open} aria-controls={id} onclick={toggle}>
    <Icon name="chevron" size="0.9rem" />
    {heading}
  </button>
</svelte:element>
<div {id} hidden={!open}>
  {@render children()}
</div>

<style>
  .fold-heading {
    margin: 1rem 0 0.4rem;
    font-size: 1.05rem;
  }

  h3.fold-heading {
    margin: 0.75rem 0 0.3rem;
    font-size: 0.95rem;
  }

  .disclosure {
    display: inline-flex;
    align-items: center;
    gap: 0.2rem;
    margin-left: -0.3rem;
    padding: 0.1rem 0.3rem;
    border: none;
    border-radius: var(--radius-sm);
    background: none;
    font: inherit;
    color: inherit;
  }

  .disclosure:hover {
    background: var(--hover);
  }

  .disclosure :global(svg) {
    transition: transform 0.15s ease;
  }

  .disclosure.open :global(svg) {
    transform: rotate(90deg);
  }

  @media (prefers-reduced-motion: reduce) {
    .disclosure :global(svg) {
      transition: none;
    }
  }
</style>
