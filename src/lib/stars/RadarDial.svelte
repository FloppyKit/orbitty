<script lang="ts">
  import type { StarsCheckin } from "../persist";
  import {
    RADAR_R,
    RADAR_VB,
    handleCenterRadius,
    handleHitRadius,
    handleVisibleRadius,
    labelPlacement,
    spokeAngle,
    parseRating,
    radarFillPolys,
    radarRing,
    radarVertex,
    ratingFromKey,
    snapOnSpoke,
    spokeDistance,
  } from "./radarGeom";

  type Spoke = { key: string; label: string };

  let {
    spokes,
    shown = null,
    editable = false,
    avgMode = false,
    draft = false,
    activeId = null,
    onSet,
  }: {
    spokes: readonly Spoke[];
    shown?: StarsCheckin | null;
    editable?: boolean;
    avgMode?: boolean;
    draft?: boolean;
    activeId?: string | null;
    onSet: (key: string, value: number) => void;
  } = $props();

  let dialEl: HTMLDivElement | null = $state(null);
  let svgEl: SVGSVGElement | null = $state(null);
  let editKey = $state<string | null>(null);
  let draftNum = $state("");
  let editEl: HTMLInputElement | null = $state(null);
  let svgScale = $state(1);
  let hotKey = $state<string | null>(null);
  let dragging = $state(false);

  const hitR = $derived(handleHitRadius(svgScale));
  const visR = $derived(handleVisibleRadius(svgScale));

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

  function valueText(spoke: Spoke): string {
    return `${spoke.label} ${numberText(spoke.key)} of 5`;
  }

  const fillPolys = $derived(radarFillPolys(spokes.map((s) => spokeValue(s.key))));

  function measureScale() {
    const svg = svgEl;
    if (!svg) return;
    const w = svg.getBoundingClientRect().width;
    svgScale = w > 0 ? w / RADAR_VB.w : 1;
  }

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
    if (!editable || avgMode) return;
    e.preventDefault();
    e.stopPropagation();
    const key = spokes[i]?.key;
    if (!key) return;
    dragging = true;
    hotKey = key;
    (e.currentTarget as Element).setPointerCapture?.(e.pointerId);
    const apply = (ev: PointerEvent) => {
      const p = svgPoint(ev);
      if (!p) return;
      onSet(key, snapOnSpoke(spokeDistance(i, p.x, p.y), RADAR_R));
    };
    const up = (ev: PointerEvent) => {
      apply(ev);
      dragging = false;
      window.removeEventListener("pointermove", apply);
      window.removeEventListener("pointerup", up);
    };
    apply(e);
    window.addEventListener("pointermove", apply);
    window.addEventListener("pointerup", up);
  }

  function onHandleKey(e: KeyboardEvent, key: string) {
    if (!editable || avgMode) return;
    const next = ratingFromKey(spokeValue(key), e.key);
    if (next == null) return;
    e.preventDefault();
    if (next === 0) {
      if (draft) onSet(key, 0);
      return;
    }
    onSet(key, next);
  }

  function badgeStyle(i: number): string {
    const p = labelPlacement(i);
    const left = ((p.x - RADAR_VB.x) / RADAR_VB.w) * 100;
    const top = ((p.y - RADAR_VB.y) / RADAR_VB.h) * 100;
    return `left:${left}%;top:${top}%`;
  }

  function openNum(key: string) {
    if (!editable || avgMode) return;
    editKey = key;
    const v = spokeValue(key);
    draftNum = v >= 1 ? String(Math.round(v)) : "";
  }

  function commitNum() {
    if (!editKey) return;
    const key = editKey;
    const n = parseRating(draftNum);
    editKey = null;
    if (n != null) onSet(key, n);
  }

  function cancelNum() {
    editKey = null;
  }

  $effect(() => {
    if (!editKey) return;
    editEl?.focus();
    editEl?.select();
  });

  $effect(() => {
    const root = dialEl;
    if (!root) return;
    const ro = new ResizeObserver(() => measureScale());
    ro.observe(root);
    measureScale();
    return () => ro.disconnect();
  });

  function knobR(key: string): number {
    const hot = hotKey === key || dragging;
    return hot ? Math.max(9, visR + 2) : visR;
  }

  function center(i: number, key: string): [number, number] {
    const v = spokeValue(key);
    const r = handleCenterRadius(v >= 1 ? v : 1, visR);
    const ang = spokeAngle(i);
    return [Math.cos(ang) * r, Math.sin(ang) * r];
  }
</script>

