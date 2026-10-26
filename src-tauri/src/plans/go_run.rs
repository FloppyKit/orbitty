//! Fast preflight and detached spawn for `go.start`. The conductor is
//! `scripts/go-run.sh` (copied to `~/bin/go-run.sh` after PASS). This
//! module does not move a card and does not write permissions.

use serde_json::{json, Value};
use std::os::unix::fs::PermissionsExt;
use std::os::unix::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use super::dirs::Roots;
use super::go_runner::{self, yaml_top_scalar, DISK_MIN_KIB};
use super::locate::{locate, Locate};
use super::seal::seal_note;

pub fn runs_dir(roots: &Roots) -> PathBuf {
    crate::plans::plans_dir(roots).join("_go-runs")
}

pub fn lock_dir(roots: &Roots) -> PathBuf {
    runs_dir(roots).join("LOCK")
}

/// True when the run lock names this card. doing_fire uses it to skip.
pub fn lock_names_card(roots: &Roots, card_id: &str) -> bool {
    let raw = std::fs::read_to_string(lock_dir(roots).join("card")).unwrap_or_default();
    let named = raw.trim();
    !card_id.trim().is_empty() && named == card_id.trim()
}

pub fn lock_names_live(card_id: &str) -> bool {
    lock_names_card(&Roots::real(), card_id)
}

pub fn validate_run_id(run_id: &str) -> Result<(), String> {
    if run_id.is_empty()
        || !run_id
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '.' || c == '_' || c == '-')
    {
        return Err(format!("bad run id {run_id:?}"));
    }
    Ok(())
}

pub fn state_path(roots: &Roots, run_id: &str) -> Result<PathBuf, String> {
    validate_run_id(run_id)?;
    Ok(runs_dir(roots).join(run_id).join("state.json"))
}

pub fn read_run_state(run_id: &str) -> Result<Value, String> {
    let path = state_path(&Roots::real(), run_id)?;
    let raw = std::fs::read_to_string(&path)
        .map_err(|_| format!("no run state for {run_id}"))?;
    serde_json::from_str(&raw).map_err(|e| format!("bad run state for {run_id}: {e}"))
}

pub fn run_result_value(run_id: &str) -> Result<Value, String> {
    let st = read_run_state(run_id)?;
    Ok(json!({
        "run_id": st.get("run_id").cloned().unwrap_or(json!(run_id)),
        "stage": st.get("stage").cloned().unwrap_or(Value::Null),
        "result_line": st.get("result_line").cloned().unwrap_or(Value::Null),
        "tip": st.get("tip").cloned().unwrap_or(Value::Null),
        "go": st.get("go").cloned().unwrap_or(Value::Null),
    }))
}

pub fn parse_df_available_kib(text: &str) -> Result<u64, String> {
    for line in text.lines().skip(1) {
        let mut cols = line.split_whitespace();
        let _fs = cols.next();
        let _blocks = cols.next();
        let _used = cols.next();
        if let Some(avail) = cols.next() {
            if let Ok(n) = avail.parse::<u64>() {
                return Ok(n);
            }
        }
    }
    Err("disk check failed".into())
}

pub fn disk_refusal(kib: u64) -> Option<String> {
    if kib < DISK_MIN_KIB {
        Some(format!("disk {kib} KiB free, need {DISK_MIN_KIB}"))
    } else {
        None
    }
}

pub fn live_disk_kib() -> Result<u64, String> {
    let out = Command::new("/bin/df")
        .args(["-k", "/"])
        .output()
        .map_err(|e| format!("disk check failed: {e}"))?;
    if !out.status.success() {
        return Err("disk check failed".into());
    }
    parse_df_available_kib(&String::from_utf8_lossy(&out.stdout))
}

