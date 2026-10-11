// The OS the app runs on, as the webview reports it (PLAN.md Phases 8–10),
// for the text and keys that differ between platforms: `t` prefers a
// message's `<key>@<platform>` form, and `keys.ts` writes shortcuts as the
// platform does. macOS has no forms of its own: `en.json`'s plain messages
// are its text.

export type Platform = "macos" | "ios" | "windows" | "linux";

/** The platform from `navigator.platform` and its touch points: an iPad
    asking for desktop pages reports "MacIntel", but has a touch screen. */
export function detect(name: string, touchPoints = 0): Platform {
  if (/iPhone|iPad|iPod/.test(name)) return "ios";
  if (/Mac/.test(name)) return touchPoints > 1 ? "ios" : "macos";
  if (/Win/.test(name)) return "windows";
  return "linux";
}

export const platform: Platform =
  typeof navigator === "undefined" ? "macos" : detect(navigator.platform ?? "", navigator.maxTouchPoints ?? 0);

/** Whether shortcuts are written and pressed with ⌘ (macOS, and an iPad's
    keyboard), rather than Ctrl. */
export const commandKey = platform === "macos" || platform === "ios";
