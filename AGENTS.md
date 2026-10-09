# Agents

How to find things in this repo. Start at the folder README, then the entry it names.

## Where to look

1. The README in the folder you are in.
2. `src-tauri/src/lib.rs` for the invoke list (`generate_handler!`).
3. `src/routes/+page.svelte` for which page a window label mounts.

Parent module paths stay. A `mod.rs` re-export keeps `crate::module::` working after a file becomes a directory. `windows.rs` still `pub use`s the public window fns. `RailApi` stays in `src/lib/rail/api.ts`.

## Tests and gates

- `cargo test --lib` from `src-tauri` (offline).
- `node --test qa/*.test.ts` from the repo root. These are source scans, not a GUI walk.
- `npm run check`
- `zsh ./scripts/gate.sh --quick` from the repo root (never bash or sh).
- Release link, from `src-tauri`: `cargo build --release --bin orbitty --features dev-thin`, then `nm -u target/release/orbitty | grep -c isPlatformVersionAtLeast` is 0.

## Conventions

A move commit stays a move: `git mv` plus the lines that keep the old path compiling (`mod` / `pub use` / `pub(crate) use`, `#[path]`, `include_str!` / `concat!`, the `gate_allowlist.txt` path column, qa `readFileSync` URLs, and a Cargo `[[bin]]` path). Expected strings do not change. No behaviour change inside a map commit.

`StarsPage.svelte`, `src/lib/stars/starsFocus.ts`, `src/lib/stars/RadarDial.svelte`, and `src-tauri/src/stars/stars_ctl.rs` stay whole. The parked `go/stars-keys` branch edits the stars UI.
