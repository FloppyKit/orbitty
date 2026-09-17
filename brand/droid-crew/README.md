# droid-crew

Agent walkers on the planet. Three bodies (dog, penguin, octopus), four
dirs (e/s/w/n), four walk frames, 64×64. Habitat `walker_fit` keeps them
the same on-disc size as NES bitties.

`droid_pack.rs` loads `packs/orbitty-droid-{dog,penguin,octopus}`. Dummy
12×14 in `droid.rs` is the fallback if these PNGs are missing.
