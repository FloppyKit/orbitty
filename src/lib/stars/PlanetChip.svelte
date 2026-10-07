<script lang="ts">
  import { planetCss } from "../planetColor";

  let { id, size = 10, label = "" }: { id: string; size?: number; label?: string } = $props();
  const color = $derived(planetCss(id));
</script>

<span
  class="planet-chip"
  style="--chip: {size}px; --p: {color}"
  title={label || undefined}
  aria-hidden="true"
></span>

<style>
  .planet-chip {
    display: inline-block;
    flex: none;
    width: var(--chip);
    height: var(--chip);
    min-width: var(--chip);
    min-height: var(--chip);
    aspect-ratio: 1 / 1;
    border-radius: 50%;
    background: radial-gradient(
      circle at 34% 30%,
      color-mix(in srgb, var(--p) 78%, var(--ink) 22%) 0 18%,
      var(--p) 46%,
      color-mix(in srgb, var(--p) 62%, black) 100%
    );
    box-shadow: 0 0 0 1px rgba(0, 0, 0, 0.28);
  }

  :global(:root[data-theme="terminal"]) .planet-chip {
    background: var(--p);
    box-shadow: none;
  }
</style>
