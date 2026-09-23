<script lang="ts">
  import type { StarsCheckin } from "../persist";
  import {
    RADAR_VB,
    labelPlacement,
    radarRing,
    radarVertex,
    valueFromPointer,
  } from "./radarGeom";

  type Spoke = { key: string; label: string };

  let {
    spokes,
    shown = null,
    editable = false,
    avgMode = false,
    activeId = null,
    onSet,
  }: {
    spokes: readonly Spoke[];
    shown?: StarsCheckin | null;
    editable?: boolean;
    avgMode?: boolean;
    activeId?: string | null;
    onSet: (key: string, value: number) => void;
  } = $props();

  let svgEl: SVGSVGElement | null = $state(null);

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

  function points(): string {
    if (!shown) return "";
    return spokes
      .map((s, i) => radarVertex(i, spokeValue(s.key)).join(","))
      .join(" ");
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
    if (!editable) return;
    e.preventDefault();
    e.stopPropagation();
    const key = spokes[i]?.key;
    if (!key) return;
    (e.currentTarget as Element).setPointerCapture?.(e.pointerId);
    const apply = (ev: PointerEvent) => {
      const p = svgPoint(ev);
      if (!p) return;
      onSet(key, valueFromPointer(i, p.x, p.y));
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
</script>

<div class="dial">
  <svg
    bind:this={svgEl}
    viewBox="{RADAR_VB.x} {RADAR_VB.y} {RADAR_VB.w} {RADAR_VB.h}"
    role="img"
    aria-label={avgMode ? "STARS average radar" : "STARS radar"}
  >
    <polygon class="grid" points={radarRing(5)} />
    <polygon class="grid inner" points={radarRing(3)} />
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
        aria-valuenow={Math.min(5, Math.max(1, Math.round(spokeValue(spoke.key) || 1)))}
        aria-disabled={!editable}
        x1="0"
        y1="0"
        x2={ax}
        y2={ay}
        onpointerdown={(e) => startDrag(e, i)}
        onkeydown={(e) => {
          if (!editable) return;
          const cur = spokeValue(spoke.key);
          const base = cur >= 1 ? Math.round(cur) : 1;
          if (e.key === "ArrowUp" || e.key === "ArrowRight") {
            e.preventDefault();
            onSet(spoke.key, Math.min(5, base + 1));
          } else if (e.key === "ArrowDown" || e.key === "ArrowLeft") {
            e.preventDefault();
            onSet(spoke.key, Math.max(1, base - 1));
          }
        }}
      />
    {/each}
    {#if shown}
      <polygon class="shape" points={points()} />
      {#each spokes as spoke, i (spoke.key)}
        {@const [x, y] = radarVertex(i, spokeValue(spoke.key))}
        <circle class="dot" cx={x} cy={y} r="3.4" />
      {/each}
    {/if}
    {#each spokes as spoke, i (spoke.key)}
      {@const label = labelPlacement(i)}
      <text
        class="tag"
        class:on={activeId === `spoke-${spoke.key}`}
        data-mkey={`spoke-${spoke.key}`}
        x={label.x}
        y={label.y}
        text-anchor={label.anchor}
        dominant-baseline="middle"
      >
        <tspan class="word">{spoke.label}</tspan>
        <tspan class="num" dx="4">{numberText(spoke.key)}</tspan>
      </text>
    {/each}
  </svg>
</div>

<style>
  .dial {
    flex: 0 0 auto;
    min-width: 0;
  }

  svg {
    width: 100%;
    height: auto;
    display: block;
    touch-action: none;
  }

  .grid {
    fill: none;
    stroke: var(--edge);
    stroke-width: 1.5;
  }

  .grid.inner {
    stroke-dasharray: 3 4;
    opacity: 0.7;
  }

  .axis {
    stroke: var(--edge);
    stroke-width: 1;
    opacity: 0.7;
  }

  .hit {
    stroke: transparent;
    stroke-width: 16;
    cursor: grab;
  }

  .hit.locked {
    pointer-events: none;
  }

  .shape {
    fill: rgba(61, 184, 176, 0.38);
    stroke: #c8a45c;
    stroke-width: 3;
    stroke-linejoin: round;
    pointer-events: none;
  }

  .dot {
    fill: #e59e3d;
    pointer-events: none;
  }

  .tag {
    font-family: inherit;
    font-size: 10px;
    pointer-events: none;
  }

  .word {
    fill: var(--ink);
    font-weight: 600;
  }

  .num {
    fill: #e59e3d;
    font-weight: 700;
    font-variant-numeric: tabular-nums;
  }

  .tag.on .word {
    fill: var(--accent);
  }
</style>
