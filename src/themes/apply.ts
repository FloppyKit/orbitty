// S4.2 theme application shared by the rail window and the Kanban window.
// System themes bind the Tahoe semantic table from host.ts inline at runtime;
// Custom skins stay pure CSS (:root[data-theme] blocks).
//
// S4.3 opacity: WKWebView's color-mix support was unreliable enough that the
// slider did nothing visible, so the fill alpha is computed here in JS —
// applyWindowOpacity reads the resolved --bg/--panel, multiplies the alpha by
// the slider value, and publishes --bg-fill / --panel-fill. Every painted
// chrome fill uses those vars; text/icons/buttons/accent never fade.

import { systemTokens } from "./host.ts";

export const HOST_VARS = [
  "--bg",
  "--panel",
  "--ink",
  "--dim",
  "--edge",
  "--accent",
  "--on-accent",
  "--mark",
  "--focus",
  "--bloom",
  "--field",
] as const;

export const MIN_WINDOW_OPACITY = 0.2;

/** Tile veil. The opacity slider scales this. It does not paint the page root. */
export const GLASS_VEIL_ALPHA = 0.72;

export function veilAlpha(slider: number): number {
  return clampWindowOpacity(slider) * GLASS_VEIL_ALPHA;
}

/** A light slab is every channel at least 230 with alpha at least 0.5. */
export function isLightSlab(r: number, g: number, b: number, a: number): boolean {
  return a >= 0.5 && r >= 230 && g >= 230 && b >= 230;
}

export function clampWindowOpacity(v: number): number {
  if (!Number.isFinite(v) || v <= 0) return 1;
  return Math.min(1, Math.max(MIN_WINDOW_OPACITY, v));
}

export function applyHostTokens(id: string) {
  const el = document.documentElement;
  const t = systemTokens(id);
  if (!t) {
    for (const k of HOST_VARS) el.style.removeProperty(k);
    el.style.removeProperty("color-scheme");
    el.style.removeProperty("accent-color");
    return;
  }
  el.style.setProperty("--bg", t.bg);
  el.style.setProperty("--panel", t.panel);
  el.style.setProperty("--ink", t.ink);
  el.style.setProperty("--dim", t.dim);
  el.style.setProperty("--edge", t.edge);
  el.style.setProperty("--accent", t.accent);
  el.style.setProperty("--on-accent", t.onAccent);
  el.style.setProperty("--mark", t.mark);
  el.style.setProperty("--focus", t.focus);
  el.style.setProperty("--bloom", t.bloom);
  el.style.setProperty("--field", t.field);
  el.style.setProperty("color-scheme", t.colorScheme);
  el.style.setProperty("accent-color", t.accent);
}

// #rgb / #rrggbb / #rrggbbaa / rgb() / rgba() → [r, g, b, a].
function parseColor(raw: string): [number, number, number, number] | null {
  const v = raw.trim();
  if (v.startsWith("#")) {
    const h = v.slice(1);
    if (h.length === 3 || h.length === 4) {
      const r = parseInt(h[0] + h[0], 16);
      const g = parseInt(h[1] + h[1], 16);
      const b = parseInt(h[2] + h[2], 16);
      const a = h.length === 4 ? parseInt(h[3] + h[3], 16) / 255 : 1;
      return [r, g, b, a];
    }
    if (h.length === 6 || h.length === 8) {
      const r = parseInt(h.slice(0, 2), 16);
      const g = parseInt(h.slice(2, 4), 16);
      const b = parseInt(h.slice(4, 6), 16);
      const a = h.length === 8 ? parseInt(h.slice(6, 8), 16) / 255 : 1;
      return [r, g, b, a];
    }
    return null;
  }
  const m = v.match(/rgba?\(\s*([^)]+)\)/);
  if (m) {
    const parts = m[1].split(",").map((s) => parseFloat(s));
    if (parts.length >= 3 && parts.slice(0, 3).every(Number.isFinite)) {
      return [parts[0], parts[1], parts[2], parts[3] ?? 1];
    }
  }
  return null;
}

// Must run after the theme tokens are on :root (it reads the resolved values).
// Publishes:
//   --bg-fill / --panel-fill  theme fill × slider alpha (all chrome fills)
//   --bg-solid                full-strength bg (text-on-accent, readability halo)
//   .low-fill on <html>       below 50% — CSS adds a slight text halo so ink
//                             stays readable with wallpaper bleeding through.
export function applyWindowOpacity(opacity: number) {
  const o = clampWindowOpacity(opacity);
  const el = document.documentElement;
  el.style.setProperty("--win-opacity", String(o));
  const cs = getComputedStyle(el);
  const setFill = (src: string, dest: string, alpha: number) => {
    const parsed = parseColor(cs.getPropertyValue(src));
    if (!parsed) {
      el.style.removeProperty(dest);
      return;
    }
    const [r, g, b, a] = parsed;
    el.style.setProperty(
      dest,
      `rgba(${r}, ${g}, ${b}, ${(a * alpha).toFixed(3)})`,
    );
  };
  setFill("--bg", "--bg-fill", o);
  setFill("--panel", "--panel-fill", veilAlpha(o));
  setFill("--bg", "--bg-solid", 1);
  el.classList.toggle("low-fill", o < 0.5);
}
