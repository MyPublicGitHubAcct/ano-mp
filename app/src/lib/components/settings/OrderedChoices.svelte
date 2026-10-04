<script lang="ts" generics="T extends string">
  // Some of `options`, in an order: the chosen ones listed with buttons to
  // move and remove each, and a menu to add one of the rest. Every change
  // goes to `onchange` at once.
  import { t } from "$lib/i18n";
  import Icon from "../Icon.svelte";

  let {
    options,
    value,
    onchange,
    label,
    disabled = false,
    min = 0,
    addLabel = t("choices.add"),
    empty = t("display.none"),
  }: {
    options: { id: T; name: string }[];
    value: T[];
    onchange: (value: T[]) => void;
    /** Names the list for assistive technology. */
    label: string;
    disabled?: boolean;
    /** Fewest that may be left. */
    min?: number;
    addLabel?: string;
    /** Shown when none are chosen. */
    empty?: string;
  } = $props();

  const nameOf = (id: T) => options.find((option) => option.id === id)?.name ?? id;
  const rest = $derived(options.filter((option) => !value.includes(option.id)));

  function move(index: number, by: number) {
    const next = [...value];
    const [item] = next.splice(index, 1);
    next.splice(index + by, 0, item);
    onchange(next);
  }
</script>

<div class="choices">
  <ol aria-label={label}>
    {#each value as id, index (id)}
      <li>
        <span class="position muted">{index + 1}</span>
        <span class="name">{nameOf(id)}</span>
        <button
          class="icon"
          title={t("choices.moveUp")}
          aria-label={t("choices.moveUpName", { name: nameOf(id) })}
          disabled={disabled || index === 0}
          onclick={() => move(index, -1)}><Icon name="up" size="1rem" /></button
        >
        <button
          class="icon"
          title={t("choices.moveDown")}
          aria-label={t("choices.moveDownName", { name: nameOf(id) })}
          disabled={disabled || index === value.length - 1}
          onclick={() => move(index, 1)}><Icon name="down" size="1rem" /></button
        >
        <button
          class="icon"
          title={t("queue.removeOne")}
          aria-label={t("queue.removeName", { name: nameOf(id) })}
          disabled={disabled || value.length <= min}
          onclick={() => onchange(value.filter((other) => other !== id))}><Icon name="close" size="1rem" /></button
        >
      </li>
    {:else}
      <li class="muted">{empty}</li>
    {/each}
  </ol>
  {#if rest.length > 0}
    <select
      aria-label="{addLabel} ({label})"
      {disabled}
      value=""
      onchange={(event) => {
        const id = event.currentTarget.value as T;
        event.currentTarget.value = "";
        if (id) onchange([...value, id]);
      }}
    >
      <option value="" disabled>{addLabel}</option>
      {#each rest as option (option.id)}
        <option value={option.id}>{option.name}</option>
      {/each}
    </select>
  {/if}
</div>

<style>
  .choices {
    display: flex;
    flex-direction: column;
    align-items: flex-start;
    gap: 0.4rem;
    max-width: 24rem;
  }

  ol {
    list-style: none;
    margin: 0;
    padding: 0;
    width: 100%;
    border: 1px solid var(--border);
    border-radius: var(--radius-lg);
    background: var(--surface);
  }

  li {
    display: flex;
    align-items: center;
    gap: 0.4rem;
    min-height: 2.1rem;
    padding: 0.1rem 0.3rem 0.1rem 0.7rem;
  }

  li + li {
    border-top: 1px solid var(--border);
  }

  .position {
    width: 1.5ch;
    font-variant-numeric: tabular-nums;
  }

  .name {
    flex: 1;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
</style>
