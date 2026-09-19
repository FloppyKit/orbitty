<script lang="ts">
  import type { Snippet } from "svelte";
  import KanbanPage from "../KanbanPage.svelte";
  import StarsPage from "../StarsPage.svelte";
  import RailPage from "../RailPage.svelte";
  import CompSpirit from "./Spirit.svelte";

  type HostMode = "off" | "launchpad" | "stars" | "inspector";

  let {
    hosted,
    sprite,
    swapPrev,
    swapDir,
    prismR,
    wellpaneEl = $bindable(null),
    expanded,
    captions,
    caption,
    dismissHost,
    popoutLaunchpad,
    popoutInspector,
    onNeededFocus,
    onNeededAnswer,
    createPlanetTick,
    headShowing,
    bodyId,
    sending,
    is3D,
    glbFailed,
    missing,
    drawFrames,
    frameIdx,
    artSmooth,
    onDragMouseDown,
    onGlbFail,
  }: {
    hosted: HostMode;
    sprite: boolean;
    swapPrev: HostMode | null;
    swapDir: number;
    prismR: number;
    wellpaneEl?: HTMLElement | null;
    expanded: boolean;
    captions: boolean;
    caption: Snippet<[boolean, boolean]>;
    dismissHost: () => void;
    popoutLaunchpad: () => void;
    popoutInspector: () => void;
    onNeededFocus?: (id: string | null, title: string | null) => void;
    onNeededAnswer?: () => void;
    createPlanetTick: number;
    headShowing: boolean;
    bodyId: string;
    sending: boolean;
    is3D: boolean;
    glbFailed: boolean;
    missing: boolean;
    drawFrames: string[];
    frameIdx: number;
    artSmooth: boolean;
    onDragMouseDown: (e: MouseEvent) => void;
    onGlbFail: () => void;
  } = $props();
</script>

{#if hosted !== "off"}
  <!-- HOST-SPLIT + HOST-CC: the well — left ~2/3 is the embedded
       Launchpad / STARS / Inspector, right 1/3 is the Head spirit
       stage. The well stays MOUNTED while expanded — the transcript
       overlays it inside .wellwrap. The two capped CC lines +
       agentrow + composer stay full-width below the well. -->
  <div class="wellwrap">
    <div class="hostwell">
      <!-- SPRITE-OFF: no .wellhead when the sprite is hidden — the
           pane takes the full well (the card keeps its ring). -->
      <div
        class="wellpane"
        class:fullwell={!sprite}
        class:turning={swapPrev !== null}
        style:--prism-r="{prismR}px"
        bind:this={wellpaneEl}
      >
        {#if swapPrev !== null}
          <div class="prismface face-out" class:face-back={swapDir === -1}>
            {#if swapPrev === "launchpad"}
              <KanbanPage
                embedded
                onClose={dismissHost}
                onPopout={popoutLaunchpad}
                {onNeededFocus}
                {onNeededAnswer}
              />
            {:else if swapPrev === "stars"}
              <StarsPage embedded onClose={dismissHost} />
            {:else}
              <RailPage
                embedded
                onClose={dismissHost}
                onPopout={popoutInspector}
                {createPlanetTick}
              />
            {/if}
          </div>
        {/if}
        <div
          class="wellpage"
          class:face-in={swapPrev !== null}
          class:face-back={swapPrev !== null && swapDir === -1}
        >
          {#if hosted === "launchpad"}
            <KanbanPage
              embedded
              onClose={dismissHost}
              onPopout={popoutLaunchpad}
              {onNeededFocus}
              {onNeededAnswer}
            />
          {:else if hosted === "stars"}
            <StarsPage embedded onClose={dismissHost} />
          {:else if hosted === "inspector"}
            <RailPage
              embedded
              onClose={dismissHost}
              onPopout={popoutInspector}
              {createPlanetTick}
            />
          {/if}
        </div>
      </div>
      {#if sprite}
        <div class="wellhead">
          {#if !expanded}
            <div class="spirshift">
              <CompSpirit
                {headShowing}
                {bodyId}
                {sending}
                {is3D}
                {glbFailed}
                {missing}
                {drawFrames}
                {frameIdx}
                {artSmooth}
                {onDragMouseDown}
                {onGlbFail}
              />
            </div>
          {/if}
        </div>
      {/if}
    </div>
    {#if expanded && captions}
      {@render caption(true, true)}
    {/if}
  </div>
{:else if expanded && captions}
  {@render caption(true, false)}
{/if}
