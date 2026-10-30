# herdr-core read side

Orbitty reads herdr through one client (`src-tauri/src/herdr`). The flag `herdr_core` defaults off. Off is today's `herdr` TUI probes, byte for byte. On, those reads use `hc` (herdr-core). Launch, send, wait, stop, and focus stay on the TUI argv until 05c-b.

## Health

herdr-core is down when its binary is missing, `hc server status` is not ok, or the `hc events` stream ends. Orbitty rechecks every 30 seconds and switches back after two healthy checks, at most one switch per 30 seconds. Each switch writes one `herdr-health` event whose `source` is `herdr` or `fallback`. While down, liveness is unknown. It is not died.

The hot path never runs `hc server start`. A launchd job owns the core.

## Locate

`ORBITTY_HC`, then the bundled `herdr-core` next to the Orbitty binary, then `~/.local/bin/herdr-core`, then `hc` on `PATH`.

## Peek

`herdr.status` and `herdr.peek` are read verbs. `orbittyctl herdr status` prints `up/source=herdr` and the hc version when the core is up. `orbittyctl herdr peek <card> [--limit N]` prints a read-only preview. The preview asks for 40 lines by default and never more than 200. After stripping terminal escapes it keeps at most 200 lines, 16 KB, and 400 characters per line. Secrets are replaced with `[redacted]` before the text leaves Rust. The preview element has no input handler.

Clicking a droid opens that capsule instead of focusing the pane. The focus function is still in the tree for 05c-b.

## Supervisor template

`scripts/launchd/space.orbitty.herdr-core.plist` is a template (`KeepAlive`, logs under `~/.buzz/ops/herdr-core/`). Replace `__HOME__`. Chuck installs it after this GO passes and adopts the core that is already running. This tree does not call `launchctl` and does not write `~/Library/LaunchAgents`.

## Bundling

Ship the `herdr-core` binary only, with its Apache-2.0 `LICENSE` and `NOTICE`. Re-sign and notarize the app after the binary is inside the bundle. Updates follow the Orbitty update channel; do not point Orbitty at a second updater. End-user bundling build is out of scope here.

## Flag

`orbittyctl flags set herdr_core off` is the kill switch. Nightly may turn the flag on only after 05c-a and 05c-b both Verify PASS and Matt dogfoods. Until then `auto_on_verify` is false.
