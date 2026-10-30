/**
 * Microtags on standard NIP-24 `t` tags.
 *
 * The only standard is NIP-24: a `t` value must be lowercase. `namespace:value`
 * (planet, kind, area) is Orbitty's proposal, not a NIP. See docs/MICROTAGS.md.
 *
 * ASCII only. `unicode-normalization` is not in Cargo.lock, so any non-ASCII
 * input is refused. That matches the sealed default (no NFKC).
 */

export const RESERVED = ["orbitty", "local-only", "root", "root-revision"] as const;
export const NAMESPACES = ["planet", "kind", "area"] as const;
export const KIND_VALUES = ["bug", "feature", "chore", "question"] as const;

export function normalize(raw: string): string | null {
  if (!isAscii(raw)) return null;
  let s = raw.toLowerCase().trim();
  if (s.startsWith("#")) s = s.slice(1);
  let out = "";
  for (const c of s) {
    out += c === " " || c === "_" ? "-" : c;
  }
  while (out.includes("--")) out = out.replaceAll("--", "-");
  if (out.length < 1 || out.length > 64) return null;
  if (out.startsWith("-") || out.endsWith("-")) return null;
  let colons = 0;
  for (const c of out) {
    if (c === ":") {
      colons += 1;
      if (colons > 1) return null;
      continue;
    }
    const ok =
      (c >= "a" && c <= "z") || (c >= "0" && c <= "9") || c === "-";
    if (!ok) return null;
  }
  const colon = out.indexOf(":");
  if (colon !== -1) {
    const ns = out.slice(0, colon);
    const val = out.slice(colon + 1);
    if (ns.length === 0 || val.length === 0) return null;
    if (edged(ns) || edged(val)) return null;
    if (ns === "status") return null;
    if (ns === "kind" && !KIND_VALUES.includes(val as (typeof KIND_VALUES)[number])) {
      return null;
    }
  }
  if (RESERVED.includes(out as (typeof RESERVED)[number])) return null;
  return out;
}

/** Project id, not the display name. A rename does not move the slug. */
export function planetSlug(projectId: string, displayName: string): string | null {
  void displayName;
  return normalize(projectId);
}

export function toTTags(tags: string[], planetSlugValue?: string): [string, string][] {
  const out: [string, string][] = [];
  const seen = new Set<string>();
  for (const raw of tags) {
    const n = normalize(raw);
    if (n === null) continue;
    if (n.startsWith("planet:")) continue;
    if (seen.has(n)) continue;
    seen.add(n);
    out.push(["t", n]);
  }
  if (planetSlugValue !== undefined) {
    const tag = planetTag(planetSlugValue);
    if (tag !== null) out.push(["t", tag]);
  }
  return out;
}

/**
 * Flag off: tags are unchanged and nothing is refused (the event stays
 * byte-identical). Flag on: microtags are inserted immediately after
 * `t=orbitty`. Returns the raw card tags that normalize refused.
 */
export function applyBoardMicrotags(
  tags: [string, string][],
  flagOn: boolean,
  names: string[],
  projectId: string,
  displayName: string,
): string[] {
  if (!flagOn) return [];
  const slug = planetSlug(projectId, displayName);
  const extra = toTTags(names, slug === null ? undefined : slug);
  const refused = names.filter((n) => normalize(n) === null);
  const orbitty = tags.findIndex((t) => t[0] === "t" && t[1] === "orbitty");
  const at = orbitty === -1 ? 0 : orbitty + 1;
  tags.splice(at, 0, ...extra);
  return refused;
}

/** Failed values stay the original display string and are never microtags. */
export function inbound(raw: string): { microtag: string | null; display: string } {
  return { microtag: normalize(raw), display: raw };
}

function isAscii(raw: string): boolean {
  for (let i = 0; i < raw.length; i++) {
    if (raw.charCodeAt(i) > 0x7f) return false;
  }
  return true;
}

function edged(s: string): boolean {
  return s.length === 0 || s.startsWith("-") || s.endsWith("-");
}

function planetTag(slug: string): string | null {
  const n = normalize(slug);
  if (n === null) return null;
  if (n.startsWith("planet:")) return n;
  return normalize(`planet:${n}`);
}
