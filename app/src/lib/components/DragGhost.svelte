<script lang="ts">
  // What a drag of tracks carries, beside the pointer (PLAN.md F4).
  import { drag } from "$lib/state/drag.svelte";
  import Icon from "./Icon.svelte";
</script>

{#if drag.current}
  <div
    class="ghost"
    class:over={drag.current.over !== null}
    style:left="{drag.current.x + 14}px"
    style:top="{drag.current.y + 10}px"
  >
    <Icon name="note" size="1rem" />
    <span>{drag.current.label}</span>
  </div>
{/if}

<style>
  .ghost {
    position: fixed;
    z-index: 200;
    display: flex;
    align-items: center;
    gap: 0.4rem;
    max-width: 18rem;
    padding: 0.3rem 0.6rem;
    border-radius: var(--radius);
    background: var(--surface);
    border: 1px solid var(--border);
    box-shadow: var(--shadow);
    pointer-events: none;
    font-size: 0.85rem;
  }

  .ghost span {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .over {
    border-color: var(--accent);
    color: var(--accent);
  }

  :global(body.dragging-items) {
    cursor: grabbing;
    user-select: none;
  }
</style>
