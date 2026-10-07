// Pure window-chrome decisions shared by Board, Stars and the companion.
// macOS uses native traffic lights; Linux / Windows keep the custom ×.

// @ts-expect-error Node's test runner needs the .ts suffix; the bundler flag for it is off.
import { planetCss } from "./planetColor.ts";

export function chromeMode(ua: string, platform: string): "mac" | "custom" {
  if (/Mac/i.test(platform)) return "mac";
  if (ua.includes("Macintosh")) return "mac";
  return "custom";
}

export function navUa(): string {
  return typeof navigator === "undefined" ? "" : navigator.userAgent;
}

export function navPlatform(): string {
  if (typeof navigator === "undefined") return "";
  const nav = navigator as Navigator & { platform?: string };
  return nav.platform ?? "";
}

/** Today's listTitle: Today / Done / Inbox / planet name / unknown → Inbox. */
export function boardWindowTitle(
  selected: string,
  projects: readonly { id: string; name?: string }[],
): string {
  if (selected === "today") return "Today";
  if (selected === "done") return "Done";
  if (selected === "inbox") return "Inbox";
  const name = projects.find((p) => p.id === selected)?.name;
  if (name && name.length > 0) return name;
  return "Inbox";
}

/** Planet lists only. Today / Done / Inbox / unknown id → null. */
export function boardTitleDot(
  selected: string,
  projects: readonly { id: string }[],
): string | null {
  if (selected === "today" || selected === "done" || selected === "inbox") return null;
  if (!projects.some((p) => p.id === selected)) return null;
  return planetCss(selected);
}
