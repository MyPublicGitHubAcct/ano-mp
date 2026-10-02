// The UI's text (PLAN.md F19): every string the UI shows comes from a
// message catalogue, one JSON file per locale (`en.json` is the only one so
// far). Keys are typed from `en.json`, so a missing key fails `npm run
// check`. A message may hold `{name}` placeholders, and a plural message is
// an object of Intl.PluralRules categories ("one", "other"…) whose `{count}`
// picks the form. Numbers in parameters are formatted for the locale.
//
// Errors from the backend may come coded (`{code, params, message}` as
// JSON, see `errorText`): their text is the catalogue's "error.<code>",
// else the English message they carry.

import en from "./en.json";

type Catalogue = typeof en;
export type MessageKey = keyof Catalogue;
type Plural = Partial<Record<Intl.LDMLPluralRule, string>> & { other: string };
export type Params = Record<string, string | number | null | undefined>;

const catalogues: Record<string, Partial<Record<MessageKey, string | Plural>>> = { en };

/** The user's locale as the webview reports it; English text until a
    catalogue for it exists. */
export const locale = typeof navigator !== "undefined" ? navigator.language || "en" : "en";

const messages = catalogues[locale] ?? catalogues[locale.split("-")[0]] ?? catalogues.en;
const plurals = new Intl.PluralRules(locale);
const numbers = new Intl.NumberFormat(locale);

function fill(template: string, params: Params) {
  return template.replace(/\{(\w+)\}/g, (whole, name: string) => {
    const value = params[name];
    if (value === undefined || value === null) return whole;
    return typeof value === "number" ? numbers.format(value) : value;
  });
}

/** The text of message `key`, with `params` filled in. */
export function t(key: MessageKey, params: Params = {}): string {
  const message = (messages[key] ?? en[key]) as string | Plural | undefined;
  if (message === undefined) return key;
  if (typeof message === "string") return fill(message, params);
  const count = typeof params.count === "number" ? params.count : 0;
  return fill(message[plurals.select(count)] ?? message.other, params);
}

/** Whether the catalogue has `key`. */
export function has(key: string): key is MessageKey {
  return key in messages || key in en;
}

/** An error from a command as the user should read it: a coded error's
    catalogue text, else its message, else the error as text. */
export function errorText(error: unknown): string {
  const text = error instanceof Error ? error.message : String(error);
  if (!text.startsWith("{")) return text;
  try {
    const coded = JSON.parse(text) as { code?: string; params?: Params; message?: string };
    if (typeof coded.code === "string") {
      // A reason may have its own message: `error.<code>.<reason>`.
      const reason = coded.params?.reason;
      const specific = `error.${coded.code}.${reason}`;
      if (typeof reason === "string" && has(specific)) return t(specific, coded.params ?? {});
      const key = `error.${coded.code}`;
      if (has(key)) return t(key, coded.params ?? {});
    }
    return coded.message ?? text;
  } catch {
    return text;
  }
}

/** A coded error's code, else null. */
export function errorCode(error: unknown): string | null {
  const text = error instanceof Error ? error.message : String(error);
  if (!text.startsWith("{")) return null;
  try {
    const code = (JSON.parse(text) as { code?: unknown }).code;
    return typeof code === "string" ? code : null;
  } catch {
    return null;
  }
}

/** A count with its noun, from a plural message (`{count} tracks`). */
export const count = (key: MessageKey, n: number) => t(key, { count: n });
