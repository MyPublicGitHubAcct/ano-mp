// What track lists can show besides the title (`DisplaySettings.trackColumns`),
// and the album facts an album's summary line can give.

import type { AlbumFact, FeatureSettings, Track, TrackColumn } from "$lib/api";
import { fileName, formatDay, formatTime } from "$lib/format";
import { t } from "$lib/i18n";

export const TRACK_COLUMNS: { id: TrackColumn; name: string }[] = [
  { id: "trackNumber", name: t("column.trackNumber") },
  { id: "artist", name: t("column.artist") },
  { id: "album", name: t("column.album") },
  { id: "albumArtist", name: t("column.albumArtist") },
  { id: "year", name: t("column.year") },
  { id: "genre", name: t("column.genre") },
  { id: "duration", name: t("column.duration") },
  { id: "format", name: t("column.format") },
  { id: "bitrate", name: t("column.bitrate") },
  { id: "sampleRate", name: t("column.sampleRate") },
  { id: "playCount", name: t("column.playCount") },
  { id: "lastPlayed", name: t("column.lastPlayed") },
  { id: "dateAdded", name: t("column.dateAdded") },
  { id: "composer", name: t("column.composer") },
  { id: "rating", name: t("column.rating") },
];

/** Whether a column's feature is on: plays need the listening history, the composer the classical tags. */
export function columnAvailable(column: TrackColumn, features: FeatureSettings) {
  if (column === "playCount" || column === "lastPlayed") return features.listeningHistory;
  if (column === "composer") return features.classical;
  return true;
}

export const ALBUM_FACTS: { id: AlbumFact; name: string }[] = [
  { id: "date", name: t("column.date") },
  { id: "label", name: t("column.label") },
  { id: "country", name: t("column.country") },
  { id: "format", name: t("column.format") },
  { id: "type", name: t("column.type") },
];

/** The columns shown as text beside the title (or under it, when narrow): all but the number, which leads
    the row, and the length, which ends it. */
export const textColumns = (columns: TrackColumn[]) =>
  columns.filter((column) => column !== "trackNumber" && column !== "duration");

/** Columns that hold a short number rather than a name, so they get a narrow fixed width. */
export const isShortColumn = (column: TrackColumn) =>
  column === "year" ||
  column === "format" ||
  column === "bitrate" ||
  column === "sampleRate" ||
  column === "playCount" ||
  column === "lastPlayed" ||
  column === "dateAdded" ||
  column === "rating";

/** "FLAC", "MP3"…: the file's extension. */
export function formatOf(path: string) {
  const name = fileName(path);
  const dot = name.lastIndexOf(".");
  return dot > 0 ? name.slice(dot + 1).toUpperCase() : "";
}

/** The text of `column` for `track`; "" when the track doesn't have it. */
export function columnText(column: TrackColumn, track: Track): string {
  switch (column) {
    case "trackNumber":
      return track.trackNumber === null ? "" : String(track.trackNumber);
    case "artist":
      return track.artist ?? "";
    case "album":
      return track.album ?? "";
    case "albumArtist":
      return track.albumArtist ?? "";
    case "year":
      return track.year === null ? "" : String(track.year);
    case "genre":
      return track.genre ?? "";
    case "duration":
      return formatTime(track.duration);
    case "format":
      return formatOf(track.path);
    case "bitrate":
      return track.bitrateKbps === null ? "" : t("info.kbps", { rate: track.bitrateKbps });
    case "sampleRate":
      return track.sampleRate > 0 ? t("info.kHz", { rate: (track.sampleRate / 1000).toLocaleString() }) : "";
    case "playCount":
      return track.playCount > 0 ? track.playCount.toLocaleString() : "";
    case "lastPlayed":
      return track.lastPlayed === null ? "" : formatDay(track.lastPlayed);
    case "dateAdded":
      return track.addedAt > 0 ? formatDay(track.addedAt) : "";
    case "composer":
      return track.composer ?? "";
    case "rating":
      return track.rating ? "★".repeat(track.rating) : "";
  }
}
