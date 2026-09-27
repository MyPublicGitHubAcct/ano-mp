// The queue and transport as the backend last reported them (the
// `queue-changed` and `player-state` events). The position is separate
// (`position.svelte.ts`).

import { on, onAll, player as playerApi, queue as queueApi, type PlayerState, type QueueItem, type QueueState, type Repeat } from "$lib/api";
import { playback } from "./position.svelte";
import { attempt, toasts } from "./toasts.svelte";

class PlayerStore {
  state = $state<PlayerState>("empty");
  volume = $state(1);
  /** The queue in play order. Replaced, never mutated. */
  items = $state.raw<QueueItem[]>([]);
  current = $state<number | null>(null);
  currentItem = $state.raw<QueueItem | null>(null);
  shuffle = $state(false);
  repeat = $state<Repeat>("off");
  unavailable = $state.raw(new Set<number>());
  hasNext = $state(false);
  hasPrevious = $state(false);
  loaded = $state(false);
  resumeAt = $state(0);
  /** Library radio keeps adding tracks (O9). */
  radio = $state(false);
  #revision = 0;

  get playing() {
    return this.state === "playing";
  }

  /** Where the current track is, in seconds, whether or not it is open. */
  get position() {
    return this.loaded ? playback.position : this.resumeAt;
  }

  get duration() {
    return this.loaded && playback.duration > 0 ? playback.duration : (this.currentItem?.duration ?? 0);
  }

  apply(state: QueueState) {
    if (state.revision <= this.#revision && state.items === null) return; // stale
    this.#revision = state.revision;
    if (state.items !== null) this.items = state.items;
    this.current = state.current;
    this.currentItem = state.currentItem;
    this.shuffle = state.shuffle;
    this.repeat = state.repeat;
    this.unavailable = new Set(state.unavailable);
    this.hasNext = state.hasNext;
    this.hasPrevious = state.hasPrevious;
    this.loaded = state.loaded;
    this.resumeAt = state.resumeAt;
    this.radio = state.radio;
    for (const skipped of state.skipped) toasts.show(`Skipped “${skipped.title}”: ${skipped.error}`);
  }

  /** Follows the backend; returns a function that stops. */
  connect() {
    const stop = onAll([
      on("queue-changed", (state) => this.apply(state)),
      on("player-state", (state) => (this.state = state)),
      on("player-position", ({ position, duration }) => {
        playback.position = position;
        playback.duration = duration;
      }),
    ]);
    attempt(async () => {
      const status = await playerApi.status();
      this.state = status.state;
      this.volume = status.volume;
      playback.position = status.position;
      playback.duration = status.duration;
      this.apply(await queueApi.state());
    });
    return stop;
  }

  toggle = () => attempt(queueApi.toggle);
  next = () => attempt(queueApi.next);
  previous = () => attempt(queueApi.previous);
  seek = (seconds: number) => {
    const duration = this.duration;
    const target = Math.max(0, duration > 0 ? Math.min(seconds, duration) : seconds);
    if (this.loaded) playback.position = target; // No wait for the next position event.
    return attempt(() => queueApi.seek(target));
  };
  setVolume = (volume: number) => {
    this.volume = volume;
    return attempt(() => playerApi.setVolume(volume));
  };
  toggleShuffle = () => attempt(() => queueApi.setShuffle(!this.shuffle));
  cycleRepeat = () => {
    const next: Record<Repeat, Repeat> = { off: "all", all: "one", one: "off" };
    return attempt(() => queueApi.setRepeat(next[this.repeat]));
  };
}

export const player = new PlayerStore();
