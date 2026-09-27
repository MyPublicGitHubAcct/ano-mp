<script lang="ts">
  // The current track's lyrics (O13), read from its tags or a .lrc file
  // when shown. Synced lines light up as the track plays and seek there
  // when clicked; unsynced ones are plain text.
  import { t } from "$lib/i18n";
  import { untrack } from "svelte";
  import { features as api, type Lyrics } from "$lib/api";
  import { playback } from "$lib/state/position.svelte";
  import { player } from "$lib/state/player.svelte";

  let { trackId }: { trackId: number } = $props();

  let lyrics = $state.raw<Lyrics | null>(null);
  let list = $state<HTMLOListElement>();

  $effect(() => {
    const id = trackId;
    untrack(async () => {
      lyrics = null;
      const loaded = await api.lyrics(id).catch(() => null);
      if (id === trackId) lyrics = loaded;
    });
  });

  /** The line playing: the last one that has started. */
  const current = $derived.by(() => {
    const lines = lyrics?.lines ?? [];
    const position = player.loaded ? playback.position : player.resumeAt;
    let index = -1;
    for (let i = 0; i < lines.length && lines[i].time <= position + 0.15; i++) index = i;
    return index;
  });

  $effect(() => {
    const index = current;
    if (index < 0 || !list) return;
    const line = list.children[index] as HTMLElement | undefined;
    line?.scrollIntoView({ block: "center", behavior: "smooth" });
  });
</script>

{#if lyrics}
  <section class="lyrics" aria-label={t("lyrics.label")}>
    {#if lyrics.lines.length > 0}
      <ol bind:this={list}>
        {#each lyrics.lines as line, index (index)}
          <li class:current={index === current}>
            <button class="line" onclick={() => player.seek(line.time)}>{line.text || "♪"}</button>
          </li>
        {/each}
      </ol>
    {:else if lyrics.text}
      <p class="text">{lyrics.text}</p>
    {/if}
    <p class="source muted">{t(lyrics.source === "lrc" ? "lyrics.fromLrc" : "lyrics.fromTags")}</p>
  </section>
{/if}

<style>
  .lyrics {
    margin-top: 1rem;
    max-height: 18rem;
    overflow-y: auto;
    padding-right: 0.5rem;
    mask-image: linear-gradient(transparent, black 1.5rem, black calc(100% - 1.5rem), transparent);
  }

  ol {
    list-style: none;
    margin: 0;
    padding: 1.5rem 0;
  }

  .line {
    border: none;
    background: none;
    padding: 0.15rem 0;
    text-align: left;
    font-size: 1.05rem;
    color: var(--text-muted);
  }

  .line:hover:not(:disabled) {
    background: none;
    color: var(--text);
  }

  .current .line {
    color: var(--text);
    font-weight: 700;
  }

  .text {
    white-space: pre-wrap;
    margin: 0;
    padding: 1.5rem 0 0;
  }

  .source {
    font-size: 0.75rem;
    margin: 0.25rem 0 1rem;
  }
</style>
