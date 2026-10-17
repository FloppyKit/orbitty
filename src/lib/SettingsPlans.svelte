<script lang="ts">
  let {
    plansDir = $bindable(),
    savePlansDir,
    revealPlans,
    plansRunner = $bindable(),
    plansHarness = $bindable(),
    plansModel = $bindable(),
    saveRunner,
    plansNote,
  } = $props();
</script>

      <section data-glass class="panel">
        <p class="block-label">Plans folder</p>
        <div class="setting-row">
          <input class="plans-path" bind:value={plansDir} spellcheck="false" />
          <button class="mini" onclick={savePlansDir}>Save</button>
          <button class="mini" onclick={revealPlans} title="Reveal in Finder">Reveal in Finder</button>
        </div>
        <p class="help">Shared by Nightly and stable. Default is the visible Orbitty GOs folder in your home.</p>
        <p class="block-label">Default runner</p>
        <p class="help">Doing uses this when a GO has no agent. A new install starts blank. Change it any time.</p>
        <div class="setting-row">
          <input class="plans-path" bind:value={plansRunner} placeholder="runner name" spellcheck="false" />
          <input class="plans-path" bind:value={plansHarness} placeholder="cursor, grok, hermes, pi" spellcheck="false" />
          <input class="plans-path" bind:value={plansModel} placeholder="model" spellcheck="false" />
          <button class="mini" onclick={saveRunner}>Save</button>
        </div>
        {#if plansNote}
          <p class="help err">{plansNote}</p>
        {/if}
      </section>

<style>
.panel {
    background: var(--panel-fill, var(--panel));
    border: 1px solid var(--edge);
    border-radius: 10px;
    padding: 10px 12px 12px;
  }
.block-label {
    margin: 8px 0 4px;
    font-size: 0.64rem;
    text-transform: uppercase;
    letter-spacing: 0.08em;
    color: var(--dim);
  }
.setting-row {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 4px;
  }
.plans-path {
    flex: 1;
    min-width: 12rem;
    font: inherit;
    font-size: 0.72rem;
    color: var(--ink);
    background: var(--bg-fill, var(--bg));
    border: 1px solid var(--edge);
    border-radius: 7px;
    padding: 3px 8px;
  }
.mini {
    font: inherit;
    font-size: 0.68rem;
    color: var(--ink);
    background: var(--bg-fill, var(--bg));
    border: 1px solid var(--edge);
    border-radius: 7px;
    padding: 3px 8px;
    cursor: pointer;
  }
.mini:hover {
    border-color: var(--accent);
  }
.mini:disabled {
    opacity: 0.5;
    cursor: default;
  }
.help {
    margin: 8px 0 0;
    font-size: 0.64rem;
    line-height: 1.5;
    color: var(--dim);
  }
.help.err {
    color: var(--accent);
  }
</style>
