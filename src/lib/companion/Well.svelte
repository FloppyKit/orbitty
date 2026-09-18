<script lang="ts">
  import CaptionPane from "../CaptionPane.svelte";
  import type { CompanionApi } from "./api";
  import KanbanPage from "../KanbanPage.svelte";
  import StarsPage from "../StarsPage.svelte";
  import RailPage from "../RailPage.svelte";
  import Companion3D from "../Companion3D.svelte";
  import SpiritStage from "../SpiritStage.svelte";
  let { comp }: { comp: CompanionApi } = $props();
</script>

  {#if comp.hosted !== "off"}
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
          class:fullwell={!comp.sprite}
          class:turning={comp.swapPrev !== null}
          style:--prism-r="{comp.prismR}px"
          bind:this={comp.wellpaneEl}
        >
          <!-- on a hosted dest swap
               the outgoing page mounts as a second face (swapPrev) and
               both faces ride a ±120deg rotateY around the card's
               vertical axis — the crease between them is the read. Each
               face IS the tile for the turn (fill / 1px edge / 12px
               radius ride WITH it); the pane's own chrome goes
               transparent so no static frame rings the moving cards.
               Both mounted the whole 400ms, so the well never flashes
               blank. Not turning: one static layer, no 3D, same flex
               sizing as before. -->
          {#if comp.swapPrev !== null}
            <div class="prismface face-out" class:face-back={comp.swapDir === -1}>
              {#if comp.swapPrev === "launchpad"}
                <KanbanPage
                  embedded
                  onClose={comp.dismissHost}
                  onPopout={comp.popoutLaunchpad}
                  onNeededFocus={comp.onNeededFocus}
                  onNeededAnswer={comp.onNeededAnswer}
                />
              {:else if comp.swapPrev === "stars"}
                <StarsPage embedded onClose={comp.dismissHost} />
              {:else}
                <RailPage
                  embedded
                  onClose={comp.dismissHost}
                  onPopout={comp.popoutInspector}
                  createPlanetTick={comp.createPlanetTick}
                />
              {/if}
            </div>
          {/if}
          <div
            class="wellpage"
            class:face-in={comp.swapPrev !== null}
            class:face-back={comp.swapPrev !== null && comp.swapDir === -1}
          >
            {#if comp.hosted === "launchpad"}
              <KanbanPage
                embedded
                onClose={comp.dismissHost}
                onPopout={comp.popoutLaunchpad}
                onNeededFocus={comp.onNeededFocus}
                onNeededAnswer={comp.onNeededAnswer}
              />
            {:else if comp.hosted === "stars"}
              <StarsPage embedded onClose={comp.dismissHost} />
            {:else if comp.hosted === "inspector"}
              <RailPage
                embedded
                onClose={comp.dismissHost}
                onPopout={comp.popoutInspector}
                createPlanetTick={comp.createPlanetTick}
              />
            {/if}
          </div>
        </div>
        {#if comp.sprite}
          <!-- Idle figure is held through native grow, then this column
               FLIPs (translate + scale, same Head/full as idle). -->
          <div class="wellhead">
            {#if !comp.expanded}
              <div class="spirshift">
                {#if comp.is3D && !comp.glbFailed}
                  {#key comp.bodyId}
                    <Companion3D
                      headShowing={comp.headShowing}
                      bodyId={comp.bodyId}
                      thinking={comp.sending}
                      onDrag={comp.onDragMouseDown}
                      onFail={comp.onGlbFail}
                    />
                  {/key}
                {:else}
                  <SpiritStage
                    headShowing={comp.headShowing}
                    missing={comp.missing}
                    frames={comp.drawFrames}
                    frameIdx={comp.frameIdx}
                    thinking={comp.sending}
                    smooth={comp.artSmooth}
                    onDrag={comp.onDragMouseDown}
                  />
                {/if}
              </div>
            {/if}
          </div>
        {/if}
      </div>
      {#if comp.expanded && comp.captions}
        <CaptionPane
    expanded={true}
    hosted={true}
    userLine={comp.userLine}
    botLine={comp.botLine}
    sending={comp.sending}
    ackLine={comp.ackLine}
    turns={comp.turns}
    speak={comp.speak}
    draft={comp.draft}
    ready={comp.captionReady}
    onSpeakError={comp.onCaptionSpeakError}
    onOverlayChange={comp.onCaptionOverlayChange}
  />
      {/if}
    </div>
  {:else if comp.expanded && comp.captions}
    <CaptionPane
    expanded={true}
    hosted={false}
    userLine={comp.userLine}
    botLine={comp.botLine}
    sending={comp.sending}
    ackLine={comp.ackLine}
    turns={comp.turns}
    speak={comp.speak}
    draft={comp.draft}
    ready={comp.captionReady}
    onSpeakError={comp.onCaptionSpeakError}
    onOverlayChange={comp.onCaptionOverlayChange}
  />
  {/if}

