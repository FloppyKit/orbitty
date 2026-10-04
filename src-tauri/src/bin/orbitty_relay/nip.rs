//! NIP-01 verify + NIP-42 AUTH + the t=orbitty allowlist.
//! Never logs the device secret.

use secp256k1::{Keypair, Message, Secp256k1, SecretKey, XOnlyPublicKey, schnorr::Signature};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::path::Path;

pub const KIND_TODO: u64 = 420;
pub const KIND_DOING: u64 = 421;
pub const KIND_DONE: u64 = 422;
pub const KIND_AUTH: u64 = 22242;

pub struct Device {
    keypair: Keypair,
    pub pubkey: String,
}

impl std::fmt::Debug for Device {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Device")
    }
}

/// Unix: group/other bits on a regular file are chmod'd to 0600.
/// A directory is left at its mode. Returns whether it tightened.
fn ensure_private_mode(path: &Path) -> Result<bool, String> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let meta = std::fs::metadata(path).map_err(|_| "device key unreadable".to_string())?;
        if !meta.is_file() {
            return Ok(false);
        }
        if meta.permissions().mode() & 0o077 != 0 {
            std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600)).map_err(|_| {
                "device key: could not tighten permissions to 0600".to_string()
            })?;
            return Ok(true);
        }
        return Ok(false);
    }
    #[cfg(not(unix))]
    {
        let _ = path;
        Ok(false)
    }
}

pub fn load_device(nsec_path: &Path) -> Result<Device, String> {
    if ensure_private_mode(nsec_path)? {
        eprintln!("orbitty-relay: device.nsec permissions tightened to 0600");
    }
    let raw = std::fs::read_to_string(nsec_path).map_err(|_| "device key unreadable".to_string())?;
    let bytes = parse_hex32(raw.trim()).ok_or_else(|| "device key malformed".to_string())?;
    let secp = Secp256k1::signing_only();
    let sk = SecretKey::from_slice(&bytes).map_err(|_| "device key malformed".to_string())?;
    let keypair = Keypair::from_secret_key(&secp, &sk);
    let (xonly, _) = keypair.x_only_public_key();
    Ok(Device {
        pubkey: hex_of(&xonly.serialize()),
        keypair,
    })
}

fn bind_host(bind: &str) -> &str {
    bind.rsplit_once(':')
        .map(|(h, _)| h.trim_matches(|c| c == '[' || c == ']'))
        .unwrap_or(bind)
}

fn bind_is_loopback(bind: &str) -> bool {
    let host = bind_host(bind);
    host == "127.0.0.1" || host == "::1" || host == "localhost"
}

/// Fixed, secret-free refusal when a non-loopback serve has `device.nsec`
/// but `device.npub` is missing, unreadable, or not 64-hex.
const REPLICA_NEEDS_OWNER_NPUB: &str = "refusing replica: device.nsec needs a valid device.npub (64-hex Mac owner key) to prove it is not the Mac key";

/// Refuse a non-loopback serve when `device.nsec` is present unless
/// `device.npub` is a 64-hex owner key that does not match it (a copied
/// Mac key is still refused). A missing, unreadable, directory, empty,
/// or non-64-hex npub (including bech32 `npub1…`) is refused. An
/// npub-only dir and any loopback bind are allowed. No secret in the error.
pub fn replica_refuses_owner_nsec(bind: &str, data_dir: &Path) -> Result<(), String> {
    if bind_is_loopback(bind) {
        return Ok(());
    }
    let nsec = data_dir.join("device.nsec");
    if !nsec.exists() {
        return Ok(());
    }
    let owner = match std::fs::read_to_string(data_dir.join("device.npub")) {
        Ok(s) => s,
        Err(_) => return Err(REPLICA_NEEDS_OWNER_NPUB.into()),
    };
    let owner = owner.trim();
    if parse_hex32(owner).is_none() {
        return Err(REPLICA_NEEDS_OWNER_NPUB.into());
    }
    let device = load_device(&nsec)?;
    if device.pubkey.eq_ignore_ascii_case(owner) {
        return Err(
            "refusing replica: device.nsec matches the Mac owner npub (copied key)".into(),
        );
    }
    Ok(())
}

