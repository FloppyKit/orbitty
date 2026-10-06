<script lang="ts">
  import PlanetHead from "../PlanetHead.svelte";
  import type { AgentRun, KanbanCard, Project, WindowInfo } from "../persist";
  import RailPlanetRm from "./PlanetRm.svelte";
  import RailPlanetsSheet from "./PlanetsSheet.svelte";
  import BoardSnapshot from "./BoardSnapshot.svelte";
  import WindowTiles from "./WindowTiles.svelte";
  import { boardSnapshot } from "./habitat";
  import type { RailApi } from "./api";
  import { wheelCenter, wheelSlot, type StageEdge } from "./stageLook";

  let { rail }: { rail: RailApi } = $props();

  let prevCenter = $state(0);
  let wheelStamp = 0;

  const wheelIds = $derived([
    ...rail.projects.map((p: { id: string }) => p.id),
    rail.UNMAPPED_KEY,
  ]);
  const openPlanetId = $derived(
    wheelIds.find((id: string) => rail.stageRole(id) === "open") ?? null,
  );
  const centerIndex = $derived(
    wheelCenter(wheelIds, openPlanetId, rail.menuRing.activeId, prevCenter),
  );

  $effect(() => {
    if (prevCenter !== centerIndex) prevCenter = centerIndex;
  });

  function stageEdge(): StageEdge {
    const edge = rail.railEdge;
    return edge === "right" || edge === "top" || edge === "bottom" ? edge : "left";
  }

  function tileStyle(index: number, pid: string): string {
    const slot = wheelSlot(index - centerIndex, stageEdge());
    const tint = pid === rail.UNMAPPED_KEY ? "#8a8f98" : rail.planetCss(pid);
    return `--wx:${slot.x};--wy:${slot.y};--wr:${slot.rotate};--ws:${slot.scale};--wo:${slot.opacity};--wz:${slot.z};--planet:${tint}`;
  }

  function tileHidden(index: number): boolean {
    return wheelSlot(index - centerIndex, stageEdge()).hidden;
  }

  function onWheelColumn(e: WheelEvent) {
    const dy = Math.abs(e.deltaY) >= Math.abs(e.deltaX) ? e.deltaY : e.deltaX;
    if (dy === 0 || wheelIds.length === 0) return;
    const now = Date.now();
    if (now - wheelStamp < 120) return;
    wheelStamp = now;
    const dir = dy > 0 ? 1 : -1;
    const next = Math.max(0, Math.min(wheelIds.length - 1, centerIndex + dir));
    if (next !== prevCenter) prevCenter = next;
  }
</script>

