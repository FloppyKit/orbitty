//! Device nsec for local NIP-01 ids. Not the FKT wallet.
//! Never log the secret. Never Spark it. File 0600 in app data.

use secp256k1::{Keypair, Message, Secp256k1, SecretKey, XOnlyPublicKey, schnorr::Signature};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use tauri::Manager;

const KEY_FILE: &str = "device.nsec";
static KEY_LOCK: Mutex<()> = Mutex::new(());

pub struct DeviceKey {
    keypair: Keypair,
}

impl std::fmt::Debug for DeviceKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("DeviceKey")
    }
}

pub struct Signed {
    pub id: String,
    pub pubkey: String,
    pub sig: String,
}

#[derive(Debug, PartialEq, Eq)]
pub enum LineClass {
    /// Empty pubkey and sig — pre-keys uuid-v4 (or unsigned after the toggle).
    Unsigned,
    /// Non-empty pubkey+sig, id matches NIP-01 hash, schnorr checks.
    Signed,
    /// pubkey/sig present but id or sig is wrong. Caller skips the line.
    Corrupt,
}

pub fn signed_locally(app: &tauri::AppHandle) -> bool {
    crate::state::load_state(app)
        .map(|s| s.signed_locally)
        .unwrap_or(true)
}

pub fn key_path(dir: &Path) -> PathBuf {
    dir.join(KEY_FILE)
}

fn app_data_dir(app: &tauri::AppHandle) -> Result<PathBuf, String> {
    let dir = app
        .path()
        .app_data_dir()
        .map_err(|e| format!("app data dir: {e}"))?;
    std::fs::create_dir_all(&dir).map_err(|e| format!("create {}: {e}", dir.display()))?;
    Ok(dir)
}

/// Load the device key, generating once if the file is missing.
/// An unreadable or malformed existing file is an error — never overwrite.
pub fn load_or_create(app: &tauri::AppHandle) -> Result<DeviceKey, String> {
    load_or_create_in(&app_data_dir(app)?)
}

pub fn load_or_create_in(dir: &Path) -> Result<DeviceKey, String> {
    let _guard = KEY_LOCK
        .lock()
        .map_err(|e| format!("device key lock: {e}"))?;
    std::fs::create_dir_all(dir).map_err(|e| format!("create {}: {e}", dir.display()))?;
    let path = key_path(dir);
    if path.exists() {
        return read_key(&path);
    }
    let key = generate_key()?;
    write_key(&path, &key)?;
    Ok(key)
}

pub fn sign_new(
    app: &tauri::AppHandle,
    created_at: u64,
    kind: u64,
    tags: &[[String; 2]],
    content: &str,
) -> Result<Signed, String> {
    let key = load_or_create(app)?;
    sign_with(&key, created_at, kind, tags, content)
}

pub fn sign_with(
    key: &DeviceKey,
    created_at: u64,
    kind: u64,
    tags: &[[String; 2]],
    content: &str,
) -> Result<Signed, String> {
    let pubkey = pubkey_hex(key);
    let id = nip01_id(&pubkey, created_at, kind, tags, content)?;
    let sig = schnorr_sign(key, &id)?;
    Ok(Signed { id, pubkey, sig })
}

pub fn classify_value(v: &Value) -> LineClass {
    let pubkey = v.get("pubkey").and_then(|x| x.as_str()).unwrap_or("").trim();
    let sig = v.get("sig").and_then(|x| x.as_str()).unwrap_or("").trim();
    if pubkey.is_empty() && sig.is_empty() {
        return LineClass::Unsigned;
    }
    if pubkey.is_empty() || sig.is_empty() {
        return LineClass::Corrupt;
    }
    let Some(id) = v.get("id").and_then(|x| x.as_str()) else {
        return LineClass::Corrupt;
    };
    let Some(created_at) = v.get("created_at").and_then(|x| x.as_u64()) else {
        return LineClass::Corrupt;
    };
    let Some(kind) = v.get("kind").and_then(|x| x.as_u64()) else {
        return LineClass::Corrupt;
    };
    let Some(content) = v.get("content").and_then(|x| x.as_str()) else {
        return LineClass::Corrupt;
    };
    let Some(tags) = parse_tags(v.get("tags")) else {
        return LineClass::Corrupt;
    };
    match verify_nip01(id, pubkey, created_at, kind, &tags, content, sig) {
        Ok(()) => LineClass::Signed,
        Err(_) => LineClass::Corrupt,
    }
}

pub fn classify_line(line: &str) -> LineClass {
    match serde_json::from_str::<Value>(line) {
        Ok(v) => classify_value(&v),
        Err(_) => LineClass::Corrupt,
    }
}