/// Allowlist pubkey for serve. Prefers `device.nsec` (Mac kitchen, or a
/// replica with its own key). Falls back to `device.npub`. Never logs
/// the secret. A non-loopback serve whose nsec matches `device.npub` is
/// refused by `replica_refuses_owner_nsec` (copied Mac key).
pub fn load_allowlist_pubkey(data_dir: &Path) -> Result<String, String> {
    let nsec = data_dir.join("device.nsec");
    if nsec.exists() {
        return Ok(load_device(&nsec)?.pubkey);
    }
    let npub = data_dir.join("device.npub");
    let raw = std::fs::read_to_string(&npub).map_err(|_| {
        "need device.nsec (Mac) or device.npub (replica — nsec stays on the Mac)".to_string()
    })?;
    let hex = raw.trim();
    if parse_hex32(hex).is_none() {
        return Err("device.npub malformed".into());
    }
    Ok(hex.to_ascii_lowercase())
}

pub fn sign_event(
    device: &Device,
    created_at: u64,
    kind: u64,
    tags: &[Vec<String>],
    content: &str,
) -> Result<Value, String> {
    let id = nip01_id(&device.pubkey, created_at, kind, tags, content)?;
    let sig = schnorr_sign(device, &id)?;
    Ok(json!({
        "id": id,
        "pubkey": device.pubkey,
        "created_at": created_at,
        "kind": kind,
        "tags": tags,
        "content": content,
        "sig": sig,
    }))
}

pub fn verify_event(v: &Value) -> Result<(), String> {
    let id = v.get("id").and_then(Value::as_str).ok_or("id")?;
    let pubkey = v.get("pubkey").and_then(Value::as_str).ok_or("pubkey")?;
    let sig = v.get("sig").and_then(Value::as_str).ok_or("sig")?;
    let created_at = v.get("created_at").and_then(Value::as_u64).ok_or("created_at")?;
    let kind = v.get("kind").and_then(Value::as_u64).ok_or("kind")?;
    let content = v.get("content").and_then(Value::as_str).ok_or("content")?;
    if pubkey.trim().is_empty() || sig.trim().is_empty() {
        return Err("unsigned".into());
    }
    let tags = tags_of(v)?;
    let want = nip01_id(pubkey, created_at, kind, &tags, content)?;
    if !id.eq_ignore_ascii_case(&want) {
        return Err("id mismatch".into());
    }
    let id_bytes = parse_hex32(id).ok_or("id hex")?;
    let pk_bytes = parse_hex32(pubkey).ok_or("pubkey hex")?;
    let sig_bytes = parse_hex64(sig).ok_or("sig hex")?;
    let secp = Secp256k1::verification_only();
    let xonly = XOnlyPublicKey::from_slice(&pk_bytes).map_err(|e| e.to_string())?;
    let sig = Signature::from_slice(&sig_bytes).map_err(|e| e.to_string())?;
    secp.verify_schnorr(&sig, &Message::from_digest(id_bytes), &xonly)
        .map_err(|e| e.to_string())
}

/// Allowlist for stored board proofs. AUTH 22242 is checked separately.
pub fn allow_store(v: &Value, device_pubkey: &str) -> Result<(), String> {
    verify_event(v)?;
    let kind = v.get("kind").and_then(Value::as_u64).unwrap_or(0);
    if kind != KIND_TODO && kind != KIND_DOING && kind != KIND_DONE {
        return Err("blocked: kind not 420/421/422".into());
    }
    let pubkey = v.get("pubkey").and_then(Value::as_str).unwrap_or("");
    if !pubkey.eq_ignore_ascii_case(device_pubkey) {
        return Err("restricted: not the device pubkey".into());
    }
    let t_tags = tag_values(v, "t");
    if t_tags.iter().any(|t| t == "nch" || t == "publishable" || t == "local-only") {
        return Err("blocked: t=orbitty required".into());
    }
    if !t_tags.iter().any(|t| t == "orbitty") {
        return Err("blocked: t=orbitty required".into());
    }
    Ok(())
}