{#snippet winRow(w: WindowInfo, project: Project | null)}
  {@const age = project ? rail.ageClass(w.id) : "fresh"}
  {@const sleeping = !!w.sleeping}
  <li>
    <div
      class="rowwrap"
      class:dragging={rail.dragWindowId === w.id}
      class:zfade={age === "fade"}
      class:zpark={age === "park"}
      class:zgone={age === "gone"}
      class:sleeping
      role="group"
      draggable="true"
      ondragstart={(e) => rail.onDragStart(e, w)}
      ondragend={rail.onDragEnd}
      title={sleeping
        ? "Asleep — the window is closed; click to wake it. Drag still re-homes."
        : "Drag onto a project to claim"}
    >
      {#if project}
        <button
          class="circle-btn"
          class:filling={rail.completingIds.includes(w.id)}
          onclick={() => rail.circleComplete(w.id, () => rail.completeWindow(w, project))}
          title="Complete — marks the card done, retires the sprite, closes the window"
          aria-label="Complete"
          >{#if rail.completingIds.includes(w.id)}✓{/if}</button
        >
      {/if}
      {#if rail.renamingId === w.id}
        <span class="row editing">
          <input
            class="rename-input"
            bind:value={rail.renameValue}
            placeholder={w.title || w.appName}
            aria-label="Rename window title (empty restores the live title)"
            autofocus
            onfocus={(e) => e.currentTarget.select()}
            onkeydown={(e) => {
              if (e.key === "Enter") rail.commitRename(w);
              if (e.key === "Escape") rail.renamingId = null;
            }}
            onblur={() => rail.commitRename(w)}
          />
        </span>
      {:else}
        <button
          class="row"
          class:front={w.isFrontmost}
          class:min={w.isMinimized}
          class:sleeping
          class:locate-flash={rail.locateFlashIds.includes(w.id)}
          onclick={() => (sleeping ? rail.wakeSleeper(w) : rail.locateRow(w))}
          ondblclick={() =>
            sleeping
              ? rail.wakeSleeper(w)
              : w.isMinimized
                ? rail.restore(w)
                : rail.stow(w)}
          oncontextmenu={(e) => rail.openCtxMenu(e, w)}
          title={sleeping
            ? `${w.appName} — ${w.title} · asleep — click wakes`
            : w.isMinimized
              ? `${w.appName} — ${w.title} · click glows, double-click restores`
              : `${w.appName} — ${w.title} · click glows, double-click minimizes`}
        >
          {#if w.iconBase64}
            <img class="app-icon" src="data:image/png;base64,{w.iconBase64}" alt="" />
          {:else}
            <span class="app-icon fallback" aria-hidden="true"
              >{w.appName.charAt(0) || "?"}</span
            >
          {/if}
          <span class="wtitle">{rail.titleFor(w)}</span>
          {#if sleeping}<span class="tag sleeptag">asleep</span>{/if}
          {#if !sleeping && w.isMinimized}<span class="tag stowtag">stowed</span>{/if}
          {#each rail.tagsFor(w.id) as t (t.id)}
            <span class="tag wtag" title="Tag: {t.name}">{t.name}</span>
          {/each}
          {#if age !== "fresh"}<span class="tag ztag">{age}</span>{/if}
          {#if w.isFrontmost}<span class="tag front-tag">front</span>{/if}
        </button>
      {/if}
    </div>
  </li>

{/snippet}

{#snippet agentRow(run: AgentRun)}
  {@const agent = rail.agentShortcuts.find((a) => a.id === run.agentShortcutId)}
  {@const card = rail.kanbanCards.find((k) => k.id === run.cardId)}
  {@const harness = (agent?.harness || run.herdr?.kind || "agent").toLowerCase()}
  <li>
    <div class="rowwrap" role="group" title="Live agent — click glows the droid and focuses Herdr">
      <button
        class="row"
        class:locate-flash={rail.locateFlashIds.includes(`agent:${run.cardId}`)}
        onclick={() => rail.locateAgent(run)}
        ondblclick={() => rail.locateAgent(run)}
      >
        <span class="app-icon fallback" aria-hidden="true">{harness.charAt(0).toUpperCase()}</span>
        <span class="wtitle">{agent?.name ?? harness}{card?.title ? ` — ${card.title}` : ""}</span>
        <span class="tag wtag">{harness}</span>
        {#if run.state === "blocked"}<span class="tag sleeptag">blocked</span>
        {:else if run.state === "working" || run.state === "starting"}<span class="tag front-tag">{run.state}</span>
        {/if}
      </button>
    </div>
  </li>

{/snippet}

{#snippet planetGlyph()}
  <svg class="line-icon" viewBox="0 0 32 32" aria-hidden="true">
    <circle cx="16" cy="16" r="6.5" fill="none" stroke="#8a8f98" stroke-width="1.7" />
    <ellipse
      cx="16"
      cy="16"
      rx="12"
      ry="4.5"
      fill="none"
      stroke="var(--planet, #8a8f98)"
      stroke-width="1.7"
      transform="rotate(-18 16 16)"
    />
  </svg>
{/snippet}

{#snippet starGlyph()}
  <svg class="line-icon" viewBox="0 0 32 32" aria-hidden="true">
    <path
      d="M16 5.5 18.7 12h6.8l-5.5 4.2 2.1 6.8L16 18.8 10 23l2.1-6.8L6.5 12h6.8Z"
      fill="none"
      stroke="#8a8f98"
      stroke-width="1.6"
      stroke-linejoin="round"
    />
  </svg>
{/snippet}

{#snippet gearGlyph()}
  <svg class="line-icon" viewBox="0 0 32 32" aria-hidden="true">
    <circle cx="16" cy="16" r="4" fill="none" stroke="#8a8f98" stroke-width="1.7" />
    <path
      d="M16 4.5v3.2M16 24.3v3.2M4.5 16h3.2M24.3 16h3.2M7.4 7.4l2.3 2.3M22.3 22.3l2.3 2.3M24.6 7.4l-2.3 2.3M9.7 22.3l-2.3 2.3"
      fill="none"
      stroke="#8a8f98"
      stroke-width="1.7"
      stroke-linecap="round"
    />
  </svg>
{/snippet}

<div class="scroll">
    {#if rail.status && !rail.status.trusted}
      <section class="card">
        <h2>Accessibility access required</h2>
        <p class="body">
          Orbitty needs Accessibility to list and focus windows. macOS ties the
          grant to this exact build — enable it for:
        </p>
        <p class="path">{rail.status.bundlePath}</p>
        <p class="signing">{rail.status.signingLine}</p>
        <div class="actions">
          <button class="primary" onclick={rail.openSettings}>Open System Settings</button>
          <button onclick={rail.retry}>Retry</button>
          <button class="ghost" onclick={rail.copyPath}>{rail.copied ? "Copied" : "Copy path"}</button>
        </div>
        <p class="hint">
          Privacy &amp; Security → Accessibility → enable the toggle for this app,
          then Retry. This view updates automatically once granted.
        </p>
      </section>
    {:else if rail.status && rail.stateLoaded}
      {#if rail.embedded}
        <RailPlanetsSheet {rail} />
      {:else if rail.lane === "agents"}
        <p class="lane-hint">
          Hermes opens your existing chat app. Soul is a note you Save —
          Orbitty doesn't write it for you.
        </p>
        {#if rail.hermesNote}
          <div class="hermes-note" class:err={rail.hermesNote.kind === "err"}>
            <span class="hermes-text">{rail.hermesNote.text}</span>
            <button class="mini ghost" onclick={() => (rail.hermesNote = null)} aria-label="Dismiss">×</button>
          </div>
        {/if}
        {#if rail.projects.length === 0}
          <p class="lane-empty">
            No projects yet — create one in Projects and its Agents show up here.
          </p>
        {:else}
          <ul class="wins">
            {#each rail.projects as p (p.id)}
              <li class="agent-row">
                <span class="agent-name">{p.name}</span>
                <button
                  class="mini"
                  onclick={() => rail.launchHermes(p)}
                  title="Hermes — focus or launch the existing harness with this planet's titles-only context"
                >Hermes</button>
                <button
                  class="mini"
                  class:active={rail.soulFor === p.id}
                  onclick={() => rail.toggleSoul(p)}
                  title="soul.md — read always; writes only via Save"
                >Soul</button>
              </li>
              {#if rail.soulFor === p.id}
                <li>
                  <div class="soul-panel agent-soul">
                    <textarea
                      class="soul-body"
                      rows="6"
                      bind:value={rail.soulBody}
                      spellcheck="false"
                      placeholder="Loading soul.md…"
                    ></textarea>
                    <div class="soul-actions">
                      <button
                        class="mini"
                        onclick={rail.saveSoul}
                        title="The only writer of soul.md">Save</button>
                      <button
                        class="mini"
                        onclick={rail.openSoulEditor}
                        title={rail.soulPath
                          ? `Open in the default editor: ${rail.soulPath} (opens the file on disk — Save first if edited)`
                          : "Open in the default editor"}>Open in editor</button>
                      {#if rail.soulMsg}<span class="soul-msg">{rail.soulMsg}</span>{/if}
                    </div>
                  </div>
                </li>
              {/if}
            {/each}
          </ul>
        {/if}
      {:else if rail.windows.length === 0 && rail.projects.length === 0}
        <p class="empty">No windows found.</p>
      {:else}
        {#if rail.nudgeWindow}
          <div class="nudge">
            <span class="nudge-text">
              <b>{rail.nudgeWindow.appName}</b>
              {rail.nudgeWindow.title || "window"} minimized — no home, no sprite. Claim
              one?
            </span>
            <div class="nudge-actions">
              {#each rail.projects as p (p.id)}
                <button class="mini" onclick={() => rail.nudgeClaim(p.id)}>{p.name}</button>
              {/each}
              <span class="newproj">
                <input
                  bind:value={rail.nudgeNewName}
                  placeholder="New project…"
                  onkeydown={(e) => e.key === "Enter" && rail.nudgeCreate()}
                />
                <button class="mini" onclick={rail.nudgeCreate}>Claim</button>
              </span>
              <button class="mini ghost" onclick={() => (rail.nudgeFor = null)}>Later</button>
            </div>
          </div>
        {/if}
        {#if rail.projects.length === 0}
          <p class="proj-hint">
            No projects yet — create one with <b>+ Project</b> above, then drag
            a window onto it to claim.
          </p>
        {/if}
        <div class="stage-hit" aria-hidden="true"></div>
        <div class="stage-strip">
        <div class="pinned-cluster">
        <div class="stage-front" aria-label="Rail">
          <button
            type="button"
            class="wheel-tile pinned-tile"
            data-mkey="rstars"
            class:mkey-sel={rail.menuRing.activeId === "rstars"}
            title="Stars"
            aria-label="Stars"
            onclick={() => rail.openStageDest("stars")}
          >{@render starGlyph()}<span class="wheel-name">Stars</span></button>
          <button
            type="button"
            class="wheel-tile pinned-tile"
            data-mkey="rsettings"
            class:mkey-sel={rail.menuRing.activeId === "rsettings"}
            title="Settings"
            aria-label="Settings"
            onclick={() => rail.openStageDest("settings")}
          >{@render gearGlyph()}<span class="wheel-name">Settings</span></button>
        </div>
        <div class="col-toggles" role="group" aria-label="Board columns">
          {#each [{ id: "todo", label: "Todo" }, { id: "doing", label: "Doing" }, { id: "done", label: "Done" }] as col (col.id)}
            <button
              type="button"
              class="col-toggle"
              class:on={rail.railBoardCols.includes(col.id)}
              class:mkey-sel={rail.menuRing.activeId === `rcol-${col.id}`}
              data-mkey={`rcol-${col.id}`}
              aria-pressed={rail.railBoardCols.includes(col.id)}
              onclick={() => rail.toggleBoardCol(col.id)}
            >{col.label}</button>
          {/each}
        </div>
        </div>
        <div class="wheel-col" onwheel={onWheelColumn}>
          <div class="wheel-band" aria-hidden="true"></div>
          <div class="wheel-notch" aria-hidden="true"></div>
        {#if rail.wakeNote}
          <div class="hermes-note" class:err={rail.wakeNote.kind === "err"}>
            <span class="hermes-text">{rail.wakeNote.text}</span>
            <button class="mini ghost" onclick={() => (rail.wakeNote = null)} aria-label="Dismiss">×</button>
          </div>
        {/if}
        <ul class="wins">
          {#each rail.projects as p, i (p.id)}
            {@const rows = rail.rowsFor(p.id)}
            {@const shown = rail.visibleRows(rows)}
            {@const agents = rail.agentRowsFor(p.id)}
            {@const snap = boardSnapshot(rail.kanbanCards, p.id, rail.railBoardCols)}
            {@const role = rail.stageRole(p.id)}
            <li
              class="section stage-planet"
              class:receded={role === "receded"}
              class:open={role === "open"}
              class:drop-ok={rail.dropTarget === p.id}
              ondragenter={(e) => rail.onSectionDragEnter(e, p.id)}
              ondragover={(e) => rail.onSectionDragOver(e, p.id)}
              ondragleave={(e) => rail.onSectionDragLeave(e, p.id)}
              ondrop={(e) => rail.onSectionDrop(e, p.id)}
            >
              <button
                type="button"
                class="wheel-tile"
                class:is-selected={i === centerIndex}
                class:is-hidden={tileHidden(i)}
                style={tileStyle(i, p.id)}
                data-mkey={`rhead-${p.id}`}
                class:mkey-sel={rail.menuRing.activeId === `rhead-${p.id}`}
                aria-expanded={role === "open"}
                aria-label={role === "open" ? p.name : `Expand ${p.name}`}
                title={p.name}
                onclick={() => rail.ensureExpand(p.id)}
              >{@render planetGlyph()}<span class="wheel-name">{p.name}</span></button>
              {#if role === "open"}
                <div class="stage-preview" style={`--planet:${rail.planetCss(p.id)}`}>
                  <div class="name-pill">
                    {@render planetGlyph()}
                    <span class="wheel-name">{p.name}</span>
                  </div>
                  <div class="preview-body">
                  <div class="head-row preview-head">
                  <PlanetHead
                    name={p.name}
                    chip={rail.planetCss(p.id)}
                    count={shown.length + agents.length + rail.planetCount(p.id)}
                    pid={p.id}
                    dropOk={rail.dropTarget === p.id}
                    hint="This planet is open"
                    onclick={() => rail.collapsePlanet(p.id)}
                  />
                  <button
                    type="button"
                    class="mini raise-btn"
                    class:mkey-sel={rail.menuRing.activeId === `rraise-${p.id}`}
                    data-mkey={`rraise-${p.id}`}
                    title="Bring this planet's windows forward"
                    aria-label={`Bring ${p.name} windows forward`}
                    onclick={() => rail.raisePlanetWindows(p.id)}
                  >Raise</button>
                  <button
                    type="button"
                    class="mini ghost"
                    title="Back — bring the other planets forward"
                    aria-label={`Collapse ${p.name}`}
                    onclick={() => rail.collapsePlanet(p.id)}
                  >Back</button>
                  <RailPlanetRm {rail} project={p} />
                  </div>
              {#if rail.rmErrorFor?.id === p.id}
                <p class="rm-err" role="alert">{rail.rmErrorFor.msg}</p>
              {/if}
                {#if p.name.trim().toLowerCase() !== "void"}
                  <input
                    class="root-path"
                    value={p.workspaceRoot ?? ""}
                    placeholder="workspace root"
                    title="Planet workspace root — task snapshots clone this path"
                    onchange={(e) => rail.setWorkspaceRoot(p.id, e.currentTarget.value)}
                  />
                {/if}
                <WindowTiles {rail} project={p} rows={shown} />
                <BoardSnapshot
                  columns={snap}
                  completingIds={rail.completingIds}
                  activeId={rail.menuRing.activeId}
                  onOpen={(id) => rail.openKanbanFocus(id)}
                  onComplete={(id) => {
                    const card = rail.kanbanCards.find((c: KanbanCard) => c.id === id);
                    if (card) rail.circleComplete(card.id, () => rail.completeTodo(card));
                  }}
                />
                <button
                  type="button"
                  class="todos-jump"
                  class:mkey-sel={rail.menuRing.activeId === `rtodos-${p.id}`}
                  data-mkey={`rtodos-${p.id}`}
                  onclick={() => rail.openPlanetTodos(p.id)}
                >Todos {rail.planetCount(p.id)} ›</button>
                {#if agents.length > 0}
                  <ul class="wins">
                    {#each agents as r (r.cardId)}
                      {@render agentRow(r)}
                    {/each}
                  </ul>
                {/if}
                {#if shown.length === 0 && agents.length === 0}
                  <p class="section-empty">
                    {rail.tagFilter
                      ? "No windows with this tag"
                      : "Drop a window here to claim it"}
                  </p>
                {/if}
                <div class="todo-add">
                    {#if rail.addingTodoFor === p.id}
                      <span class="newproj">
                        <input
                          bind:value={rail.newTodoTitle}
                          placeholder="New to-do…"
                          onkeydown={(e) => e.key === "Enter" && rail.addTodoTo(p.id)}
                        />
                        <button class="mini" onclick={() => rail.addTodoTo(p.id)}>Add</button>
                        <button class="mini ghost" onclick={() => (rail.addingTodoFor = null)}>×</button>
                      </span>
                    {:else}
                      <button
                        class="mini ghost"
                        onclick={() => {
                          rail.addingTodoFor = p.id;
                          rail.newTodoTitle = "";
                        }}>+ To-do</button
                      >
                    {/if}
                </div>
                  </div>
                </div>
              {/if}
            </li>
          {/each}

          <li
            class="section stage-planet"
            class:receded={rail.stageRole(rail.UNMAPPED_KEY) === "receded"}
            class:open={rail.stageRole(rail.UNMAPPED_KEY) === "open"}
            class:drop-ok={rail.dropTarget === rail.UNMAPPED_KEY}
            ondragenter={(e) => rail.onSectionDragEnter(e, rail.UNMAPPED_KEY)}
            ondragover={(e) => rail.onSectionDragOver(e, rail.UNMAPPED_KEY)}
            ondragleave={(e) => rail.onSectionDragLeave(e, rail.UNMAPPED_KEY)}
            ondrop={(e) => rail.onSectionDrop(e, null)}
          >
            <button
              type="button"
              class="wheel-tile"
              class:is-selected={rail.projects.length === centerIndex}
              class:is-hidden={tileHidden(rail.projects.length)}
              style={tileStyle(rail.projects.length, rail.UNMAPPED_KEY)}
              aria-expanded={rail.stageRole(rail.UNMAPPED_KEY) === "open"}
              title="Unmapped"
              aria-label="Unmapped"
              onclick={() =>
                rail.stageRole(rail.UNMAPPED_KEY) === "open"
                  ? rail.collapsePlanet(rail.UNMAPPED_KEY)
                  : rail.ensureExpand(rail.UNMAPPED_KEY)}
            >{@render planetGlyph()}<span class="wheel-name">Unmapped</span></button>
            {#if rail.stageRole(rail.UNMAPPED_KEY) === "open"}
              <div class="stage-preview" style="--planet:#8a8f98">
                <div class="name-pill">
                  {@render planetGlyph()}
                  <span class="wheel-name">Unmapped</span>
                </div>
                <div class="preview-body">
              <ul class="wins">
                {#each rail.visibleRows(rail.unmapped) as w (w.id)}
                  {@render winRow(w, null)}
                {/each}
                {#if rail.visibleRows(rail.unmapped).length === 0}
                  <li class="section-empty">
                    {rail.tagFilter ? "No windows with this tag" : "Drop here to unclaim"}
                  </li>
                {/if}
              </ul>
                </div>
              </div>
            {/if}
          </li>
        </ul>
        </div>
        </div>
      {/if}
    {/if}

</div>

  {#if !rail.embedded}
    <footer class="foot">
      Drag a row onto a project to claim · stowed home windows walk their planet
      · <a
        href="https://orbitty.space"
        onclick={(e) => {
          e.preventDefault();
          rail.shellOpen("https://orbitty.space").catch(() => {});
        }}>orbitty.space</a>
    </footer>
  {/if}

