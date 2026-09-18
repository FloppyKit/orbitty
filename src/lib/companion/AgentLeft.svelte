<script lang="ts">
  import type { CompanionApi } from "./api";
  let { dock }: { dock: CompanionApi } = $props();
</script>

    <div class="agentleft">
    <!-- Always-on purple well left of
         the agent pick. Glow/pulse only while sending or hackLive.
         Not a button: it drags with the dock chrome like any gap. -->
    <span
      class="hackled"
      class:on={dock.hackOn}
      role="img"
      aria-label={dock.hackOn ? "agent working" : "agent idle"}
      title={dock.hackTitle}
    ></span>
    <span class="agentwrap">
      <button
        id="companion-agent"
        type="button"
        class="agentpick"
        onclick={() => (dock.agentOpen = !dock.agentOpen)}
        disabled={dock.sending}
        aria-haspopup="listbox"
        aria-expanded={dock.agentOpen}
        aria-label="Which agent soul is the companion"
      >{dock.agentLabel(dock.agent)}</button>
      {#if dock.agentOpen}
        <div
          class="agentbubble"
          role="listbox"
          aria-label="Agents"
          onpointerdown={(e) => e.stopPropagation()}
        >
          {#each dock.agents as a}
            <button
              type="button"
              class="agentopt"
              class:on={a === dock.agent}
              role="option"
              aria-selected={a === dock.agent}
              onclick={() => dock.pickAgent(a)}
            >{dock.agentLabel(a)}</button>
          {/each}
        </div>
      {/if}
    </span>
    <button
      class="expandbtn iconbtn promptsbtn"
      class:on={dock.prompts}
      onclick={dock.togglePrompts}
      aria-pressed={dock.prompts}
      aria-label="STARS prompts"
      title="STARS prompts — one quiet strip when today's check-in has a thin spoke. Off by default."
    >
      <svg viewBox="0 0 24 24" fill="currentColor" aria-hidden="true">
        <path d="M9 3l1.4 4.6L15 9l-4.6 1.4L9 15l-1.4-4.6L3 9l4.6-1.4z" />
        <path d="M17 11l.9 2.9 2.9.9-2.9.9-.9 2.9-.9-2.9-2.9-.9 2.9-.9z" />
        <path d="M16.5 3l.7 2.3 2.3.7-2.3.7-.7 2.3-.7-2.3-2.3-.7 2.3-.7z" />
      </svg>
    </button>
    {#if dock.nextCount > 0}
      <button
        class="nextpip iconbtn"
        onclick={dock.toggleNextCard}
        aria-label="Next card — {dock.nextCount} Needed"
        data-chip="Needed"
        title="Next card (⌘⇧N)"
      >
        <span class="nextpip-n">{dock.nextCount > 9 ? "9+" : dock.nextCount}</span>
      </button>
    {/if}
    </div>

