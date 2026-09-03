<script lang="ts">
  // GO-ORBITTY-COMPANION-3D: a bundled 3D body (Tang / Female) living in
  // the exact slots SpiritStage owns — idle stage, hosted wellhead,
  // 96×96 corner — with the same .stage/.floatwrap markup so ghost hit
  // reporting (set_companion_hits) and window drag are untouched. three.js
  // is dynamically imported ONLY here, so the rail / kanban / settings
  // first paint never loads it. Plays the glTF `idle` clip looping; a
  // send in flight crossfades to `think` when the file carries it (never
  // nod/wave — that is GO-ORBITTY-COMPANION-PLAY). A slow camera orbit
  // (~16 s) is the fallback ONLY when `idle` is missing. Reduce Motion
  // holds the rest pose; a hidden pane stops the render loop. Fail-soft:
  // no WebGL2 / loader error / missing GLB → onFail once and the parent
  // reverts to the blessed spirit — never a black stage.
  import { invoke, convertFileSrc } from "@tauri-apps/api/core";
  import { onMount, onDestroy } from "svelte";
  import { listenPaneVis } from "./paneVis";
  import type * as T3 from "three";
  import type { GLTF } from "three/addons/loaders/GLTFLoader.js";

  let {
    corner = false,
    headShowing = false,
    bodyId,
    thinking = false,
    onDrag,
    onFail,
  }: {
    corner?: boolean;
    headShowing?: boolean;
    bodyId: string;
    thinking?: boolean;
    onDrag?: (e: MouseEvent) => void;
    onFail?: (msg: string) => void;
  } = $props();

  // The GLBs carry no cameras — both views are computed from the bounds
  // and the DEF-HEAD bone. Corner 96×96 is always Head.
  interface CamFrame {
    target: T3.Vector3;
    dist: number;
    yaw: number;
    lift: number;
    fov: number;
  }

  let wrapEl: HTMLElement | null = null;
  let ready = false;
  let failed = false;
  let disposed = false;
  let hidden = false;
  let reduceMotion = false;

  let renderer: T3.WebGLRenderer | null = null;
  let scene: T3.Scene | null = null;
  let camera: T3.PerspectiveCamera | null = null;
  let mixer: T3.AnimationMixer | null = null;
  let clock: T3.Clock | null = null;
  let idleAction: T3.AnimationAction | null = null;
  let thinkAction: T3.AnimationAction | null = null;
  let orbit = false;
  let orbitT = 0;
  let fullFrame: CamFrame | null = null;
  let headFrame: CamFrame | null = null;
  let motionMq: MediaQueryList | null = null;
  let ro: ResizeObserver | null = null;
  let unlistenVis: (() => void) | null = null;

  function fail(msg: string) {
    if (failed) return;
    failed = true;
    onFail?.(msg || "3D body failed");
  }

  function placeCamera(f: CamFrame, yaw: number) {
    if (!camera) return;
    camera.position.set(
      f.target.x + Math.sin(yaw) * f.dist,
      f.target.y + f.lift,
      f.target.z + Math.cos(yaw) * f.dist,
    );
    camera.lookAt(f.target);
  }

  function applyView() {
    if (!camera || !fullFrame || !headFrame) return;
    const f = corner || headShowing ? headFrame : fullFrame;
    camera.fov = f.fov;
    placeCamera(f, f.yaw);
    camera.updateProjectionMatrix();
    if (reduceMotion || hidden) renderOnce();
  }

  function renderOnce() {
    if (!renderer || !scene || !camera) return;
    renderer.render(scene, camera);
  }

  function tick() {
    if (!renderer || !scene || !camera || !clock) return;
    const dt = clock.getDelta();
    if (mixer) mixer.update(dt);
    if (orbit && fullFrame) {
      orbitT += dt;
      placeCamera(fullFrame, fullFrame.yaw + (orbitT * Math.PI * 2) / 16);
    }
    renderer.render(scene, camera);
  }

  // Reduce Motion / hidden: no loop, pose held at rest (idle keys are
  // first=last, so mixer time 0 IS the rest pose). One still render so
  // resize / view flips never leave a stale frame.
  function syncLoop() {
    if (!renderer || !clock) return;
    if (reduceMotion || hidden) {
      renderer.setAnimationLoop(null);
      if (reduceMotion && mixer) mixer.setTime(0);
      renderOnce();
    } else {
      clock.getDelta();
      renderer.setAnimationLoop(tick);
    }
  }

  function onMotionChange() {
    reduceMotion = motionMq?.matches ?? false;
    syncLoop();
  }

  function resize() {
    if (!renderer || !wrapEl || !camera) return;
    const w = wrapEl.clientWidth;
    const h = wrapEl.clientHeight;
    if (w < 2 || h < 2) return;
    renderer.setSize(w, h, false);
    camera.aspect = w / h;
    camera.updateProjectionMatrix();
    if (reduceMotion || hidden) renderOnce();
  }

  // Think-v9 stays on the spirit sheet; a 3D body crossfades to the glTF
  // `think` clip while a send is in flight, else holds `idle`. In orbit
  // fallback (no idle) the orbit holds — no invented motion.
  function setThinking(on: boolean) {
    if (!mixer || !idleAction || !thinkAction || orbit) return;
    if (on === thinkAction.isRunning()) return;
    if (on) {
      thinkAction.reset().play();
      idleAction.crossFadeTo(thinkAction, 0.3, false);
    } else {
      idleAction.reset().play();
      thinkAction.crossFadeTo(idleAction, 0.3, false);
    }
  }

  async function boot() {
    const path = await invoke<string>("companion_glb_url", { id: bodyId });
    if (!path) throw new Error("glb missing");
    const probe = document.createElement("canvas");
    if (!probe.getContext("webgl2")) throw new Error("no WebGL2");
    const T = await import("three");
    const { GLTFLoader } = await import(
      "three/addons/loaders/GLTFLoader.js"
    );
    if (disposed || !wrapEl) return;

    scene = new T.Scene();
    camera = new T.PerspectiveCamera(35, 1, 0.01, 1000);
    // Exported glTF materials stay as shipped (KHR_materials_specular);
    // one hemisphere + one key light, no toon restyle.
    const hemi = new T.HemisphereLight(0xffffff, 0x3a4a5a, 1.15);
    const key = new T.DirectionalLight(0xffffff, 1.8);
    key.position.set(1.6, 2.6, 2.2);
    scene.add(hemi, key);

    renderer = new T.WebGLRenderer({ antialias: true, alpha: true });
    renderer.setClearColor(0x000000, 0);
    renderer.setPixelRatio(Math.min(window.devicePixelRatio || 1, 2));
    wrapEl.appendChild(renderer.domElement);

    const gltf: GLTF = await new Promise((resolve, reject) => {
      new GLTFLoader().load(convertFileSrc(path), resolve, undefined, reject);
    });
    if (disposed) return;
    const root = gltf.scene;
    scene.add(root);
    root.updateMatrixWorld(true);

    const box = new T.Box3().setFromObject(root);
    const size = box.getSize(new T.Vector3());
    const center = box.getCenter(new T.Vector3());
    const height = Math.max(size.y, 1e-4);
    const maxDim = Math.max(size.x, size.y, size.z, 1e-4);
    // Full: frame the whole figure, slight 3/4 yaw, FOV 35.
    const distFull =
      (maxDim / 2 / Math.tan(T.MathUtils.degToRad(35 / 2))) * 1.25;
    fullFrame = {
      target: center.clone(),
      dist: distFull,
      yaw: T.MathUtils.degToRad(20),
      lift: height * 0.05,
      fov: 35,
    };
    // Head: DEF-HEAD world position, FOV 28, fill the box.
    const headObj = root.getObjectByName("DEF-HEAD");
    const headTarget = headObj
      ? headObj.getWorldPosition(new T.Vector3())
      : new T.Vector3(center.x, box.max.y - height * 0.1, center.z);
    headTarget.y += height * 0.04;
    headFrame = {
      target: headTarget,
      dist: (height * 0.3) / (2 * Math.tan(T.MathUtils.degToRad(28 / 2))),
      yaw: T.MathUtils.degToRad(10),
      lift: height * 0.03,
      fov: 28,
    };
    camera.near = Math.max(height / 100, 1e-5);
    camera.far = distFull * 8 + height * 8;

    mixer = new T.AnimationMixer(root);
    clock = new T.Clock();
    const clips = gltf.animations ?? [];
    const clip = (name: string) =>
      clips.find((c) => c.name.trim().toLowerCase() === name);
    const idle = clip("idle");
    const think = clip("think");
    if (idle) {
      idleAction = mixer.clipAction(idle);
      idleAction.play();
    } else {
      orbit = true;
    }
    if (think) thinkAction = mixer.clipAction(think);

    ro = new ResizeObserver(resize);
    ro.observe(wrapEl);
    ready = true;
    resize();
    applyView();
    if (thinking) setThinking(true);
    syncLoop();
  }

  $effect(() => {
    const t = thinking;
    if (ready) setThinking(t);
  });

  $effect(() => {
    void headShowing;
    void corner;
    if (ready) applyView();
  });

  onMount(async () => {
    motionMq = window.matchMedia("(prefers-reduced-motion: reduce)");
    reduceMotion = motionMq.matches;
    motionMq.addEventListener("change", onMotionChange);
    unlistenVis = await listenPaneVis("companion", (v) => {
      hidden = !v;
      syncLoop();
    });
    try {
      await boot();
    } catch (e) {
      fail(e instanceof Error ? e.message : String(e));
    }
  });

  onDestroy(() => {
    disposed = true;
    unlistenVis?.();
    motionMq?.removeEventListener("change", onMotionChange);
    ro?.disconnect();
    renderer?.setAnimationLoop(null);
    mixer?.stopAllAction();
    scene?.traverse((o) => {
      const mesh = o as T3.Mesh;
      if (!mesh.isMesh) return;
      mesh.geometry?.dispose();
      const mats = Array.isArray(mesh.material)
        ? mesh.material
        : [mesh.material];
      for (const m of mats) {
        if (!m) continue;
        for (const v of Object.values(m)) {
          const t = v as T3.Texture;
          if (t && t.isTexture) t.dispose();
        }
        m.dispose();
      }
    });
    renderer?.dispose();
    renderer?.domElement.remove();
  });