<div class="plate" class:draft bind:this={dialEl}>
  {#if draft}<span class="draft-flag">Draft</span>{/if}
  <svg
    bind:this={svgEl}
    viewBox="{RADAR_VB.x} {RADAR_VB.y} {RADAR_VB.w} {RADAR_VB.h}"
    role="group"
    aria-label={avgMode ? "STARS average radar" : "STARS radar"}
  >
    <defs>
      <filter id="stars-spoke-glow" x="-40%" y="-40%" width="180%" height="180%">
        <feGaussianBlur stdDeviation="2.2" result="blur" />
        <feMerge>
          <feMergeNode in="blur" />
          <feMergeNode in="SourceGraphic" />
        </feMerge>
      </filter>
    </defs>
    <rect
      class="plate-fill"
      x={RADAR_VB.x}
      y={RADAR_VB.y}
      width={RADAR_VB.w}
      height={RADAR_VB.h}
      rx="18"
    />
    {#each fillPolys as poly (poly)}
      <polygon class="shape" class:draft class:avg={avgMode} points={poly} />
    {/each}
    {#each [1, 2, 3, 4, 5] as ring (ring)}
      <polygon class="ring" class:avg={ring === 3} points={radarRing(ring)} />
    {/each}
    <text class="avg-label" x={radarVertex(1, 3)[0] + 6} y={radarVertex(1, 3)[1]}>avg</text>
    {#each spokes as spoke, i (spoke.key)}
      {@const [ax, ay] = radarVertex(i, 5)}
      <line class="axis" x1="0" y1="0" x2={ax} y2={ay} />
    {/each}
    {#if !avgMode}
      {#each spokes as spoke, i (spoke.key)}
        {@const v = spokeValue(spoke.key)}
        {@const [x, y] = center(i, spoke.key)}
        {@const now = v >= 1 ? Math.round(v) : 0}
        <g
          class="handle"
          class:on={activeId === `spoke-${spoke.key}`}
          role="slider"
          tabindex="0"
          aria-label={spoke.label}
          aria-valuemin="1"
          aria-valuemax="5"
          aria-valuenow={now >= 1 ? now : undefined}
          aria-valuetext={valueText(spoke)}
          data-mkey={`spoke-${spoke.key}`}
          onpointerdown={(e) => startDrag(e, i)}
          onpointerenter={() => (hotKey = spoke.key)}
          onpointerleave={() => {
            if (!dragging) hotKey = null;
          }}
          onkeydown={(e) => onHandleKey(e, spoke.key)}
        >
          <circle class="hit" cx={x} cy={y} r={hitR} />
          <circle class="knob" cx={x} cy={y} r={knobR(spoke.key)} opacity={v >= 1 ? 1 : 0} />
        </g>
      {/each}
    {/if}
  </svg>
  {#each spokes as spoke, i (spoke.key)}
    {#if editKey === spoke.key}
      <input
        bind:this={editEl}
        class="num-in"
        type="number"
        min="1"
        max="5"
        style={badgeStyle(i)}
        bind:value={draftNum}
        aria-label={spoke.label}
        onkeydown={(e) => {
          if (e.key === "Enter") {
            e.preventDefault();
            commitNum();
          } else if (e.key === "Escape") {
            e.preventDefault();
            cancelNum();
          }
        }}
        onblur={commitNum}
      />
    {:else}
      <button
        type="button"
        class="badge"
        style={badgeStyle(i)}
        disabled={!editable || avgMode}
        onclick={() => openNum(spoke.key)}
      >
        <span class="word">{spoke.label}</span>
        <span class="val">{numberText(spoke.key)}</span>
      </button>
    {/if}
  {/each}
</div>

<style>
  .plate {
    position: relative;
    border-radius: 18px;
    background: var(--panel-fill);
    box-shadow: inset 0 1px 0 color-mix(in srgb, var(--ink) 16%, transparent);
    min-height: 180px;
  }

  .draft-flag {
    position: absolute;
    top: 8px;
    right: 10px;
    z-index: 1;
    font-size: 0.62rem;
    letter-spacing: 0.04em;
    color: var(--dim);
  }

  svg {
    display: block;
    width: 100%;
    height: auto;
    overflow: visible;
  }

  .plate-fill {
    fill: transparent;
  }

  .ring {
    fill: none;
    stroke: color-mix(in srgb, var(--ink) 14%, transparent);
    stroke-width: 1;
  }

  .ring.avg {
    stroke: color-mix(in srgb, var(--ink) 30%, transparent);
  }

  .axis {
    stroke: color-mix(in srgb, var(--ink) 14%, transparent);
    stroke-width: 1;
  }

  .avg-label {
    fill: var(--dim);
    font-size: 11px;
    font-family: "SF Pro Text", "SF Pro Display", system-ui, sans-serif;
  }

  .shape {
    fill: color-mix(in srgb, var(--accent) 32%, transparent);
    stroke: var(--accent);
    stroke-width: 1.5;
    filter: url(#stars-spoke-glow);
  }

  .shape.draft,
  .shape.avg {
    fill: none;
    stroke-dasharray: 4 3;
  }

  .shape.draft {
    fill: color-mix(in srgb, var(--accent) 18%, transparent);
  }

  .handle {
    cursor: grab;
    outline: none;
  }

  .handle:active {
    cursor: grabbing;
  }

  .hit {
    fill: transparent;
  }

  .knob {
    fill: color-mix(in srgb, var(--ink) 78%, transparent);
    stroke: var(--accent);
    stroke-width: 1.5;
    filter: drop-shadow(0 1px 2px rgba(0, 0, 0, 0.45));
  }

  .handle:focus-visible {
    outline: none;
    box-shadow: var(--focus-ring);
  }

  .handle:focus .knob,
  .handle.on .knob {
    stroke-width: 2.5;
  }

  .badge,
  .num-in {
    position: absolute;
    transform: translate(-50%, -50%);
    font: inherit;
    font-size: 11px;
    font-family: "SF Pro Text", "SF Pro Display", system-ui, sans-serif;
    color: var(--dim);
    background: transparent;
    border: none;
    padding: 0;
    cursor: pointer;
  }

  .badge .val {
    color: var(--ink);
    margin-left: 4px;
    font-variant-numeric: tabular-nums;
  }

  .badge:disabled {
    cursor: default;
  }

  .num-in {
    width: 3rem;
    color: var(--ink);
    background: var(--panel-fill);
    border: 1px solid var(--edge);
    border-radius: 6px;
    padding: 2px 4px;
    z-index: 2;
  }
</style>
