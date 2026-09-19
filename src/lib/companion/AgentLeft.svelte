<script lang="ts">
  let {
    hackOn,
    hackTitle,
    agentOpen = $bindable(false),
    agent,
    agents,
    sending,
    agentLabel,
    pickAgent,
    prompts,
    togglePrompts,
    nextCount,
    toggleNextCard,
  }: {
    hackOn: boolean;
    hackTitle: string;
    agentOpen?: boolean;
    agent: string;
    agents: string[];
    sending: boolean;
    agentLabel: (id: string) => string;
    pickAgent: (id: string) => void;
    prompts: boolean;
    togglePrompts: () => void;
    nextCount: number;
    toggleNextCard: () => void;
  } = $props();

</script>

    <div class="agentleft">
    <!-- Always-on purple well left of
         the agent pick. Glow/pulse only while sending or hackLive.
         Not a button: it drags with the dock chrome like any gap. -->
    <span
      class="hackled"
      class:on={hackOn}
      role="img"
      aria-label={hackOn ? "agent working" : "agent idle"}
      title={hackTitle}
    ></span>
    <span class="agentwrap">
      <button
        id="companion-agent"
        type="button"
        class="agentpick"
        onclick={() => (agentOpen = !agentOpen)}
        disabled={sending}
        aria-haspopup="listbox"
        aria-expanded={agentOpen}
        aria-label="Which agent soul is the companion"
      >{agentLabel(agent)}</button>
      {#if agentOpen}
        <div
          class="agentbubble"
          role="listbox"
          aria-label="Agents"
          onpointerdown={(e) => e.stopPropagation()}
        >
          {#each agents as a}
            <button
              type="button"
              class="agentopt"
              class:on={a === agent}
              role="option"
              aria-selected={a === agent}
              onclick={() => pickAgent(a)}
            >{agentLabel(a)}</button>
          {/each}
        </div>
      {/if}
    </span>
    <button
      class="expandbtn iconbtn promptsbtn"
      class:on={prompts}
      onclick={togglePrompts}
      aria-pressed={prompts}
      aria-label="STARS prompts"
      title="STARS prompts — one quiet strip when today's check-in has a thin spoke. Off by default."
    >
      <svg viewBox="0 0 24 24" fill="currentColor" aria-hidden="true">
        <path d="M9 3l1.4 4.6L15 9l-4.6 1.4L9 15l-1.4-4.6L3 9l4.6-1.4z" />
        <path d="M17 11l.9 2.9 2.9.9-2.9.9-.9 2.9-.9-2.9-2.9-.9 2.9-.9z" />
        <path d="M16.5 3l.7 2.3 2.3.7-2.3.7-.7 2.3-.7-2.3-2.3-.7 2.3-.7z" />
      </svg>
    </button>
    {#if nextCount > 0}
      <button
        class="nextpip iconbtn"
        onclick={toggleNextCard}
        aria-label="Next card — {nextCount} Needed"
        data-chip="Needed"
        title="Next card (⌘⇧N)"
      >
        <span class="nextpip-n">{nextCount > 9 ? "9+" : nextCount}</span>
      </button>
    {/if}
    </div>
