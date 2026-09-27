// What track lists can show besides the title (`DisplaySettings.trackColumns`),
// and the album facts an album's summary line can give.

import type { AlbumFact, FeatureSettings, Track, TrackColumn } from "$lib/api";
import { fileName, formatDay, formatTime } from "$lib/format";

export const TRACK_COLUMNS: { id: TrackColumn; name: string }[] = [
  { id: "trackNumber", name: "Track number" },
  { id: "artist", name: "Artist" },
  { id: "album", name: "Album" },
  { id: "albumArtist", name: "Album artist" },
  { id: "year", name: "Year" },
  { id: "genre", name: "Genre" },
  { id: "duration", name: "Length" },
  { id: "format", name: "Format" },
  { id: "bitrate", name: "Bit rate" },
  { id: "sampleRate", name: "Sample rate" },
  { id: "playCount", name: "Plays" },
  { id: "lastPlayed", name: "Last played" },
  { id: "dateAdded", name: "Date added" },
  { id: "composer", name: "Composer" },
];

/** Whether a column's feature is on: plays need the listening history, the composer the classical tags. */
export function columnAvailable(column: TrackColumn, features: FeatureSettings) {
  if (column === "playCount" || column === "lastPlayed") return features.listeningHistory;
  if (column === "composer") return features.classical;
  return true;
}

export const ALBUM_FACTS: { id: AlbumFact; name: string }[] = [
  { id: "date", name: "Release date" },
  { id: "label", name: "Label and catalogue number" },
  { id: "country", name: "Country" },
  { id: "format", name: "Format" },
  { id: "type", name: "Type" },
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
  column === "dateAdded";

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
      return track.bitrateKbps === null ? "" : `${track.bitrateKbps} kbps`;
    case "sampleRate":
      return track.sampleRate > 0 ? `${(track.sampleRate / 1000).toLocaleString()} kHz` : "";
    case "playCount":
      return track.playCount > 0 ? track.playCount.toLocaleString() : "";
    case "lastPlayed":
      return track.lastPlayed === null ? "" : formatDay(track.lastPlayed);
    case "dateAdded":
      return track.addedAt > 0 ? formatDay(track.addedAt) : "";
    case "composer":
      return track.composer ?? "";
  }
}
