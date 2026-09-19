<script lang="ts">
  import Companion3D from "../Companion3D.svelte";
  import SpiritStage from "../SpiritStage.svelte";

  let {
    corner = false,
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
    corner?: boolean;
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

{#if is3D && !glbFailed}
  {#key bodyId}
    <Companion3D
      {corner}
      {headShowing}
      {bodyId}
      thinking={sending}
      onDrag={onDragMouseDown}
      onFail={onGlbFail}
    />
  {/key}
{:else}
  <SpiritStage
    {corner}
    {headShowing}
    {missing}
    frames={drawFrames}
    {frameIdx}
    thinking={sending}
    smooth={artSmooth}
    onDrag={onDragMouseDown}
  />
{/if}
