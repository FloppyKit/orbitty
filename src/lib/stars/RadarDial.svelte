<script lang="ts">
  import type { StarsCheckin } from "../persist";
  import { radarRing, radarVertex, valueFromPointer } from "./radarGeom";

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

  function letter(label: string): string {
    return label.slice(0, 1).toUpperCase();
  }

  function spokeValue(key: string): number {
    if (!shown) return 0;
    const v = shown[key as keyof StarsCheckin];
    return typeof v === "number" ? v : 0;
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
    viewBox="-62 -62 124 124"
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
        aria-label="{spoke.label} 1 to 5"
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
        <circle class="dot" cx={x} cy={y} r="2.8" />
      {/each}
    {/if}
  </svg>

  <div class="letters">
    {#each spokes as spoke (spoke.key)}
      <div
        class="letter-row"
        class:mkey-sel={activeId === `spoke-${spoke.key}`}
        data-mkey={`spoke-${spoke.key}`}
      >
        <span class="letter" title={spoke.label}>{letter(spoke.label)}</span>
        <span class="name">{spoke.label}</span>
        {#if avgMode}
          <span class="mean">{shown ? spokeValue(spoke.key).toFixed(1) : "—"}</span>
        {:else}
          <span class="digits" role="group" aria-label="{spoke.label} 1 to 5">
            {#each [1, 2, 3, 4, 5] as v (v)}
              <button
                type="button"
                class="digit"
                class:on={spokeValue(spoke.key) === v}
                disabled={!editable}
                aria-pressed={spokeValue(spoke.key) === v}
                onclick={() => onSet(spoke.key, v)}
              >{v}</button>
            {/each}
          </span>
        {/if}
      </div>
    {/each}
  </div>
</div>

<style>
  .dial {
    display: flex;
    flex-direction: column;
    align-items: stretch;
    gap: 4px;
    min-width: 0;
  }

  svg {
    width: 148px;
    height: 148px;
    display: block;
    margin: 0 auto;
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
    stroke-width: 14;
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

  .letters {
    display: flex;
    flex-direction: column;
    gap: 4px;
  }

  .letter-row {
    display: flex;
    align-items: center;
    gap: 6px;
    min-width: 0;
  }

  .letter {
    flex: none;
    width: 1.1rem;
    font-size: 0.78rem;
    font-weight: 700;
    color: var(--ink);
  }

  .name {
    flex: 1;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    font-size: 0.68rem;
    color: var(--dim);
  }

  .digits {
    display: inline-flex;
    gap: 3px;
    flex: none;
  }

  .digit {
    font: inherit;
    font-size: 0.72rem;
    width: 22px;
    height: 22px;
    padding: 0;
    color: var(--ink);
    background: var(--bg-fill, var(--bg));
    border: 1px solid var(--edge);
    border-radius: 6px;
    cursor: pointer;
  }

  .digit.on {
    background: var(--accent);
    border-color: var(--focus, #6e4db0);
    color: var(--bg);
    font-weight: 700;
  }

  .digit:disabled {
    cursor: default;
  }

  .digit:disabled:not(.on) {
    opacity: 0.45;
  }

  .mean {
    flex: none;
    font-size: 0.78rem;
    font-weight: 700;
    font-variant-numeric: tabular-nums;
  }
</style>
