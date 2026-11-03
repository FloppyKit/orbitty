<script lang="ts">
  let {
    attach,
    onDropAttach,
    onPickAttach,
    onThin,
    sending,
    draft = $bindable(""),
    fieldEl = $bindable(null),
    onFieldKeydown,
    ghost = "",
    onCaret,
    onComposing,
    neededHint = null,
    listOpen = false,
    activeDescendant = "",
    talking,
    talkStopOnly,
    talkStopSend,
    talkStart,
    sendLine,
    onSubmit,
    fmtSize,
    variant = "dock",
  }: {
    attach: { path: string; name: string; size: number } | null;
    onDropAttach: () => void;
    onPickAttach: () => void;
    onThin?: () => void;
    sending: boolean;
    draft?: string;
    fieldEl?: HTMLTextAreaElement | null;
    onFieldKeydown: (e: KeyboardEvent) => void;
    /** Faint rest of the top match. Dock variant only, aria-hidden. */
    ghost?: string;
    /** Caret moved / text changed: the dock re-decides the ghost. */
    onCaret?: () => void;
    /** IME open/closed: no ghost while composing. */
    onComposing?: (on: boolean) => void;
    neededHint?: string | null;
    listOpen?: boolean;
    activeDescendant?: string;
    talking: boolean;
    talkStopOnly: () => void;
    talkStopSend: () => void;
    talkStart: () => void;
    sendLine: () => void;
    /** Dock variant only. The line variant keeps sendLine. */
    onSubmit?: () => void;
    fmtSize: (n: number) => string;
    variant?: "line" | "dock";
  } = $props();

</script>

<form
    class="composer"
    class:line={variant === "line"}
    onsubmit={(e) => {
      e.preventDefault();
      if (variant === "line") {
        sendLine();
        return;
      }
      onSubmit?.();
    }}
  >
    {#if attach}
      <!-- The one chip — name + size, × to drop.
           Vanishes on send (CompanionPage clears it); an honest send
           error brings it back. -->
      <div class="chiprow">
        <span class="attachchip" title={attach.path}>
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
          <span class="attachname">{attach.name}</span>
          <span class="attachsize">{fmtSize(attach.size)}</span>
          <button
            type="button"
            class="attachx"
            onclick={onDropAttach}
            aria-label="Drop file"
            title="Drop file — it was never copied"
          >×</button>
        </span>
      </div>
    {/if}
    {#if variant !== "line"}
      <button
        type="button"
        class="thinbtn"
        onmousedown={(e) => e.stopPropagation()}
        onpointerdown={(e) => e.stopPropagation()}
        onclick={() => onThin?.()}
        disabled={sending}
        aria-label="Thin — label this chat"
        title="Label this chat thin (⌘⌥T). Mouth keeps going."
      >thin</button>
    {/if}
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
        onclick={onPickAttach}
        disabled={sending}
        aria-label="Attach one file"
        title="Attach one file (25 MB cap)"
      >+</button>
      <textarea
        class="field"
        rows="1"
        bind:this={fieldEl}
        bind:value={draft}
        onmousedown={(e) => e.stopPropagation()}
        onpointerdown={(e) => e.stopPropagation()}
        onkeydown={onFieldKeydown}
        oninput={onCaret}
        onkeyup={onCaret}
        onselect={onCaret}
        onclick={onCaret}
        oncompositionstart={() => onComposing?.(true)}
        oncompositionend={() => onComposing?.(false)}
        placeholder={neededHint
          ? `answer: ${neededHint}  ·  Rule: … to remember`
          : "type…  ·  todo: buy milk -fkt"}
        role="combobox"
        aria-autocomplete="list"
        aria-expanded={listOpen}
        aria-controls={listOpen ? "companion-launcher" : undefined}
        aria-activedescendant={activeDescendant || undefined}
        aria-label={neededHint ? `Answer: ${neededHint}` : "Message your companion"}
        autocomplete="off"
        spellcheck="false"
      ></textarea>
      {#if variant !== "line" && ghost}
        <!-- Ghost: the rest of the top match, faintly, right after the
             text. aria-hidden and click-through; Return never takes it. -->
        <span class="fieldghost" aria-hidden="true">
          <span class="fieldghost-typed">{draft}</span><span class="fieldghost-rest">{ghost}</span>
        </span>
      {/if}
      <span class="talkslot">
        {#if talking}
          <button
            type="button"
            class="talkic"
            onclick={talkStopOnly}
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
            onclick={talkStopSend}
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
            onclick={talkStart}
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
      disabled={sending || (!draft.trim() && !attach)}
      aria-label="Send"
      title="Send"
    >
      <svg viewBox="0 0 24 24" fill="currentColor" aria-hidden="true">
        <path d="M12 4l8 11h-4.5L12 10.4 8.5 15H4z" />
      </svg>
    </button>
  </form>
