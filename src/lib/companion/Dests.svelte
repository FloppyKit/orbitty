<script lang="ts">
  let {
    showDests = false,
    destOpen,
    toggleDest,
    planetsHidden = false,
    onPlanetsClick,
    onPlanetsRight,
  }: {
    showDests?: boolean;
    destOpen: { launchpad: boolean; stars: boolean; inspector: boolean };
    toggleDest: (d: "launchpad" | "stars" | "inspector") => void;
    planetsHidden?: boolean;
    onPlanetsClick: (e: MouseEvent) => void;
    onPlanetsRight: (e: MouseEvent) => void;
  } = $props();

</script>

{#if showDests}
    <span class="dests" role="group" aria-label="Destinations">
      <button
        class="dest iconbtn"
        class:on={destOpen.launchpad}
        onclick={() => toggleDest("launchpad")}
        aria-label="Launchpad"
        data-chip="Launchpad"
        title={destOpen.launchpad
          ? "Launchpad is open — click to hide"
          : "Launchpad — click to open"}
      >
        <!-- DEST-CHROME: the same ship flying ↗ — the blessed rocket
             rotated 45° so the nose rides upper-right, never the
             upright capsule. -->
        <svg viewBox="0 0 24 24" fill="currentColor" aria-hidden="true">
          <g transform="rotate(45 12 12)">
            <path
              fill-rule="evenodd"
              d="M12 2.5 C14.6 4.6 16 7.9 16 11.2 C16 13.1 15.7 14.9 15.2 16.2 L8.8 16.2 C8.3 14.9 8 13.1 8 11.2 C8 7.9 9.4 4.6 12 2.5 Z M10.5 9.3 a1.5 1.5 0 1 0 3 0 a1.5 1.5 0 1 0 -3 0 Z"
            />
            <path d="M8.7 12.3 C7.2 13.4 6.2 15.4 5.8 17.8 L8.9 16.6 C8.6 15.2 8.6 13.7 8.7 12.3 Z" />
            <path d="M15.3 12.3 C16.8 13.4 17.8 15.4 18.2 17.8 L15.1 16.6 C15.4 15.2 15.4 13.7 15.3 12.3 Z" />
            <path d="M12 17.6 C12.8 18.7 13.2 19.8 13.2 21 C12.5 20.6 11.5 20.6 10.8 21 C10.8 19.8 11.2 18.7 12 17.6 Z" />
          </g>
        </svg>
      </button>
      <button
        class="dest iconbtn"
        class:on={destOpen.stars}
        onclick={() => toggleDest("stars")}
        aria-label="STARS"
        data-chip="STARS"
        title={destOpen.stars
          ? "STARS is open — click to hide"
          : "STARS — click to open"}
      >
        <svg viewBox="0 0 24 24" fill="currentColor" aria-hidden="true">
          <path d="M12 3 L14.1 9.1 L20.6 9.2 L15.4 13.1 L17.3 19.3 L12 15.6 L6.7 19.3 L8.6 13.1 L3.4 9.2 L9.9 9.1 Z" />
        </svg>
      </button>
      <button
        class="dest iconbtn planetsdest"
        class:on={!planetsHidden}
        onclick={onPlanetsClick}
        oncontextmenu={onPlanetsRight}
        aria-label="Planets"
        data-chip="Planets"
        title={!planetsHidden
          ? "Planets on — click to hide discs. Right-click toggles Inspector"
          : "Planets off — click to show discs. Right-click toggles Inspector"}
      >
        <svg viewBox="0 0 24 24" fill="currentColor" aria-hidden="true">
          <circle cx="12" cy="12" r="5" />
          <ellipse
            cx="12"
            cy="12"
            rx="9.4"
            ry="3.3"
            transform="rotate(-20 12 12)"
            fill="none"
            stroke="currentColor"
            stroke-width="1.7"
          />
        </svg>
      </button>
    </span>
{/if}
