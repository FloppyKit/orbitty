# VRoid Harness

Python control plane for **VRoid Studio** via **macos-harness** (no native VRoid MCP).

Unity UI has almost no AX — we use screenshots + calibrated clicks + keyboard.
Menu bar is thin AX only.

## One-time permissions

System Settings → Privacy & Security:

1. **Accessibility** → **Berd** on  
2. **Screen & System Audio Recording** → **Berd** on  
3. **Quit Berd fully** (Cmd+Q) and reopen so the agent host (`goosed`) picks it up  

```bash
macos-harness doctor
# screen_recording must be true
python3 ".../brand/vroid/harness/vroid_ctl.py" doctor
```

## Commands

```bash
vroid-ctl doctor
vroid-ctl axislock
vroid-ctl state          # AX only, no screenshot
vroid-ctl see --name start
vroid-ctl key "cmd+s"
vroid-ctl click 120 340
vroid-ctl save-point face_tab 120 340 --note "Face category"
vroid-ctl click-point face_tab
vroid-ctl list-points
vroid-ctl recipes
```

Inside a `macos-harness` session, import `VRoidDriver(mac=mac)` for zero-subprocess bursts.

## AxisLock

`../AXISLOCK.md` — frozen. Recipes refuse to invent past it.

## After Berd relaunch

Ping pixel-artist: “screen recording on — drive VRoid”. We’ll `see`, calibrate, run Traveler M then F recipes, export VRM to `../models/`.