pub fn auth_ok(v: &Value, device_pubkey: &str, challenge: &str, relay_url: &str) -> Result<(), String> {
    verify_event(v)?;
    let kind = v.get("kind").and_then(Value::as_u64).unwrap_or(0);
    if kind != KIND_AUTH {
        return Err("auth-required: kind 22242".into());
    }
    let pubkey = v.get("pubkey").and_then(Value::as_str).unwrap_or("");
    if !pubkey.eq_ignore_ascii_case(device_pubkey) {
        return Err("restricted: not the device pubkey".into());
    }
    let challenges = tag_values(v, "challenge");
    if !challenges.iter().any(|c| c == challenge) {
        return Err("auth-required: bad challenge".into());
    }
    let relays = tag_values(v, "relay");
    // Accept the live relay URL, or the classic loopback form (SSH -L
    // keeps AUTH tags on 127.0.0.1 even when the process is remote).
    if !relays.iter().any(|r| {
        r == relay_url
            || r.contains("127.0.0.1:13120")
            || (relay_url.contains("127.0.0.1") && r.contains("127.0.0.1"))
    }) {
        return Err("auth-required: bad relay tag".into());
    }
    Ok(())
}

pub fn event_matches(ev: &Value, filter: &Value) -> bool {
    if let Some(ids) = filter.get("ids").and_then(Value::as_array) {
        let id = ev.get("id").and_then(Value::as_str).unwrap_or("");
        if !ids.iter().any(|x| x.as_str() == Some(id)) {
            return false;
        }
    }
    if let Some(authors) = filter.get("authors").and_then(Value::as_array) {
        let pk = ev.get("pubkey").and_then(Value::as_str).unwrap_or("");
        if !authors.iter().any(|x| x.as_str().is_some_and(|a| a.eq_ignore_ascii_case(pk))) {
            return false;
        }
    }
    if let Some(kinds) = filter.get("kinds").and_then(Value::as_array) {
        let kind = ev.get("kind").and_then(Value::as_u64);
        if !kinds.iter().any(|x| x.as_u64() == kind) {
            return false;
        }
    }
    if let Some(since) = filter.get("since").and_then(Value::as_u64) {
        if ev.get("created_at").and_then(Value::as_u64).unwrap_or(0) < since {
            return false;
        }
    }
    if let Some(until) = filter.get("until").and_then(Value::as_u64) {
        if ev.get("created_at").and_then(Value::as_u64).unwrap_or(0) > until {
            return false;
        }
    }
    if let Some(obj) = filter.as_object() {
        for (k, vals) in obj {
            if let Some(tag) = k.strip_prefix('#') {
                if tag.len() != 1 {
                    continue;
                }
                let Some(want) = vals.as_array() else { continue };
                let have = tag_values(ev, tag);
                if !want.iter().any(|w| w.as_str().is_some_and(|s| have.iter().any(|h| h == s))) {
                    return false;
                }
            }
        }
    }
    true
}

pub fn tag_values(v: &Value, name: &str) -> Vec<String> {
    let mut out = Vec::new();
    let Some(tags) = v.get("tags").and_then(Value::as_array) else {
        return out;
    };
    for t in tags {
        let Some(arr) = t.as_array() else { continue };
        if arr.first().and_then(Value::as_str) == Some(name) {
            if let Some(val) = arr.get(1).and_then(Value::as_str) {
                out.push(val.to_string());
            }
        }
    }
    out
}

pub fn unix_now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

