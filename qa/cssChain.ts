/** CSS chain + comment stripping for the rail-stage polish guards.
 * Not a test file. Does not import stageArch.
 */

/** Drop // and block comments. Strings and template literals stay intact. */
export function stripComments(src: string): string {
  let out = "";
  let i = 0;
  while (i < src.length) {
    const c = src[i];
    const next = src[i + 1];
    if (c === "/" && next === "/") {
      while (i < src.length && src[i] !== "\n") i++;
      continue;
    }
    if (c === "/" && next === "*") {
      i += 2;
      while (i < src.length && !(src[i] === "*" && src[i + 1] === "/")) i++;
      i = Math.min(src.length, i + 2);
      continue;
    }
    if (c === '"' || c === "'" || c === "`") {
      const end = skipString(src, i);
      out += src.slice(i, end);
      i = end;
      continue;
    }
    out += c;
    i++;
  }
  return out;
}

function skipString(src: string, i: number): number {
  const q = src[i];
  i++;
  while (i < src.length) {
    if (src[i] === "\\") {
      i += 2;
      continue;
    }
    if (q === "`" && src[i] === "$" && src[i + 1] === "{") {
      i += 2;
      let depth = 1;
      while (i < src.length && depth > 0) {
        if (src[i] === '"' || src[i] === "'" || src[i] === "`") {
          i = skipString(src, i);
          continue;
        }
        if (src[i] === "/" && src[i + 1] === "/") {
          while (i < src.length && src[i] !== "\n") i++;
          continue;
        }
        if (src[i] === "/" && src[i + 1] === "*") {
          i += 2;
          while (i < src.length && !(src[i] === "*" && src[i + 1] === "/")) i++;
          i = Math.min(src.length, i + 2);
          continue;
        }
        if (src[i] === "{") depth++;
        else if (src[i] === "}") depth--;
        i++;
      }
      continue;
    }
    if (src[i] === q) return i + 1;
    i++;
  }
  return i;
}

export interface CssLen {
  v: number;
  unit: "px" | "%";
}

export interface CssRule {
  selector: string;
  body: string;
}

export interface IconDecl {
  subject: string;
  left: number;
  top: number;
  centred: boolean;
}

export interface ArchCssChain {
  dx: number;
  dy: number;
  /** True when the positioning rule itself uses translate (hover scale owns transform). */
  visTranslate: boolean;
  icons: IconDecl[];
}

/** Length declaration. A unitless 0 counts as px. Ignores `margin-left` / `border-top`. */
export function cssLength(body: string, prop: string): CssLen | null {
  const re = new RegExp(
    `(?:^|[;\\s])${prop}\\s*:\\s*(-?(?:\\d*\\.\\d+|\\d+))(px|%)?`,
  );
  const m = re.exec(body);
  if (!m) return null;
  return { v: Number(m[1]), unit: (m[2] as "px" | "%" | undefined) ?? "px" };
}

function scanBlockEnd(src: string, open: number): number {
  let depth = 0;
  let quote: string | null = null;
  for (let i = open; i < src.length; i++) {
    const c = src[i];
    if (quote) {
      if (c === "\\") {
        i++;
        continue;
      }
      if (c === quote) quote = null;
      continue;
    }
    if (c === '"' || c === "'") {
      quote = c;
      continue;
    }
    if (c === "{") depth++;
    else if (c === "}") {
      depth--;
      if (depth === 0) return i;
    }
  }
  return src.length;
}

export function cssRuleBlocks(css: string): CssRule[] {
  const src = stripComments(css);
  const blocks: CssRule[] = [];
  let i = 0;
  while (i < src.length) {
    const open = src.indexOf("{", i);
    if (open < 0) break;
    const selector = src.slice(i, open).trim();
    const close = scanBlockEnd(src, open);
    const body = src.slice(open + 1, close);
    if (/^@media\b/.test(selector) || /^@supports\b/.test(selector)) {
      blocks.push(...cssRuleBlocks(body));
    } else if (selector && !selector.startsWith("@")) {
      blocks.push({ selector, body });
    }
    i = close + 1;
  }
  return blocks;
}

function subjects(selector: string): string[] {
  return selector.split(",").map((part) => {
    const bits = part.trim().split(/\s+/);
    return bits[bits.length - 1] ?? "";
  });
}

function toPx(len: CssLen, box: number): number {
  return len.unit === "%" ? (box * len.v) / 100 : len.v;
}

const ICON_SUBJECTS = [".arch-glyph", ".arch-marble-wrap", ".l2-app-icon"] as const;

/**
 * `.arch-hit` left/top plus `.arch-vis` / `.l2-vis` left/top, percentages
 * resolved against the hit box. Icon children are centred on that origin.
 */
export function archCssChain(css: string): ArchCssChain {
  const blocks = cssRuleBlocks(css);
  const hit = blocks.find((b) => {
    const subs = subjects(b.selector);
    return (
      subs.includes(".arch-hit") &&
      !subs.includes(".l2-hit") &&
      cssLength(b.body, "left") &&
      cssLength(b.body, "top") &&
      cssLength(b.body, "width") &&
      cssLength(b.body, "height")
    );
  });
  const vis = blocks.find((b) => {
    const subs = subjects(b.selector);
    return (
      subs.includes(".arch-vis") &&
      subs.includes(".l2-vis") &&
      !b.selector.includes(":hover") &&
      !b.selector.includes(":active") &&
      !b.selector.includes("reduce-motion") &&
      cssLength(b.body, "left") &&
      cssLength(b.body, "top")
    );
  });
  if (!hit || !vis) throw new Error("arch hit/vis positioning rules missing");
  const hw = cssLength(hit.body, "width")!;
  const hh = cssLength(hit.body, "height")!;
  const hl = cssLength(hit.body, "left")!;
  const ht = cssLength(hit.body, "top")!;
  const vl = cssLength(vis.body, "left")!;
  const vt = cssLength(vis.body, "top")!;
  if (hw.unit !== "px" || hh.unit !== "px") throw new Error("hit box size is not px");
  const dx = toPx(hl, hw.v) + toPx(vl, hw.v);
  const dy = toPx(ht, hh.v) + toPx(vt, hh.v);
  const icons: IconDecl[] = ICON_SUBJECTS.map((subject) => {
    const rule = blocks.find((b) => {
      return subjects(b.selector).includes(subject) && cssLength(b.body, "left") && cssLength(b.body, "top");
    });
    if (!rule) throw new Error(`positioned rule missing for ${subject}`);
    const left = cssLength(rule.body, "left")!;
    const top = cssLength(rule.body, "top")!;
    if (left.unit !== "px" || top.unit !== "px") throw new Error(`${subject} left/top not px`);
    const centred = /translate\(\s*-50%\s*,\s*-50%\s*\)/.test(rule.body);
    return { subject, left: left.v, top: top.v, centred };
  });
  return {
    dx,
    dy,
    visTranslate: /translate\s*\(/.test(vis.body),
    icons,
  };
}