pub fn rig_ok_present(home: &Path) -> Option<PathBuf> {
    let root = home.join("rgl-proofs");
    let Ok(jobs) = std::fs::read_dir(&root) else {
        return None;
    };
    for job in jobs.flatten() {
        let p = job.path().join("logs").join("RIG_OK");
        if p.is_file() {
            return Some(p);
        }
    }
    None
}

pub fn heavy_refusal(ps: &str) -> Option<String> {
    for line in ps.lines() {
        if line.contains("vizdoom") {
            return Some("heavy job running (vizdoom)".into());
        }
        let low = line.to_ascii_lowercase();
        if low.contains("python") && (low.contains("rgl-proofs") || low.contains("vizdoom")) {
            return Some("heavy job running (RGL python)".into());
        }
        let parts: Vec<&str> = line.split_whitespace().collect();
        for (i, part) in parts.iter().enumerate() {
            let base = part.rsplit('/').next().unwrap_or(part);
            if base == "cargo" {
                let next = parts.get(i + 1).copied().unwrap_or("");
                if matches!(
                    next,
                    "build" | "test" | "check" | "run" | "clippy" | "install" | "bench"
                ) {
                    return Some(format!("heavy job running (cargo {next})"));
                }
            }
        }
    }
    None
}

fn live_ps() -> Result<String, String> {
    let out = Command::new("/bin/ps")
        .args(["-ax", "-o", "command="])
        .output()
        .map_err(|e| format!("process check failed: {e}"))?;
    Ok(String::from_utf8_lossy(&out.stdout).into_owned())
}

pub fn worktree_refusal(wt: &Path, base: &str) -> Result<(), String> {
    if !wt.exists() {
        return Ok(());
    }
    let head = git_out(wt, &["rev-parse", "HEAD"])?;
    if head != base {
        return Err(format!("base {base} != worktree HEAD {head}"));
    }
    let dirty = git_out(wt, &["status", "--porcelain"]).unwrap_or_default();
    if !dirty.is_empty() {
        return Err("worktree is dirty".into());
    }
    Ok(())
}

fn git_out(wt: &Path, args: &[&str]) -> Result<String, String> {
    let mut cmd = Command::new("git");
    cmd.arg("-C").arg(wt);
    cmd.args(args);
    let out = cmd.output().map_err(|e| format!("git failed: {e}"))?;
    if !out.status.success() {
        return Err(format!(
            "git {} failed",
            args.first().copied().unwrap_or("?")
        ));
    }
    Ok(String::from_utf8_lossy(&out.stdout).trim().to_string())
}

pub struct StartPlan {
    pub run_id: String,
    pub card: String,
    pub go_file: PathBuf,
    pub state_file: PathBuf,
    pub log_file: PathBuf,
    pub script: PathBuf,
}

fn executable_file(path: &Path) -> bool {
    let Ok(meta) = std::fs::metadata(path) else {
        return false;
    };
    meta.is_file() && meta.permissions().mode() & 0o111 != 0
}

/// `None` when `path` is an executable file. The refusal text is what
/// `go.start` / `go_start` write on the card. No spawn.
pub fn ctl_missing_reason(path: &Path) -> Option<String> {
    if executable_file(path) {
        None
    } else {
        Some(format!(
            "bundled orbittyctl missing at {}",
            path.display()
        ))
    }
}

pub fn missing_bundled_ctl() -> Option<String> {
    ctl_missing_reason(&crate::coordinator::bundled_orbittyctl())
}

fn conductor_script() -> PathBuf {
    #[cfg(test)]
    if let Ok(p) = std::env::var("ORBITTY_GO_RUN_SH") {
        if !p.is_empty() {
            return PathBuf::from(p);
        }
    }
    Roots::real().home.join("bin").join("go-run.sh")
}

fn mint_run_id() -> String {
    let n = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    format!("r-{}-{n}", std::process::id())
}

