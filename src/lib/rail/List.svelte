<script lang="ts">
  import PlanetHead from "../PlanetHead.svelte";
  import type { AgentRun, KanbanCard, Project, WindowInfo } from "../persist";
  import RailPlanetRm from "./PlanetRm.svelte";
  import RailPlanetsSheet from "./PlanetsSheet.svelte";
  import BoardSnapshot from "./BoardSnapshot.svelte";
  import WindowTiles from "./WindowTiles.svelte";
  import { boardSnapshot } from "./habitat";
  import type { RailApi } from "./api";

  let { rail }: { rail: RailApi } = $props();
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
        {#if rail.wakeNote}
          <div class="hermes-note" class:err={rail.wakeNote.kind === "err"}>
            <span class="hermes-text">{rail.wakeNote.text}</span>
            <button class="mini ghost" onclick={() => (rail.wakeNote = null)} aria-label="Dismiss">×</button>
          </div>
        {/if}
        <ul class="wins">
          {#each rail.projects as p (p.id)}
            {@const rows = rail.rowsFor(p.id)}
            {@const shown = rail.visibleRows(rows)}
            {@const agents = rail.agentRowsFor(p.id)}
            {@const snap = boardSnapshot(rail.kanbanCards, p.id, rail.railBoardCols)}
            <li
              class="section"
              class:drop-ok={rail.dropTarget === p.id}
              ondragenter={(e) => rail.onSectionDragEnter(e, p.id)}
              ondragover={(e) => rail.onSectionDragOver(e, p.id)}
              ondragleave={(e) => rail.onSectionDragLeave(e, p.id)}
              ondrop={(e) => rail.onSectionDrop(e, p.id)}
            >
              <div class="head-row">
                <button
                  type="button"
                  class="chev-btn"
                  aria-expanded={!rail.isCollapsed(p.id)}
                  aria-label={rail.isCollapsed(p.id) ? `Expand ${p.name}` : `Collapse ${p.name}`}
                  title={rail.isCollapsed(p.id) ? "Expand" : "Collapse"}
                  onclick={() => rail.toggleCollapse(p.id)}
                >
                  <span class="chev" class:open={!rail.isCollapsed(p.id)} aria-hidden="true">▸</span>
                </button>
                <PlanetHead
                  name={p.name}
                  chip={rail.planetCss(p.id)}
                  count={shown.length + agents.length + rail.planetCount(p.id)}
                  mkey={`rhead-${p.id}`}
                  mkeySel={rail.menuRing.activeId === `rhead-${p.id}`}
                  pid={p.id}
                  dropOk={rail.dropTarget === p.id}
                  hint="Bring this planet's windows forward"
                  onclick={() => rail.raisePlanetWindows(p.id)}
                />
                <RailPlanetRm {rail} project={p} />
              </div>
              {#if rail.rmErrorFor?.id === p.id}
                <p class="rm-err" role="alert">{rail.rmErrorFor.msg}</p>
              {/if}
              {#if !rail.isCollapsed(p.id)}
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
              {/if}
            </li>
          {/each}

          <li
            class="section"
            class:drop-ok={rail.dropTarget === rail.UNMAPPED_KEY}
            ondragenter={(e) => rail.onSectionDragEnter(e, rail.UNMAPPED_KEY)}
            ondragover={(e) => rail.onSectionDragOver(e, rail.UNMAPPED_KEY)}
            ondragleave={(e) => rail.onSectionDragLeave(e, rail.UNMAPPED_KEY)}
            ondrop={(e) => rail.onSectionDrop(e, null)}
          >
            <button
              class="section-head"
              onclick={() => rail.toggleCollapse(rail.UNMAPPED_KEY)}
              aria-expanded={!rail.isCollapsed(rail.UNMAPPED_KEY)}
              title={rail.isCollapsed(rail.UNMAPPED_KEY)
                ? "Expand — you can also drop a window on this header"
                : "Collapse"}
            >
              <span class="chev" class:open={!rail.isCollapsed(rail.UNMAPPED_KEY)} aria-hidden="true">▸</span>
              <span class="section-name">Unmapped</span>
              <span class="section-count">{rail.visibleRows(rail.unmapped).length}</span>
            </button>
            {#if !rail.isCollapsed(rail.UNMAPPED_KEY)}
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
            {/if}
          </li>
        </ul>
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

