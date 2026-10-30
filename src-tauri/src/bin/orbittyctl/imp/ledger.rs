//! `orbittyctl ledger` — local reads of the shim ledger. No control socket.

use super::parse::Opts;
use std::path::{Path, PathBuf};

#[path = "../../../ledger/mod.rs"]
mod store;

const SHIMS_SRC: &str = include_str!("../../../shims.rs");

pub(crate) fn run(opts: &Opts) -> Result<String, String> {
    let words: Vec<&str> = opts.words.iter().map(String::as_str).collect();
    match words.as_slice() {
        ["ledger", "export"] => {}
        ["ledger", other] => {
            return Err(format!("unknown ledger command {other} — export"));
        }
        _ => return Err("usage: orbittyctl ledger export [--out PATH]".into()),
    }
    let home = std::env::var_os("HOME")
        .filter(|h| !h.is_empty())
        .map(PathBuf::from)
        .ok_or("ledger export: HOME unset")?;
    let today = chrono::Local::now().format("%Y-%m-%d").to_string();
    export_with(&home, &app_data(opts), opts.out.as_deref(), &today)
}

pub(crate) fn export_with(
    home: &Path,
    app_data: &Path,
    out: Option<&str>,
    today: &str,
) -> Result<String, String> {
    let rows = store::parse_shims(SHIMS_SRC)?;
    store::export_at(home, app_data, &rows, out, today)
}

fn app_data(opts: &Opts) -> PathBuf {
    super::sock_path(opts)
        .parent()
        .map(Path::to_path_buf)
        .unwrap_or_else(|| {
            let home = std::env::var("HOME").unwrap_or_default();
            PathBuf::from(home).join("Library/Application Support/space.orbitty.mac")
        })
}
