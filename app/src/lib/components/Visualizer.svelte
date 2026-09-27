<script lang="ts">
  // A canvas that draws a visualization once per animation frame from the
  // core's analysis (streamed while this is mounted), with the current
  // track's cover and, unless the settings say otherwise, the colours taken
  // from it. Frames arrive up to 60 times a second (the settings' frame
  // rate) outside Svelte's reactivity; the draw loop reads the latest, with
  // the spectrum scaled by the settings' sensitivity.
  import { artUrl, subscribeToAnalysis } from "$lib/api";
  import { library } from "$lib/state/library.svelte";
  import { player } from "$lib/state/player.svelte";
  import { appSettings } from "$lib/state/settings.svelte";
  import { toasts } from "$lib/state/toasts.svelte";
  import { visualizer } from "$lib/state/visualizer.svelte";
  import { visualization } from "$lib/visualizer";
  import { decodeFrame, silentFrame } from "$lib/visualizer/frame";
  import { DEFAULT_PALETTE, paletteFrom } from "$lib/visualizer/palette";
  import type { Palette, Renderer, Scene } from "$lib/visualizer/types";

  let { id }: { id: string } = $props();

  let canvas: HTMLCanvasElement;
  const frame = silentFrame();
  let fresh = false;
  let beat = false;
  let cover: HTMLImageElement | null = null;
  /** The cover's colours, if it has been read. */
  let coverPalette: Palette | null = null;
  const colorsFromCover = $derived(appSettings.visualizer.colorsFromCover);
  const sensitivity = $derived(appSettings.visualizer.sensitivity);

  // The analysis stream.
  $effect(() => {
    let stop: (() => Promise<void>) | null = null;
    let cancelled = false;
    subscribeToAnalysis((buffer) => {
      if (!decodeFrame(buffer, frame)) return;
      if (sensitivity !== 1) {
        for (let i = 0; i < frame.bands.length; i++) frame.bands[i] = Math.min(1, frame.bands[i] * sensitivity);
      }
      fresh = true;
      beat ||= frame.beat;
    })
      .then((unsubscribe) => {
        if (cancelled) void unsubscribe();
        else stop = unsubscribe;
      })
      .catch((error) => toasts.show(`The visualizer can't follow the audio: ${error}`));
    return () => {
      cancelled = true;
      void stop?.();
    };
  });

  // The current cover, loaded with CORS so its colours can be read.
  const item = $derived(player.currentItem);
  const coverUrl = $derived(
    item === null
      ? null
      : item.albumId !== null
        ? artUrl({ albumId: item.albumId }, library.version, library.artVersions.get(item.albumId))
        : artUrl({ trackId: item.trackId }, library.version),
  );
  $effect(() => {
    const url = coverUrl;
    if (url === null) {
      cover = null;
      coverPalette = null;
      return;
    }
    let current = true;
    const load = (cors: boolean) => {
      const image = new Image();
      if (cors) image.crossOrigin = "anonymous";
      image.decoding = "async";
      image.onload = () => {
        if (!current) return;
        cover = image;
        coverPalette = paletteFrom(image);
      };
      image.onerror = () => {
        if (!current) return;
        // A copy cached before the art had CORS headers fails with CORS; show it without its colours.
        if (cors) load(false);
        else {
          cover = null;
          coverPalette = null;
        }
      };
      image.src = url;
    };
    load(true);
    return () => {
      current = false;
    };
  });

  // The draw loop, with a new renderer whenever the visualization changes.
  $effect(() => {
    const renderer: Renderer = visualization(id).create();
    const ctx = canvas.getContext("2d");
    if (!ctx) return;
    let width = 0;
    let height = 0;
    let dpr = 1;
    const resize = () => {
      dpr = window.devicePixelRatio || 1;
      width = canvas.clientWidth;
      height = canvas.clientHeight;
      canvas.width = Math.max(1, Math.round(width * dpr));
      canvas.height = Math.max(1, Math.round(height * dpr));
    };
    const observer = new ResizeObserver(resize);
    observer.observe(canvas);
    resize();

    const start = performance.now();
    let previous = start;
    let request = requestAnimationFrame(function tick(now) {
      const dt = Math.min(0.1, (now - previous) / 1000);
      previous = now;
      if (width > 0 && height > 0) {
        ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
        const current = player.currentItem;
        const scene: Scene = {
          ctx,
          width,
          height,
          dpr,
          time: (now - start) / 1000,
          dt,
          frame,
          fresh,
          beat,
          track: current && { trackId: current.trackId, albumId: current.albumId },
          cover,
          palette: (colorsFromCover && coverPalette) || DEFAULT_PALETTE,
          settings: { coverBasis: visualizer.coverBasis },
        };
        renderer.draw(scene);
        fresh = false;
        beat = false;
      }
      request = requestAnimationFrame(tick);
    });

    return () => {
      cancelAnimationFrame(request);
      observer.disconnect();
      renderer.dispose?.();
    };
  });
</script>

<canvas bind:this={canvas} aria-hidden="true"></canvas>

<style>
  canvas {
    display: block;
    width: 100%;
    height: 100%;
    background: rgb(9 9 12);
  }
</style>
