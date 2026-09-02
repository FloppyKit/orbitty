# Cursor Ultra — GO-ORBITTY-MCP-SEAT

**Model:** `kimi-k3-max` if listed, else `composer-2.5` · **one writer**
**Gate:** Matt **GO-ORBITTY-MCP-SEAT** via Pixel Artist **2026-08-27** (applied)
**Tree:** `/Users/dude/goose artifacts/livesprite-tauri`
**SoT:** `~/.buzz/PLANS/ORBITTY_NEXT_PATH_v0.md` + `ORBITTY_CONTROL_PLANE_v0.md`

Matt 2026-08-27: **yes, auto-write the Goose seat.** Cascade stage 4 was copy-paste-only; this GO replaces that with an idempotent merge.

**This GO writes Matt’s Goose config. It does not add Orbitty tools. It does not start MCP-1.**

## Why
MCP-0 is built (`~/.local/bin/orbitty-mcp`). Pixel pasted the snippet once. New chats still missed the tools until the extension was enabled in-session. Auto-write on install makes the seat survive rebuilds without a human YAML paste.

Framing is newline JSON (Goose/rmcp). Binary already accepts both NDJSON and Content-Length.

## Law
1. **Merge, never replace.** Other extensions stay. Backup first.
2. Idempotent: running twice leaves one `orbitty:` block.
3. Do **not** restart Berd/Goose. Next new chat picks it up.
4. Do **not** auto-enable broken Sugar/Tavily. Do not touch secrets.
5. Do **not** bind TCP. Do not add tools. Do not stamp MCP-0 (that’s a Matt phrase).
6. Fail obvious: missing yaml → write a minimal file **only if none exists**; if yaml is unreadable, abort and leave a `.bak`.

## Do

### 1. Helper script
`scripts/write-goose-orbitty-seat.sh`

```
TARGET="$HOME/.config/goose/config.yaml"
BACKUP="$TARGET.bak-orbitty-$(date +%Y%m%d-%H%M%S)"
```

- If `config.yaml` exists: copy to BACKUP, then Python/YAML merge:
  - `extensions.orbitty` =
    ```yaml
    enabled: true
    type: stdio
    name: orbitty
    description: Drive a running Orbitty over its local control socket (windows, companion, packs, state)
    cmd: /Users/dude/.local/bin/orbitty-mcp
    args: []
    envs: {}
    env_keys: []
    timeout: 360
    cwd: null
    bundled: null
    ```
  - Preserve key order as much as PyYAML allows. Prefer ruamel if already in PATH; otherwise PyYAML dump is OK.
- `cmd` must stay the **absolute** path `~/.local/bin/orbitty-mcp` expanded (`$HOME/.local/bin/orbitty-mcp`).
- `timeout: 360` (companion.send can poll Berd 5 min). Initialize must not hang; that’s the adapter’s job.
- Print: backup path, whether block was created or updated.

### 2. Hook install-stable
At the end of `scripts/install-stable.sh`, after copying `orbitty-mcp`:

```zsh
echo "==> Writing Goose orbitty seat (idempotent merge)"
"$PWD/scripts/write-goose-orbitty-seat.sh" || echo "WARNING: Goose seat write failed — MCP binary is still installed" >&2
```

Update the Done blurb: item 7 no longer says “copy-paste only.”

### 3. Docs
Update `docs/ORBITTY_MCP_GOOSE_SNIPPET.md`: snippet remains as a manual fallback; note that `install-stable` now merges it.

## Out of scope
- Enabling the extension inside an already-open Berd session (UI toggle is Berd’s)
- Writing `control.json` / `control.token`
- Pixelorama MCP, WebMCP, Sugar, Tavily
- PTT, Spark, Jacob

## Verify
```bash
./scripts/write-goose-orbitty-seat.sh
# second run: still one orbitty block, new .bak
python3 -c 'import yaml; d=yaml.safe_load(open("/Users/dude/.config/goose/config.yaml")); print(d["extensions"]["orbitty"]["cmd"], d["extensions"]["orbitty"]["enabled"])'
```

Expect: `$HOME/.local/bin/orbitty-mcp` and `True`. Other stdio extensions still present (github, fleet-bus, …).

Acceptance: `docs/ACCEPTANCE-ORBITTY-MCP-SEAT.md`
