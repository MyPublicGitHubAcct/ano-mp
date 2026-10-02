// The cover wall: the current album's cover large in the middle, and
// around it the covers of albums from the same year or by the same artist
// (`library_cover_wall`). Each tile brightens and swells with one band of
// the spectrum, bass at the centre and treble at the edges, and on beats
// a few tiles flip over to another album.

import { errorText, t } from "$lib/i18n";
import { artUrl, library as api, type CoverWall } from "$lib/api";
import { library } from "$lib/state/library.svelte";
import { visualizer } from "$lib/state/visualizer.svelte";
import type { Renderer, Scene, Visualization } from "../types";
import { approach, clearStage, rgba } from "../util";

const THUMBNAIL = 256;
const LOADING_AT_ONCE = 6;
const FLIP_SECONDS = 0.55;
/** Without beats (ambient music), a tile flips this often while audio plays. */
const QUIET_FLIP_SECONDS = 2.5;

type Tile = {
  album: number | null;
  /** Flipping over to this album, since `flipStart`. */
  next: number | null;
  flipStart: number;
  level: number;
};

function create(): Renderer {
  let disposed = false;
  let wallKey: string | null = null;
  let wall: CoverWall | null = null;
  let currentAlbum: number | null = null;
  let message: string | null = null;

  // Thumbnails by album id, kept across walls; failed loads (no cover) aren't retried.
  const thumbnails = new Map<number, HTMLCanvasElement>();
  const failed = new Set<number>();
  let queue: number[] = [];
  let loading = 0;
  /** The wall's albums with a cover, other than the current one. */
  let pool: number[] = [];

  let layoutKey = "";
  let cell = 0;
  let columns = 0;
  let rows = 0;
  let span = 1;
  let originX = 0;
  let originY = 0;
  let centre = { column: 0, row: 0 };
  let tiles: Tile[] = [];
  let lastFlip = 0;
  let pulse = 0;
  let flash = 0;

  async function fetchWall(key: string, trackId: number, basis: Scene["settings"]["coverBasis"]) {
    message = t("coverWall.finding");
    try {
      const result = await api.coverWall(trackId, basis);
      if (disposed || key !== wallKey) return;
      setWall(result);
      message = result ? null : basis === "year" ? t("coverWall.noYear") : t("coverWall.noArtist");
    } catch (error) {
      if (!disposed && key === wallKey) message = errorText(error);
    }
  }

  function setWall(next: CoverWall | null) {
    const ids = next?.albums.map((album) => album.id) ?? [];
    const before = new Set(wall?.albums.map((album) => album.id));
    // The same albums (another track from the same year): the tiles stay.
    const same = wall !== null && ids.length === before.size && ids.every((id) => before.has(id));
    wall = next;
    const others = ids.filter((id) => id !== currentAlbum);
    pool = others.filter((id) => thumbnails.has(id));
    queue = others.filter((id) => !thumbnails.has(id) && !failed.has(id));
    if (!same) {
      for (const tile of tiles) {
        tile.album = null;
        tile.next = null;
      }
    }
    loadMore();
  }

  function loadMore() {
    while (!disposed && loading < LOADING_AT_ONCE && queue.length > 0) {
      const id = queue.shift()!;
      loading++;
      const image = new Image();
      image.decoding = "async";
      image.onload = () => {
        loading--;
        if (disposed) return;
        thumbnails.set(id, thumbnail(image));
        if (wall?.albums.some((album) => album.id === id) && id !== currentAlbum) pool.push(id);
        loadMore();
      };
      image.onerror = () => {
        loading--;
        failed.add(id);
        loadMore();
      };
      image.src = artUrl({ albumId: id }, library.version, library.artVersions.get(id) ?? 0);
    }
  }

  function layout(width: number, height: number) {
    const key = `${width}x${height}`;
    if (key === layoutKey) return;
    layoutKey = key;
    cell = Math.min(170, Math.max(64, Math.sqrt((width * height) / 60)));
    columns = Math.ceil(width / cell) + 1;
    rows = Math.ceil(height / cell) + 1;
    span = Math.min(columns, rows) >= 7 ? 3 : Math.min(columns, rows) >= 4 ? 2 : 1;
    originX = (width - columns * cell) / 2;
    originY = (height - rows * cell) / 2;
    centre = { column: Math.floor((columns - span) / 2), row: Math.floor((rows - span) / 2) };
    tiles = Array.from({ length: columns * rows }, () => ({ album: null, next: null, flipStart: 0, level: 0 }));
  }

  const inCentre = (column: number, row: number) =>
    column >= centre.column && column < centre.column + span && row >= centre.row && row < centre.row + span;

  function pick(avoid: number | null) {
    if (pool.length === 0) return null;
    if (pool.length === 1) return pool[0];
    let album = avoid;
    while (album === avoid) album = pool[Math.floor(Math.random() * pool.length)];
    return album;
  }

  function flip(time: number, count: number) {
    const candidates = tiles.filter((tile, i) => tile.next === null && !inCentre(i % columns, Math.floor(i / columns)));
    for (let n = 0; n < count && candidates.length > 0; n++) {
      const tile = candidates.splice(Math.floor(Math.random() * candidates.length), 1)[0];
      tile.next = pick(tile.album);
      tile.flipStart = time;
    }
    lastFlip = time;
  }

  return {
    draw(scene: Scene) {
      const { ctx, width, height, dt, time, frame, track, palette } = scene;
      const key = track ? `${track.trackId}:${scene.settings.coverBasis}` : null;
      if (key !== wallKey) {
        wallKey = key;
        currentAlbum = track?.albumId ?? null;
        if (track) void fetchWall(key!, track.trackId, scene.settings.coverBasis);
        else {
          setWall(null);
          message = t("coverWall.nothing");
        }
      }
      visualizer.setCaption(
        message ??
          (wall &&
            t(wall.basis === "year" ? "coverWall.fromYear" : "coverWall.byArtist", {
              label: wall.label,
              count: pool.length + (currentAlbum !== null ? 1 : 0),
            })),
      );

      layout(width, height);
      pulse = approach(pulse, Math.max(frame.rms[0], frame.rms[1]), dt, 0.04, 0.3);
      flash = approach(flash, scene.beat ? 1 : 0, dt, 0, 0.3);
      if (scene.beat) flip(time, Math.random() < 0.35 ? 2 : 1);
      // Calm: a tile changes every so often, whatever the music does.
      else if (!frame.silent && time - lastFlip > QUIET_FLIP_SECONDS * (scene.calm ? 2 : 1)) flip(time, 1);

      clearStage(scene);
      const bands = frame.bands;
      const reach = Math.hypot(columns, rows) / 2;
      const centreX = centre.column + span / 2;
      const centreY = centre.row + span / 2;

      for (let row = 0; row < rows; row++) {
        for (let column = 0; column < columns; column++) {
          if (inCentre(column, row)) continue;
          const tile = tiles[row * columns + column];
          if (tile.album === null) tile.album = pick(null);

          // Bass at the centre, treble at the edges.
          const distance = Math.hypot(column + 0.5 - centreX, row + 0.5 - centreY) / reach;
          const band = Math.min(bands.length - 1, Math.floor(distance * bands.length * 0.8));
          tile.level = approach(tile.level, bands[band] ?? 0, dt, 0.03, 0.35);
          const energy = tile.level ** 1.6;

          let scaleX = 1;
          if (tile.next !== null) {
            const progress = (time - tile.flipStart) / FLIP_SECONDS;
            scaleX = Math.abs(Math.cos(Math.PI * Math.min(1, progress)));
            if (progress >= 0.5 && tile.album !== tile.next) tile.album = tile.next;
            if (progress >= 1) tile.next = null;
          }

          const size = cell * (0.84 + 0.12 * energy);
          const x = originX + (column + 0.5) * cell;
          const y = originY + (row + 0.5) * cell;
          const image = tile.album !== null ? thumbnails.get(tile.album) : undefined;
          ctx.save();
          ctx.translate(x, y);
          ctx.scale(Math.max(0.02, scaleX), 1);
          ctx.beginPath();
          ctx.roundRect(-size / 2, -size / 2, size, size, 4);
          if (image) {
            ctx.save();
            ctx.clip();
            ctx.drawImage(image, -size / 2, -size / 2, size, size);
            ctx.restore();
            ctx.fillStyle = rgba([0, 0, 0], 0.72 * (1 - energy));
          } else {
            ctx.fillStyle = rgba(palette.colors[(row + column) % palette.colors.length], 0.05 + 0.2 * energy);
          }
          ctx.fill();
          ctx.restore();
        }
      }

      // The current cover.
      const size = span * cell * (0.9 + 0.08 * Math.min(1, pulse * 2) + 0.03 * flash);
      const x = originX + centreX * cell;
      const y = originY + centreY * cell;
      ctx.save();
      ctx.shadowColor = rgba(palette.colors[0], 0.35 + 0.5 * flash);
      ctx.shadowBlur = 24 + 40 * flash;
      ctx.beginPath();
      ctx.roundRect(x - size / 2, y - size / 2, size, size, 8);
      ctx.fillStyle = rgba([20, 20, 24]);
      ctx.fill();
      ctx.shadowBlur = 0;
      if (scene.cover) {
        ctx.clip();
        ctx.drawImage(scene.cover, x - size / 2, y - size / 2, size, size);
      }
      ctx.restore();
    },
    dispose() {
      disposed = true;
      visualizer.setCaption(null);
    },
  };
}

/** A small copy, so drawing a hundred tiles a frame doesn't scale full-size covers. */
function thumbnail(image: HTMLImageElement) {
  const canvas = document.createElement("canvas");
  canvas.width = canvas.height = THUMBNAIL;
  const ctx = canvas.getContext("2d");
  if (ctx) {
    // Square covers fill it; others are cropped to their middle.
    const side = Math.min(image.naturalWidth, image.naturalHeight);
    ctx.imageSmoothingQuality = "high";
    ctx.drawImage(
      image,
      (image.naturalWidth - side) / 2,
      (image.naturalHeight - side) / 2,
      side,
      side,
      0,
      0,
      THUMBNAIL,
      THUMBNAIL,
    );
  }
  return canvas;
}

export const covers: Visualization = {
  id: "covers",
  get name() {
    return t("viz.covers.name");
  },
  get description() {
    return t("viz.covers.description");
  },
  create,
};
