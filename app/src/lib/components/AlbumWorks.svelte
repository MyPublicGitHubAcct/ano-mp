<script lang="ts">
  // An album's classical works (O6): each work's movements under it, with
  // the composer and conductor, and "Play work" to queue the whole work.
  // Shown on album pages whose tracks carry work tags.
  import { t } from "$lib/i18n";
  import { queue, type AlbumTrack } from "$lib/api";
  import { formatTime } from "$lib/format";
  import { attempt } from "$lib/state/toasts.svelte";

  let { tracks }: { tracks: AlbumTrack[] } = $props();

  type Work = { name: string; composer: string | null; conductor: string | null; movements: AlbumTrack[] };

  /** Works in the order their first movement comes on the album. */
  const works = $derived.by(() => {
    // eslint-disable-next-line svelte/prefer-svelte-reactivity -- scratch, inside the derivation
    const byName = new Map<string, Work>();
    for (const track of tracks) {
      if (!track.work) continue;
      let work = byName.get(track.work);
      if (!work) {
        work = { name: track.work, composer: track.composer, conductor: track.conductor, movements: [] };
        byName.set(track.work, work);
      }
      work.movements.push(track);
    }
    for (const work of byName.values()) {
      work.movements.sort((a, b) => (a.movementNumber ?? Infinity) - (b.movementNumber ?? Infinity));
    }
    return [...byName.values()];
  });

  const roman = (n: number) => {
    const numerals: [number, string][] = [
      [10, "X"],
      [9, "IX"],
      [5, "V"],
      [4, "IV"],
      [1, "I"],
    ];
    let result = "";
    for (const [value, numeral] of numerals) {
      while (n >= value) {
        result += numeral;
        n -= value;
      }
    }
    return result;
  };

  const playWork = (work: Work) =>
    attempt(() =>
      queue.play(
        work.movements.map((track) => track.id),
        0,
      ),
    );
</script>

{#if works.length > 0}
  <section class="works" aria-label={t("works.label")}>
    {#each works as work (work.name)}
      <div class="work">
        <div class="head">
          <div>
            <h3>{work.name}</h3>
            <p class="muted small">
              {[work.composer, work.conductor ? t("works.conductedBy", { name: work.conductor }) : null]
                .filter(Boolean)
                .join(" · ")}
            </p>
          </div>
          <button onclick={() => playWork(work)}>{t("works.play")}</button>
        </div>
        <ol>
          {#each work.movements as movement (movement.id)}
            <li>
              <button
                class="link"
                onclick={() =>
                  attempt(() =>
                    queue.play(
                      work.movements.map((m) => m.id),
                      work.movements.indexOf(movement),
                    ),
                  )}
              >
                {movement.movementNumber ? `${roman(movement.movementNumber)}. ` : ""}{movement.movementName ??
                  movement.title}
              </button>
              <span class="muted small">{formatTime(movement.duration)}</span>
            </li>
          {/each}
        </ol>
      </div>
    {/each}
  </section>
{/if}

<style>
  .works {
    display: grid;
    gap: 0.75rem;
    margin-top: 0.75rem;
  }

  .work {
    border: 1px solid var(--border);
    border-radius: 8px;
    padding: 0.6rem 0.8rem;
  }

  .head {
    display: flex;
    align-items: flex-start;
    justify-content: space-between;
    gap: 1rem;
  }

  h3 {
    margin: 0;
    font-size: 0.95rem;
  }

  p {
    margin: 0.1rem 0 0;
  }

  .small {
    font-size: 0.8rem;
  }

  ol {
    list-style: none;
    margin: 0.4rem 0 0;
    padding: 0;
  }

  li {
    display: flex;
    justify-content: space-between;
    gap: 1rem;
    padding: 0.15rem 0;
  }

  li .link {
    color: var(--text);
    text-align: left;
  }

  li .link:hover {
    color: var(--accent);
  }
</style>