pub fn nip01_id(
    pubkey: &str,
    created_at: u64,
    kind: u64,
    tags: &[[String; 2]],
    content: &str,
) -> Result<String, String> {
    let payload = serde_json::json!([0, pubkey, created_at, kind, tags, content]);
    let text = serde_json::to_string(&payload).map_err(|e| format!("nip01 serialize: {e}"))?;
    Ok(hex32(&sha256(text.as_bytes())))
}

pub fn pubkey_hex(key: &DeviceKey) -> String {
    let (xonly, _) = key.keypair.x_only_public_key();
    hex32(&xonly.serialize())
}

fn verify_nip01(
    id: &str,
    pubkey: &str,
    created_at: u64,
    kind: u64,
    tags: &[[String; 2]],
    content: &str,
    sig: &str,
) -> Result<(), String> {
    let want = nip01_id(pubkey, created_at, kind, tags, content)?;
    if !id.eq_ignore_ascii_case(&want) {
        return Err("id mismatch".into());
    }
    let id_bytes = parse_hex32(id).ok_or_else(|| "id hex".to_string())?;
    let pk_bytes = parse_hex32(pubkey).ok_or_else(|| "pubkey hex".to_string())?;
    let sig_bytes = parse_hex64(sig).ok_or_else(|| "sig hex".to_string())?;
    let secp = Secp256k1::verification_only();
    let xonly = XOnlyPublicKey::from_slice(&pk_bytes).map_err(|e| format!("pubkey: {e}"))?;
    let sig = Signature::from_slice(&sig_bytes).map_err(|e| format!("sig: {e}"))?;
    let msg = Message::from_digest(id_bytes);
    secp.verify_schnorr(&sig, &msg, &xonly)
        .map_err(|e| format!("schnorr: {e}"))
}

fn schnorr_sign(key: &DeviceKey, id_hex: &str) -> Result<String, String> {
    let id_bytes = parse_hex32(id_hex).ok_or_else(|| "id hex".to_string())?;
    let msg = Message::from_digest(id_bytes);
    let mut aux = [0u8; 32];
    fill_random(&mut aux);
    let secp = Secp256k1::signing_only();
    let sig = secp.sign_schnorr_with_aux_rand(&msg, &key.keypair, &aux);
    Ok(hex64(&sig.serialize()))
}

fn generate_key() -> Result<DeviceKey, String> {
    let secp = Secp256k1::signing_only();
    for _ in 0..8 {
        let mut buf = [0u8; 32];
        fill_random(&mut buf);
        if let Ok(sk) = SecretKey::from_slice(&buf) {
            let keypair = Keypair::from_secret_key(&secp, &sk);
            buf.fill(0);
            return Ok(DeviceKey { keypair });
        }
        buf.fill(0);
    }
    Err("device key: urandom produced no valid secret".into())
}

fn read_key(path: &Path) -> Result<DeviceKey, String> {
    let raw = std::fs::read_to_string(path).map_err(|_| "device key unreadable".to_string())?;
    let hex = raw.trim();
    let bytes = parse_hex32(hex).ok_or_else(|| "device key malformed".to_string())?;
    let secp = Secp256k1::signing_only();
    let sk = SecretKey::from_slice(&bytes).map_err(|_| "device key malformed".to_string())?;
    Ok(DeviceKey {
        keypair: Keypair::from_secret_key(&secp, &sk),
    })
}

fn write_key(path: &Path, key: &DeviceKey) -> Result<(), String> {
    let secret = key.keypair.secret_bytes();
    let mut text = hex32(&secret);
    text.push('\n');
    let tmp = path.with_extension("nsec.tmp");
    let mut opts = std::fs::OpenOptions::new();
    opts.create(true).write(true).truncate(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        opts.mode(0o600);
    }
    {
        use std::io::Write;
        let mut f = opts
            .open(&tmp)
            .map_err(|e| format!("write device key: {e}"))?;
        f.write_all(text.as_bytes())
            .map_err(|e| format!("write device key: {e}"))?;
    }
    std::fs::rename(&tmp, path).map_err(|e| format!("write device key: {e}"))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600));
    }
    Ok(())
}

fn parse_tags(v: Option<&Value>) -> Option<Vec<[String; 2]>> {
    let arr = v.and_then(|x| x.as_array())?;
    let mut out = Vec::with_capacity(arr.len());
    for tag in arr {
        let pair = tag.as_array()?;
        if pair.len() != 2 {
            return None;
        }
        let a = pair[0].as_str()?.to_string();
        let b = pair[1].as_str()?.to_string();
        out.push([a, b]);
    }
    Some(out)
}

fn sha256(bytes: &[u8]) -> [u8; 32] {
    Sha256::digest(bytes).into()
}

fn hex32(bytes: &[u8; 32]) -> String {
    hex_of(bytes)
}

