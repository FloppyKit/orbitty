use std::path::{ Path };
use std::process::{Command, Stdio};
use super::send::{tool_error_line};

pub(crate) fn is_uuid(s: &str) -> bool {
    let lens = [8usize, 4, 4, 4, 12];
    let parts: Vec<&str> = s.split('-').collect();
    parts.len() == 5
        && parts
            .iter()
            .zip(lens)
            .all(|(p, n)| p.len() == n && p.chars().all(|c| c.is_ascii_hexdigit()))
}

/// v4 UUID via macOS `uuidgen` (always present on the dogfood target).
pub(crate) fn mint_uuid() -> Result<String, String> {
    let out = Command::new("uuidgen")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .output()
        .map_err(|e| format!("launch uuidgen: {e}"))?;
    if !out.status.success() {
        return Err("uuidgen failed — cannot mint a grok session id.".into());
    }
    let id = String::from_utf8_lossy(&out.stdout).trim().to_ascii_lowercase();
    if !is_uuid(&id) {
        return Err("uuidgen returned a bad id.".into());
    }
    Ok(id)
}

/// Goose session ids look like `20260909_2` (YYYYMMDD_N).
pub(crate) fn is_goose_session_id(s: &str) -> bool {
    let mut parts = s.split('_');
    let (Some(d), Some(n), None) = (parts.next(), parts.next(), parts.next()) else {
        return false;
    };
    d.len() == 8
        && d.chars().all(|c| c.is_ascii_digit())
        && !n.is_empty()
        && n.chars().all(|c| c.is_ascii_digit())
}

/// Spawn a mouth CLI, poll with a timeout, kill the child on expiry.
/// stdout on exit 0; one honest stderr line otherwise. No desktop lock is
/// ever set here — BERDCTL_LOCK belongs to the berd escape path only.
pub(crate) fn run_mouth_cli(
    tool: &str,
    bin: &Path,
    args: &[String],
    envs: &[(&str, &str)],
    timeout_secs: u64,
) -> Result<String, String> {
    run_mouth_cli_watched(tool, bin, args, envs, timeout_secs, None)
}

/// run_mouth_cli + a stderr watch: a reader thread forwards stderr lines
/// as they arrive, and `kill_on` matching a line kills the child AT ONCE,
/// returning that line as the error. Grok prints a
/// dead-resume 404 (`session get failed: 404 Not Found`) in ~0.5s but
/// STAYS ALIVE — an unwatched poll waits out the whole timeout for a
/// corpse. The kill is line-matched, never time-based: a fat `--rules`
/// soul may slow startup, and a live slow generate keeps the full
/// timeout.
pub(crate) fn run_mouth_cli_watched(
    tool: &str,
    bin: &Path,
    args: &[String],
    envs: &[(&str, &str)],
    timeout_secs: u64,
    kill_on: Option<&dyn Fn(&str) -> bool>,
) -> Result<String, String> {
    let prepared = crate::policy_guard::prepare_mouth(bin, args);
    let unsandboxed = prepared.unsandboxed;
    let result = run_mouth_cli_watched_fenced(tool, bin, prepared, envs, timeout_secs, kill_on);
    crate::policy_guard::note_unsandboxed(result, unsandboxed)
}