</script>

{#if corner}
  <!-- CORNER: 96×96 absolute upper-right, always the Head camera. -->
  <div class="exphead" onmousedown={onDrag} role="presentation">
    <div class="floatwrap glb" bind:this={wrapEl}></div>
  </div>
{:else}
  <!-- Idle full-width stage, or hosted right column (.wellhead). -->
  <div class="stage" role="presentation">
    <div
      class="floatwrap glb"
      bind:this={wrapEl}
      onmousedown={onDrag}
      role="presentation"
    ></div>
  </div>
{/if}

<style>
  /* Same layout contract as SpiritStage so the ghost floatwrap hit box,
     the hosted wellhead column, and the corner portrait all keep their
     shape — the canvas just fills the wrap. */
  .stage {
    flex: 1;
    min-height: 120px;
    width: 100%;
    display: flex;
    align-items: center;
    justify-content: center;
    user-select: none;
    -webkit-user-select: none;
    cursor: grab;
  }

  .stage:active {
    cursor: grabbing;
  }

  :global(.wellhead) .stage {
    flex: 1;
    width: auto;
    min-height: 0;
  }

  .stage .floatwrap {
    width: 100%;
    height: 100%;
    display: flex;
    align-items: center;
    justify-content: center;
  }

  :global(.ghost) .stage {
    container-type: size;
  }

  :global(.ghost) .stage .floatwrap {
    width: min(100cqw, 100cqh);
    height: min(100cqw, 100cqh);
  }

  .exphead {
    position: absolute;
    top: 8px;
    left: 10px;
    right: auto;
    z-index: 2;
    width: 96px;
    height: 96px;
    display: flex;
    justify-content: center;
    align-items: center;
    user-select: none;
    -webkit-user-select: none;
    cursor: grab;
  }

  .exphead:active {
    cursor: grabbing;
  }

  .exphead .floatwrap {
    line-height: 0;
    width: 96px;
    height: 96px;
    display: flex;
    align-items: center;
    justify-content: center;
  }

  .floatwrap :global(canvas) {
    display: block;
    width: 100%;
    height: 100%;
  }
</style>
