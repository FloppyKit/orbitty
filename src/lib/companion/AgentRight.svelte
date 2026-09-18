<script lang="ts">
  import type { CompanionApi } from "./api";
  let { dock }: { dock: CompanionApi } = $props();
</script>

    <div class="agentright">
    <span class="volwrap">
      <button
        class="expandbtn iconbtn speakbtn"
        class:on={dock.speak && dock.speakVolume > 0}
        class:muted={!dock.speak || dock.speakVolume <= 0}
        onclick={dock.onSpeakClick}
        oncontextmenu={dock.onSpeakRight}
        aria-pressed={dock.speak && dock.speakVolume > 0}
        aria-label="Companion volume"
        title="Click: volume · Right-click: mute"
      >
        <svg
          viewBox="0 0 24 24"
          fill="none"
          stroke="currentColor"
          stroke-width="1.8"
          stroke-linecap="round"
          stroke-linejoin="round"
          aria-hidden="true"
        >
          <polygon
            points="11 5 6 9 2.5 9 2.5 15 6 15 11 19 11 5"
            fill="currentColor"
            stroke="none"
          />
          <path d="M15 9.2a4.2 4.2 0 0 1 0 5.6" />
          {#if !dock.speak || dock.speakVolume <= 0}
            <line x1="5.6" y1="18.4" x2="18.4" y2="5.6" />
          {/if}
        </svg>
      </button>
      {#if dock.volOpen}
        <div
          class="volpop"
          role="dialog"
          aria-label="Companion volume"
          onpointerdown={(e) => e.stopPropagation()}
        >
          <input
            type="range"
            min="0"
            max="100"
            step="1"
            value={Math.round(dock.speakVolume * 100)}
            oninput={dock.onVolInput}
            aria-label="Volume"
          />
        </div>
      {/if}
    </span>
    <button
      class="expandbtn iconbtn viewbtn"
      class:on={dock.headShowing}
      class:muted={!dock.sprite}
      onclick={dock.toggleView}
      oncontextmenu={(e) => {
        e.preventDefault();
        dock.toggleSprite(e);
      }}
      disabled={dock.viewBusy}
      aria-pressed={dock.headShowing}
      aria-label="Head or full body, or right-click to hide sprite"
      title={dock.viewTitle}
    >
      {#if dock.headShowing}
        <svg viewBox="0 0 24 24" fill="currentColor" aria-hidden="true">
          <circle cx="12" cy="8" r="4.4" />
          <path d="M4.6 21c.6-4 3.6-6.4 7.4-6.4s6.8 2.4 7.4 6.4z" />
          {#if !dock.sprite}
            <line
              x1="5.6"
              y1="18.4"
              x2="18.4"
              y2="5.6"
              stroke="currentColor"
              stroke-width="1.8"
              stroke-linecap="round"
            />
          {/if}
        </svg>
      {:else}
        <!-- Full body: restroom-man (head + shouldered torso + two legs),
             never the circle + rounded-rect that read as a fat i. -->
        <svg viewBox="0 0 24 24" fill="currentColor" aria-hidden="true">
          <circle cx="12" cy="4.6" r="2.6" />
          <path
            d="M9.2 8.4h5.6c1.3 0 2.2 1 2.2 2.2v4.4h-1.6v-3.6h-.6V21h-1.9v-6h-1.8v6H9.4v-9.6h-.6V15H7.2v-4.4c0-1.2.9-2.2 2-2.2z"
          />
          {#if !dock.sprite}
            <line
              x1="5.6"
              y1="18.4"
              x2="18.4"
              y2="5.6"
              stroke="currentColor"
              stroke-width="1.8"
              stroke-linecap="round"
            />
          {/if}
        </svg>
      {/if}
    </button>
    <button
      class="expandbtn iconbtn"
      onclick={dock.toggleExpanded}
      aria-label={dock.expanded ? "Collapse transcript" : "Expand transcript"}
      title={dock.expanded ? "Collapse transcript" : "Expand transcript"}
    >
      {#if dock.expanded}
        <svg
          viewBox="0 0 24 24"
          fill="none"
          stroke="currentColor"
          stroke-width="2.2"
          stroke-linecap="round"
          stroke-linejoin="round"
          aria-hidden="true"
        >
          <line x1="18" y1="6" x2="7.5" y2="16.5" />
          <polyline points="15.5 16.5 7.5 16.5 7.5 8.5" />
        </svg>
      {:else}
        <svg
          viewBox="0 0 24 24"
          fill="none"
          stroke="currentColor"
          stroke-width="2.2"
          stroke-linecap="round"
          stroke-linejoin="round"
          aria-hidden="true"
        >
          <line x1="6" y1="18" x2="16.5" y2="7.5" />
          <polyline points="8.5 7.5 16.5 7.5 16.5 15.5" />
        </svg>
      {/if}
    </button>
    <button
      type="button"
      class="gear"
      onclick={() => dock.invoke("toggle_labeled", { label: "settings" }).catch(() => {})}
      title="Settings — click again to hide"
      aria-label="Toggle Settings"
    >
      <svg
        viewBox="0 0 24 24"
        fill="none"
        stroke="currentColor"
        stroke-width="2"
        stroke-linecap="round"
        stroke-linejoin="round"
        aria-hidden="true"
      >
        <circle cx="12" cy="12" r="3" />
        <path
          d="M19.4 15a1.7 1.7 0 0 0 .3 1.8l.1.1a2 2 0 1 1-2.8 2.8l-.1-.1a1.7 1.7 0 0 0-1.8-.3 1.7 1.7 0 0 0-1 1.5V21a2 2 0 0 1-4 0v-.2a1.7 1.7 0 0 0-1-1.5 1.7 1.7 0 0 0-1.8.3l-.1.1a2 2 0 1 1-2.8-2.8l.1-.1a1.7 1.7 0 0 0 .3-1.8 1.7 1.7 0 0 0-1.5-1H3a2 2 0 0 1 0-4h.2a1.7 1.7 0 0 0 1.5-1 1.7 1.7 0 0 0-.3-1.8l-.1-.1a2 2 0 1 1 2.8-2.8l.1.1a1.7 1.7 0 0 0 1.8.3H9a1.7 1.7 0 0 0 1-1.5V3a2 2 0 0 1 4 0v.2a1.7 1.7 0 0 0 1 1.5 1.7 1.7 0 0 0 1.8-.3l.1-.1a2 2 0 1 1 2.8 2.8l-.1.1a1.7 1.7 0 0 0-.3 1.8V9c.3.6.9 1 1.5 1H21a2 2 0 0 1 0 4h-.2a1.7 1.7 0 0 0-1.5 1z"
        />
      </svg>
    </button>
    </div>
