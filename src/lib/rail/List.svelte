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
  import SettingsPage from "../SettingsPage.svelte";
  import StarsPage from "../StarsPage.svelte";
  import { invoke } from "@tauri-apps/api/core";
  import RailPlanetsSheet from "./PlanetsSheet.svelte";
  import type { RailApi } from "./api";
  import {
    SPIN_SETTLE_MS,
    spinCenterFor,
    spinConfirm,
    spinInitial,
    spinItems,
    spinJump,
    spinOffset,
    spinStep,
    type SpinItem,
    type StageEdge,
  } from "./stageLook";
  import {
    archCentreChanged,
    archConfirm,
    archEnter,
    enterBoard,
    archInitial,
    archOrbitArc,
    archOrbitFade,
    archLabelBox,
    archMarbleBox,
    L2_RING,
    thumbShowsImage,
    archScroll2,
    archSlot,
    mirrorDepth,
    wheelZone,
    mirrorItems,
    mirrorLayout,
    stageStackLayout,
    STAGE_INK,
    STAGE_INK_DIM,
    STAGE_PLATTER_RGBA,
    type ArchCard,
    type ArchFront,
    type ArchState,
    type Layer2Item,
  } from "./stageArch";
  import { archHalo, outwardDir, pickFromKind, selWrapBox } from "./stageFocus";
  import { archGutter, centreStep, depthOut, spinStepsCap, SPIN_CAP_IDLE, type SpinCapState } from "./railInput";

  let { rail }: { rail: RailApi } = $props();

  const NEUTRAL = "#8a8f98";

  let center = $state(0);
  let booted = false;
  let stageEl = $state<HTMLDivElement | null>(null);
  let boardPlanet = $state("inbox");
  let nameEl = $state<HTMLInputElement | null>(null);
  let cap: SpinCapState = {
    spin: { ...SPIN_CAP_IDLE.spin },
    windowStart: 0,
    windowSteps: 0,
  };
  let prevOffsets: number[] = [];
  let havePrev = false;
  let jumpMask = $state<boolean[]>([]);
  let l2JumpMask = $state<boolean[]>([]);
  let prevL2: number[] = [];
  let havePrevL2 = false;
  let frameW = $state(1040);
  let frameH = $state(760);

  const stageVars = `--stage-platter:rgba(${STAGE_PLATTER_RGBA[0]},${STAGE_PLATTER_RGBA[1]},${STAGE_PLATTER_RGBA[2]},${STAGE_PLATTER_RGBA[3]});--stage-ink:${STAGE_INK};--stage-ink-dim:${STAGE_INK_DIM}`;

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
    return mirrorItems({
      pid,
      cards: (rail.planetCards(pid) ?? []) as ArchCard[],
      rows: rowsAsArch(pid),
    });
  });
  const mirror = $derived(mirrorLayout(stageEdge(), frameW, frameH));
  const centred2 = $derived(layer2[archState?.center2 ?? 0] ?? null);
  let thumb = $state<{ state: string; jpeg?: string | null; forId: string } | null>(null);
  const thumbForCard = $derived(
    thumb && centred2 && thumb.forId === centred2.id ? thumb : null,
  );

  $effect(() => {
    const front = archState.front;
    const item = centred2;
    const shown = !!rail.stageRevealed;
    if (!shown || front !== "layer2" || item?.kind !== "window" || !item.id) {
      thumb = null;
      return;
    }
    const id = item.id;
    const title = item.title ?? "";
    let dead = false;
    void invoke<{ state: string; jpeg?: string | null }>("window_thumb", {
      id,
      title,
      pid: 0,
      focusEpoch: 0,
    })
      .then((res) => {
        if (!dead) thumb = { state: res.state, jpeg: res.jpeg, forId: id };
      })
      .catch(() => {
        if (!dead) thumb = { state: "error", forId: id };
      });
    const c = archState.center2 ?? 0;
    for (const j of [c - 1, c + 1]) {
      const n = layer2[j];
      if (n?.kind === "window" && n.id) {
        void invoke("window_thumb", {
          id: n.id,
          title: n.title ?? "",
          pid: 0,
          focusEpoch: 0,
        }).catch(() => {});
      }
    }
    return () => {
      dead = true;
    };
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
    const step = centreStep(rail.centreCause);
    if (next !== center) {
      center = next;
      if (step.apply) rail.applyBeside(pickFromKind(list[next]?.kind));
    }
    rail.centreCause = step.next;
    if (rail.spinCenter !== center) rail.spinCenter = center;
  });

  $effect(() => {
    const pid = archState?.pid as string | null;
    const front = archState?.front as ArchFront | undefined;
    if ((front !== "board" && front !== "layer2") || !pid || pid === rail.UNMAPPED_KEY) return;
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
    const n = layer2.length;
    const c = archState?.center2 ?? 0;
    const next = layer2.map((_, i) => spinOffset(i, c, n));
    if (!havePrevL2) {
      prevL2 = next.slice();
      havePrevL2 = true;
      return;
    }
    const mask = next.map((k, i) => spinJump(prevL2[i] ?? k, k));
    prevL2 = next.slice();
    if (mask.some(Boolean)) {
      l2JumpMask = mask;
      requestAnimationFrame(() => {
        l2JumpMask = [];
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
      const front = (rail.arch?.front ?? "arch") as ArchFront;
      const main = stageEl?.closest("main");
      const host = main instanceof HTMLElement ? main : stageEl;
      const rect = host ? host.getBoundingClientRect() : null;
      const relX = rect ? e.clientX - rect.left : -1;
      const relY = rect ? e.clientY - rect.top : -1;
      const unmapped = front === "layer2" && rail.arch?.pid === rail.UNMAPPED_KEY;
      const zone = rect
        ? wheelZone({ w: rect.width, h: rect.height }, { x: relX, y: relY }, front, unmapped)
        : "none";
      // Scroll never changes depth. wheelZone is the front layer anywhere inside the frame.
      if (zone === "none") return;
      e.preventDefault();
      const out = spinStepsCap(
        cap,
        { deltaX: e.deltaX, deltaY: e.deltaY, deltaMode: e.deltaMode },
        Date.now(),
      );
      cap = out.state;
      if (out.steps === 0) return;
      if (typeof rail.noteWheelSpin === "function") rail.noteWheelSpin();
      if (zone === "arch") {
        const n = items.length;
        if (n === 0) return;
        const next = spinStep(center, out.steps, n);
        center = next;
        const active = rail.menuRing.activeId as string | null;
        if (active && items.some((it) => it.stop === active)) {
          rail.menuRing.select(items[next].stop);
        }
        rail.applyBeside(pickFromKind(items[next]?.kind));
        return;
      }
      const next = archScroll2(rail.arch, out.steps, layer2.length);
      rail.archDo(next);
      const active = rail.menuRing.activeId as string | null;
      if (next.front === "layer2" && active && layer2.some((it) => it.stop === active)) {
        const hit = layer2[next.center2];
        if (hit) rail.menuRing.select(hit.stop);
      }
      rail.applyBeside("planet");
    };
    el.addEventListener("wheel", onWheel, { passive: false });
    return () => el.removeEventListener("wheel", onWheel);
  });

  $effect(() => {
    const el = stageEl;
    const main = el?.closest("main");
    const host = main instanceof HTMLElement ? main : el;
    if (!host) return;
    const apply = () => {
      const rect = host.getBoundingClientRect();
      if (rect.width > 0) frameW = rect.width;
      if (rect.height > 0) frameH = rect.height;
    };
    apply();
    const ro = new ResizeObserver(apply);
    ro.observe(host);
    return () => ro.disconnect();
  });

  function stageEdge(): StageEdge {
    const edge = rail.railEdge;
    return edge === "right" || edge === "top" || edge === "bottom" ? edge : "left";
  }

  function tintFor(item: SpinItem): string {
    if (item.kind === "planet" && item.pid) return rail.planetCss(item.pid);
    return NEUTRAL;
  }

  function frameNow(): { w: number; h: number } {
    return { w: frameW, h: frameH };
  }

  function slotStyle(k: number, ring: 1 | 2, tint: string): string {
    const slot = archSlot(k, stageEdge(), ring, frameNow());
    return `--wx:${slot.x};--wy:${slot.y};--ws:${slot.scale};--wo:${slot.opacity};--wz:${slot.z};--planet:${tint}`;
  }

  function archHidden(index: number): boolean {
    return archSlot(offsets[index] ?? 0, stageEdge(), 1, frameNow()).hidden;
  }

  function rectStyle(r: { x: number; y: number; w: number; h: number }): string {
    return `left:${r.x}px;top:${r.y}px;width:${Math.max(0, r.w)}px;height:${Math.max(0, r.h)}px`;
  }

  function settingsBox(): string {
    return rectStyle(stageStackLayout(stageEdge(), frameW, frameH, 0, 0).settings);
  }

  function labelStyle(k: number, ring: 1 | 2): string {
    const frame = frameNow();
    const edge = stageEdge();
    const avoid = [mirror.expanded];
    const box = archLabelBox(k, edge, ring, frame, { avoid });
    const marble = archMarbleBox(k, edge, ring, frame, k === 0);
    const cx = marble.x + marble.w / 2;
    const cy = marble.y + marble.h / 2;
    return `left:${box.x - cx}px;top:${box.y - cy}px;width:${box.w}px;max-width:${box.w}px`;
  }

  function labelSide(k: number, ring: 1 | 2): "below" | "inboard" {
    const avoid = [mirror.expanded];
    return archLabelBox(k, stageEdge(), ring, frameNow(), { avoid }).side;
  }

  function orbitOrigin(): string {
    const edge = stageEdge();
    let x = 0;
    let y = frameH / 2;
    if (edge === "right") x = frameW;
    else if (edge === "top") {
      x = frameW / 2;
      y = 0;
    } else if (edge === "bottom") {
      x = frameW / 2;
      y = frameH;
    }
    return `translate(${x} ${y})`;
  }

  function l2Offset(index: number): number {
    return spinOffset(index, archState?.center2 ?? 0, layer2.length);
  }

  function selStyle(k: number, ring: 1 | 2): string {
    const frame = frameNow();
    const edge = stageEdge();
    const avoid = [mirror.expanded];
    const marble = archMarbleBox(k, edge, ring, frame, k === 0);
    const label = archLabelBox(k, edge, ring, frame, { avoid });
    const wrap = selWrapBox(marble, label);
    const cx = marble.x + marble.w / 2;
    const cy = marble.y + marble.h / 2;
    return `left:${wrap.x - cx}px;top:${wrap.y - cy}px;width:${wrap.w}px;height:${wrap.h}px`;
  }

  function l2Hidden(index: number): boolean {
    return archSlot(l2Offset(index), stageEdge(), 1, frameNow()).hidden;
  }

  function depthStyle(layer: 1 | 2): string {
    const front = (archState?.front ?? "arch") as ArchFront;
    const d = mirrorDepth(front, layer, !!rail.reduceMotion);
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
    const front = (rail.arch?.front ?? "arch") as ArchFront;
    const step = depthOut(front);
    if (step.hide) return;
    rail.archDo({ ...rail.arch, front: step.front, pending: null });
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
        isFrontmost: !!w.isFrontmost,
      }));
  }

  function planetName(pid: string): string {
    const project = rail.projects.find((p: { id: string; name?: string }) => p.id === pid);
    return project?.name ?? "Planet";
  }

  function windowStateLabel(item: Layer2Item): string {
    if (item.sleeping) return "asleep";
    if (item.minimized) return "minimized";
    if (item.frontmost) return "frontmost";
    return "open";
  }

  function onTileClick(item: SpinItem, index: number) {
    const spinning = typeof rail.wheelAnimating === "function" && rail.wheelAnimating();
    if (spinning) {
      item = items[center] ?? item;
      index = center;
    }
    if (archHidden(index)) return;
    center = index;
    rail.menuRing.select(item.stop);
    if (item.kind === "planet" && item.pid) {
      rail.applyBeside("planet");
      rail.archDo(
        archEnter(
          rail.arch,
          item,
          mirrorItems({
            pid: item.pid,
            cards: rail.planetCards(item.pid) as ArchCard[],
            rows: rowsAsArch(item.pid),
          }),
        ),
      );
      return;
    }
    if (item.kind === "unmapped") {
      rail.applyBeside("unmapped");
      rail.archDo(archEnter(rail.arch, item, []));
      return;
    }
    const action = spinConfirm(item.stop);
    if (action === "stars") {
      rail.applyBeside("stars");
      rail.openStageDest("stars");
    } else if (action === "settings") {
      rail.applyBeside("settings");
      rail.openStageDest("settings");
    } else if (action === "newProject") rail.creatingProject = true;
  }

  function selectLayer(item: Layer2Item) {
    const i = layer2.findIndex((it) => it.stop === item.stop);
    if (i < 0) return;
    if (item.kind === "board") {
      const base = {
        ...rail.arch,
        front: "layer2" as const,
        center2: i,
        pending: null,
      };
      const pid = rail.arch.pid as string | null;
      const cards = pid
        ? ((rail.planetCards(pid) ?? []) as { id: string; status?: string }[])
        : [];
      const land = enterBoard(base, cards);
      if (land) {
        rail.archDo(land.state);
        rail.walkInto("board");
        rail.focusBoardLanding?.(land.focus);
      }
      return;
    }
    rail.applyBeside("planet");
    rail.archDo({
      front: rail.arch.front,
      pid: rail.arch.pid,
      center2: i,
      pending: null,
    });
    rail.menuRing?.select(item.stop);
  }

  function raiseWindow(item: Layer2Item) {
    if (item.kind !== "window" || !item.id) return;
    const action = archConfirm(item);
    const pid = rail.arch?.pid as string | null;
    if (!pid) return;
    const w = (rail.rowsFor(pid) as WindowInfo[]).find((row) => row.id === item.id);
    if (!w) return;
    const parked = () => rail.parkAfterRaise();
    if (action === "wake") void rail.wakeSleeper(w).then(parked);
    else if (action === "restore") void rail.restore(w).then(() => rail.focusRow(w)).then(parked);
    else void rail.focusRow(w).then(parked);
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
          data-mkey={project == null ? `umap-${w.id}` : undefined}
          class:mkey-sel={project == null && rail.keyOwner === "unmapped" && rail.menuRing.activeId === `umap-${w.id}`}
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

{#snippet boardGlyph()}
  <svg class="line-icon" viewBox="0 0 32 32" aria-hidden="true">
    <rect x="5" y="6" width="6" height="20" rx="1.5" fill="none" stroke="#8a8f98" stroke-width="1.6" />
    <rect x="13" y="6" width="6" height="14" rx="1.5" fill="none" stroke="#8a8f98" stroke-width="1.6" />
    <rect x="21" y="6" width="6" height="9" rx="1.5" fill="none" stroke="#8a8f98" stroke-width="1.6" />
  </svg>
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
    <path
      fill="#8a8f98"
      fill-rule="evenodd"
      d="M16.27 6.4 L17.86 2.93 L23.93 5.44 L22.64 9.07 L22.98 9.41 L26.56 8.07 L29.07 14.14 L25.6 15.8 L25.6 16.27 L29.07 17.86 L26.56 23.93 L22.93 22.64 L22.59 22.98 L23.93 26.56 L17.86 29.07 L16.2 25.6 L15.73 25.6 L14.14 29.07 L8.07 26.56 L9.36 22.93 L9.02 22.59 L5.44 23.93 L2.93 17.86 L6.4 16.2 L6.4 15.73 L2.93 14.14 L5.44 8.07 L9.07 9.36 L9.41 9.02 L8.07 5.44 L14.14 2.93 L15.8 6.4 Z M16 11.8 L18.1 12.36 L19.64 13.9 L20.2 16 L19.64 18.1 L18.1 19.64 L16 20.2 L13.9 19.64 L12.36 18.1 L11.8 16 L12.36 13.9 L13.9 12.36 Z"
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
      {:else}
        <div class="stage-hit" aria-hidden="true"></div>
        {#if rail.lane === "agents"}
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
        <div class="stage-strip" bind:this={stageEl} style={stageVars}>
        <div class="stage-scrim" aria-hidden="true"></div>
        <svg class="stage-orbit" width={frameW} height={frameH} aria-hidden="true">
          <g transform={orbitOrigin()}>
            <defs>
              <linearGradient
                id="stage-orbit-fade"
                gradientUnits="userSpaceOnUse"
                x1={archOrbitFade(archOrbitArc(stageEdge(), 1, frameNow())).x1}
                y1={archOrbitFade(archOrbitArc(stageEdge(), 1, frameNow())).y1}
                x2={archOrbitFade(archOrbitArc(stageEdge(), 1, frameNow())).x2}
                y2={archOrbitFade(archOrbitArc(stageEdge(), 1, frameNow())).y2}
              >
                <stop offset="0%" stop-color="white" stop-opacity="0" />
                <stop offset="15%" stop-color="white" stop-opacity="1" />
                <stop offset="100%" stop-color="white" stop-opacity="1" />
              </linearGradient>
              <mask id="stage-orbit-mask">
                <rect x="-4000" y="-4000" width="8000" height="8000" fill="url(#stage-orbit-fade)" />
              </mask>
            </defs>
            <path d={archOrbitArc(stageEdge(), 1, frameNow()).d} mask="url(#stage-orbit-mask)" stroke-linecap="butt" />
          </g>
        </svg>
        <div
          class="arch-ring"
          class:is-receded={archState.front !== "arch"}
          style={depthStyle(1)}
        >
          {#if archState.front !== "arch"}
            <button
              type="button"
              class="arch-back"
              aria-label="Back"
              style={rectStyle(mirror.back)}
              onclick={bringArch}
            ><span class="arch-sel" aria-hidden="true"></span></button>
          {/if}
          {#if rail.claimFlash}
            <div class="claim-toast">
              <span class="claim-dot" style={`background:${rail.planetCss(rail.claimFlash.pid)}`}></span>
              <span>{rail.claimFlash.title} → {planetName(rail.claimFlash.pid)}</span>
            </div>
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
            {@const halo = archHalo(item.kind, { center: i === center, active: rail.menuRing.activeId === item.stop })}
            <li
              class="arch-slot"
              data-halo={halo}
              class:is-selected={i === center}
              class:is-hidden={archHidden(i)}
              class:spin-jump={jumpMask[i] === true}
              class:drop-ok={item.pid != null && rail.dropTarget === item.pid}
              class:is-neutral={item.kind !== "planet"}
              class:is-claim-pulse={item.kind === "planet" && rail.claimFlash?.pid === item.pid}
              style={slotStyle(offsets[i] ?? 0, 1, tintFor(item))}
              ondragenter={(e) => item.pid != null && rail.onSectionDragEnter(e, item.pid)}
              ondragover={(e) => item.pid != null && rail.onSectionDragOver(e, item.pid)}
              ondragleave={(e) => item.pid != null && rail.onSectionDragLeave(e, item.pid)}
              ondrop={(e) => {
                if (item.kind === "planet" && item.pid) rail.onSectionDrop(e, item.pid);
                else if (item.kind === "unmapped") rail.onSectionDrop(e, null);
              }}
            >
              {#if halo !== "none"}
                <span class="arch-sel" aria-hidden="true" style={selStyle(offsets[i] ?? 0, 1)}></span>
              {/if}
              <button
                type="button"
                class="arch-hit"
                data-mkey={item.stop}
                tabindex={archHidden(i) ? -1 : 0}
                aria-current={i === center ? "true" : undefined}
                aria-label={itemLabel(item)}
                title={itemLabel(item)}
                onclick={() => onTileClick(item, i)}
              >
                <span class="arch-vis">
                  {#if item.kind === "planet"}
                    <span class="arch-marble-wrap">
                      <span class="arch-ringlet" aria-hidden="true"></span>
                      <span class="arch-marble"></span>
                      {#if item.pid && rail.planetCount(item.pid) > 0}
                        <span class="arch-count">{rail.planetCount(item.pid)}</span>
                      {/if}
                    </span>
                  {:else}
                    <span class="arch-glyph">
                      {#if item.kind === "stars"}{@render starGlyph()}
                      {:else if item.kind === "settings"}{@render gearGlyph()}
                      {:else if item.kind === "add"}{@render plusGlyph()}
                      {:else}{@render planetGlyph()}{/if}
                    </span>
                  {/if}
                  <span
                    class="arch-name"
                    class:label-below={labelSide(offsets[i] ?? 0, 1) === "below"}
                    class:label-inboard={labelSide(offsets[i] ?? 0, 1) === "inboard"}
                    style={labelStyle(offsets[i] ?? 0, 1)}
                  >{#if item.kind === "planet" && item.pid}<span class="arch-dot" style={`background:${rail.planetCss(item.pid)}`}></span>{:else}<span class="arch-dot"></span>{/if}{itemLabel(item)}</span>
                </span>
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
            style={`${depthStyle(2)};--preview-gutter:${archGutter(220)}px`}
          >
            {#if archState.pid === rail.UNMAPPED_KEY}
              <div class="layer2-unmapped" style={rectStyle(mirror.expanded)}>
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
              <ul class="arch-list l2-arch">
                {#each layer2 as item, i (item.stop)}
                  {@const halo2 = archHalo(item.kind, { center: i === archState.center2, active: rail.menuRing.activeId === item.stop })}
                  <li
                    class="arch-slot"
                    data-halo={halo2}
                    class:is-selected={i === archState.center2}
                    class:is-hidden={l2Hidden(i)}
                    class:spin-jump={l2JumpMask[i] === true}
                    style={slotStyle(l2Offset(i), L2_RING, boardTint())}
                  >
                    {#if halo2 !== "none"}
                      <span class="arch-sel" aria-hidden="true" style={selStyle(l2Offset(i), L2_RING)}></span>
                    {/if}
                    <button
                      type="button"
                      class="arch-hit"
                      data-mkey={item.stop}
                      tabindex={l2Hidden(i) ? -1 : 0}
                      aria-current={i === archState.center2 ? "true" : undefined}
                      aria-label={item.title || item.kind}
                      title={item.title || item.kind}
                      onclick={() => {
                        selectLayer(item);
                        if (item.kind === "needed") rail.armNeededMouth(item);
                      }}
                    >
                      <span class="arch-vis">
                        {#if item.kind === "window"}
                          {#if item.iconBase64}
                            <img class="l2-app-icon" src="data:image/png;base64,{item.iconBase64}" alt="" />
                          {:else}
                            <span class="l2-app-icon fallback" aria-hidden="true">{(item.appName || item.title || "?").charAt(0)}</span>
                          {/if}
                          {#if item.sleeping || item.minimized}
                            <span class="l2-badge">{item.sleeping ? "asleep" : "min"}</span>
                          {/if}
                        {:else if item.kind === "needed"}
                          <span class="arch-glyph l2-bang" aria-hidden="true">!</span>
                        {:else}
                          <span class="arch-glyph" aria-hidden="true">{@render boardGlyph()}</span>
                        {/if}
                        <span
                          class="arch-name"
                          class:label-below={labelSide(l2Offset(i), L2_RING) === "below"}
                          class:label-inboard={labelSide(l2Offset(i), L2_RING) === "inboard"}
                          style={labelStyle(l2Offset(i), L2_RING)}
                        ><span class="arch-dot" style={`background:${rail.planetCss(archState.pid)}`}></span>{item.title || (item.kind === "board" ? "Board" : item.kind)}</span>
                      </span>
                    </button>
                  </li>
                {/each}
              </ul>
            {/if}
          </div>
          {#if archState.pid !== rail.UNMAPPED_KEY && (archState.front === "layer2" || archState.front === "board") && centred2}
                <div
                  class="l2-expanded"
                  style={rectStyle(mirror.expanded)}
                >
                  {#if centred2.kind === "board"}
                    <div class="l2-board" data-stage-board tabindex="-1">
                      <KanbanPage
                        embedded
                        railPlanet={boardPlanet}
                        ringActive={rail.keyOwner === "board"}
                        outwardDir={outwardDir(stageEdge())}
                        onRingExit={() => rail.walkOut()}
                      />
                    </div>
                    <button type="button" class="mini l2-pop" onclick={() => rail.popOutBoard()}>Pop out</button>
                  {:else if centred2.kind === "window"}
                    <button
                      type="button"
                      class="l2-window-card"
                      data-win-preview={centred2.id}
                      aria-label={centred2.title || "Window"}
                      onclick={() => raiseWindow(centred2)}
                    >
                      {#if thumbForCard && thumbShowsImage(thumbForCard.state) && thumbForCard.jpeg}
                        <img class="l2-thumb" alt="" src="data:image/jpeg;base64,{thumbForCard.jpeg}" />
                      {:else if centred2.iconBase64}
                        <img class="l2-card-icon" src="data:image/png;base64,{centred2.iconBase64}" alt="" />
                      {:else}
                        <span class="l2-card-icon fallback" aria-hidden="true">{(centred2.appName || "?").charAt(0)}</span>
                      {/if}
                      <span class="l2-card-copy">
                        <span class="l2-card-title">{centred2.title || "Untitled"}</span>
                        {#if centred2.appName}<span class="l2-card-app">{centred2.appName}</span>{/if}
                        <span class="l2-card-state">{windowStateLabel(centred2)}</span>
                        {#if thumbForCard?.state === "denied"}
                          <span class="l2-thumb-need">Screen Recording needed for previews</span>
                        {/if}
                      </span>
                    </button>
                  {:else if centred2.kind === "needed"}
                    <div class="l2-needed-card">
                      <span class="l2-card-title">{centred2.title || "Needed"}</span>
                      {#if centred2.why}<span class="l2-card-why">{centred2.why}</span>{/if}
                      <button type="button" class="mini" onclick={() => rail.armNeededMouth(centred2)}>Answer</button>
                      <button type="button" class="mini" onclick={() => centred2.id && rail.openKanbanFocus(centred2.id)}>Open in board</button>
                    </div>
                  {/if}
                </div>
          {/if}
        {/if}
        {#if rail.stageEmbed === "settings"}
          <div class="stage-settings" data-stage-settings style={settingsBox()}>
            <SettingsPage
              embedded
              ringActive={rail.keyOwner === "settings"}
              outwardDir={outwardDir(stageEdge())}
              onRingExit={() => rail.walkOut()}
              onClose={() => rail.closeStageEmbed()}
              onPopOut={() => rail.popOutSettings()}
            />
          </div>
        {:else if rail.stageEmbed === "stars"}
          <div class="stage-settings" data-stage-stars style={settingsBox()}>
            <StarsPage
              embedded
              ringActive={rail.keyOwner === "stars"}
              outwardDir={outwardDir(stageEdge())}
              onRingExit={() => rail.walkOut()}
              onClose={() => rail.closeStageEmbed()}
            />
            <button type="button" class="mini l2-pop" onclick={() => rail.popOutStars()}>Pop out</button>
          </div>
        {/if}
        </div>
        {/if}
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

