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

pub fn load_device(nsec_path: &Path) -> Result<Device, String> {
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
    if !relays.iter().any(|r| r.contains("127.0.0.1:13120") || r == relay_url) {
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
}