fn run_mouth_cli_watched_fenced(
    tool: &str,
    bin: &Path,
    mut prepared: crate::policy_guard::PreparedMouth,
    envs: &[(&str, &str)],
    timeout_secs: u64,
    kill_on: Option<&dyn Fn(&str) -> bool>,
) -> Result<String, String> {
    use std::io::BufRead as _;
    let mut child = prepared
        .cmd
        .args(&prepared.args)
        .envs(envs.iter().copied())
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("launch {}: {e}", bin.display()))?;
    // stderr → line channel. EOF (child dead, pipe closed) drops the
    // sender. The thread is never joined — a grandchild inheriting the
    // pipe must not hang the send path.
    let (tx, rx) = std::sync::mpsc::channel::<String>();
    if let Some(stderr) = child.stderr.take() {
        std::thread::spawn(move || {
            for line in std::io::BufReader::new(stderr).lines() {
                let Ok(line) = line else { break };
                if tx.send(line).is_err() {
                    break;
                }
            }
        });
    }
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(timeout_secs);
    let mut stderr_lines: Vec<String> = Vec::new();
    loop {
        // Drain arrived stderr lines; a kill-line ends it now.
        while let Ok(line) = rx.try_recv() {
            if kill_on.is_some_and(|k| k(&line)) {
                let _ = child.kill();
                let _ = child.wait();
                return Err(format!("{tool}: {line}"));
            }
            stderr_lines.push(line);
        }
        match child.try_wait() {
            Ok(Some(_)) => {
                // stderr was taken for the watch, so wait_with_output
                // only collects stdout here.
                let out = child
                    .wait_with_output()
                    .map_err(|e| format!("collect {tool} output: {e}"))?;
                if !out.status.success() {
                    // The last stderr lines may still be in flight behind
                    // the exit — drain bounded (Disconnected = reader hit
                    // EOF; a line cap guards a grandchild holding the
                    // pipe), then the usual one honest error line.
                    loop {
                        match rx.recv_timeout(std::time::Duration::from_secs(2)) {
                            Ok(line) if stderr_lines.len() < 400 => stderr_lines.push(line),
                            Ok(_) | Err(_) => break,
                        }
                    }
                    let stderr = stderr_lines.join("\n").into_bytes();
                    return Err(tool_error_line(tool, &stderr, &out.stdout, out.status.code()));
                }
                return Ok(String::from_utf8_lossy(&out.stdout).trim().to_string());
            }
            Ok(None) => {
                if std::time::Instant::now() >= deadline {
                    let _ = child.kill();
                    let _ = child.wait();
                    return Err(format!(
                        "{tool} took longer than {timeout_secs}s — killed. Try again."
                    ));
                }
                std::thread::sleep(std::time::Duration::from_millis(100));
            }
            Err(e) => return Err(format!("wait on {}: {e}", tool.to_ascii_lowercase())),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::PermissionsExt;

    fn write_exe(path: &Path, body: &str) {
        std::fs::write(path, body).unwrap();
        let mut perm = std::fs::metadata(path).unwrap().permissions();
        perm.set_mode(0o755);
        std::fs::set_permissions(path, perm).unwrap();
    }

    fn body_of<'a>(src: &'a str, sig: &str) -> &'a str {
        let at = src.find(sig).unwrap_or_else(|| panic!("missing {sig}"));
        let rest = &src[at..];
        &rest[..rest.find("\n}\n").unwrap_or(rest.len())]
    }

    #[test]
    fn run_mouth_cli_stub_cannot_write_rules() {
        let (base, roots) = crate::policy_guard::temp_policy_home("mouthcli");
        let rules = roots.plans_dir.join("RULES.yaml");
        std::fs::write(&rules, crate::policy_guard::RULES_FIXTURE).unwrap();
        let before = std::fs::read(&rules).unwrap();
        let script = base.join("mouth.py");
        write_exe(&script, crate::policy_guard::d4_edit_script());
        let err = crate::policy_guard::with_roots(roots.clone(), || {
            run_mouth_cli(
                "Goose",
                &script,
                &[rules.display().to_string()],
                &[],
                30,
            )
        })
        .unwrap_err();
        assert!(
            err.to_ascii_lowercase().contains("not permitted"),
            "{err}"
        );
        assert_eq!(std::fs::read(&rules).unwrap(), before);
        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn mouth_env_path_bundle_first() {
        let bundle = crate::coordinator::bundled_orbittyctl()
            .parent()
            .unwrap()
            .to_path_buf();
        let ctl = bundle.join("orbittyctl");
        let created = !ctl.exists();
        if created {
            write_exe(&ctl, "#!/bin/sh\necho bundle-ctl\n");
        }
        let (base, roots) = crate::policy_guard::temp_policy_home("env");
        let script = base.join("env.sh");
        write_exe(
            &script,
            "#!/bin/sh\necho \"PATH=$PATH\"\necho \"ORBITTY_CTL=$ORBITTY_CTL\"\necho \"ORBITTY_MCP=$ORBITTY_MCP\"\ncommand -v orbittyctl || true\n",
        );
        let out = crate::policy_guard::with_roots(roots, || {
            run_mouth_cli("Goose", &script, &[], &[], 30)
        })
        .unwrap();
        let bundle_s = bundle.display().to_string();
        let path = out.lines().find(|l| l.starts_with("PATH=")).unwrap();
        assert!(
            path.trim_start_matches("PATH=").starts_with(&bundle_s),
            "{out}"
        );
        assert!(out.contains(&format!("ORBITTY_CTL={bundle_s}/orbittyctl")), "{out}");
        assert!(out.contains(&format!("ORBITTY_MCP={bundle_s}/orbitty-mcp")), "{out}");
        let which = out.lines().last().unwrap();
        assert!(which.starts_with(&bundle_s), "{which}");
        assert!(!which.contains(".local/bin"), "{which}");
        if created {
            let _ = std::fs::remove_file(&ctl);
        }
        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn no_sandbox_means_tools_off() {
        let (base, roots) = crate::policy_guard::temp_policy_home("nosb");
        let script = base.join("args.sh");
        write_exe(
            &script,
            "#!/bin/sh\nfor a in \"$@\"; do echo \"ARG:$a\"; done\n",
        );
        let goose = crate::coordinator::goose_args("soul", "hi", None, true);
        let gout = crate::policy_guard::with_override(Some(roots.clone()), true, || {
            run_mouth_cli("Goose", &script, &goose, &[], 30)
        })
        .unwrap();
        assert!(gout.contains("ARG:--no-profile"), "{gout}");
        assert!(!gout.contains("--with-builtin"), "{gout}");
        let grok = crate::coordinator::grok_args("sid", false, "soul", "hi", true, None);
        let kout = crate::policy_guard::with_override(Some(roots.clone()), true, || {
            run_mouth_cli("Grok", &script, &grok, &[], 30)
        })
        .unwrap();
        assert!(kout.contains("ARG:--disallowed-tools"), "{kout}");
        assert!(kout.contains("ARG:bash,shell,computer"), "{kout}");
        let marker = base.join("hermes-ran");
        let hscript = base.join("hermes.sh");
        write_exe(
            &hscript,
            "#!/bin/sh\ntouch \"$1\"\n",
        );
        let err = crate::policy_guard::with_override(Some(roots), true, || {
            crate::coordinator::run_hermes_reply(&hscript, &[marker.display().to_string()])
        })
        .unwrap_err();
        assert!(
            err.contains("Hermes tools need the write fence (sandbox-exec missing)"),
            "{err}"
        );
        assert!(!marker.exists());
        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn no_raw_mouth_spawn() {
        let watched = body_of(include_str!("mouth_cli.rs"), "pub(crate) fn run_mouth_cli_watched(");
        assert!(!watched.contains("Command::new(bin)"), "{watched}");
        let streamed = body_of(
            include_str!("../companion/mouth_blip.rs"),
            "pub fn run_streamed(",
        );
        assert!(!streamed.contains("Command::new(bin)"));
        let hermes = body_of(include_str!("send.rs"), "pub(crate) fn run_hermes_reply(");
        assert!(!hermes.contains("Command::new(bin)"));
    }
}

