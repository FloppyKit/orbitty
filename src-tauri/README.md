# src-tauri

The Tauri crate. Rust and the Objective-C files live under `src/`.

Start at `build.rs`. On macOS it compiles `src/objc_try.m`, `src/attach/attach_mac.m`, `src/companion/talk_mac.m`, and `src/windows/window_thumb_mac.m`, then calls `tauri_build::build()`.

The app binary is built from `src/main.rs`. Commands are registered in `src/lib.rs`.
