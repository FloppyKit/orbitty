# Orbitty mouth sidecar (1B)

Harsha-style constrained decode on loopback. Spark taps five verbs. No English from the 1B. Weights live wherever the config points (`ORBITTY_MOUTH_WEIGHTS`, or `weights` in `~/.config/orbitty/mouth-sidecar.json`) — never inside `Orbitty.app`.

- Bind: `127.0.0.1:13121`
- `GET /health` · `POST /classify` `{"line":"…"}`
- LaunchAgent: `space.orbitty.mouth` (install-stable)

Quick ack in Settings = Built-in uses this if it answers under 200 ms with P≥0.40, else the keyword classifier.
