<script lang="ts">
  import type { CompanionApi } from "./api";
  import type { PersistedState } from "../persist";
  let { comp }: { comp: CompanionApi } = $props();
</script>

{#if comp.claimToast}
  <div class="claim-toast" role="status">
    <span>Claimed to {comp.claimToast.projectName}</span>
    <button
      type="button"
      onclick={() => {
        if (!comp.claimToast) return;
        void (async () => {
          try {
            const s = await comp.invoke<PersistedState>("load_state");
            const list = s.projects ?? [];
            if (!list.length) return;
            const i = list.findIndex((p) => p.id === comp.claimToast!.projectId);
            const next = list[(i + 1) % list.length];
            if (!next) return;
            await comp.invoke("claim_front_change", { projectId: next.id });
            comp.claimToast = {
              ...comp.claimToast!,
              projectId: next.id,
              projectName: next.name,
            };
          } catch {
            /* ignore */
          }
        })();
      }}>Change</button
    >
    <button
      type="button"
      onclick={() => {
        comp.invoke("claim_front_undo").catch(() => {});
        if (comp.claimToastTimer) clearTimeout(comp.claimToastTimer);
        comp.claimToast = null;
      }}>Undo</button
    >
  </div>
{/if}
