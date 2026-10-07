<script lang="ts">
  // The embeddable Cloud field (words + mute +
  // empty), split out of SkyPage so STARS Review can mount it without the
  // Cloud banner/×. Bag law unchanged: done titles + done notes + tag
  // names + the planet/project name (weak, half weight) + user companion
  // lines (role "you" / lastUserLine). Bot replies never enter the bag.
  // Bubble size = presence count (documents containing the word), never
  // minutes or points. Click = co-occurrence highlight (same done / same
  // user turn) — geometry, not a rec. Mute is the only persisted slice
  // (state.sky.muted); tokens are recomputed from a fresh load_state on
  // every open and are never stored. Empty is valid.
  import { invoke } from "@tauri-apps/api/core";
  import { listen } from "@tauri-apps/api/event";
  import { onMount, onDestroy } from "svelte";
  import "./orbit-field.css";
  import type { PersistedState } from "./persist";
  import { buildDocs, pageCloudDocs, type SkyDoc, type SkyRange } from "./skyBag";

  let {
    from = undefined,
    to = undefined,
    compact = false,
    filter = undefined,
    pageBody = undefined,
  }: {
    from?: string;
    to?: string;
    compact?: boolean;
    filter?: (doc: SkyDoc) => boolean;
    pageBody?: string;
  } = $props();

  // One document = one done card (title + notes + tag names + weak planet
  // name) or one user companion line. Presence is per-document: a word
  // counts once per doc (planet-name tokens at half weight).
  interface WordInfo {
    word: string;
    count: number;
    lastAt: number;
  }

  let words = $state<WordInfo[]>([]);
  let cooccur = new Map<string, Set<string>>();
  let muted = $state<string[]>([]);
  let selected = $state<string | null>(null);

  const mutedSet = $derived(new Set(muted));
  const visible = $derived(words.filter((w) => !mutedSet.has(w.word)));
  const maxCount = $derived(visible.reduce((m, w) => Math.max(m, w.count), 0));
  const nearSet = $derived(
    selected ? (cooccur.get(selected) ?? new Set<string>()) : null,
  );

  const FRESH_WINDOW_S = 14 * 86400; // ~14 local days, then a word dims
  function isFresh(w: WordInfo): boolean {
    return Date.now() / 1000 - w.lastAt <= FRESH_WINDOW_S;
  }
  function fontSize(w: WordInfo): number {
    if (maxCount <= 0) return 0.78;
    return Math.min(1.7, 0.74 + Math.sqrt(w.count / maxCount) * 0.95);
  }

  // djb2 — stable per word, so color and drift never jump between opens.
  function hashWord(s: string): number {
    let h = 5381;
    for (let i = 0; i < s.length; i++) h = ((h << 5) + h + s.charCodeAt(i)) | 0;
    return Math.abs(h);
  }

  const SKY_COLORS = ["#e59e3d", "#3d8f8a", "#c793e6"]; // ochre / teal / violet
  function wordColor(word: string): string {
    return SKY_COLORS[hashWord(word) % SKY_COLORS.length];
  }

  // Idle drift: 1–3px amplitude, unique delay/direction/duration per word.
  function driftStyle(word: string): string {
    const h = hashWord(word);
    const dx = (1 + (h % 3)) * (h & 0x8 ? -1 : 1);
    const dy = (1 + ((h >> 4) % 3)) * (h & 0x40 ? -1 : 1);
    const dur = 5.5 + ((h >> 6) % 40) / 10;
    const del = -(((h >> 10) % 90) / 10);
    return `--dx: ${dx}px; --dy: ${dy}px; --dur: ${dur}s; --del: ${del}s;`;
  }

  interface Placement {
    x: number; // % of field width
    y: number; // % of field height
    fs: number; // rem after uniform scale-to-fit
  }
  let fieldEl = $state<HTMLDivElement | null>(null);
  let measureEl = $state<HTMLDivElement | null>(null);
  let fieldSize = $state({ w: 0, h: 0 });
  let placed = $state<Record<string, Placement> | null>(null);

  $effect(() => {
    const el = fieldEl;
    if (!el) return;
    const ro = new ResizeObserver((entries) => {
      const r = entries[0]?.contentRect;
      if (r) fieldSize = { w: r.width, h: r.height };
    });
    ro.observe(el);
    fieldSize = { w: el.clientWidth, h: el.clientHeight };
    return () => ro.disconnect();
  });

  // Greedy Archimedean spiral from the field center, biggest count first,
  // AABB + 6px pad, skipping spiral steps that overlap. If the packed cloud
  // exceeds the field, the whole cloud (glyphs + offsets) scales down
  // uniformly. Recomputes on visible-set change and field resize.
  $effect(() => {
    const list = visible.slice(0, 80);
    const meas = measureEl;
    const fw = fieldSize.w;
    const fh = fieldSize.h;
    if (!meas || fw < 60 || fh < 60 || list.length === 0) {
      placed = null;
      return;
    }
    const nodes = meas.querySelectorAll<HTMLElement>(".m-wrap");
    if (nodes.length < list.length) return;
    const PAD = 6;
    const items: { word: string; w: number; h: number; fs: number }[] = [];
    for (let i = 0; i < list.length; i++) {
      const el = nodes[i];
      items.push({
        word: list[i].word,
        w: el.offsetWidth,
        h: el.offsetHeight,
        fs: fontSize(list[i]),
      });
    }
    const cx = fw / 2;
    const cy = fh / 2;
    const aspect = Math.min(1.4, Math.max(0.55, fh / fw));
    const rects: { x: number; y: number; w: number; h: number }[] = [];
    const spots = new Map<string, { x: number; y: number }>();
    for (const it of items) {
      const bw = it.w + PAD * 2;
      const bh = it.h + PAD * 2;
      let theta = 0;
      let px = cx;
      let py = cy;
      for (let tries = 0; tries < 4000; tries++) {
        const r = 5.5 * theta;
        px = cx + r * Math.cos(theta);
        py = cy + r * Math.sin(theta) * aspect;
        const rx = px - bw / 2;
        const ry = py - bh / 2;
        let hit = false;
        for (const q of rects) {
          if (
            rx < q.x + q.w &&
            rx + bw > q.x &&
            ry < q.y + q.h &&
            ry + bh > q.y
          ) {
            hit = true;
            break;
          }
        }
        if (!hit) break;
        theta += 0.16;
      }
      rects.push({ x: px - bw / 2, y: py - bh / 2, w: bw, h: bh });
      spots.set(it.word, { x: px, y: py });
    }
    let minX = Infinity;
    let minY = Infinity;
    let maxX = -Infinity;
    let maxY = -Infinity;
    for (const r of rects) {
      minX = Math.min(minX, r.x);
      minY = Math.min(minY, r.y);
      maxX = Math.max(maxX, r.x + r.w);
      maxY = Math.max(maxY, r.y + r.h);
    }
    const s = Math.min(
      1,
      (fw - 20) / Math.max(1, maxX - minX),
      (fh - 20) / Math.max(1, maxY - minY),
    );
    const next: Record<string, Placement> = {};
    for (const it of items) {
      const p = spots.get(it.word)!;
      next[it.word] = {
        x: ((cx + (p.x - cx) * s) / fw) * 100,
        y: ((cy + (p.y - cy) * s) / fh) * 100,
        fs: it.fs * s,
      };
    }
    placed = next;
  });

  let loaded = $state<PersistedState | null>(null);

  function buildSky(s: PersistedState) {
    const ranged = !!(from || to);
    const range: SkyRange | null = ranged
      ? { from: from || to || "", to: to || from || "" }
      : null;
    // No range: one-argument buildDocs, same bag the Sky window has always shown.
    let docs = ranged ? buildDocs(s, range, filter) : buildDocs(s);
    if (pageBody && pageBody.trim()) {
      docs = docs.concat(pageCloudDocs({ body: pageBody, dayKey: from }));
    }

    const byWord = new Map<string, WordInfo>();
    const pairs = new Map<string, Set<string>>();
    for (const doc of docs) {
      const present = new Map<string, number>();
      for (const e of doc.entries) {
        present.set(e.token, Math.max(present.get(e.token) ?? 0, e.weight));
      }
      const toks = [...present.keys()];
      for (const [tok, w] of present) {
        const cur = byWord.get(tok);
        if (cur) {
          cur.count += w;
          cur.lastAt = Math.max(cur.lastAt, doc.at);
        } else {
          byWord.set(tok, { word: tok, count: w, lastAt: doc.at });
        }
      }
      for (let i = 0; i < toks.length; i++) {
        for (let j = i + 1; j < toks.length; j++) {
          let a = pairs.get(toks[i]);
          if (!a) pairs.set(toks[i], (a = new Set()));
          a.add(toks[j]);
          let b = pairs.get(toks[j]);
          if (!b) pairs.set(toks[j], (b = new Set()));
          b.add(toks[i]);
        }
      }
    }

    words = [...byWord.values()].sort(
      (a, b) => b.count - a.count || a.word.localeCompare(b.word),
    );
    cooccur = pairs;
    if (selected && !byWord.has(selected)) selected = null;
  }

  async function loadState() {
    try {
      const s = await invoke<PersistedState>("load_state");
      muted = s.sky?.muted ?? [];
      loaded = s;
    } catch {
      // next state:changed / visibility refresh retries
    }
  }

  $effect(() => {
    const s = loaded;
    if (!s) return;
    from;
    to;
    pageBody;
    filter;
    buildSky(s);
  });

  // Mute is the only write: load fresh, touch only state.sky.muted, save
  // the whole object back — every other slice survives byte-for-byte.
  async function muteWord(word: string) {
    try {
      const s = await invoke<PersistedState>("load_state");
      const sky = s.sky ?? { muted: [] };
      if (!sky.muted.includes(word)) sky.muted = [...sky.muted, word];
      s.sky = sky;
      await invoke("save_state", { state: s });
      muted = sky.muted;
      if (selected === word) selected = null;
    } catch {
      // next state:changed restores the truth
    }
  }

  async function unmuteWord(word: string) {
    try {
      const s = await invoke<PersistedState>("load_state");
      const sky = s.sky ?? { muted: [] };
      sky.muted = sky.muted.filter((w) => w !== word);
      s.sky = sky;
      await invoke("save_state", { state: s });
      muted = sky.muted;
    } catch {
      // next state:changed restores the truth
    }
  }

  function clickWord(word: string) {
    selected = selected === word ? null : word;
  }

  // A re-show of whichever window hosts this field reloads from disk and
  // recomputes the sky.
  const onVisibility = () => {
    if (!document.hidden) loadState();
  };

  let unlisten: (() => void) | undefined;
  onMount(async () => {
    await loadState();
    unlisten = await listen("state:changed", loadState);
    document.addEventListener("visibilitychange", onVisibility);
  });
  onDestroy(() => {
    unlisten?.();
    document.removeEventListener("visibilitychange", onVisibility);
  });
