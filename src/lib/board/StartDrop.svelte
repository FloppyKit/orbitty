<script lang="ts">
  let {
    active,
    dropOk = false,
    onOver,
    onLeave,
    onDrop,
  }: {
    active: boolean;
    dropOk?: boolean;
    onOver: (e: DragEvent) => void;
    onLeave: (e: DragEvent) => void;
    onDrop: (e: DragEvent) => void;
  } = $props();
</script>

{#if active}
  <div
    class="start-drop"
    class:drop-ok={dropOk}
    data-col="doing"
    ondragover={onOver}
    ondragleave={onLeave}
    ondrop={onDrop}
  >
    Drop to start
  </div>
{/if}

<style>
  .start-drop {
    margin: 6px 10px 2px;
    min-height: 36px;
    display: flex;
    align-items: center;
    justify-content: center;
    border: 1px dashed var(--dim, #8a97a8);
    border-radius: 8px;
    color: var(--dim, #8a97a8);
    font-size: 0.82rem;
    letter-spacing: 0.02em;
  }
  .start-drop.drop-ok {
    border-color: var(--ink, #d5dce6);
    color: var(--ink, #d5dce6);
    background: color-mix(in srgb, var(--ink, #d5dce6) 8%, transparent);
  }
</style>
