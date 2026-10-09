# scripts

Gates and installers.

`gate.sh` is the literal scan (`zsh ./scripts/gate.sh --quick` from the repo root). `gate_allowlist.txt` is the path column plus the substring. `install-nightly.sh` and `install-stable.sh` copy the bundle. They do not `ln -sfn` an `orbitty_relay` shim. They still `rm -f` `orbitty_relay` on the build tree and after ditto. `install-kitchen.sh` is the kitchen installer. The relay name in those scripts is `orbitty-relay`.

`orbitty-needed/` is the Python bus. `thin/` writes the dev stamp.
