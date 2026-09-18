<script lang="ts">
  import Companion3D from "../Companion3D.svelte";
  import SpiritStage from "../SpiritStage.svelte";
  import CaptionPane from "../CaptionPane.svelte";
  import type { CompanionApi } from "./api";
  let { comp }: { comp: CompanionApi } = $props();
</script>

  {#if comp.expanded && comp.sprite}
    {#if comp.is3D && !comp.glbFailed}
      {#key comp.bodyId}
        <Companion3D
          corner
          headShowing={comp.headShowing}
          bodyId={comp.bodyId}
          thinking={comp.sending}
          onDrag={comp.onDragMouseDown}
          onFail={comp.onGlbFail}
        />
      {/key}
    {:else}
      <SpiritStage
        corner
        headShowing={comp.headShowing}
        missing={comp.missing}
        frames={comp.drawFrames}
        frameIdx={comp.frameIdx}
        thinking={comp.sending}
        smooth={comp.artSmooth}
        onDrag={comp.onDragMouseDown}
      />
    {/if}
  {/if}


  {#if comp.sprite && !comp.expanded && (comp.hosted === "off" || comp.spiritHold)}
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
  {/if}

  {#if !comp.expanded && comp.captions}
    <CaptionPane
    expanded={false}
    hosted={comp.hosted !== "off"}
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