</script>

<div class="sky-field" class:compact>
  {#if visible.length > 0}
    <div class="field orbit-field" bind:this={fieldEl} role="list" aria-label="Your words">
      {#each visible.slice(0, 80) as w (w.word)}
        {@const p = placed?.[w.word]}
        <span
          class="word-wrap"
          class:unplaced={!p}
          role="listitem"
          style="font-size: {p ? p.fs : fontSize(w)}rem; left: {p?.x ?? 50}%; top: {p?.y ?? 50}%; --wc: {wordColor(w.word)}; {driftStyle(w.word)}"
        >
          <button
            class="word"
            data-focus-zone={compact ? "Z7" : undefined}
            data-focus-item={compact ? "" : undefined}
            class:dim={!isFresh(w)}
            class:picked={selected === w.word}
            class:near={nearSet !== null && selected !== w.word && nearSet.has(w.word)}
            class:far={nearSet !== null && selected !== w.word && !nearSet.has(w.word)}
            title={selected === null
              ? `${w.word} — shows up ${w.count}×`
              : `${w.word} — click to clear`}
            onclick={() => clickWord(w.word)}
            oncontextmenu={(e) => {
              e.preventDefault();
              muteWord(w.word);
            }}>{w.word}</button
          >
          <button
            class="smash"
            data-focus-zone={compact ? "Z7" : undefined}
            data-focus-item={compact ? "" : undefined}
            title="Mute “{w.word}”"
            aria-label="Mute {w.word}"
            onclick={(e) => {
              e.stopPropagation();
              muteWord(w.word);
            }}>×</button
          >
        </span>
      {/each}
    </div>
    <!-- Natural-size twins the packer measures, so a scaled-down cloud
         never feeds back into its own measurement. -->
    <div class="measure" aria-hidden="true" bind:this={measureEl}>
      {#each visible.slice(0, 80) as w (w.word)}
        <span class="word-wrap m-wrap" style="font-size: {fontSize(w)}rem"
          ><span class="word">{w.word}</span></span
        >
      {/each}
    </div>
    {#if selected}
      <p class="hint">Alongside “{selected}” — click it again to let go.</p>
    {/if}
  {:else if muted.length > 0}
    <p class="empty">Every word is muted right now.</p>
  {:else}
    <p class="empty">
      Nothing in the cloud yet — done titles and what you say to Spark land
      here.
    </p>
  {/if}

  {#if muted.length > 0}
    <footer class="muted-row" aria-label="Muted words">
      <span class="muted-label">muted</span>
      {#each muted as m (m)}
        <button
          class="muted-chip"
          data-focus-zone={compact ? "Z7" : undefined}
          data-focus-item={compact ? "" : undefined}
          title="Bring “{m}” back"
          onclick={() => unmuteWord(m)}>{m}</button
        >
      {/each}
    </footer>
  {/if}
</div>

<style>
  .sky-field {
    flex: 1;
    min-height: 0;
    display: flex;
    flex-direction: column;
    position: relative;
  }

  .sky-field.compact {
    min-height: 120px;
  }

  /* One dark space field: greedy spiral pack, scale-to-fit, no wrap, no
     scroll. The four-layer dust recipe lives in orbit-field.css (the one
     shared definition) — this class only sizes and clips. */
  .field {
    position: relative;
    flex: 1;
    min-height: 0;
    overflow: hidden;
    border-radius: 10px;
  }

  .word-wrap {
    position: relative;
    display: inline-flex;
    align-items: baseline;
  }

  .field .word-wrap {
    position: absolute;
    transform: translate(-50%, -50%);
    animation: sky-drift var(--dur, 7s) ease-in-out var(--del, 0s) infinite
      alternate;
  }

  .word-wrap.unplaced {
    visibility: hidden;
  }

  /* Idle drift: 1–3px, per-word delay/direction from the word hash. */
  @keyframes sky-drift {
    to {
      transform: translate(
        calc(-50% + var(--dx, 0px)),
        calc(-50% + var(--dy, 0px))
      );
    }
  }

  /* Natural-size twins for the packer — never visible, never tabbable. */
  .measure {
    position: absolute;
    left: -99999px;
    top: 0;
    visibility: hidden;
    pointer-events: none;
  }

  .measure .word {
    display: inline-block;
  }

  .word {
    font: inherit;
    font-size: inherit;
    line-height: 1.35;
    padding: 1px 4px;
    color: var(--wc, var(--ink));
    background: transparent;
    border: 1px solid transparent;
    border-radius: 7px;
    cursor: pointer;
    transition: transform 140ms ease-out;
  }

  .word:hover,
  .word:focus-visible {
    transform: scale(1.06);
    border-color: rgb(255 255 255 / 0.18);
    background: rgb(255 255 255 / 0.07);
  }

  /* Old words dim — never red, never a lecture. */
  .word.dim {
    opacity: 0.42;
  }

  .word.picked {
    color: var(--bg);
    background: var(--accent);
    border-color: var(--accent);
    font-weight: 700;
  }

  .word.near {
    color: var(--accent);
    border-color: var(--accent);
    font-weight: 700;
  }

  .word.far {
    opacity: 0.28;
  }

  .smash {
    position: absolute;
    top: -7px;
    right: -7px;
    width: 14px;
    height: 14px;
    padding: 0;
    font: inherit;
    font-size: 0.6rem;
    line-height: 1;
    display: inline-flex;
    align-items: center;
    justify-content: center;
    color: #0b1020;
    background: rgb(232 236 248 / 0.92);
    border: none;
    border-radius: 999px;
    cursor: pointer;
    opacity: 0;
    pointer-events: none;
  }

  .word-wrap:hover .smash,
  .smash:focus-visible {
    opacity: 1;
    pointer-events: auto;
  }

  /* Stars folio (.compact) only. The Sky window keeps .word.picked and .smash above. */
  .sky-field.compact .word.picked {
    background: color-mix(in srgb, var(--accent) 24%, transparent);
    color: var(--ink);
  }

  .sky-field.compact .smash {
    background: var(--panel-fill);
    color: var(--ink);
  }

  .hint {
    margin: 10px 0 0;
    font-size: 0.64rem;
    line-height: 1.5;
    color: var(--dim);
    text-align: center;
  }

  .empty {
    margin: auto;
    padding: 24px 18px;
    font-size: 0.74rem;
    line-height: 1.6;
    color: var(--dim);
    text-align: center;
  }

  .muted-row {
    flex-shrink: 0;
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 4px;
    margin-top: 10px;
    padding-top: 8px;
    border-top: 1px solid var(--edge);
    max-height: 84px;
    overflow-y: auto;
  }

  .muted-label {
    font-size: 0.62rem;
    color: var(--dim);
  }

  .muted-chip {
    font: inherit;
    font-size: 0.64rem;
    padding: 1px 7px;
    color: var(--dim);
    background: transparent;
    border: 1px dashed var(--edge);
    border-radius: 999px;
    cursor: pointer;
  }

  .muted-chip:hover {
    color: var(--ink);
    border-color: var(--accent);
  }

  /* Reduce Motion stills all of it: no drift, no hover scale. */
  @media (prefers-reduced-motion: reduce) {
    .field .word-wrap {
      animation: none;
    }

    .word {
      transition: none;
    }

    .word:hover,
    .word:focus-visible {
      transform: none;
    }
  }
</style>