pub fn fill_random(buf: &mut [u8]) {
    use std::io::Read;
    if let Ok(mut f) = std::fs::File::open("/dev/urandom") {
        if f.read_exact(buf).is_ok() {
            return;
        }
    }
    let mut x = unix_now() ^ (std::process::id() as u64);
    for b in buf.iter_mut() {
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        *b = (x & 0xff) as u8;
    }
}

fn nip01_id(
    pubkey: &str,
    created_at: u64,
    kind: u64,
    tags: &[Vec<String>],
    content: &str,
) -> Result<String, String> {
    let payload = json!([0, pubkey, created_at, kind, tags, content]);
    let text = serde_json::to_string(&payload).map_err(|e| e.to_string())?;
    Ok(hex_of(&sha256(text.as_bytes())))
}

fn tags_of(v: &Value) -> Result<Vec<Vec<String>>, String> {
    let arr = v.get("tags").and_then(Value::as_array).ok_or("tags")?;
    let mut out = Vec::with_capacity(arr.len());
    for t in arr {
        let pair = t.as_array().ok_or("tag")?;
        let mut row = Vec::new();
        for x in pair {
            row.push(x.as_str().ok_or("tag str")?.to_string());
        }
        out.push(row);
    }
    Ok(out)
}

fn schnorr_sign(device: &Device, id_hex: &str) -> Result<String, String> {
    let id_bytes = parse_hex32(id_hex).ok_or("id hex")?;
    let mut aux = [0u8; 32];
    fill_random(&mut aux);
    let secp = Secp256k1::signing_only();
    let sig = secp.sign_schnorr_with_aux_rand(&Message::from_digest(id_bytes), &device.keypair, &aux);
    Ok(hex_of(&sig.serialize()))
}

fn sha256(bytes: &[u8]) -> [u8; 32] {
    Sha256::digest(bytes).into()
}

fn hex_of(bytes: &[u8]) -> String {
    let mut s = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        s.push_str(&format!("{b:02x}"));
    }
    s
}

fn parse_hex32(s: &str) -> Option<[u8; 32]> {
    parse_hex(s)?.try_into().ok()
}

fn parse_hex64(s: &str) -> Option<[u8; 64]> {
    parse_hex(s)?.try_into().ok()
}

fn parse_hex(s: &str) -> Option<Vec<u8>> {
    let s = s.trim();
    if s.len() % 2 != 0 || s.is_empty() {
        return None;
    }
    let mut out = Vec::with_capacity(s.len() / 2);
    let b = s.as_bytes();
    for i in (0..b.len()).step_by(2) {
        let hi = from_hex(b[i])?;
        let lo = from_hex(b[i + 1])?;
        out.push((hi << 4) | lo);
    }
    Some(out)
}

