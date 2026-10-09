# orbittyctl

Control-socket client. The Cargo `name` stays `orbittyctl`.

`main.rs` holds the crate `//!` docs, `#[cfg(unix)] mod imp;`, and both `main`s. `main` calls `imp::run()`.

The client implementation is `imp/`. Help text is the `USAGE` string in `imp/usage.rs`. `-h` and `--help` print it and exit 0.
