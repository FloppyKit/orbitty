# GO-ORBITTY-TABBITY-BRIDGE Phase 0

Read only. No product code in this commit. Not run against a live browser: cook does not load the extension or install the host.

## connectNative and the worker

Chromium keeps an MV3 service worker alive while a `connectNative` port is open. Brave is Chromium, so the same rule applies. The port can still drop (browser restart, host exit, worker kill). The worker must reconnect with backoff. Not confirmed on this machine.

## windows.update and raising the app

`chrome.windows.update(id, {focused: true})` focuses that browser window. On macOS it does not always activate the app (another Space, another front app). D-4 fallback stays: on focus and reopen, Orbitty also activates the browser by bundle (`open -b`), the same idea as `activate` in the window facade. Not confirmed live.

## Host parent

Chrome and Brave launch a native host as a child process. `getppid()` is the browser. The host logs that pid on stderr. It does not refuse on ppid (a test harness is not the browser). The fence is the extension id, not the parent pid.

## Host manifest directories

User-level, under `$HOME` (no absolute home prefix in the repo):

- Chrome: `$HOME/Library/Application Support/Google/Chrome/NativeMessagingHosts`
- Brave: `$HOME/Library/Application Support/BraveSoftware/Brave-Browser/NativeMessagingHosts`

File name: `space.orbitty.tabbity.json`. System-wide dirs under `/Library/...` exist and are not used.

## Who renders rowsFor

`src/lib/RailPage.svelte` calls `rowsFor`. `src/lib/rail/List.svelte` paints those rows. Walkers do not use `rowsFor`. `syncHabitat` in `src/lib/rail/pageClaim.svelte.ts` builds its own rows and passes them to `habitatEntryFor`. A tab claim is absent from `list_windows`, so it is an orphan and today goes through `sleeperRowFor` (always asleep, title from `titleAtClaim`, which a tab claim leaves null). S5 has to feed tab rows through both `rowsFor` and that orphan path or the walker never matches the rail row.

## Claim literals and rematch

TS literals that build a Claim (do not spread `prev`, so an unknown field dies):

- `src/lib/rail/pageBeing.svelte.ts` `claim()` — rail Assign and walker Assign-to-planet
- `src/lib/NextCard.svelte` `claimWindow()` — same shape, saves via `save_state` (merge can carry `tab`)

TS writers that spread the claim (keep `tab` if the field exists): `setSpritePack`, being-editor maps in `pageBeing.svelte.ts`, `rewriteWindowId` in `pageDrag.svelte.ts` (also stamps bundle/title — must not run on a tab claim).

Rust literals:

- `windows/claim_front.rs` `apply_claim` (window door; tab claims must not use it)
- `windows/claim_front_tests.rs` `claim_of` (tests)
- `habitat/herdr.rs` test helper `claim` (tests)
- `agents/starter.rs` builds Claim through serde, so a new default field loads as absent

Rematch:

- `pageDrag.svelte.ts` `rematchClaims` — stable key, bundle+title, file path, then soft (sole unused window of the bundle)
- `pageClaim.svelte.ts` `claimStableKey` — null unless bundle + titleAtClaim + titleIndex (a tab claim with those null already fails the key)
- `coordinator/soul_wake.rs` `wake_sleeper_inner` 1b — same bundle+title, then sole window of the bundle; step 5 opens the bundle if set

A tab claim that copied Brave's bundle id would be adopted onto the one open Brave window. Guards have to skip any claim with `tab` or a `tab:` id, and must not stamp a bundle onto it.

## save_with_events and a null fingerprint

`save_with_events` diffs claims through `event_log::diff_claims`. The fingerprint is `bundle|title|titleIndex`. All three are null on a tab claim, so the fingerprint is None. A new `tab:` row is a normal `claim` event `{windowId, projectId}`. Removal is a normal `unclaim` event. It is never paired as a silent reboot adoption. No new event kind. Null fingerprint fields stay omitted (`skip_serializing_if`).

## Batch undo

One tab batch arms the single user undo slot (same slot as a tray claim). Undo puts back every id in that batch. A second claim within 3 s replaces that slot. Documented, not a second slot.
