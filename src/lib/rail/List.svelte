<script lang="ts">
  import type { Project, WindowInfo } from "../persist";
  import {
    COACH_CLAIM_LABEL,
    COACH_CLAIM_STOP,
    COACH_LINE,
    COACH_SKIP_STOP,
    COACH_TIP,
    coachDue,
    emptyDeskShowsWheel,
    normalizeStory,
  } from "./coach";
  import KanbanPage from "../KanbanPage.svelte";
  import RailPlanetsSheet from "./PlanetsSheet.svelte";
  import type { RailApi } from "./api";
  import {
    SPIN_SETTLE_MS,
    spinAccumulate,
    spinCenterFor,
    spinConfirm,
    spinInitial,
    spinItems,
    spinJump,
    spinOffset,
    spinStep,
    type SpinAccumState,
    type SpinItem,
    type StageEdge,
  } from "./stageLook";
  import {
    archBack,
    archCentreChanged,
    archConfirm,
    archDepth,
    archEnter,
    archForward,
    archInitial,
    archLayer2Items,
    archLayout,
    archOpenBoard,
    archScroll2,
    archSlot,
    archZone,
    type ArchCard,
    type ArchFront,
    type ArchState,
    type ArchZone,
    type Layer2Item,
  } from "./stageArch";

  let { rail }: { rail: RailApi } = $props();

  const NEUTRAL = "#8a8f98";

  let center = $state(0);
  let booted = false;
  let stageEl = $state<HTMLDivElement | null>(null);
  let boardPlanet = $state("inbox");
  let nameEl = $state<HTMLInputElement | null>(null);
  let accum: SpinAccumState = { residual: 0, lastAt: 0 };
  let prevOffsets: number[] = [];
  let havePrev = false;
  let jumpMask = $state<boolean[]>([]);

  const items = $derived(
    spinItems(
      rail.projects.map((p: { id: string }) => p.id),
      rail.UNMAPPED_KEY,
    ),
  );
  const offsets = $derived(items.map((_, i) => spinOffset(i, center, items.length)));
  const archState = $derived(rail.arch as ArchState);
  const layer2 = $derived.by(() => {
    const pid = archState?.pid as string | null;
    if (!pid || pid === rail.UNMAPPED_KEY) return [] as Layer2Item[];
    const cards = (rail.planetCards(pid) ?? []) as ArchCard[];
    const rows = rail.rowsFor(pid) as WindowInfo[];
    return archLayer2Items({
      pid,
      cards,
      rows: rows.map((w) => ({
        id: w.id,
        title: rail.titleFor(w),
        sleeping: !!w.sleeping,
        isMinimized: !!w.isMinimized,
        iconBase64: w.iconBase64,
        appName: w.appName,
      })),
    });
  });

  $effect(() => {
    const list = items;
    const ready = !!rail.stateLoaded;
    const openId = typeof rail.openSectionId === "function" ? rail.openSectionId() : null;
    const active = (rail.menuRing?.activeId ?? null) as string | null;
    if (!booted) {
      if (!ready) return;
      center = spinInitial(list, openId);
      rail.spinCenter = center;
      const planetCount = list.filter((it) => it.kind === "planet").length;
      rail.archDo(archInitial(list, center, planetCount));
      booted = true;
      return;
    }
    const onWheel = !!active && list.some((it) => it.stop === active);
    const next = spinCenterFor(list, onWheel ? active : null, center);
    if (next !== center) center = next;
    if (rail.spinCenter !== center) rail.spinCenter = center;
  });

  $effect(() => {
    const pid = archState?.pid as string | null;
    const front = archState?.front as ArchFront | undefined;
    if (front !== "board" || !pid || pid === rail.UNMAPPED_KEY) return;
    boardPlanet = pid;
  });

  $effect(() => {
    const next = center;
    const item = items[next];
    if (!booted) return;
    const handle = setTimeout(() => {
      rail.archDo(archCentreChanged(rail.arch, item));
    }, SPIN_SETTLE_MS);
    return () => clearTimeout(handle);
  });

  $effect(() => {
    const next = offsets;
    if (!havePrev) {
      prevOffsets = next.slice();
      havePrev = true;
      return;
    }
    const mask = next.map((k, i) => spinJump(prevOffsets[i] ?? k, k));
    prevOffsets = next.slice();
    if (mask.some(Boolean)) {
      jumpMask = mask;
      requestAnimationFrame(() => {
        jumpMask = [];
      });
    }
  });

  $effect(() => {
    if (!rail.creatingProject) return;
    queueMicrotask(() => nameEl?.focus());
  });

  $effect(() => {
    const el = stageEl;
    if (!el) return;
    const onWheel = (e: WheelEvent) => {
      const zone = zoneAt(e);
      const front = (rail.arch?.front ?? "arch") as ArchFront;
      if (zone === "none" || zone === "board") return;
      if (zone === "layer2" && front === "layer2" && rail.arch?.pid === rail.UNMAPPED_KEY) return;
      e.preventDefault();
      if (zone === "arch") {
        if (front !== "arch") {
          bringArch();
          return;
        }
        const n = items.length;
        if (n === 0) return;
        const out = spinAccumulate(
          accum,
          { deltaX: e.deltaX, deltaY: e.deltaY, deltaMode: e.deltaMode },
          Date.now(),
        );
        accum = out.state;
        if (out.steps === 0) return;
        const next = spinStep(center, out.steps, n);
        center = next;
        const active = rail.menuRing.activeId as string | null;
        if (active && items.some((it) => it.stop === active)) {
          rail.menuRing.select(items[next].stop);
        }
        return;
      }
      if (zone === "layer2") {
        if (front !== "layer2") {
          if (front === "board") rail.archDo(archBack(rail.arch));
          else rail.archDo(archForward(rail.arch));
          return;
        }
        const out = spinAccumulate(
          accum,
          { deltaX: e.deltaX, deltaY: e.deltaY, deltaMode: e.deltaMode },
          Date.now(),
        );
        accum = out.state;
        if (out.steps === 0) return;
        const next = archScroll2(rail.arch, out.steps, layer2.length);
        rail.archDo(next);
        const active = rail.menuRing.activeId as string | null;
        if (next.front === "layer2" && active && layer2.some((it) => it.stop === active)) {
          const hit = layer2[next.center2];
          if (hit) rail.menuRing.select(hit.stop);
        }
      }
    };
    el.addEventListener("wheel", onWheel, { passive: false });
    return () => el.removeEventListener("wheel", onWheel);
  });

  function stageEdge(): StageEdge {
    const edge = rail.railEdge;
    return edge === "right" || edge === "top" || edge === "bottom" ? edge : "left";
  }

  function tintFor(item: SpinItem): string {
    if (item.kind === "planet" && item.pid) return rail.planetCss(item.pid);
    return NEUTRAL;
  }

  function slotStyle(k: number, ring: 1 | 2, tint: string): string {
    const slot = archSlot(k, stageEdge(), ring);
    return `--wx:${slot.x};--wy:${slot.y};--ws:${slot.scale};--wo:${slot.opacity};--wz:${slot.z};--planet:${tint}`;
  }

  function archHidden(index: number): boolean {
    return archSlot(offsets[index] ?? 0, stageEdge(), 1).hidden;
  }

  function layerHidden(index: number): boolean {
    const k = index - (archState?.center2 ?? 0);
    return archSlot(k, stageEdge(), 2).hidden;
  }

  function depthStyle(layer: 1 | 2): string {
    const front = (archState?.front ?? "arch") as ArchFront;
    const d = archDepth(front, layer, !!rail.reduceMotion);
    const edge = stageEdge();
    let x = 0;
    let y = 0;
    if (edge === "left") x = -d.shift;
    else if (edge === "right") x = d.shift;
    else if (edge === "top") y = -d.shift;
    else y = d.shift;
    const blur = d.blur > 0 ? `blur(${d.blur}px)` : "none";
    return `--ad-scale:${d.scale};--ad-opacity:${d.opacity};--ad-x:${x}px;--ad-y:${y}px;--ad-ms:${d.durationMs}ms;filter:${blur}`;
  }

  function bringArch() {
    let s = rail.arch as ArchState;
    if (s.front === "board") s = archBack(s);
    if (s.front === "layer2") s = archBack(s);
    rail.archDo(s);
  }

  function zoneAt(e: { clientX: number; clientY: number }): ArchZone {
    const el = stageEl;
    if (!el) return "none";
    const rect = el.getBoundingClientRect();
    if (rect.width <= 0 || rect.height <= 0) return "none";
    return archZone(
      archLayout(stageEdge(), rect.width, rect.height),
      { x: e.clientX - rect.left, y: e.clientY - rect.top },
      (rail.arch?.front ?? "arch") as ArchFront,
    );
  }

  function itemLabel(item: SpinItem): string {
    if (item.kind === "stars") return "Stars";
    if (item.kind === "settings") return "Settings";
    if (item.kind === "unmapped") return "Unmapped";
    if (item.kind === "add") return "+ Project";
    const project = rail.projects.find((p: { id: string }) => p.id === item.pid);
    return project?.name ?? "Planet";
  }

  function rowsAsArch(pid: string) {
    return (rail.rowsFor(pid) as WindowInfo[]).map((w) => ({
      id: w.id,
      title: rail.titleFor(w) as string,
      sleeping: !!w.sleeping,
      isMinimized: !!w.isMinimized,
      iconBase64: w.iconBase64 as string | null,
      appName: w.appName as string,
    }));
  }

  function onTileClick(item: SpinItem, index: number) {
    if ((rail.arch?.front ?? "arch") !== "arch") {
      bringArch();
      return;
    }
    if (index !== center) {
      center = index;
      rail.menuRing.select(item.stop);
      return;
    }
    if (item.kind === "planet" && item.pid) {
      const l2 = archLayer2Items({
        pid: item.pid,
        cards: rail.planetCards(item.pid) as ArchCard[],
        rows: rowsAsArch(item.pid),
      });
      rail.archDo(archEnter(rail.arch, item, l2));
      return;
    }
    if (item.kind === "unmapped") {
      rail.archDo(archEnter(rail.arch, item, []));
      return;
    }
    const action = spinConfirm(item.stop);
    if (action === "stars") rail.openStageDest("stars");
    else if (action === "settings") rail.openStageDest("settings");
    else if (action === "newProject") rail.creatingProject = true;
  }

  function onLayerClick(item: Layer2Item) {
    const front = (rail.arch?.front ?? "arch") as ArchFront;
    if (front !== "layer2") {
      if (front === "board") rail.archDo(archBack(rail.arch));
      else rail.archDo(archForward(rail.arch));
      return;
    }
    const action = archConfirm(item);
    if (action === "board") {
      rail.archDo(archOpenBoard(rail.arch, rail.UNMAPPED_KEY));
      return;
    }
    const pid = rail.arch?.pid as string | null;
    if (!pid || !item.id) return;
    const w = (rail.rowsFor(pid) as WindowInfo[]).find((row) => row.id === item.id);
    if (!w) return;
    if (action === "wake") void rail.wakeSleeper(w);
    else if (action === "restore") void rail.restore(w).then(() => rail.focusRow(w));
    else void rail.focusRow(w);
  }

  function l2Label(item: Layer2Item): string {
    if (item.kind === "board") return "Open board";
    if (item.kind === "more") return `+${item.count ?? 0} more`;
    return item.title || "Untitled";
  }

  const coachStory = $derived(normalizeStory(rail.starterStory));
  const coachLineDue = $derived(
    coachDue({
      trusted: !!rail.status?.trusted,
      stateLoaded: !!rail.stateLoaded,
      claimsCount: (rail.claims?.length ?? 0) as number,
      story: coachStory,
      embedded: !!rail.embedded,
    }),
  );
  const freshWheel = $derived(
    emptyDeskShowsWheel({
      windows: rail.windows.length,
      projects: rail.projects.length,
      story: coachStory,
    }),
  );

  function boardTint(): string {
    const pid = archState?.pid as string | null;
    if (pid && pid !== rail.UNMAPPED_KEY) return rail.planetCss(pid);
    return NEUTRAL;
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

{#snippet plusGlyph()}
  <svg class="line-icon" viewBox="0 0 32 32" aria-hidden="true">
    <path
      d="M16 7v18M7 16h18"
      fill="none"
      stroke="#8a8f98"
      stroke-width="1.8"
      stroke-linecap="round"
    />
  </svg>
{/snippet}

{#snippet boardGlyph()}
  <svg class="line-icon" viewBox="0 0 32 32" aria-hidden="true">
    <rect x="6" y="7" width="6" height="18" rx="1.4" fill="none" stroke="#8a8f98" stroke-width="1.6" />
    <rect x="13" y="7" width="6" height="12" rx="1.4" fill="none" stroke="#8a8f98" stroke-width="1.6" />
    <rect x="20" y="7" width="6" height="15" rx="1.4" fill="none" stroke="#8a8f98" stroke-width="1.6" />
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
      {:else if rail.windows.length === 0 && rail.projects.length === 0 && !freshWheel}
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
            No projects yet — use the <b>+ Project</b> item on the arch, then drag
            a window onto it to claim.
          </p>
        {/if}
        <div class="stage-hit" aria-hidden="true"></div>
        <div class="stage-strip" bind:this={stageEl}>
        <div
          class="arch-ring"
          class:is-receded={archState.front !== "arch"}
          style={depthStyle(1)}
        >
          {#if archState.front !== "arch"}
            <button type="button" class="arch-platter" aria-label="Show planets" onclick={bringArch}></button>
          {:else}
            <div class="arch-platter" aria-hidden="true"></div>
          {/if}
          {#if coachLineDue}
            <div class="coach-line">
              <span class="coach-text">{COACH_LINE}</span>
              <button
                type="button"
                class="mini"
                data-mkey={COACH_CLAIM_STOP}
                class:mkey-sel={rail.menuRing.activeId === COACH_CLAIM_STOP}
                title={COACH_TIP}
                onclick={() => rail.coachClaim()}
              >{COACH_CLAIM_LABEL}</button>
              <button
                type="button"
                class="mini ghost"
                data-mkey={COACH_SKIP_STOP}
                class:mkey-sel={rail.menuRing.activeId === COACH_SKIP_STOP}
                aria-label="Dismiss"
                onclick={() => rail.coachSkip()}
              >×</button>
              {#if rail.coachError}
                <span class="coach-err">{rail.coachError}</span>
              {/if}
            </div>
          {/if}
        {#if rail.wakeNote}
          <div class="hermes-note" class:err={rail.wakeNote.kind === "err"}>
            <span class="hermes-text">{rail.wakeNote.text}</span>
            <button class="mini ghost" onclick={() => (rail.wakeNote = null)} aria-label="Dismiss">×</button>
          </div>
        {/if}
        <ul class="arch-list">
          {#each items as item, i (item.stop)}
            <li
              class="arch-slot"
              class:is-selected={i === center}
              class:is-hidden={archHidden(i)}
              class:spin-jump={jumpMask[i] === true}
              class:drop-ok={item.pid != null && rail.dropTarget === item.pid}
              class:is-neutral={item.kind !== "planet"}
              style={slotStyle(offsets[i] ?? 0, 1, tintFor(item))}
              ondragenter={(e) => item.pid != null && rail.onSectionDragEnter(e, item.pid)}
              ondragover={(e) => item.pid != null && rail.onSectionDragOver(e, item.pid)}
              ondragleave={(e) => item.pid != null && rail.onSectionDragLeave(e, item.pid)}
              ondrop={(e) => {
                if (item.kind === "planet" && item.pid) rail.onSectionDrop(e, item.pid);
                else if (item.kind === "unmapped") rail.onSectionDrop(e, null);
              }}
            >
              <button
                type="button"
                class="arch-hit"
                data-mkey={item.stop}
                class:mkey-sel={rail.menuRing.activeId === item.stop}
                tabindex={archHidden(i) ? -1 : 0}
                aria-label={itemLabel(item)}
                title={itemLabel(item)}
                onclick={() => onTileClick(item, i)}
              >
                <span class="arch-glyph">
                  {#if item.kind === "stars"}{@render starGlyph()}
                  {:else if item.kind === "settings"}{@render gearGlyph()}
                  {:else if item.kind === "add"}{@render plusGlyph()}
                  {:else}{@render planetGlyph()}{/if}
                </span>
                <span class="arch-name">{itemLabel(item)}</span>
              </button>
              {#if item.kind === "add" && rail.creatingProject && !archHidden(i)}
                <span class="newproj spin-newproj">
                  <input
                    bind:this={nameEl}
                    bind:value={rail.newProjectName}
                    placeholder="Project name"
                    aria-label="Project name"
                    onkeydown={(e) => e.key === "Enter" && rail.createProjectOnly()}
                  />
                  <button class="mini" onclick={rail.createProjectOnly}>Add</button>
                  <button class="mini ghost" onclick={() => (rail.creatingProject = false)}>×</button>
                </span>
              {/if}
            </li>
          {/each}
        </ul>
        </div>
        {#if archState.pid}
          <div
            class="layer2-ring"
            class:is-receded={archState.front !== "layer2"}
            style={depthStyle(2)}
          >
            {#if archState.pid === rail.UNMAPPED_KEY}
              <div class="layer2-unmapped">
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
            {:else}
              {#if archState.front !== "layer2"}
                <button
                  type="button"
                  class="arch-platter layer2-platter"
                  aria-label="Show this planet"
                  onclick={() => rail.archDo(archState.front === "board" ? archBack(rail.arch) : archForward(rail.arch))}
                ></button>
              {:else}
                <div class="arch-platter layer2-platter" aria-hidden="true"></div>
              {/if}
              <ul class="arch-list">
                {#each layer2 as item, i (item.stop)}
                  <li
                    class="l2-slot"
                    class:is-hidden={layerHidden(i)}
                    class:is-selected={i === archState.center2 && archState.front === "layer2"}
                    style={slotStyle(i - archState.center2, 2, boardTint())}
                  >
                    <button
                      type="button"
                      class="l2-hit"
                      data-mkey={item.stop}
                      class:mkey-sel={rail.menuRing.activeId === item.stop}
                      tabindex={layerHidden(i) ? -1 : 0}
                      aria-label={l2Label(item)}
                      title={l2Label(item)}
                      onclick={() => onLayerClick(item)}
                    >
                      {#if item.kind === "window"}
                        {#if item.iconBase64}
                          <img class="app-icon" src="data:image/png;base64,{item.iconBase64}" alt="" />
                        {:else}
                          <span class="app-icon fallback" aria-hidden="true">{(item.appName || "?").charAt(0)}</span>
                        {/if}
                        <span class="l2-title">{l2Label(item)}</span>
                        {#if item.sleeping}<span class="l2-moon" aria-hidden="true">☾</span>{/if}
                        {#if item.minimized && !item.sleeping}<span class="l2-dock" aria-hidden="true"></span>{/if}
                      {:else if item.kind === "needed"}
                        <span class="l2-bang" aria-hidden="true">!</span>
                        <span class="l2-title">{l2Label(item)}</span>
                      {:else if item.kind === "doing" || item.kind === "todo"}
                        <span class="l2-dot" class:is-doing={item.kind === "doing"} aria-hidden="true"></span>
                        <span class="l2-title">{l2Label(item)}</span>
                      {:else if item.kind === "more"}
                        <span class="l2-title">{l2Label(item)}</span>
                      {:else}
                        <span class="arch-glyph l2-glyph">{@render boardGlyph()}</span>
                        <span class="l2-title">Open board</span>
                      {/if}
                    </button>
                  </li>
                {/each}
              </ul>
            {/if}
          </div>
        {/if}
        <div class="spin-board" class:is-off={archState.front !== "board"} style={`--planet:${boardTint()}`}>
          <div class="spin-kanban">
            <KanbanPage embedded railPlanet={boardPlanet} />
          </div>
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

