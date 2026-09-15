// Tiny markdown subset for the companion —
// one parser shared by the expanded transcript bubbles and the collapsed
// CC. HTML is escaped FIRST, then we emit only: <strong> <em> <code>
// <ul>/<ol>/<li> <table>/<thead>/<tbody>/<tr>/<th>/<td> <br>.
// No headings-as-chrome, no images, no raw HTML, no script, no
// navigating links. {@html} must only ever consume this output.

function escapeHtml(s: string): string {
  return s.replace(/&/g, "&amp;").replace(/</g, "&lt;").replace(/>/g, "&gt;");
}

// Inline pass on one already-escaped line/cell: `code` first (its body is
// inert behind a placeholder so * / ** never reach inside), then **bold**,
// then *italic*. Emphasis content may not start/end with whitespace or `*`,
// so "2 * 3 * 4" stays literal.
function inline(s: string): string {
  const codes: string[] = [];
  let out = s.replace(/`([^`\n]+)`/g, (_m, c: string) => {
    codes.push(c);
    return `\u0000${codes.length - 1}\u0000`;
  });
  out = out.replace(/\*\*([^*\s](?:.*?[^*\s])?)\*\*/g, "<strong>$1</strong>");
  out = out.replace(/\*([^*\s](?:[^*]*[^*\s])?)\*/g, "<em>$1</em>");
  out = out.replace(
    /\u0000(\d+)\u0000/g,
    (_m, k: string) => `<code>${codes[Number(k)]}</code>`,
  );
  return out;
}

function splitRow(line: string): string[] {
  let s = line.trim();
  if (s.startsWith("|")) s = s.slice(1);
  if (s.endsWith("|")) s = s.slice(0, -1);
  return s.split("|").map((c) => c.trim());
}

// GFM delimiter row: every cell is dashes with optional alignment colons.
function isTableSep(line: string): boolean {
  if (!line.includes("-")) return false;
  const cells = splitRow(line);
  return cells.length > 0 && cells.every((c) => /^:?-{2,}:?$/.test(c));
}

const UL_ITEM = /^\s*[-*]\s+(.+)$/;
const OL_ITEM = /^\s*\d+[.)]\s+(.+)$/;
const DASH_RULE = /^\s*-{3,}\s*$/;

// The collapsed pager fills the live CC box
// (fitPages below). wordPages remains for a fixed token budget.
// block parse stays exported next to the render.
// mdLite(src) === mdLiteJoin(mdLiteChunks(src)) — byte-identical.
export interface MdLiteChunk {
  html: string;
  block: boolean;
}

export function mdLiteChunks(src: string): MdLiteChunk[] {
  const lines = escapeHtml(src ?? "").split(/\r?\n/);
  const chunks: MdLiteChunk[] = [];
  let para: string[] = [];
  let i = 0;

  const flushPara = () => {
    if (para.length === 0) return;
    chunks.push({ html: para.map(inline).join("<br>"), block: false });
    para = [];
  };

  while (i < lines.length) {
    const line = lines[i];

    if (/^\s*$/.test(line)) {
      flushPara();
      i++;
      continue;
    }

    // A lone dash ruler is a paragraph break — never rendered as typed.
    if (DASH_RULE.test(line)) {
      flushPara();
      i++;
      continue;
    }

    // Simple GFM table: a pipe line whose next line is a delimiter row
    // with the same cell count; body rows run until a non-pipe line.
    if (line.includes("|") && i + 1 < lines.length && isTableSep(lines[i + 1])) {
      const header = splitRow(line);
      if (splitRow(lines[i + 1]).length === header.length) {
        flushPara();
        i += 2;
        const rows: string[][] = [];
        while (i < lines.length && lines[i].includes("|") && !/^\s*$/.test(lines[i])) {
          rows.push(splitRow(lines[i]));
          i++;
        }
        let html = "<table><thead><tr>";
        for (const c of header) html += `<th>${inline(c)}</th>`;
        html += "</tr></thead>";
        if (rows.length > 0) {
          html += "<tbody>";
          for (const r of rows) {
            html += "<tr>";
            for (let k = 0; k < header.length; k++) {
              html += `<td>${inline(r[k] ?? "")}</td>`;
            }
            html += "</tr>";
          }
          html += "</tbody>";
        }
        chunks.push({ html: html + "</table>", block: true });
        continue;
      }
    }

    if (UL_ITEM.test(line)) {
      flushPara();
      let html = "<ul>";
      while (i < lines.length) {
        const m = lines[i].match(UL_ITEM);
        if (!m) break;
        html += `<li>${inline(m[1])}</li>`;
        i++;
      }
      chunks.push({ html: html + "</ul>", block: true });
      continue;
    }

    if (OL_ITEM.test(line)) {
      flushPara();
      let html = "<ol>";
      while (i < lines.length) {
        const m = lines[i].match(OL_ITEM);
        if (!m) break;
        html += `<li>${inline(m[1])}</li>`;
        i++;
      }
      chunks.push({ html: html + "</ol>", block: true });
      continue;
    }

    para.push(line);
    i++;
  }
  flushPara();
  return chunks;
}

// Two text chunks need a visible gap; block chunks break on their own.
export function mdLiteJoin(chunks: MdLiteChunk[]): string {
  let out = "";
  let prevBlock = true;
  for (const c of chunks) {
    if (out && !c.block && !prevBlock) out += "<br><br>";
    out += c.html;
    prevBlock = c.block;
  }
  return out;
}

export function mdLite(src: string): string {
  return mdLiteJoin(mdLiteChunks(src));
}

// Split RAW text into prefixes that `fits`
// accepts. `fits(prefix, hasMore)` is a DOM measure (scrollHeight vs the
// live `.cc.bot` cap). Cuts land on a word boundary; prefer a paragraph
// break then a sentence end in the back half of the fitted window.
export function fitPages(
  src: string,
  fits: (prefix: string, hasMore: boolean) => boolean,
): string[] {
  let rest = (src ?? "").replace(/\r\n/g, "\n").trim();
  const pages: string[] = [];
  while (rest) {
    const end = longestFit(rest, fits);
    if (end <= 0) {
      const tok = firstToken(rest);
      if (!tok) break;
      pages.push(tok);
      rest = rest.slice(tok.length).trim();
      continue;
    }
    const piece = rest.slice(0, end).trim();
    if (!piece) break;
    pages.push(piece);
    rest = rest.slice(end).trim();
  }
  return pages;
}

function firstToken(s: string): string {
  const m = s.match(/\S+/);
  return m ? m[0] : s.slice(0, 1);
}

function longestFit(
  rest: string,
  fits: (prefix: string, hasMore: boolean) => boolean,
): number {
  const toks = Array.from(rest.matchAll(/\S+/g));
  if (toks.length === 0) return 0;
  if (fits(rest, false)) return rest.length;
  let lo = 1;
  let hi = toks.length;
  let best = 0;
  while (lo <= hi) {
    const mid = (lo + hi) >> 1;
    const token = toks[mid - 1];
    const end = (token.index ?? 0) + token[0].length;
    const prefix = rest.slice(0, end);
    if (fits(prefix, mid < toks.length)) {
      best = end;
      lo = mid + 1;
    } else {
      hi = mid - 1;
    }
  }
  if (best <= 0) return 0;
  const prefix = rest.slice(0, best);
  const para = prefix.lastIndexOf("\n\n");
  if (para >= best / 2) return para;
  let sen = -1;
  for (let i = prefix.length - 1; i >= Math.floor(best / 2); i--) {
    if ("!?.".includes(prefix[i])) {
      sen = i + 1;
      break;
    }
  }
  return sen > 0 ? sen : best;
}

// Fixed token-budget split (paragraph, then sentence, then word).
export function wordPages(src: string, budget = 60): string[] {
  const count = (s: string) => s.split(/\s+/).filter(Boolean).length;
  const paras = (src ?? "")
    .replace(/\r\n/g, "\n")
    .split(/\n\s*\n+/)
    .map((p) => p.trim())
    .filter((p) => p.length > 0);
  const pages: string[] = [];
  let cur = "";
  for (const para of paras) {
    const cand = cur ? `${cur}\n\n${para}` : para;
    if (count(cand) <= budget) {
      cur = cand;
      continue;
    }
    if (cur) {
      pages.push(cur);
      cur = "";
    }
    if (count(para) <= budget) {
      cur = para;
      continue;
    }
    const parts = splitLong(para, budget);
    for (let k = 0; k < parts.length - 1; k++) pages.push(parts[k]);
    cur = parts[parts.length - 1] ?? "";
  }
  if (cur) pages.push(cur);
  return pages;
}

function splitLong(text: string, budget: number): string[] {
  const out: string[] = [];
  let rest = text.trim();
  for (;;) {
    const toks = Array.from(rest.matchAll(/\S+/g), (m) => ({
      start: m.index,
      end: m.index + m[0].length,
    }));
    if (toks.length <= budget) break;
    const win = toks.slice(0, budget);
    // The last sentence end (.!?) inside the window wins — but only in
    // the back half; earlier than that the full window makes the fuller
    // page. No sentence end → cut at the window's last word boundary.
    let senIdx = -1;
    for (let k = win.length - 1; k >= 0; k--) {
      if (/[.!?]+["')\]]*$/.test(rest.slice(win[k].start, win[k].end))) {
        senIdx = k;
        break;
      }
    }
    const cutIdx = senIdx >= budget / 2 ? senIdx : win.length - 1;
    const cut = win[cutIdx].end;
    out.push(rest.slice(0, cut).trim());
    rest = rest.slice(cut).trim();
  }
  if (rest) out.push(rest);
  return out.filter((p) => p.length > 0);
}
