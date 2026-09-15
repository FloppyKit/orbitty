// Orbitty host theme packs.
//
// Law: never invent primary palettes for the System pair. macOS values are
// the Tahoe semantic table (live aqua dump on Tahoe 26.5 + documented dark
// semantics — the CLI dark dump never resolved, so dark trusts the docs).
// Brand lock (2026-08-29) overrides that law for
// accent / onAccent / mark / focus ONLY: the System pair wears the Orbitty
// brand triad (planet teal accent, ochre mark, violet focus). bg / panel /
// ink / dim / edge stay the Tahoe table.
// Habitat brand lock (2026-08-29) splits the purple: focus is now
// the deeper violet #6e4db0 (selected/hover outline), and the pair gains
// bloom #c793e6 (pinky — words/tags/secondary chips, never the ring) plus
// field #0b1020 (dark habitat pane body; Light keeps Tahoe chrome).
// S4.2 retired Clear: translucency is now the windowOpacity slider fading
// the theme --bg fill, never an NSVisualEffect material.
//
// Linux rows are RESERVED design-time fallbacks (no window module yet),
// keyed by XDG_CURRENT_DESKTOP later: Adwaita / Yaru / Breeze only.
// No WhiteSur, no McMojave, no KWin, never macOS hex on Linux.
// Custom skins (Terminal only for now) stay secondary and live in CSS.

export interface HostTokens {
  bg: string;
  panel: string;
  ink: string;
  dim: string;
  edge: string;
  accent: string;
  onAccent: string;
  mark: string;
  focus: string;
  bloom: string;
  field: string;
  colorScheme: "light" | "dark";
}

export type SystemThemeId = "systemLight" | "systemDark";

// macOS Tahoe semantic colors:
//   windowBackground · controlBackground · label · secondaryLabel ·
//   separator — accent/mark/focus are the Orbitty brand triad (lock
//   2026-08-29), not controlAccent/systemBlue.
export const MAC_SYSTEM_TOKENS: Record<SystemThemeId, HostTokens> = {
  systemLight: {
    bg: "#ECECEC",
    panel: "#FFFFFF",
    ink: "rgba(0, 0, 0, 0.85)",
    dim: "rgba(0, 0, 0, 0.50)",
    edge: "rgba(0, 0, 0, 0.10)",
    accent: "#3d8f8a",
    onAccent: "#ffffff",
    mark: "#e59e3d",
    focus: "#6e4db0",
    bloom: "#c793e6",
    field: "#0b1020",
    colorScheme: "light",
  },
  systemDark: {
    bg: "#1E1E1E",
    panel: "#2C2C2E",
    ink: "rgba(255, 255, 255, 0.85)",
    dim: "rgba(235, 235, 245, 0.60)",
    edge: "rgba(84, 84, 88, 0.65)",
    accent: "#3d8f8a",
    onAccent: "#ffffff",
    mark: "#e59e3d",
    focus: "#6e4db0",
    bloom: "#c793e6",
    field: "#0b1020",
    colorScheme: "dark",
  },
};

// Tokens for a System-block theme id, or null for Custom themes (those are
// pure CSS via :root[data-theme] blocks in +page.svelte).
export function systemTokens(id: string): HostTokens | null {
  return (MAC_SYSTEM_TOKENS as Record<string, HostTokens>)[id] ?? null;
}

// ---------------------------------------------------------------------------
// RESERVED — Linux DE defaults (not this slice).
// Design-time hex fallback only; at implement time prefer semantic sources
// (libadwaita CSS variables, KColorScheme, portal accent-color) and read the
// installed theme rather than baking more hex. Detection order:
//   1. XDG_CURRENT_DESKTOP → pack below
//   2. portal org.freedesktop.appearance accent-color when present
//   3. fallback: Adwaita (widest GTK/libadwaita footprint)
// ---------------------------------------------------------------------------

export interface LinuxDePack {
  lightBg: string;
  darkBg: string;
  accent: string;
  source: string;
}

export const LINUX_DE_PACKS: Record<string, LinuxDePack> = {
  // GNOME / unknown GTK — libadwaita css-variables
  // (window #FAFAFB / #222226, view #FFFFFF / #1D1D20, accent Blue).
  adwaita: {
    lightBg: "#FAFAFB",
    darkBg: "#222226",
    accent: "#3584E4",
    source: "libadwaita css-variables",
  },
  // Ubuntu — Yaru (light #FAFAFA, dark ≈ jet lighten 8% #2C2C2C, orange).
  yaru: {
    lightBg: "#FAFAFA",
    darkBg: "#2C2C2C",
    accent: "#E95420",
    source: "ubuntu/yaru accent-colors.scss",
  },
  // KDE Plasma — Breeze Light/Dark .colors
  // (window #EFF0F1 / #202326, view #FFFFFF / #141618).
  breeze: {
    lightBg: "#EFF0F1",
    darkBg: "#202326",
    accent: "#3DAEE9",
    source: "KDE/breeze colors",
  },
};