/// Fixed checks, then the selected runner. No spawn. No card write.
pub fn prepare_start(card: &str) -> Result<StartPlan, String> {
    let roots = Roots::real();
    let card = card.trim();
    if card.is_empty() {
        return Err("go.start needs card".into());
    }
    let go = match locate(&roots, card, "", &[]) {
        Locate::Found(g) => g,
        Locate::Bad { note } => return Err(note),
        Locate::Missing => return Err(format!("no GO for card {card}")),
    };
    if let Some(note) = seal_note(&roots, &go) {
        return Err(note);
    }
    let base = yaml_top_scalar(&go.yaml, "base").unwrap_or_default();
    let wt_raw = yaml_top_scalar(&go.yaml, "worktree").unwrap_or_default();
    if base.is_empty() || wt_raw.is_empty() {
        return Err("GO header missing base or worktree".into());
    }
    worktree_refusal(Path::new(&wt_raw), &base)?;
    if lock_dir(&roots).exists() {
        return Err("another Go run holds the lock".into());
    }
    if let Some(p) = rig_ok_present(&roots.home) {
        return Err(format!("RIG_OK present ({}); not starting", p.display()));
    }
    let ps = live_ps()?;
    if let Some(why) = heavy_refusal(&ps) {
        return Err(why);
    }
    let kib = live_disk_kib()?;
    if let Some(why) = disk_refusal(kib) {
        return Err(why);
    }
    go_runner::validate_selected(&roots).map_err(|e| e)?;
    let script = conductor_script();
    if !script.is_file() {
        return Err(format!(
            "conductor {} is not runnable",
            script.display()
        ));
    }
    if let Some(why) = missing_bundled_ctl() {
        return Err(why);
    }
    let run_id = mint_run_id();
    let dir = runs_dir(&roots).join(&run_id);
    std::fs::create_dir_all(&dir).map_err(|e| format!("mkdir {}: {e}", dir.display()))?;
    Ok(StartPlan {
        run_id,
        card: card.to_string(),
        go_file: go.path,
        state_file: dir.join("state.json"),
        log_file: dir.join("conductor.log"),
        script,
    })
}

pub fn spawn_plan(plan: &StartPlan) -> Result<Value, String> {
    spawn_detached(
        &plan.script,
        &[plan.card.clone(), plan.go_file.display().to_string(), plan.run_id.clone()],
        &plan.log_file,
    )?;
    Ok(json!({
        "run_id": plan.run_id,
        "state_file": plan.state_file.display().to_string(),
    }))
}

pub fn spawn_detached(script: &Path, args: &[String], log: &Path) -> Result<(), String> {
    if let Some(parent) = log.parent() {
        std::fs::create_dir_all(parent).map_err(|e| format!("mkdir {}: {e}", parent.display()))?;
    }
    let file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(log)
        .map_err(|e| format!("log {}: {e}", log.display()))?;
    let err = file
        .try_clone()
        .map_err(|e| format!("log {}: {e}", log.display()))?;
    let mut cmd = Command::new(script);
    cmd.args(args)
        .env(
            "GO_RUN_CTL",
            crate::coordinator::bundled_orbittyctl(),
        )
        .stdin(Stdio::null())
        .stdout(Stdio::from(file))
        .stderr(Stdio::from(err));
    // New session so a Nightly quit does not take the conductor with it.
    // Safety: runs in the forked child, before exec.
    unsafe {
        cmd.pre_exec(|| {
            if setsid() < 0 {
                return Err(std::io::Error::last_os_error());
            }
            Ok(())
        });
    }
    cmd.spawn()
        .map_err(|e| format!("spawn {}: {e}", script.display()))?;
    Ok(())
}

