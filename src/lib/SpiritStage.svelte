<script lang="ts">
  // GO-ORBITTY-CLEAN-COMPANION-STAGE: idle stage + hosted wellhead +
  // expanded corner portrait. CompanionPage mounts this; sprite-off
  // draws none of those three. Think-v9 timing stays on the parent.

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
</script>

{#if corner}
  <!-- CORNER: 96×96 absolute upper-right. HOST-CC: same portrait on
       the hosted overlay. -->
  <div
    class="exphead"
    class:headview={headShowing}
    onmousedown={onDrag}
    role="presentation"
  >
    {#if !missing && frames.length > 0}
      <div class="floatwrap" class:headview={headShowing}>
        <img
          class="expimg"
          class:smooth
          class:react={headShowing && thinking}
          src={frames[frameIdx] ?? frames[0]}
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
        onmousedown={onDrag}
        role="presentation"
      >
        <img
          class="spirit"
          class:smooth
          class:react={headShowing && thinking}
          src={frames[frameIdx] ?? frames[0]}
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
  }

  .stage .floatwrap {
    width: 100%;
    height: 100%;
    display: flex;
    align-items: center;
    justify-content: center;
  }

  :global(.ghost) .stage {
    container-type: size;
  }

  :global(.ghost) .stage .floatwrap {
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
