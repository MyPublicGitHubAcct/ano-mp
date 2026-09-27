import type { MusicBrainzArtist } from "$lib/api";
import { t, type MessageKey } from "$lib/i18n";

/** m:ss, or h:mm:ss from an hour up. */
export function formatTime(seconds: number) {
  const total = Math.max(0, Math.floor(Number.isFinite(seconds) ? seconds : 0));
  const h = Math.floor(total / 3600);
  const m = Math.floor((total % 3600) / 60);
  const s = String(total % 60).padStart(2, "0");
  return h > 0 ? `${h}:${String(m).padStart(2, "0")}:${s}` : `${m}:${s}`;
}

export const fileName = (path: string) => path.split(/[\\/]/).pop() ?? path;

/** "1985", "June 1985" or "1 June 1985", from "1985", "1985-06" or "1985-06-01". */
export function formatDate(date: string) {
  const [year, month, day] = date.split("-").map(Number);
  if (!month) return String(year);
  return new Date(Date.UTC(year, month - 1, day || 1)).toLocaleDateString(undefined, {
    year: "numeric",
    month: "long",
    day: day ? "numeric" : undefined,
    timeZone: "UTC",
  });
}

/** A day from Unix seconds, e.g. "26 Sept 2026". */
export const formatDay = (seconds: number) =>
  new Date(seconds * 1000).toLocaleDateString(undefined, { year: "numeric", month: "short", day: "numeric" });

/** 0.934 → "93%". */
export const percent = (score: number) => `${Math.round(score * 100)}%`;

/** Media formats in order, repeats counted: ["CD", "CD", "DVD"] → "2×CD + DVD". */
export function formatMedia(formats: (string | null)[]) {
  const runs: { format: string; count: number }[] = [];
  for (const format of formats) {
    const name = format ?? t("format.unknownMedium");
    const last = runs.at(-1);
    if (last?.format === name) last.count++;
    else runs.push({ format: name, count: 1 });
  }
  return runs.map(({ format, count }) => (count > 1 ? `${count}×${format}` : format)).join(" + ");
}

/** "XL Recordings (XLCD324)", several joined with commas. */
export const formatLabels = (labels: { name: string | null; catalogNumber: string | null }[]) =>
  labels
    .map(({ name, catalogNumber }) =>
      name && catalogNumber ? `${name} (${catalogNumber})` : (name ?? catalogNumber ?? ""),
    )
    .filter(Boolean)
    .join(", ");

/** "Formed 1985 in Abingdon · Disbanded 2001", or born and died for a person. */
export function lifeSpan(artist: MusicBrainzArtist) {
  const person = artist.type === "Person" || artist.type === "Character";
  // An event with its date and place when known: "Born 1 June 1985 in Abingdon".
  const event = (what: "born" | "formed" | "died" | "disbanded", date: string | null, area: string | null) => {
    const when = date ? formatDate(date) : null;
    const key = `life.${what}${when ? "When" : ""}${area ? "Where" : ""}` as MessageKey;
    return t(key, { when, area });
  };
  const parts = [];
  if (artist.begin || artist.beginArea) parts.push(event(person ? "born" : "formed", artist.begin, artist.beginArea));
  if (artist.end || artist.endArea) parts.push(event(person ? "died" : "disbanded", artist.end, artist.endArea));
  else if (artist.ended && !person) parts.push(t("life.disbanded"));
  return parts.join(" · ");
}
