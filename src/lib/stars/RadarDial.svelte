<script lang="ts">
  import type { StarsCheckin } from "../persist";
  import {
    RADAR_VB,
    labelPlacement,
    radarFillPolys,
    radarRing,
    radarVertex,
    scoreFromGrab,
    stepSpoke,
    valueFromPointer,
  } from "./radarGeom";

  type Spoke = { key: string; label: string };

  let {
    spokes,
    shown = null,
    editable = false,
    avgMode = false,
    plated = false,
    activeId = null,
    onSet,
  }: {
    spokes: readonly Spoke[];
    shown?: StarsCheckin | null;
    editable?: boolean;
    avgMode?: boolean;
    plated?: boolean;
    activeId?: string | null;
    onSet: (key: string, value: number) => void;
  } = $props();

  let dialEl: HTMLDivElement | null = $state(null);
  let svgEl: SVGSVGElement | null = $state(null);
  let editKey = $state<string | null>(null);
  let draftNum = $state("");
  let editEl = $state<HTMLInputElement | null>(null);
  let editAt = $state("");
  let editOpenedAt = 0;
  type Hit = { left: number; top: number; w: number; h: number };
  let hits = $state<Record<string, Hit>>({});

  const editIndex = $derived(spokes.findIndex((s) => s.key === editKey));

  function spokeValue(key: string): number {
    if (!shown) return 0;
    const v = shown[key as keyof StarsCheckin];
    return typeof v === "number" ? v : 0;
  }

  function numberText(key: string): string {
    const v = spokeValue(key);
    if (!shown || v <= 0) return "—";
    return avgMode ? v.toFixed(1) : String(Math.round(v));
  }

  const anySet = $derived(spokes.some((s) => spokeValue(s.key) >= 1));
  const fillPolys = $derived(
    radarFillPolys(spokes.map((s) => spokeValue(s.key))),
  );

  function svgPoint(e: PointerEvent): { x: number; y: number } | null {
    const svg = svgEl;
    if (!svg) return null;
    const pt = svg.createSVGPoint();
    pt.x = e.clientX;
    pt.y = e.clientY;
    const ctm = svg.getScreenCTM();
    if (!ctm) return null;
    const p = pt.matrixTransform(ctm.inverse());
    return { x: p.x, y: p.y };
  }

  function startDrag(e: PointerEvent, i: number) {
    if (!editable) return;
    e.preventDefault();
    e.stopPropagation();
    const key = spokes[i]?.key;
    if (!key) return;
    const unset = spokeValue(key) < 1;
    let moved = false;
    const x0 = e.clientX;
    const y0 = e.clientY;
    (e.currentTarget as Element).setPointerCapture?.(e.pointerId);
    const apply = (ev: PointerEvent) => {
      if (!moved && Math.hypot(ev.clientX - x0, ev.clientY - y0) >= 5) {
        moved = true;
      }
      const p = svgPoint(ev);
      const pointer = p ? valueFromPointer(i, p.x, p.y) : 0;
      const score = scoreFromGrab(pointer, unset, moved);
      if (score != null) onSet(key, score);
    };
    const up = (ev: PointerEvent) => {
      apply(ev);
      window.removeEventListener("pointermove", apply);
      window.removeEventListener("pointerup", up);
    };
    apply(e);
    window.addEventListener("pointermove", apply);
    window.addEventListener("pointerup", up);
  }

  function fallbackEditStyle(i: number): string {
    const p = labelPlacement(i);
    const left = ((p.x - RADAR_VB.x) / RADAR_VB.w) * 100;
    const top = ((p.y - RADAR_VB.y) / RADAR_VB.h) * 100;
    if (p.anchor === "end") {
      return `left:${left}%;top:${top}%;transform:translate(-100%,-130%)`;
    }
    if (p.anchor === "start") {
      return `left:${left}%;top:${top}%;transform:translate(0,-130%)`;
    }
    return `left:${left}%;top:${top}%;transform:translate(-50%,-150%)`;
  }

  function measureHits() {
    const root = dialEl;
    if (!root) return;
    const host = root.getBoundingClientRect();
    const next: Record<string, Hit> = {};
    for (const spoke of spokes) {
      const el = root.querySelector(`[data-num="${spoke.key}"]`);
      if (!(el instanceof Element)) continue;
      const b = el.getBoundingClientRect();
      if (b.width < 1 && b.height < 1) continue;
      const pad = 6;
      const w = Math.max(24, b.width + pad * 2);
      const h = Math.max(22, b.height + pad * 2);
      next[spoke.key] = {
        left: b.left - host.left + b.width / 2 - w / 2,
        top: b.top - host.top + b.height / 2 - h / 2,
        w,
        h,
      };
    }
    hits = next;
  }

  function openNum(key: string) {
    if (!editable || avgMode) return;
    editOpenedAt = performance.now();
    const hit = hits[key];
    editAt = hit
      ? `left:${hit.left}px;top:${hit.top}px`
      : fallbackEditStyle(spokes.findIndex((s) => s.key === key));
    editKey = key;
    const v = spokeValue(key);
    draftNum = v >= 1 ? String(Math.round(v)) : "";
  }

  // Empty + Enter clears the spoke back to unset. Escape cancels and keeps it.
  function commitNum() {
    if (!editKey) return;
    const raw = draftNum.trim();
    const key = editKey;
    editKey = null;
    if (raw === "") {
      onSet(key, 0);
      return;
    }
    const n = parseInt(raw, 10);
    if (n >= 1 && n <= 5) onSet(key, n);
  }

  function pickNum(n: number) {
    if (!editKey) return;
    const key = editKey;
    editKey = null;
    onSet(key, n);
  }

  $effect(() => {
    if (!editKey) return;
    editEl?.focus();
    editEl?.select();
  });

  $effect(() => {
    const root = dialEl;
    if (!root) return;
    for (const s of spokes) numberText(s.key);
    editKey;
    editable;
    avgMode;
    const ro = new ResizeObserver(() => measureHits());
    ro.observe(root);
    const id = requestAnimationFrame(() => measureHits());
    return () => {
      cancelAnimationFrame(id);
      ro.disconnect();
    };
  });