fn from_hex(b: u8) -> Option<u8> {
    match b {
        b'0'..=b'9' => Some(b - b'0'),
        b'a'..=b'f' => Some(b - b'a' + 10),
        b'A'..=b'F' => Some(b - b'A' + 10),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fake_event(kind: u64, t: &str) -> Value {
        json!({
            "id": "aa",
            "pubkey": "bb",
            "created_at": 1,
            "kind": kind,
            "tags": [["t", t], ["d", "c-1"]],
            "content": "{}",
            "sig": "cc",
        })
    }

    #[test]
    fn filter_d_tag() {
        let ev = fake_event(420, "orbitty");
        assert!(event_matches(&ev, &json!({ "#d": ["c-1"], "kinds": [420] })));
        assert!(!event_matches(&ev, &json!({ "#d": ["nope"] })));
        assert!(!event_matches(&ev, &json!({ "kinds": [31200] })));
    }

    #[test]
    fn npub_file_is_enough_without_nsec() {
        let dir = std::env::temp_dir().join(format!("orbitty-relay-npub-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let pk = "ab".repeat(32);
        std::fs::write(dir.join("device.npub"), &pk).unwrap();
        assert_eq!(load_allowlist_pubkey(&dir).unwrap(), pk);
        let _ = std::fs::remove_dir_all(&dir);
    }

    fn scratch() -> std::path::PathBuf {
        let p = std::env::temp_dir().join(format!(
            "orbitty-relay-key-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or(0)
        ));
        let _ = std::fs::remove_dir_all(&p);
        std::fs::create_dir_all(&p).unwrap();
        p
    }

    fn mint_key() -> (String, String) {
        let secp = Secp256k1::signing_only();
        for _ in 0..8 {
            let mut buf = [0u8; 32];
            super::fill_random(&mut buf);
            if let Ok(sk) = SecretKey::from_slice(&buf) {
                let kp = Keypair::from_secret_key(&secp, &sk);
                let (xonly, _) = kp.x_only_public_key();
                let sec = super::hex_of(&buf);
                let pk = super::hex_of(&xonly.serialize());
                buf.fill(0);
                return (sec, pk);
            }
            buf.fill(0);
        }
        panic!("urandom produced no valid secret");
    }

    fn write_mode(path: &std::path::Path, body: &str, mode: u32) {
        std::fs::write(path, body).unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(path, std::fs::Permissions::from_mode(mode)).unwrap();
        }
        let _ = mode;
    }

    #[cfg(unix)]
    fn unix_mode(path: &std::path::Path) -> u32 {
        use std::os::unix::fs::PermissionsExt;
        std::fs::metadata(path).unwrap().permissions().mode() & 0o777
    }

    #[test]
    fn loose_nsec_is_tightened_to_0600_and_debug_hides_secret() {
        let dir = scratch();
        let (sec, pk) = mint_key();
        let path = dir.join("device.nsec");
        write_mode(&path, &format!("{sec}\n"), 0o644);
        let dev = load_device(&path).unwrap();
        assert_eq!(dev.pubkey, pk);
        #[cfg(unix)]
        assert_eq!(unix_mode(&path), 0o600, "0644 device.nsec must tighten to 0600");
        let dbg = format!("{dev:?}");
        assert_eq!(dbg, "Device");
        assert!(!dbg.contains(&sec), "Debug must not contain the secret hex");
        std::fs::write(&path, format!("{sec}zz\n")).unwrap();
        let err = load_device(&path).unwrap_err();
        assert!(!err.contains(&sec), "error must not contain the secret hex");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn replica_copied_mac_nsec_refused_own_key_and_loopback_ok() {
        let dir = scratch();
        let (sec, pk) = mint_key();
        let (other_sec, other_pk) = mint_key();
        assert_ne!(pk, other_pk);
        write_mode(&dir.join("device.nsec"), &format!("{sec}\n"), 0o600);
        std::fs::write(dir.join("device.npub"), &pk).unwrap();

        let err = replica_refuses_owner_nsec("100.64.0.1:13120", &dir).unwrap_err();
        assert_eq!(
            err,
            "refusing replica: device.nsec matches the Mac owner npub (copied key)"
        );
        assert!(!err.contains(&sec), "refusal must not contain the secret hex");

        std::fs::write(dir.join("device.npub"), pk.to_uppercase()).unwrap();
        assert!(replica_refuses_owner_nsec("100.70.23.74:13120", &dir).is_err());

        write_mode(&dir.join("device.nsec"), &format!("{other_sec}\n"), 0o600);
        std::fs::write(dir.join("device.npub"), &pk).unwrap();
        assert!(replica_refuses_owner_nsec("100.64.0.1:13120", &dir).is_ok());

        std::fs::remove_file(dir.join("device.nsec")).unwrap();
        assert!(replica_refuses_owner_nsec("100.127.1.2:13120", &dir).is_ok());

        write_mode(&dir.join("device.nsec"), &format!("{sec}\n"), 0o600);
        std::fs::write(dir.join("device.npub"), &pk).unwrap();
        assert!(replica_refuses_owner_nsec("127.0.0.1:13120", &dir).is_ok());
        assert!(replica_refuses_owner_nsec("[::1]:13120", &dir).is_ok());
        assert!(replica_refuses_owner_nsec("localhost:13120", &dir).is_ok());
        let _ = std::fs::remove_dir_all(&dir);
    }

    fn assert_needs_owner_npub(err: String, sec: &str, leaked: &str) {
        assert_eq!(err, super::REPLICA_NEEDS_OWNER_NPUB);
        assert!(!err.contains(sec), "refusal must not contain the secret hex");
        if !leaked.is_empty() {
            assert!(!err.contains(leaked), "refusal must not echo the npub file");
        }
    }

    #[test]
    fn replica_nsec_without_valid_owner_npub_refused() {
        let dir = scratch();
        let (sec, pk) = mint_key();
        let (other_sec, other_pk) = mint_key();
        assert_ne!(pk, other_pk);
        let nsec = dir.join("device.nsec");
        let npub = dir.join("device.npub");
        write_mode(&nsec, &format!("{sec}\n"), 0o600);
        let bind = "100.64.0.1:13120";

        std::fs::remove_file(&npub).ok();
        assert_needs_owner_npub(
            replica_refuses_owner_nsec(bind, &dir).unwrap_err(),
            &sec,
            "",
        );

        let body = format!("not-hex-{other_pk}");
        std::fs::write(&npub, &body).unwrap();
        assert_needs_owner_npub(
            replica_refuses_owner_nsec(bind, &dir).unwrap_err(),
            &sec,
            &body,
        );

        std::fs::write(&npub, "\n").unwrap();
        assert_needs_owner_npub(
            replica_refuses_owner_nsec(bind, &dir).unwrap_err(),
            &sec,
            "",
        );

        let bech = "npub1qqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqq";
        std::fs::write(&npub, bech).unwrap();
        assert_needs_owner_npub(
            replica_refuses_owner_nsec(bind, &dir).unwrap_err(),
            &sec,
            bech,
        );

        std::fs::remove_file(&npub).unwrap();
        std::fs::create_dir(&npub).unwrap();
        assert_needs_owner_npub(
            replica_refuses_owner_nsec(bind, &dir).unwrap_err(),
            &sec,
            "",
        );
        std::fs::remove_dir(&npub).unwrap();

        std::fs::write(&npub, &other_pk).unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&npub, std::fs::Permissions::from_mode(0o000)).unwrap();
        }
        assert_needs_owner_npub(
            replica_refuses_owner_nsec(bind, &dir).unwrap_err(),
            &sec,
            &other_pk,
        );
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&npub, std::fs::Permissions::from_mode(0o644)).unwrap();
        }

        write_mode(&nsec, &format!("{other_sec}\n"), 0o600);
        std::fs::write(&npub, &pk).unwrap();
        assert!(replica_refuses_owner_nsec(bind, &dir).is_ok());

        std::fs::remove_file(&nsec).unwrap();
        assert!(replica_refuses_owner_nsec(bind, &dir).is_ok());

        write_mode(&nsec, &format!("{sec}\n"), 0o600);
        std::fs::remove_file(&npub).unwrap();
        assert!(replica_refuses_owner_nsec("127.0.0.1:13120", &dir).is_ok());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn group_readable_0640_nsec_is_tightened_to_0600() {
        let dir = scratch();
        let (sec, pk) = mint_key();
        let path = dir.join("device.nsec");
        write_mode(&path, &format!("{sec}\n"), 0o640);
        let dev = load_device(&path).unwrap();
        assert_eq!(dev.pubkey, pk);
        #[cfg(unix)]
        assert_eq!(unix_mode(&path), 0o600, "0640 device.nsec must tighten to 0600");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn directory_key_path_keeps_mode_and_is_unreadable() {
        let dir = scratch();
        let path = dir.join("device.nsec");
        std::fs::create_dir(&path).unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).unwrap();
        }
        let err = load_device(&path).unwrap_err();
        assert_eq!(err, "device key unreadable");
        #[cfg(unix)]
        assert_eq!(unix_mode(&path), 0o755, "a directory at the key path is not chmod'd");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
