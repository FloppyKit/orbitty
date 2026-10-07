<script lang="ts">
  let {
    win1 = "",
    win2 = "",
    grateful = "",
    editable = false,
    onChange,
    onFieldKey,
  }: {
    win1?: string;
    win2?: string;
    grateful?: string;
    editable?: boolean;
    onChange: (field: "win1" | "win2" | "grateful", value: string) => void;
    onFieldKey?: (key: "Enter" | "Escape") => void;
  } = $props();

  function onKey(e: KeyboardEvent) {
    if (e.key === "Enter" || e.key === "Escape") {
      e.preventDefault();
      e.stopPropagation();
      onFieldKey?.(e.key);
    }
  }
</script>

<div class="positives">
  <input
    class="field"
    placeholder="Win 1"
    aria-label="Win 1"
    data-focus-zone="Z3"
    data-focus-item
    maxlength="80"
    value={win1}
    readonly={!editable}
    oninput={(e) => onChange("win1", e.currentTarget.value)}
    onkeydown={onKey}
  />
  <input
    class="field"
    placeholder="Win 2"
    aria-label="Win 2"
    data-focus-zone="Z3"
    data-focus-item
    maxlength="80"
    value={win2}
    readonly={!editable}
    oninput={(e) => onChange("win2", e.currentTarget.value)}
    onkeydown={onKey}
  />
  <input
    class="field"
    placeholder="Grateful"
    aria-label="Grateful"
    data-focus-zone="Z3"
    data-focus-item
    maxlength="80"
    value={grateful}
    readonly={!editable}
    oninput={(e) => onChange("grateful", e.currentTarget.value)}
    onkeydown={onKey}
  />
</div>

<style>
  .positives {
    flex: none;
    display: flex;
    flex-direction: column;
    gap: 2px;
    margin-top: 2px;
  }

  .field {
    font: inherit;
    font-size: 0.75rem;
    color: var(--ink);
    background: transparent;
    border: none;
    border-bottom: 1px solid var(--edge);
    border-radius: 0;
    padding: 5px 2px 4px;
    min-height: 24px;
  }

  .field::placeholder {
    color: var(--dim);
    font-weight: 450;
  }

  .field:focus,
  .field:focus-visible {
    outline: none;
    border-bottom-color: var(--accent);
  }

  .field:read-only {
    opacity: 0.8;
  }
</style>
