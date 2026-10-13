# state

Persisted app state.

Start at `mod.rs`. It holds `PersistedState` and re-exports `rail_geom`, `models`, `models_companion`, `models_board`, `io`, and `merge`. `io.rs` loads and saves `state.json` (`load_state`, `save_state`). `merge.rs` merges window saves, deltas, and tombstones.

`models_companion.rs` holds `CompanionState`. The board's event-emitting save is `board::save_with_events`, not this folder.

Any process that writes `state.json` must hold `state.json.lock` (an OS file lock, taken in `io.rs::update` after the in-process mutex).
One Orbitty at a time is enforced by `orbitty.instance.lock` (`instance.rs`, taken first in `run()`).
