//! orbittyctl — CLI for the Orbitty control
//! socket. Pure std client: no Tauri, no WebView, no model, no shell.
//! One newline-JSON request → one reply.
//!
//! Exit codes: 0 ok · 2 Orbitty not running · 3 auth · 4 tool denied ·
//! 5 AX/degraded · 1 other.

#[cfg(unix)]
mod imp;

#[cfg(unix)]
fn main() {
    imp::run();
}

#[cfg(not(unix))]
fn main() {
    eprintln!("orbittyctl is unix-only (Orbitty control socket)");
    std::process::exit(1);
}
