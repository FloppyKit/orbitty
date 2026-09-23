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
  let editKey = $state<string | null>(null);
  let draftNum = $state("");
  let editEl = $state<HTMLInputElement | null>(null);

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

  function openNum(key: string) {
    if (!editable || avgMode) return;
    editKey = key;
    const v = spokeValue(key);
    draftNum = v >= 1 ? String(Math.round(v)) : "";
  }

  function commitNum() {
    if (!editKey) return;
    const n = parseInt(draftNum.trim(), 10);
    const key = editKey;
    editKey = null;
    if (n >= 1 && n <= 5) onSet(key, n);
  }

  function pickNum(n: number) {
    if (!editKey) return;
    const key = editKey;
    editKey = null;
    onSet(key, n);
  }

  function editStyle(i: number): string {
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

  $effect(() => {
    if (!editKey) return;
    editEl?.focus();
    editEl?.select();
  });
</script>

<div class="dial">
  <svg
    bind:this={svgEl}
    viewBox="{RADAR_VB.x} {RADAR_VB.y} {RADAR_VB.w} {RADAR_VB.h}"
    role="img"
    aria-label={avgMode ? "STARS average radar" : "STARS radar"}
  >
    <polygon class="grid" points={radarRing(5)} />
    <polygon class="grid ring1" points={radarRing(1)} />
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
          const cur = spokeValue(spoke.key);
          const base = cur >= 1 ? Math.round(cur) : 0;
          if (e.key === "ArrowUp" || e.key === "ArrowRight") {
            e.preventDefault();
            onSet(spoke.key, Math.min(5, base + 1));
          } else if (e.key === "ArrowDown" || e.key === "ArrowLeft") {
            e.preventDefault();
            onSet(spoke.key, base <= 1 ? 0 : base - 1);
          }
        }}
      />
    {/each}
    {#if shown}
      <polygon class="shape" points={points()} />
    {/if}
    {#each spokes as spoke, i (spoke.key)}
      {@const v = spokeValue(spoke.key)}
      {#if v >= 1}
        {@const [x, y] = radarVertex(i, v)}
        <circle
          class="knob-hit"
          class:locked={!editable}
          cx={x}
          cy={y}
          r="14"
          onpointerdown={(e) => startDrag(e, i)}
        />
        <circle class="knob-ring" cx={x} cy={y} r="9" />
        <circle class="knob-core" cx={x} cy={y} r="4" />
      {/if}
    {/each}
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
        {#if editKey !== spoke.key}
          <tspan
            class="num"
            class:live={editable && !avgMode}
            dx="4"
            onclick={(e) => {
              e.stopPropagation();
              openNum(spoke.key);
            }}
          >{numberText(spoke.key)}</tspan>
        {/if}
      </text>
    {/each}
  </svg>
  {#if editKey && editIndex >= 0}
    <div class="edit" style={editStyle(editIndex)}>
      <input
        bind:this={editEl}
        class="edit-in"
        inputmode="numeric"
        maxlength="1"
        aria-label="{spokes[editIndex].label} 1 to 5"
        bind:value={draftNum}
        onblur={() => {
          if (editKey) editKey = null;
        }}
        onkeydown={(e) => {
          if (e.key === "Enter") {
            e.preventDefault();
            commitNum();
          } else if (e.key === "Escape") {
            e.preventDefault();
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

  .grid.ring1 {
    stroke-dasharray: 2 3;
    opacity: 0.85;
  }

  .axis {
    stroke: var(--edge);
    stroke-width: 1;
    opacity: 0.7;
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
    fill: rgba(61, 184, 176, 0.38);
    stroke: #c8a45c;
    stroke-width: 3;
    stroke-linejoin: round;
    pointer-events: none;
  }

  .knob-hit {
    fill: transparent;
    cursor: grab;
  }

  .knob-hit.locked {
    pointer-events: none;
  }

  .knob-ring {
    fill: var(--bg-fill, #1c1c1e);
    stroke: #e59e3d;
    stroke-width: 2.6;
    pointer-events: none;
  }

  .knob-core {
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

  .num.live {
    pointer-events: all;
    cursor: text;
  }

  .edit {
    position: absolute;
    z-index: 2;
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
</style>
