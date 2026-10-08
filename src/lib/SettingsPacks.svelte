<script lang="ts">
  import { open as shellOpen } from "@tauri-apps/plugin-shell";
  let {
    packBusy,
    importPackFolder,
    importPackZip,
    importPackImage,
    packsDir,
    packNote,
    packs,
    removePack,
  } = $props();
</script>

      <section class="panel">
        <div class="setting-row">
          <button
            class="mini"
            disabled={packBusy}
            onclick={importPackFolder}
            title="Pick a folder of PNG frames (optional manifest.json)"
          >Import folder…</button>
          <button
            class="mini"
            disabled={packBusy}
            onclick={importPackZip}
            title="Pick a .zip you downloaded"
          >Import zip…</button>
          <button
            class="mini"
            disabled={packBusy}
            onclick={importPackImage}
            title="Pick a PNG/GIF/WebP/JPEG"
          >Import image…</button>
          {#if packsDir}
            <button
              class="mini ghost"
              onclick={() => shellOpen(packsDir).catch(() => {})}
              title={packsDir}
            >Reveal</button>
          {/if}
        </div>
        {#if packNote}
          <p class="help" class:err={packNote.kind === "err"}>{packNote.text}</p>
        {/if}
        {#if packs.length > 0}
          <ul class="pack-list">
            {#each packs.filter((p: { role: string; id: string; name: string; path: string; frameCount: number; license: string | null }) => p.role !== "companion") as p (p.id)}
              <li class="pack-row">
                <span class="pack-name" title={p.path}>{p.name}</span>
                <span class="pack-meta">{p.frameCount} frame{p.frameCount === 1 ? "" : "s"}</span>
                {#if p.license}
                  <span class="pack-license">{p.license}</span>
                {:else}
                  <span class="pack-license warn">no license ⚠</span>
                {/if}
                <button
                  class="mini ghost"
                  onclick={() => removePack(p.id)}
                  title="Remove this pack — deletes only its own subfolder"
                >×</button>
              </li>
            {/each}
          </ul>
        {:else}
          <p class="help">A single PNG/JPEG/GIF/WebP is enough (it stands still).</p>
        {/if}
        <p class="help">
          Walkers are the built-in Season 1 bitty crew or imported from your
          files — Orbitty never downloads art. Missing license warns, never
          blocks.
        </p>
      </section>

<style>
.panel {
    background: var(--panel-fill, var(--panel));
    border: 1px solid var(--edge);
    border-radius: 10px;
    padding: 10px 12px 12px;
  }
.setting-row {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 4px;
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
.mini.ghost {
    background: transparent;
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
.pack-list {
    list-style: none;
    margin: 8px 0 0;
    padding: 0;
    display: flex;
    flex-direction: column;
    gap: 2px;
  }
.pack-row {
    display: flex;
    align-items: center;
    gap: 6px;
    min-height: 22px;
  }
.pack-name {
    flex: 1;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    font-size: 0.72rem;
    font-weight: 600;
    color: var(--ink);
  }
.pack-meta {
    flex-shrink: 0;
    font-size: 0.64rem;
    color: var(--dim);
  }
.pack-license {
    flex-shrink: 0;
    font-size: 0.62rem;
    color: var(--dim);
    border: 1px solid var(--edge);
    border-radius: 999px;
    padding: 0 0.35rem;
  }
.pack-license.warn {
    color: var(--accent);
    border-color: var(--accent);
  }
</style>
