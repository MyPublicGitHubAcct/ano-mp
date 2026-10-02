// Web links the app opens in the browser (PLAN.md H3). The URL opener
// allows https only (capabilities/default.json), and many links come from
// data anyone can edit (MusicBrainz URL relations, Wikipedia), so an http
// link is upgraded to https, and anything else isn't a link at all.

/** `url` as a link the app may open: https, upgraded from http. Null for
    any other scheme, credentials in the URL, or no URL at all; an `<a>`
    given null for its `href` shows as plain text. */
export function webLink(url: string | null | undefined): string | null {
  if (!url) return null;
  let parsed: URL;
  try {
    parsed = new URL(url);
  } catch {
    return null;
  }
  if (parsed.protocol === "http:") parsed.protocol = "https:";
  if (parsed.protocol !== "https:" || parsed.username !== "" || parsed.password !== "") return null;
  return parsed.href;
}
