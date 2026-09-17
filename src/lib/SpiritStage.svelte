<script lang="ts">
  // Idle stage + hosted wellhead +
  // expanded corner portrait. CompanionPage mounts this; sprite-off
  // draws none of those three. Think-v9 timing stays on the parent.
  // Look-at overlay lives in lookAt.ts — this file paints the cell.

  import { invoke } from "@tauri-apps/api/core";
  import { listen } from "@tauri-apps/api/event";
  import { onMount, onDestroy } from "svelte";
  import {
    BLINK_EVERY_MS,
    EMPTY_CUE_SNAP,
    OVERLAY_MS,
    POKE_DRAG_PX,
    cueFromCards,
    dirIndex,
    lookEnabled,
    overlayMap,
    pickDir,
    recordPoke,
    type CueSnap,
    type LookDir,
    type OverlayKind,
  } from "./lookAt";
  import type { PersistedState } from "./persist";

  let {
    corner = false,
    headShowing = false,
    missing = false,
    frames = [],
    frameIdx = 0,
    thinking = false,
    smooth = false,
    onDrag,
  }: {
    corner?: boolean;
    headShowing?: boolean;
    missing?: boolean;
    frames?: string[];
    frameIdx?: number;
    thinking?: boolean;
    smooth?: boolean;
    onDrag?: (e: MouseEvent) => void;
  } = $props();

  let wrapEl: HTMLDivElement | undefined = $state();
  let dir: LookDir = $state("idle");
  let inDead = $state(true);
  let overlaySrc: string | null = $state(null);
  let finePointer = $state(true);
  let reducedMotion = $state(false);
  let pokeTimes: number[] = [];
  let pokeDown: { x: number; y: number } | null = null;
  let overlayTimer: ReturnType<typeof setTimeout> | undefined;
  let blinkTimer: ReturnType<typeof setInterval> | undefined;
  let lookFull = $state<string[]>([]);
  let lookBust = $state<string[]>([]);
  let overlayFull = $state<string[]>([]);
  let overlayBust = $state<string[]>([]);
  let rest0 = $state("");
  let bust0 = $state("");
  let cueSnap: CueSnap = EMPTY_CUE_SNAP;
  let overdue = $state(false);
  let sawOverdue = false;
  let unlistenBoard: (() => void) | undefined;
  let unlistenNeeded: (() => void) | undefined;

  const lookFrames = $derived(headShowing ? lookBust : lookFull);
  const overlayFrames = $derived(headShowing ? overlayBust : overlayFull);
  const isSpirit = $derived(
    !!frames[0] && (frames[0] === rest0 || frames[0] === bust0),
  );
  const overlays = $derived(overlayMap(overlayFrames));
  const lookOn = $derived(
    lookEnabled(finePointer, reducedMotion) &&
      lookFrames.length === 8 &&
      isSpirit,
  );

  const displaySrc = $derived.by(() => {
    if (overlaySrc) return overlaySrc;
    if (!thinking && lookOn && dir !== "idle") {
      const i = dirIndex(dir);
      if (i >= 0 && lookFrames[i]) return lookFrames[i];
    }
    return frames[frameIdx] ?? frames[0];
  });

  function playOverlay(kind: OverlayKind) {
    if (reducedMotion) return;
    const src = overlays[kind];
    if (!src) return;
    overlaySrc = src;
    if (overlayTimer) clearTimeout(overlayTimer);
    overlayTimer = setTimeout(() => {
      overlaySrc = null;
    }, OVERLAY_MS);
  }

  function onPtrMove(e: PointerEvent) {
    if (!lookOn || thinking) {
      dir = "idle";
      inDead = true;
      return;
    }
    const el = wrapEl;
    if (!el) return;
    const r = el.getBoundingClientRect();
    const dx = e.clientX - (r.left + r.width / 2);
    const dy = e.clientY - (r.top + r.height / 2);
    const next = pickDir(dx, dy, dir, inDead);
    dir = next.dir;
    inDead = next.inDead;
  }

  function onPtrLeave() {
    dir = "idle";
    inDead = true;
  }

  function onWrapDown(e: MouseEvent) {
    onDrag?.(e);
    if (e.button !== 0) return;
    pokeDown = { x: e.clientX, y: e.clientY };
  }

  function onWrapUp(e: MouseEvent) {
    if (!pokeDown) return;
    const moved = Math.hypot(e.clientX - pokeDown.x, e.clientY - pokeDown.y);
    pokeDown = null;
    if (moved >= POKE_DRAG_PX) return;
    if (reducedMotion) return;
    const r = recordPoke(pokeTimes, Date.now());
    pokeTimes = r.times;
    playOverlay(r.kind);
  }

  $effect(() => {
    if (!overdue) {
      sawOverdue = false;
      return;
    }
    if (sawOverdue || reducedMotion) return;
    sawOverdue = true;
    playOverlay("sleepy");
  });

  async function onBoard() {
    try {
      const s = await invoke<PersistedState>("load_state");
      const r = cueFromCards(
        s.kanbanCards ?? [],
        cueSnap,
        Math.floor(Date.now() / 1000),
      );
      cueSnap = r.snap;
      overdue = r.overdue;
      if (r.cue) playOverlay(r.cue);
    } catch {
      /* look-at cues fail soft */
    }
  }

  onMount(() => {
    const fineMq = window.matchMedia("(pointer: fine)");
    const reduceMq = window.matchMedia("(prefers-reduced-motion: reduce)");
    const syncMq = () => {
      finePointer = fineMq.matches;
      reducedMotion = reduceMq.matches;
      if (!lookEnabled(finePointer, reducedMotion)) {
        dir = "idle";
        inDead = true;
      }
    };
    syncMq();
    fineMq.addEventListener("change", syncMq);
    reduceMq.addEventListener("change", syncMq);
    window.addEventListener("pointermove", onPtrMove);
    document.addEventListener("pointerleave", onPtrLeave);
    blinkTimer = setInterval(() => {
      if (reducedMotion || thinking || overlaySrc) return;
      if (dir !== "idle") return;
      playOverlay("blink");
    }, BLINK_EVERY_MS);
    invoke<{
      frames?: string[];
      bustFrames?: string[];
      lookFrames?: string[];
      lookBustFrames?: string[];
      overlayFrames?: string[];
      overlayBustFrames?: string[];
    }>("companion_frames", { packId: null })
      .then((art) => {
        lookFull = art.lookFrames ?? [];
        lookBust = art.lookBustFrames ?? [];
        overlayFull = art.overlayFrames ?? [];
        overlayBust = art.overlayBustFrames ?? [];
        rest0 = art.frames?.[0] ?? "";
        bust0 = art.bustFrames?.[0] ?? "";
      })
      .catch(() => {});
    void onBoard();
    listen("state:changed", () => {
      void onBoard();
    }).then((u) => {
      unlistenBoard = u;
    });
    listen<{ id: string | null }>("orbitty:needed-focus", (e) => {
      if (e.payload?.id) playOverlay("sparkle");
    }).then((u) => {
      unlistenNeeded = u;
    });
    return () => {
      fineMq.removeEventListener("change", syncMq);
      reduceMq.removeEventListener("change", syncMq);
    };
  });

  onDestroy(() => {
    window.removeEventListener("pointermove", onPtrMove);
    document.removeEventListener("pointerleave", onPtrLeave);
    unlistenBoard?.();
    unlistenNeeded?.();
    if (overlayTimer) clearTimeout(overlayTimer);
    if (blinkTimer) clearInterval(blinkTimer);
  });
