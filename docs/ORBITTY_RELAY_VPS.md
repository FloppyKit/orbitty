# Orbitty relay — one-user VPS replica (Tailscale / SSH)

Card: **GO-ORBITTY-RELAY-VPS**. Same `orbitty-relay` binary as the Mac
LaunchAgent sidecar. Not a public Nostr relay. Never bind `0.0.0.0`.

Local dogfood (Mac) stays the default: `127.0.0.1:13120` via
`space.orbitty.relay`.

## Bind policy

| Bind | Allowed |
|------|---------|
| `127.0.0.1:13120` / `::1` | yes (default) |
| Tailscale CGNAT `100.64.0.0/10` | yes (one-user replica) |
| `0.0.0.0` / `::` / public IPs | **refused** |

## Path A — SSH local forward (simplest)

Keep the process on loopback on the edge box; reach it from the Mac:

```bash
# on psycharts-edge / do-edge (Linux)
mkdir -p /var/lib/orbitty-relay
# copy device.nsec once (scp); never commit it
orbitty-relay --data-dir /var/lib/orbitty-relay --bind 127.0.0.1:13120

# on Mac
ssh -N -L 13120:127.0.0.1:13120 do-edge
# Orbitty publish still uses ws://127.0.0.1:13120
```

AUTH accepts the classic `relay` tag `ws://127.0.0.1:13120` for this path.

## Path B — Tailscale IP bind

```bash
# on the Linux box that has Tailscale (e.g. psycharts-edge)
TS_IP=$(tailscale ip -4)
orbitty-relay --data-dir /var/lib/orbitty-relay --bind "${TS_IP}:13120"
```

On the Mac (Orbitty already using that device.nsec / same pubkey):

```bash
export ORBITTY_RELAY_ADDR="${TS_IP}:13120"
export ORBITTY_RELAY_WS="ws://${TS_IP}:13120"
# then launch Orbitty.app so publish_best targets the replica
```

## Smoke

```bash
orbitty-relay smoke                          # against local LaunchAgent
orbitty-relay smoke --bind 127.0.0.1:13120  # same
```

## Out of scope

- Provisioning a new VPS (Hetzner cards are PARKED; live edge is DO).
- Public internet listeners.
- NCH / FKT kinds (`t=nch` stays rejected).
