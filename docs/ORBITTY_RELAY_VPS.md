# Orbitty relay — one-user VPS replica

GO-ORBITTY-RELAY-VPS. Same `orbitty-relay` binary as the Mac LaunchAgent.
Not a public Nostr relay. Not a multi-tenant table. jsonl stays SoT.

Local dogfood stays the default: `127.0.0.1:13120` via `space.orbitty.relay`.
Device nsec stays on the Mac.

## Bind law

| Bind | Allowed |
|------|---------|
| `127.0.0.1:13120` / `::1` | yes (default; SSH `-L` / `-R`) |
| Tailscale CGNAT `100.64.0.0/10:13120` | yes (one-user replica) |
| `0.0.0.0` / `::` / public IPs / Caddy site-front | **refused** until a named bind GO |

## Settings / ctl (this Mac)

Empty replica URL = local sidecar only.

```
orbittyctl relay status
orbittyctl relay replica --url 'ws://100.64.0.1:13120'
orbittyctl relay replica --url ''
```

Settings → Event log → **Replica URL**. Private relay still has to be on.
Publish is best-effort to local, replica, or both. Replica down does not
fail the board.

SSH tunnel (local sidecar stays on 13120):

```
ssh -N -L 13122:127.0.0.1:13120 do-edge
orbittyctl relay replica --url 'ws://127.0.0.1:13122'
```

## VPS datadir (that user only)

`--data-dir DIR` is required on Linux. Layout matches the Mac:

```
DIR/
  device.npub          # 64-hex allowlist. Never device.nsec.
  relay/events.jsonl   # replica index, not kitchen SoT
```

Copy `device.npub` from the Mac pubkey (printed at sidecar start:
`orbitty-relay: pubkey …`). Do not copy `device.nsec`.

```
orbitty-relay --data-dir /var/lib/orbitty-relay --bind 127.0.0.1:13120
# or, if this box has Tailscale:
orbitty-relay --data-dir /var/lib/orbitty-relay --bind "$(tailscale ip -4):13120"
```

Unit example: `deploy/orbitty-relay.service.example` (loopback bind).

## WAIT_EXTERNAL (2026-09-17)

SSH to `do-edge` (`psycharts-edge`, `159.223.190.6`) works. Bind still
waits:

- Tailscale on that box is **NeedsLogin** — no `100.x` to bind
- Box is Linux x86_64; this kitchen builds darwin-arm64. No `rustc`/`cargo`
  on the VPS, no linux-amd64 `orbitty-relay` there
- `ss` on `:13120` is empty today — do not invent `0.0.0.0:13120`
- `bazzite-forge` Tailscale offline
- Shared VPS stack (`CASCADE_VPS_SHARED_STACK_v0.md`) stays **PARKED** —
  do not park this magnet on Ghost / LiveKit / FKT

Local bits shipped. Bind when there is a Linux binary **and** Tailscale
(or SSH `-R` to loopback) on a box you will start.

## Out of scope

Provisioning a new VPS · public internet listeners · NCH / FKT kinds ·
rewriting `events.jsonl` · LifeSprite pairing · Buzz community relay


## Live install (psycharts-edge)

As of 2026-09-17 CT: `orbitty-relay` runs under systemd on **psycharts-edge**
(`Host do-edge`), bind `127.0.0.1:13120`, data `/var/lib/orbitty-relay`
(same `device.nsec` as the Mac). Not public.

```bash
ssh -N -L 13120:127.0.0.1:13120 do-edge   # or 13129 if local LaunchAgent holds 13120
systemctl status orbitty-relay            # on the droplet
```