</script>

{#if corner}
  <!-- CORNER: 96×96 absolute upper-right. HOST-CC: same portrait on
       the hosted overlay. -->
  <div
    class="exphead"
    class:headview={headShowing}
    role="presentation"
  >
    {#if !missing && frames.length > 0}
      <div
        class="floatwrap"
        class:headview={headShowing}
        bind:this={wrapEl}
        onmousedown={onWrapDown}
        onmouseup={onWrapUp}
        role="presentation"
      >
        <img
          class="expimg"
          class:smooth
          class:react={headShowing && thinking}
          src={displaySrc}
          alt=""
          draggable="false"
        />
      </div>
    {/if}
  </div>
  {#if missing}
    <p class="missing">spirit missing</p>
  {/if}
{:else}
  <!-- Idle full-width stage, or hosted right column (.wellhead). -->
  <div class="stage" role="presentation">
    {#if missing}
      <p class="missing">spirit missing</p>
    {:else if frames.length > 0}
      <div
        class="floatwrap"
        class:headview={headShowing}
        bind:this={wrapEl}
        onmousedown={onWrapDown}
        onmouseup={onWrapUp}
        role="presentation"
      >
        <img
          class="spirit"
          class:smooth
          class:react={headShowing && thinking}
          src={displaySrc}
          alt="Your companion spirit"
          draggable="false"
        />
      </div>
    {/if}
  </div>
{/if}

<style>
  .stage {
    flex: 1;
    min-height: 120px;
    width: 100%;
    display: flex;
    align-items: center;
    justify-content: center;
    user-select: none;
    -webkit-user-select: none;
    cursor: grab;
  }

  .stage:active {
    cursor: grabbing;
  }

  :global(.wellhead) .stage {
    flex: 1;
    width: auto;
    min-height: 0;
    container-type: size;
  }

  .stage .floatwrap {
    width: 100%;
    height: 100%;
    display: flex;
    align-items: center;
    justify-content: center;
  }

  :global(.ghost) .stage,
  :global(.wellhead) .stage {
    container-type: size;
  }

  :global(.ghost) .stage .floatwrap,
  :global(.wellhead) .stage .floatwrap {
    width: min(100cqw, 100cqh);
    height: min(100cqw, 100cqh);
  }

  .spirit {
    width: 100%;
    height: 100%;
    object-fit: contain;
    image-rendering: pixelated;
    user-select: none;
    -webkit-user-select: none;
  }

  .spirit.smooth {
    image-rendering: auto;
  }

  .exphead {
    position: absolute;
    top: 8px;
    left: 10px;
    right: auto;
    z-index: 2;
    width: 96px;
    height: 96px;
    display: flex;
    justify-content: center;
    align-items: center;
    user-select: none;
    -webkit-user-select: none;
    cursor: grab;
  }

  .exphead:active {
    cursor: grabbing;
  }

  .exphead .floatwrap {
    line-height: 0;
    width: 96px;
    height: 96px;
    display: flex;
    align-items: center;
    justify-content: center;
    animation: bustfloat 6.5s ease-in-out infinite;
  }

  .exphead .floatwrap.headview {
    animation: none;
  }

  .expimg {
    display: block;
    max-width: 96px;
    max-height: 96px;
    object-fit: contain;
    image-rendering: pixelated;
    user-select: none;
    -webkit-user-select: none;
  }

  .expimg.smooth {
    image-rendering: auto;
  }

  @keyframes bustfloat {
    0%,
    100% {
      transform: translate(0, 0);
    }
    25% {
      transform: translate(1.5px, -2.5px);
    }
    50% {
      transform: translate(0, -3.5px);
    }
    75% {
      transform: translate(-1.5px, -2px);
    }
  }

  .spirit.react,
  .expimg.react {
    animation: bustreact 0.9s ease-out 1;
  }

  @keyframes bustreact {
    0% {
      transform: rotate(0deg) scaleX(1);
    }
    30% {
      transform: rotate(-3deg) scaleX(0.96);
    }
    60% {
      transform: rotate(2.4deg) scaleX(1.02);
    }
    100% {
      transform: rotate(0deg) scaleX(1);
    }
  }

  .missing {
    margin: 0;
    color: var(--dim);
    font-size: 0.8rem;
    letter-spacing: 0.04em;
    text-align: center;
  }

  @media (prefers-reduced-motion: reduce) {
    .exphead .floatwrap,
    .spirit.react,
    .expimg.react {
      animation: none;
    }
  }
</style>
