<script lang="ts">
  import { t } from "$lib/i18n";
  import { toasts } from "$lib/state/toasts.svelte";
  import Icon from "./Icon.svelte";
</script>

<div class="toasts" role="status" aria-live="polite">
  {#each toasts.list as toast (toast.id)}
    <div class="toast" class:error={toast.kind === "error"}>
      <span>{toast.text}</span>
      <button class="icon" aria-label={t("toast.dismiss")} onclick={() => toasts.dismiss(toast.id)}>
        <Icon name="close" size="1rem" />
      </button>
    </div>
  {/each}
</div>

<style>
  .toasts {
    position: fixed;
    left: 50%;
    bottom: 6.5rem;
    transform: translateX(-50%);
    z-index: 90;
    display: flex;
    flex-direction: column;
    gap: 0.5rem;
    width: min(32rem, calc(100vw - 2rem));
    pointer-events: none;
  }

  .toast {
    display: flex;
    align-items: center;
    gap: 0.5rem;
    padding: 0.5rem 0.5rem 0.5rem 0.9rem;
    border-radius: 8px;
    background: var(--surface);
    border: 1px solid var(--border);
    box-shadow: var(--shadow);
    pointer-events: auto;
    overflow-wrap: anywhere;
  }

  .toast span {
    flex: 1;
  }

  .error {
    border-left: 3px solid var(--danger);
  }
</style>
