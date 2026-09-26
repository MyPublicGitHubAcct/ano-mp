// What the metadata worker is doing, for the sidebar and the Services view.

import { metadata as api, on, onAll, type MetadataProgress } from "$lib/api";
import { attempt } from "./toasts.svelte";

const idle: MetadataProgress = { done: 0, total: 0, current: null, paused: false, unreachable: [] };

class MetadataStore {
  progress = $state.raw<MetadataProgress>(idle);

  /** Follows `metadata-progress`; returns a function that stops. */
  connect() {
    const listeners = [on("metadata-progress", (progress) => (this.progress = progress))];
    attempt(async () => (this.progress = await api.status()));
    return onAll(listeners);
  }

  /** A line saying what the worker is doing. */
  get summary() {
    const { done, total, current, paused, unreachable } = this.progress;
    if (paused) return `Paused: ${unreachable.join(", ")} can't be reached`;
    if (total === 0) return unreachable.length > 0 ? `${unreachable.join(", ")} can't be reached` : "Up to date";
    const what = current === null ? "" : current.kind === "album" ? `${current.title}: ` : `${current.name}: `;
    return `${what}${Math.min(done + 1, total).toLocaleString()} of ${total.toLocaleString()}`;
  }
}

export const metadataStatus = new MetadataStore();
