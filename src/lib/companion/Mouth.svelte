<script lang="ts">
  import type { CompanionApi } from "./api";
  let { dock }: { dock: CompanionApi } = $props();
</script>

  <form
    class="composer"
    onsubmit={(e) => {
      e.preventDefault();
      dock.sendLine();
    }}
  >
    {#if dock.attach}
      <!-- The one chip — name + size, × to drop.
           Vanishes on send (CompanionPage clears it); an honest send
           error brings it back. -->
      <div class="chiprow">
        <span class="attachchip" title={dock.attach.path}>
          <svg
            viewBox="0 0 24 24"
            fill="none"
            stroke="currentColor"
            stroke-width="1.9"
            stroke-linecap="round"
            stroke-linejoin="round"
            aria-hidden="true"
          >
            <path
              d="M21.44 11.05l-9.19 9.19a6 6 0 0 1-8.49-8.49l8.57-8.57A4 4 0 1 1 18 8.84l-8.59 8.57a2 2 0 0 1-2.83-2.83l8.49-8.48"
            />
          </svg>
          <span class="attachname">{dock.attach.name}</span>
          <span class="attachsize">{dock.fmtSize(dock.attach.size)}</span>
          <button
            type="button"
            class="attachx"
            onclick={dock.onDropAttach}
            aria-label="Drop file"
            title="Drop file — it was never copied"
          >×</button>
        </span>
      </div>
    {/if}
    <button
      type="button"
      class="thinbtn"
      onmousedown={(e) => e.stopPropagation()}
      onpointerdown={(e) => e.stopPropagation()}
      onclick={() => dock.onThin?.()}
      disabled={dock.sending}
      aria-label="Thin — label this chat"
      title="Label this chat thin (⌘⌥T). Mouth keeps going."
    >thin</button>
    <div
      class="fieldwrap"
      class:talking
      onmousedown={(e) => e.stopPropagation()}
      onpointerdown={(e) => e.stopPropagation()}
    >
      <button
        type="button"
        class="fieldplus"
        onmousedown={(e) => e.stopPropagation()}
        onpointerdown={(e) => e.stopPropagation()}
        onclick={dock.onPickAttach}
        disabled={dock.sending}
        aria-label="Attach one file"
        title="Attach one file (25 MB cap)"
      >+</button>
      <textarea
        class="field"
        rows="1"
        bind:this={dock.fieldEl}
        bind:value={dock.draft}
        onmousedown={(e) => e.stopPropagation()}
        onpointerdown={(e) => e.stopPropagation()}
        onkeydown={dock.onFieldKeydown}
        placeholder={dock.neededHint
          ? `answer: ${neededHint}  ·  Rule: … to remember`
          : "type…  ·  todo: buy milk -fkt"}
        aria-label="Message your companion"
        autocomplete="off"
        spellcheck="false"
      ></textarea>
      <span class="talkslot">
        {#if dock.talking}
          <button
            type="button"
            class="talkic"
            onclick={dock.talkStopOnly}
            aria-label="Stop talking — text stays for review"
            title="Stop talking — the text stays in the field; you hit Send"
          >
            <svg
              viewBox="0 0 24 24"
              fill="none"
              stroke="currentColor"
              stroke-width="2.6"
              stroke-linecap="round"
              stroke-linejoin="round"
              aria-hidden="true"
            >
              <polyline points="4.5 12.5 9.5 17.5 19.5 6.5" />
            </svg>
          </button>
          <button
            type="button"
            class="talkic sendnow"
            onclick={dock.talkStopSend}
            aria-label="Stop and send now"
            title="Stop and send now"
          >
            <svg
              viewBox="0 0 24 24"
              fill="none"
              stroke="currentColor"
              stroke-width="2.6"
              stroke-linecap="round"
              stroke-linejoin="round"
              aria-hidden="true"
            >
              <line x1="12" y1="19" x2="12" y2="5" />
              <polyline points="5.5 11.5 12 5 18.5 11.5" />
            </svg>
          </button>
        {:else}
          <button
            type="button"
            class="talkic"
            onclick={dock.talkStart}
            aria-label="Talk"
            title="Click to talk. Source is Settings → Talk. Text lands in the field; you hit Send"
          >
            <svg
              viewBox="0 0 24 24"
              fill="none"
              stroke="currentColor"
              stroke-width="1.9"
              stroke-linecap="round"
              stroke-linejoin="round"
              aria-hidden="true"
            >
              <rect x="9" y="2.5" width="6" height="11.5" rx="3" />
              <path d="M5.5 10.5a6.5 6.5 0 0 0 13 0" />
              <line x1="12" y1="17.5" x2="12" y2="21" />
            </svg>
          </button>
        {/if}
      </span>
    </div>
    <button
      class="send"
      type="submit"
      disabled={dock.sending || (!dock.draft.trim() && !dock.attach)}
      aria-label="Send"
      title="Send"
    >
      <svg viewBox="0 0 24 24" fill="currentColor" aria-hidden="true">
        <path d="M12 4l8 11h-4.5L12 10.4 8.5 15H4z" />
      </svg>
    </button>
  </form>

