<script lang="ts">
  let {
    win1 = "",
    win2 = "",
    grateful = "",
    editable = false,
    onChange,
  }: {
    win1?: string;
    win2?: string;
    grateful?: string;
    editable?: boolean;
    onChange: (field: "win1" | "win2" | "grateful", value: string) => void;
  } = $props();
</script>

<div class="positives">
  <input
    class="field"
    placeholder="Win 1"
    aria-label="Win 1"
    maxlength="80"
    value={win1}
    readonly={!editable}
    oninput={(e) => onChange("win1", e.currentTarget.value)}
  />
  <input
    class="field"
    placeholder="Win 2"
    aria-label="Win 2"
    maxlength="80"
    value={win2}
    readonly={!editable}
    oninput={(e) => onChange("win2", e.currentTarget.value)}
  />
  <input
    class="field"
    placeholder="Grateful"
    aria-label="Grateful"
    maxlength="80"
    value={grateful}
    readonly={!editable}
    oninput={(e) => onChange("grateful", e.currentTarget.value)}
  />
</div>

<style>
  .positives {
    display: flex;
    flex-direction: column;
    gap: 4px;
    margin-top: 6px;
  }

  .field {
    font: inherit;
    font-size: 0.72rem;
    color: var(--ink);
    background: var(--bg-fill, var(--bg));
    border: 1px solid var(--edge);
    border-radius: 7px;
    padding: 4px 6px;
  }

  .field:read-only {
    opacity: 0.8;
  }
</style>
