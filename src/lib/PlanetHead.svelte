<script lang="ts">
  let {
    chip = null,
    compact = false,
    col = null,
    count,
    doing = null,
    dragging = false,
    dropOk = false,
    hint = "",
    live = false,
    mark = "",
    mkey,
    mkeySel = false,
    name,
    onclick,
    ondblclick,
    ondragleave,
    ondragover,
    ondrop,
    onpointerdown,
    pid = null,
    sel = false,
  }: {
    chip?: string | null;
    compact?: boolean;
    col?: string | null;
    count: number | string;
    doing?: number | string | null;
    dragging?: boolean;
    dropOk?: boolean;
    hint?: string;
    live?: boolean;
    mark?: string;
    mkey?: string;
    mkeySel?: boolean;
    name: string;
    onclick?: (e: MouseEvent) => void;
    ondblclick?: (e: MouseEvent) => void;
    ondragleave?: (e: DragEvent) => void;
    ondragover?: (e: DragEvent) => void;
    ondrop?: (e: DragEvent) => void;
    onpointerdown?: (e: PointerEvent) => void;
    pid?: string | null;
    sel?: boolean;
  } = $props();

  const showDoing = $derived(
    !compact && doing != null && doing !== "" && doing !== 0,
  );
  const showMark = $derived(!!mark && !chip);
</script>

<button
  type="button"
  class="planet-head"
  class:compact
  class:sel
  class:dragging
  class:drop-ok={dropOk}
  class:mkey-sel={mkeySel}
  data-mkey={mkey}
  data-pid={pid}
  data-col={col}
  role="tab"
  aria-selected={sel}
  aria-label={compact ? `${name} ${count}` : undefined}
  title={compact ? name : hint || name}
  {onclick}
  {ondblclick}
  {ondragleave}
  {ondragover}
  {ondrop}
  {onpointerdown}
>
  <span
    class="planet-chip"
    class:live
    class:mark={showMark}
    style={chip ? `background: ${chip}; --pglow: ${chip}` : undefined}
    aria-hidden="true"
  ></span>
  {#if showMark}
    <span class="planet-head-mark">{mark}</span>
  {/if}
  {#if !compact}
    <span class="planet-head-name">{name}</span>
  {/if}
  {#if showDoing}
    <span class="planet-head-doing">{doing}</span>
  {/if}
  <span class="planet-head-count">{count}</span>
</button>

<style>
  .planet-head {
    display: flex;
    align-items: center;
    gap: 6px;
    width: 100%;
    min-height: 26px;
    padding: 4px 8px;
    font: inherit;
    text-align: left;
    color: var(--ink);
    background: transparent;
    border: none;
    border-radius: 6px;
    cursor: pointer;
  }

  .planet-head:hover {
    background: var(--panel-fill, var(--panel));
  }

  .planet-head.compact {
    gap: 4px;
    padding: 4px;
  }

  .planet-head.sel {
    background: var(--bg-fill, var(--bg));
    box-shadow: inset 0 0 0 1px var(--focus, #6e4db0);
  }

  .planet-head.drop-ok {
    box-shadow: inset 0 0 0 1px var(--accent);
  }

  .planet-head.dragging {
    opacity: 0.45;
  }

  .planet-chip {
    flex-shrink: 0;
    width: 11px;
    height: 11px;
    border-radius: 50%;
    box-shadow: inset 0 0 0 1px rgba(0, 0, 0, 0.25);
  }

  .planet-chip.mark {
    width: 16px;
    height: 16px;
    border-radius: 50%;
    background: var(--accent);
    box-shadow: inset 0 0 0 1px rgba(0, 0, 0, 0.25);
  }

  .planet-head-mark {
    flex-shrink: 0;
    font-size: 0.55rem;
    font-weight: 700;
    line-height: 1;
    color: var(--ink);
  }

  .planet-chip.live {
    box-shadow:
      inset 0 0 0 1px rgba(0, 0, 0, 0.25),
      0 0 8px 2px var(--pglow, var(--accent));
    animation: planet-head-glow 1.6s ease-in-out infinite;
  }

  @keyframes planet-head-glow {
    0%,
    100% {
      box-shadow:
        inset 0 0 0 1px rgba(0, 0, 0, 0.25),
        0 0 5px 1px var(--pglow, var(--accent));
    }
    50% {
      box-shadow:
        inset 0 0 0 1px rgba(0, 0, 0, 0.25),
        0 0 12px 4px var(--pglow, var(--accent));
    }
  }

  .planet-head-name {
    flex: 1;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    font-size: 0.68rem;
    font-weight: 700;
    text-transform: uppercase;
    letter-spacing: 0.08em;
    color: var(--dim);
  }

  .planet-head.sel .planet-head-name {
    color: var(--accent);
  }

  .planet-head-doing {
    flex-shrink: 0;
    font-size: 0.58rem;
    font-weight: 700;
    letter-spacing: 0.04em;
    text-transform: uppercase;
    color: var(--accent);
  }

  .planet-head-count {
    flex-shrink: 0;
    margin-left: auto;
    font-size: 0.64rem;
    color: var(--ink);
    border: 1px solid var(--edge);
    border-radius: 999px;
    padding: 0 0.35rem;
    font-variant-numeric: tabular-nums;
  }
</style>
