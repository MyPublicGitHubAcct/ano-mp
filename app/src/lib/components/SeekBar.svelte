<script lang="ts">
  // The position slider and times. The only component reading the position
  // store, so position updates (every 50 ms) re-render just this.
  import { formatTime } from "$lib/format";
  import { player } from "$lib/state/player.svelte";

  /** The value while the thumb is held; seeking happens on release. */
  let held = $state<number | null>(null);
  const position = $derived(held ?? player.position);
  const duration = $derived(player.duration);
  const disabled = $derived(player.currentItem === null);

  function commit() {
    if (held !== null) player.seek(held);
    held = null;
  }
</script>

<div class="seek">
  <span class="time">{formatTime(position)}</span>
  <input
    type="range"
    min="0"
    max={duration || 1}
    step="0.1"
    value={position}
    {disabled}
    aria-label="Position"
    aria-valuetext="{formatTime(position)} of {formatTime(duration)}"
    style:--progress="{duration > 0 ? (position / duration) * 100 : 0}%"
    oninput={(event) => (held = Number(event.currentTarget.value))}
    onchange={commit}
    onkeydown={(event) => event.stopPropagation()}
  />
  <span class="time">-{formatTime(Math.max(0, duration - position))}</span>
</div>

<style>
  .seek {
    display: flex;
    align-items: center;
    gap: 0.5rem;
    width: 100%;
  }

  .time {
    font-size: 0.75rem;
    font-variant-numeric: tabular-nums;
    color: var(--text-muted);
    min-width: 3.2em;
    text-align: center;
  }

  input {
    flex: 1;
    min-width: 0;
  }
</style>