extern "C" {
    fn setsid() -> i32;
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::PermissionsExt;

    fn tmp() -> PathBuf {
        let p = std::env::temp_dir().join(format!(
            "orbitty-go-run-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or(0)
        ));
        std::fs::create_dir_all(&p).unwrap();
        p
    }

    #[test]
    fn lock_names_only_the_card_in_the_file() {
        let home = tmp();
        let roots = Roots { home: home.clone() };
        let dir = lock_dir(&roots);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("card"), "c-1\n").unwrap();
        assert!(lock_names_card(&roots, "c-1"));
        assert!(!lock_names_card(&roots, "c-2"));
        assert!(!lock_names_card(&roots, ""));
        let _ = std::fs::remove_dir_all(&home);
    }

    #[test]
    fn disk_and_heavy_and_rig_are_fixed() {
        assert!(disk_refusal(DISK_MIN_KIB).is_none());
        assert!(disk_refusal(DISK_MIN_KIB - 1).unwrap().contains("need"));
        let df = "Filesystem 1024-blocks Used Available Capacity\n/dev/disk 1 1 21495808 1% /\n";
        assert_eq!(parse_df_available_kib(df).unwrap(), 21_495_808);
        assert!(heavy_refusal("rust-analyzer cargo metadata").is_none());
        assert!(heavy_refusal("/usr/bin/cargo test --lib").unwrap().contains("cargo"));
        assert!(heavy_refusal("vizdoom -iwad x").unwrap().contains("vizdoom"));
        assert!(heavy_refusal("python3 rgl-proofs/job/run.py")
            .unwrap()
            .contains("RGL"));
        let home = tmp();
        assert!(rig_ok_present(&home).is_none());
        let rig = home.join("rgl-proofs/job/logs");
        std::fs::create_dir_all(&rig).unwrap();
        std::fs::write(rig.join("RIG_OK"), b"").unwrap();
        assert!(rig_ok_present(&home).is_some());
        let _ = std::fs::remove_dir_all(&home);
    }

    #[test]
    fn worktree_base_must_match_and_be_clean() {
        let home = tmp();
        let missing = home.join("absent");
        assert!(worktree_refusal(&missing, "abc").is_ok());
        let wt = home.join("wt");
        std::fs::create_dir_all(&wt).unwrap();
        Command::new("git").args(["-C", wt.to_str().unwrap(), "init", "-q", "-b", "go/t"]).status().unwrap();
        Command::new("git").args(["-C", wt.to_str().unwrap(), "config", "user.email", "t@t"]).status().unwrap();
        Command::new("git").args(["-C", wt.to_str().unwrap(), "config", "user.name", "t"]).status().unwrap();
        std::fs::write(wt.join("a"), b"a\n").unwrap();
        Command::new("git").args(["-C", wt.to_str().unwrap(), "add", "a"]).status().unwrap();
        Command::new("git").args(["-C", wt.to_str().unwrap(), "commit", "-q", "-m", "a"]).status().unwrap();
        let head = git_out(&wt, &["rev-parse", "HEAD"]).unwrap();
        assert!(worktree_refusal(&wt, &head).is_ok());
        assert!(worktree_refusal(&wt, "0".repeat(40).as_str()).unwrap_err().contains("!="));
        std::fs::write(wt.join("a"), b"dirty\n").unwrap();
        assert!(worktree_refusal(&wt, &head).unwrap_err().contains("dirty"));
        let _ = std::fs::remove_dir_all(&home);
    }

    #[test]
    fn run_id_rejects_a_path() {
        assert!(validate_run_id("../x").is_err());
        assert!(validate_run_id("r-1").is_ok());
        assert!(read_run_state("../etc").is_err());
    }

    #[test]
    fn spawn_returns_without_waiting() {
        let home = tmp();
        let script = home.join("s.sh");
        std::fs::write(&script, "#!/bin/zsh\nsleep 3\n").unwrap();
        let mut perms = std::fs::metadata(&script).unwrap().permissions();
        perms.set_mode(0o755);
        std::fs::set_permissions(&script, perms).unwrap();
        let log = home.join("log");
        let start = std::time::Instant::now();
        spawn_detached(&script, &["c".into(), "g".into(), "r-1".into()], &log).unwrap();
        assert!(
            start.elapsed() < std::time::Duration::from_secs(2),
            "spawn waited {:?}",
            start.elapsed()
        );
        let src = include_str!("go_run.rs");
        let body = src.split("mod tests").next().unwrap();
        assert!(body.contains("setsid()"), "detached spawn must setsid");
        assert!(!body.contains(".wait()"), "must not wait");
        let _ = std::fs::remove_dir_all(&home);
    }

    #[test]
    fn prepare_calls_the_fixed_checks_before_spawn() {
        let src = include_str!("go_run.rs");
        let start = src.find("pub fn prepare_start(").unwrap();
        let body = &src[start..];
        let end = body.find("pub fn spawn_plan(").unwrap();
        let body = &body[..end];
        let seal = body.find("seal_note(").unwrap();
        let base = body.find("worktree_refusal(").unwrap();
        let lock = body.find("lock_dir(").unwrap();
        let rig = body.find("rig_ok_present(").unwrap();
        let heavy = body.find("heavy_refusal(").unwrap();
        let disk = body.find("disk_refusal(").unwrap();
        let runner = body.find("validate_selected(").unwrap();
        assert!(seal < base && base < lock && lock < rig && rig < heavy && heavy < disk && disk < runner);
        assert!(!body.contains("allow_push"), "permissions must not reach the fixed checks");
        let ctl = body.find("missing_bundled_ctl()").expect("ctl check");
        let ok_plan = body.rfind("Ok(StartPlan").expect("plan");
        assert!(ctl < ok_plan, "missing ctl must refuse before a plan is returned");
    }

    #[test]
    fn missing_ctl_refuses_with_the_path_and_does_not_spawn() {
        let home = tmp();
        let missing = home.join("orbittyctl");
        let why = ctl_missing_reason(&missing).unwrap();
        assert!(why.contains("bundled orbittyctl missing at"));
        assert!(why.contains(&missing.display().to_string()));
        std::fs::write(&missing, b"#!/bin/zsh\n").unwrap();
        let mut perms = std::fs::metadata(&missing).unwrap().permissions();
        perms.set_mode(0o644);
        std::fs::set_permissions(&missing, perms).unwrap();
        assert!(ctl_missing_reason(&missing).unwrap().contains("missing"));
        let mut perms = std::fs::metadata(&missing).unwrap().permissions();
        perms.set_mode(0o755);
        std::fs::set_permissions(&missing, perms).unwrap();
        assert!(ctl_missing_reason(&missing).is_none());
        let src = include_str!("go_run.rs");
        let spawn = src.split("pub fn spawn_plan(").nth(1).unwrap();
        let spawn = spawn.split("pub fn spawn_detached(").next().unwrap();
        assert!(!spawn.contains("Command::new"), "spawn_plan must not spawn itself");
        let _ = std::fs::remove_dir_all(&home);
    }

    #[test]
    fn spawn_sets_go_run_ctl_to_the_bundled_path() {
        let home = tmp();
        let script = home.join("s.sh");
        let out = home.join("env.txt");
        std::fs::write(
            &script,
            format!("#!/bin/zsh\nprint -r -- \"$GO_RUN_CTL\" > {}\n", out.display()),
        )
        .unwrap();
        let mut perms = std::fs::metadata(&script).unwrap().permissions();
        perms.set_mode(0o755);
        std::fs::set_permissions(&script, perms).unwrap();
        spawn_detached(&script, &[], &home.join("log")).unwrap();
        let expected = crate::coordinator::bundled_orbittyctl();
        let mut got = String::new();
        for _ in 0..50 {
            if let Ok(s) = std::fs::read_to_string(&out) {
                if !s.is_empty() {
                    got = s;
                    break;
                }
            }
            std::thread::sleep(std::time::Duration::from_millis(20));
        }
        assert_eq!(got.trim(), expected.display().to_string());
        let _ = std::fs::remove_dir_all(&home);
    }

}