</script>

<div class="dial" bind:this={dialEl}>
  <svg
    bind:this={svgEl}
    viewBox="{RADAR_VB.x} {RADAR_VB.y} {RADAR_VB.w} {RADAR_VB.h}"
    role="img"
    aria-label={avgMode ? "STARS average radar" : "STARS radar"}
  >
    <defs>
      <radialGradient
        id="stars-radar-plate"
        cx="0"
        cy="0"
        r="96"
        gradientUnits="userSpaceOnUse"
      >
        <stop offset="0" stop-color="currentColor" stop-opacity="0.14" />
        <stop offset="0.55" stop-color="currentColor" stop-opacity="0.06" />
        <stop offset="1" stop-color="currentColor" stop-opacity="0" />
      </radialGradient>
    </defs>
    <ellipse class="ground" cx="0" cy="66" rx="78" ry="12" />
    <circle class="vignette" cx="0" cy="0" r="96" fill="url(#stars-radar-plate)" />
    {#each fillPolys as poly (poly)}
      <polygon class="shape" points={poly} />
    {/each}
    {#each [1, 2, 3, 4, 5] as ring (ring)}
      <polygon
        class="grid"
        class:outer={ring === 5}
        class:empty-outer={ring === 5 && !anySet}
        points={radarRing(ring)}
      />
    {/each}
    {#each spokes as spoke, i (spoke.key)}
      {@const [ax, ay] = radarVertex(i, 5)}
      <line class="axis" x1="0" y1="0" x2={ax} y2={ay} />
      <line
        class="hit"
        class:locked={!editable}
        role="slider"
        tabindex={editable ? 0 : -1}
        aria-label="{spoke.label} {numberText(spoke.key)}"
        aria-valuemin="1"
        aria-valuemax="5"
        aria-valuenow={Math.max(0, Math.round(spokeValue(spoke.key) || 0))}
        aria-disabled={!editable}
        x1="0"
        y1="0"
        x2={ax}
        y2={ay}
        onpointerdown={(e) => startDrag(e, i)}
        onkeydown={(e) => {
          if (!editable) return;
          if (e.key === "ArrowUp" || e.key === "ArrowRight") {
            e.preventDefault();
            onSet(spoke.key, stepSpoke(spokeValue(spoke.key), 1));
          } else if (e.key === "ArrowDown" || e.key === "ArrowLeft") {
            e.preventDefault();
            onSet(spoke.key, stepSpoke(spokeValue(spoke.key), -1));
          }
        }}
      />
    {/each}
    {#if !anySet}
      <circle class="seed" cx="0" cy="0" r="2.2" />
    {/if}
    {#if plated}
      {#each fillPolys as poly (poly)}
        <polygon class="edge" points={poly} />
      {/each}
    {/if}
    {#each spokes as spoke, i (spoke.key)}
      {@const v = spokeValue(spoke.key)}
      {@const ghost = v < 1}
      {@const [x, y] = radarVertex(i, ghost ? 3 : v)}
      <!-- Hit radius is the grab target. Ring and core stay a few pixels. -->
      <g class="knob" class:ghost class:locked={!editable}>
        <circle
          class="knob-hit"
          cx={x}
          cy={y}
          r="16"
          onpointerdown={(e) => startDrag(e, i)}
        />
        <circle class="knob-ring" cx={x} cy={y} r="3.5" />
        <circle class="knob-core" cx={x} cy={y} r="2.5" />
      </g>
    {/each}
    {#each spokes as spoke, i (spoke.key)}
      {@const label = labelPlacement(i)}
      {@const scored = spokeValue(spoke.key) >= 1}
      <text
        class="tag"
        class:live={editable && !avgMode}
        class:on={activeId === `spoke-${spoke.key}`}
        data-mkey={`spoke-${spoke.key}`}
        x={label.x}
        y={label.y}
        text-anchor={label.anchor}
        dominant-baseline="middle"
        onpointerdown={(e) => {
          if (!editable || avgMode) return;
          e.preventDefault();
          e.stopPropagation();
          openNum(spoke.key);
        }}
      >
        <tspan class="word">{spoke.label}</tspan>
        {#if editKey !== spoke.key}
          <tspan
            class="dot"
            class:scored
            dx="5"
            data-num={scored ? undefined : spoke.key}
          >{scored ? "●" : "○"}</tspan>
          {#if scored}
            <tspan class="num" class:live={editable && !avgMode} data-num={spoke.key} dx="3"
              >{numberText(spoke.key)}</tspan>
          {/if}
        {/if}
      </text>
    {/each}
  </svg>
  {#each spokes as spoke (spoke.key)}
    {#if editable && !avgMode && editKey !== spoke.key && hits[spoke.key]}
      <button
        type="button"
        class="num-hit"
        style="left:{hits[spoke.key].left}px;top:{hits[spoke.key].top}px;width:{hits[spoke.key].w}px;height:{hits[spoke.key].h}px"
        aria-label="Set {spoke.label}"
        onpointerdown={(e) => {
          e.preventDefault();
          e.stopPropagation();
          openNum(spoke.key);
        }}
      ></button>
    {/if}
  {/each}
  {#if editKey && editIndex >= 0}
    <div class="edit" style={editAt}>
      <input
        bind:this={editEl}
        class="edit-in"
        inputmode="numeric"
        maxlength="1"
        aria-label="{spokes[editIndex].label} 1 to 5"
        bind:value={draftNum}
        onblur={() => {
          if (performance.now() - editOpenedAt < 250) {
            editEl?.focus();
            return;
          }
          if (editKey) editKey = null;
        }}
        onkeydown={(e) => {
          if (e.key === "Enter") {
            e.preventDefault();
            e.stopPropagation();
            commitNum();
          } else if (e.key === "Escape") {
            e.preventDefault();
            e.stopPropagation();
            editKey = null;
          }
        }}
      />
      <span class="picks" role="group" aria-label="Pick 1 to 5">
        {#each [1, 2, 3, 4, 5] as n (n)}
          <button
            type="button"
            onmousedown={(e) => e.preventDefault()}
            onclick={() => pickNum(n)}
          >{n}</button>
        {/each}
      </span>
    </div>
  {/if}
</div>

<style>
  .dial {
    position: relative;
    flex: 1 1 auto;
    min-width: 0;
    min-height: 180px;
    display: flex;
    align-items: center;
    justify-content: center;
    color: var(--ink);
    transition: transform 180ms ease;
  }

  .dial:hover {
    transform: translateY(-2px);
  }

  svg {
    width: 100%;
    height: 100%;
    display: block;
    touch-action: none;
  }

  .ground {
    fill: rgba(0, 0, 0, 0.4);
    pointer-events: none;
    transform-box: fill-box;
    transform-origin: center;
    transition: transform 180ms ease;
  }

  .dial:hover .ground {
    transform: translateY(6px);
  }

  .vignette {
    pointer-events: none;
  }

  .grid {
    fill: none;
    stroke: color-mix(in srgb, var(--ink) 28%, transparent);
    stroke-width: 1.05;
    pointer-events: none;
  }

  .grid.outer {
    stroke: color-mix(in srgb, var(--ink) 42%, transparent);
    stroke-width: 1.35;
  }

  .grid.outer.empty-outer {
    stroke-dasharray: 2.2 2.6;
  }

  .axis {
    stroke: color-mix(in srgb, var(--ink) 32%, transparent);
    stroke-width: 0.95;
    pointer-events: none;
  }

  .seed {
    fill: rgba(61, 143, 138, 0.4);
    pointer-events: none;
  }

  .hit {
    stroke: transparent;
    stroke-width: 22;
    cursor: grab;
  }

  .hit.locked {
    pointer-events: none;
  }

  .shape {
    fill: rgba(61, 143, 138, 0.3);
    stroke: none;
    pointer-events: none;
  }

  .edge {
    fill: none;
    stroke: #c8a45c;
    stroke-width: 1.55;
    stroke-opacity: 0.9;
    stroke-linejoin: round;
    pointer-events: none;
  }

  .knob.locked {
    pointer-events: none;
  }

  .knob-hit {
    fill: transparent;
    cursor: grab;
  }

  .knob-ring {
    fill: none;
    stroke: color-mix(in srgb, var(--ink) 42%, transparent);
    stroke-width: 0.65;
    pointer-events: none;
  }

  .knob-core {
    fill: color-mix(in srgb, var(--ink) 78%, transparent);
    pointer-events: none;
  }

  .knob.ghost .knob-ring {
    stroke: color-mix(in srgb, var(--ink) 46%, transparent);
    stroke-width: 0.7;
  }

  .knob.ghost .knob-core {
    fill: color-mix(in srgb, var(--ink) 16%, transparent);
  }

  .knob.ghost:hover .knob-ring {
    stroke: color-mix(in srgb, var(--ink) 48%, transparent);
  }

  .knob.ghost:hover .knob-core {
    fill: color-mix(in srgb, var(--ink) 40%, transparent);
  }

  .tag {
    font-family: inherit;
    font-size: 10px;
    pointer-events: none;
  }

  .tag.live {
    pointer-events: all;
    cursor: text;
  }

  .word {
    fill: var(--ink);
    font-weight: 600;
  }

  .dot {
    font-size: 13px;
    fill: rgba(232, 236, 248, 0.55);
  }

  .dot.scored {
    fill: var(--gold, #c8a45c);
  }

  .num {
    fill: var(--gold, #c8a45c);
    font-weight: 700;
    font-variant-numeric: tabular-nums;
  }

  .num.live {
    cursor: text;
  }

  .num-hit {
    position: absolute;
    z-index: 2;
    margin: 0;
    padding: 0;
    border: 0;
    background: transparent;
    cursor: text;
  }

  .edit {
    position: absolute;
    z-index: 3;
    display: flex;
    align-items: center;
    gap: 3px;
    padding: 2px;
    border: 1px solid var(--edge);
    border-radius: 8px;
    background: var(--panel-fill, var(--panel));
    box-shadow: 0 4px 14px rgba(0, 0, 0, 0.18);
  }

  .edit-in {
    width: 1.6rem;
    font: inherit;
    font-size: 0.75rem;
    font-weight: 700;
    text-align: center;
    color: var(--ink);
    background: var(--bg-fill, var(--bg));
    border: 1px solid var(--edge);
    border-radius: 5px;
    padding: 1px 0;
  }

  .picks {
    display: inline-flex;
    gap: 2px;
  }

  .picks button {
    font: inherit;
    font-size: 0.64rem;
    width: 16px;
    height: 16px;
    padding: 0;
    color: var(--ink);
    background: var(--bg-fill, var(--bg));
    border: 1px solid var(--edge);
    border-radius: 4px;
    cursor: pointer;
  }

  .tag.on .word {
    fill: var(--accent);
  }

  @media (prefers-reduced-motion: reduce) {
    .dial,
    .ground {
      transition: none;
    }

    .dial:hover,
    .dial:hover .ground {
      transform: none;
    }
  }
</style>
