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

Loopback replica on `do-edge` (`psycharts-edge`) is up:
`127.0.0.1:13120`, not `0.0.0.0`. Allowlist file is `device.npub`
(nsec removed from the box). Reach it with SSH `-L 13122`.

Still waiting:

- Tailscale on that box is **NeedsLogin** — no `100.x` bind
- Kitchen does not ship linux-amd64; the box has no rustc
- Shared VPS stack stays **PARKED** — do not park this magnet on Ghost /
  LiveKit / FKT

## Out of scope

Provisioning a new VPS · public internet listeners · NCH / FKT kinds ·
rewriting `events.jsonl` · LifeSprite pairing · Buzz community relay
