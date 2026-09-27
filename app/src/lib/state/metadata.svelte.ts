// What the metadata worker is doing, for the sidebar and the Services view.

import { t } from "$lib/i18n";
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
    const names = unreachable.join(", ");
    if (paused) return t("metadataStatus.paused", { names });
    if (total === 0) return unreachable.length > 0 ? t("metadataStatus.unreachable", { names }) : t("metadataStatus.upToDate");
    const step = t("metadataStatus.step", { done: Math.min(done + 1, total), total });
    if (current === null) return step;
    return `${current.kind === "album" ? current.title : current.name}: ${step}`;
  }
}

export const metadataStatus = new MetadataStore();