fn hex64(bytes: &[u8; 64]) -> String {
    hex_of(bytes)
}

fn hex_of(bytes: &[u8]) -> String {
    let mut s = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        s.push_str(&format!("{b:02x}"));
    }
    s
}

fn parse_hex32(s: &str) -> Option<[u8; 32]> {
    let b = parse_hex(s)?;
    b.try_into().ok()
}

fn parse_hex64(s: &str) -> Option<[u8; 64]> {
    let b = parse_hex(s)?;
    b.try_into().ok()
}

fn parse_hex(s: &str) -> Option<Vec<u8>> {
    let s = s.trim();
    if s.len() % 2 != 0 || s.is_empty() {
        return None;
    }
    let mut out = Vec::with_capacity(s.len() / 2);
    let bytes = s.as_bytes();
    for i in (0..bytes.len()).step_by(2) {
        let hi = from_hex(bytes[i])?;
        let lo = from_hex(bytes[i + 1])?;
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

fn fill_random(buf: &mut [u8]) {
    use std::io::Read;
    if let Ok(mut f) = std::fs::File::open("/dev/urandom") {
        if f.read_exact(buf).is_ok() {
            return;
        }
    }
    let mut x = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos() as u64)
        .unwrap_or(1)
        ^ ((std::process::id() as u64) << 32);
    for b in buf.iter_mut() {
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        *b = (x & 0xff) as u8;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmpdir() -> PathBuf {
        let mut p = std::env::temp_dir();
        p.push(format!(
            "orbitty-event-keys-{}-{}",
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

    fn tags() -> Vec<[String; 2]> {
        vec![
            ["t".into(), "orbitty".into()],
            ["d".into(), "c-test".into()],
        ]
    }

    #[test]
    fn sign_roundtrip_is_64_hex_id() {
        let dir = tmpdir();
        let key = load_or_create_in(&dir).unwrap();
        let signed = sign_with(&key, 1_700_000_000, 420, &tags(), r#"{"cardId":"c-test"}"#).unwrap();
        assert_eq!(signed.id.len(), 64, "NIP-01 id is 32-byte hex");
        assert!(signed.id.chars().all(|c| c.is_ascii_hexdigit()));
        assert_eq!(signed.pubkey.len(), 64);
        assert_eq!(signed.sig.len(), 128);
        assert_ne!(signed.id.len(), 32, "must not be uuid-v4 hex");
        let v = serde_json::json!({
            "id": signed.id,
            "pubkey": signed.pubkey,
            "created_at": 1_700_000_000u64,
            "kind": 420,
            "tags": tags(),
            "content": r#"{"cardId":"c-test"}"#,
            "sig": signed.sig,
        });
        assert_eq!(classify_value(&v), LineClass::Signed);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn generate_once_same_pubkey() {
        let dir = tmpdir();
        let a = pubkey_hex(&load_or_create_in(&dir).unwrap());
        let b = pubkey_hex(&load_or_create_in(&dir).unwrap());
        assert_eq!(a, b);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn unsigned_empty_pubkey_sig() {
        let v = serde_json::json!({
            "id": "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
            "pubkey": "",
            "created_at": 1,
            "kind": 420,
            "tags": [["t","orbitty"]],
            "content": "{}",
            "sig": "",
        });
        assert_eq!(classify_value(&v), LineClass::Unsigned);
    }

    #[test]
    fn corrupt_bad_sig_is_skipped() {
        let dir = tmpdir();
        let key = load_or_create_in(&dir).unwrap();
        let mut signed = sign_with(&key, 11, 421, &tags(), "{}").unwrap();
        signed.sig.replace_range(0..2, if &signed.sig[0..2] == "aa" { "bb" } else { "aa" });
        let v = serde_json::json!({
            "id": signed.id,
            "pubkey": signed.pubkey,
            "created_at": 11,
            "kind": 421,
            "tags": tags(),
            "content": "{}",
            "sig": signed.sig,
        });
        assert_eq!(classify_value(&v), LineClass::Corrupt);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn malformed_existing_key_is_not_overwritten() {
        let dir = tmpdir();
        let path = key_path(&dir);
        std::fs::write(&path, "not-a-key\n").unwrap();
        assert!(load_or_create_in(&dir).is_err());
        let kept = std::fs::read_to_string(&path).unwrap();
        assert!(kept.contains("not-a-key"));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn half_signed_is_corrupt() {
        let v = serde_json::json!({
            "id": "ab",
            "pubkey": "aa".repeat(32),
            "created_at": 1,
            "kind": 420,
            "tags": [],
            "content": "{}",
            "sig": "",
        });
        assert_eq!(classify_value(&v), LineClass::Corrupt);
    }
}
