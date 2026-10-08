use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use super::norm::APPROVAL_NOTE;
use super::norm::EXCERPT_BYTES;
use super::norm::EXCERPT_LINES;
use super::norm::EXEC_EXT;
use super::norm::HASH_CAP;
use super::norm::IMAGE_EXT;
use super::norm::RUN_WARNING;
use super::norm::TEXT_EXT;
use super::norm::THUMB_MAX;
use super::norm::file_url_to_path;
use super::norm::normalize_link;

#[derive(Serialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct LinkPreview {
    pub kind: String,
    pub target: String,
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub host: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub size: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub modified: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub modified_secs: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub excerpt: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub warning: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub thumbnail: Option<String>,
    /// sha256 of the executable, script, or bundle main binary. Absent when
    /// the file is over [`HASH_CAP`] or is not an executable.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content_sha256: Option<String>,
    /// sha256 of `Contents/Info.plist` for a bundle.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub plist_sha256: Option<String>,
}

/// What the preview showed. Open / Run refuses when the live target differs.
pub(crate) struct LinkFingerprint {
    pub(crate) kind: String,
    pub(crate) size: Option<u64>,
    pub(crate) modified_secs: Option<u64>,
    pub(crate) content_sha256: Option<String>,
    pub(crate) plist_sha256: Option<String>,
}

impl LinkFingerprint {
    #[cfg_attr(not(test), allow(dead_code))]
    pub(crate) fn from_preview(p: &LinkPreview) -> Self {
        Self {
            kind: p.kind.clone(),
            size: p.size,
            modified_secs: p.modified_secs,
            content_sha256: p.content_sha256.clone(),
            plist_sha256: p.plist_sha256.clone(),
        }
    }
}

#[derive(Serialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct LinkClick {
    pub action: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub preview: Option<LinkPreview>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct LinkApproval {
    pub target: String,
    pub kind: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub size: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub modified_secs: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub content_sha256: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub plist_sha256: Option<String>,
    pub approved_at: String,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct LinkApprovalFile {
    pub note: String,
    pub approvals: Vec<LinkApproval>,
}

impl Default for LinkApprovalFile {
    fn default() -> Self {
        Self {
            note: APPROVAL_NOTE.to_string(),
            approvals: Vec::new(),
        }
    }
}

/// `link-approvals.json` beside `state.json`. Not part of the synced state.
pub fn approvals_path(app: &tauri::AppHandle) -> Result<PathBuf, String> {
    use tauri::Manager;
    let dir = app
        .path()
        .app_data_dir()
        .map_err(|e| format!("app data dir: {e}"))?;
    std::fs::create_dir_all(&dir).map_err(|e| format!("create {}: {e}", dir.display()))?;
    Ok(dir.join("link-approvals.json"))
}

/// One preview for a link. No network. Does not launch anything.
pub fn preview_link(raw: &str) -> Result<LinkPreview, String> {
    let url = normalize_link(raw)?;
    if let Some(path) = file_url_to_path(&url) {
        Ok(preview_path(&path))
    } else {
        let host = url
            .strip_prefix("https://")
            .unwrap_or(&url)
            .split(['/', '?', '#'])
            .next()
            .unwrap_or("")
            .to_string();
        Ok(LinkPreview {
            kind: "https".into(),
            name: host.clone(),
            target: url,
            host: Some(host),
            size: None,
            modified: None,
            modified_secs: None,
            excerpt: None,
            warning: None,
            thumbnail: None,
            content_sha256: None,
            plist_sha256: None,
        })
    }
}

pub fn click_link_at(raw: &str, approvals: &Path) -> Result<LinkClick, String> {
    let preview = preview_link(raw)?;
    if preview.kind != "missing" {
        let file = load_approvals(approvals)?;
        if approval_matches(&preview, &file) {
            launch(&preview)?;
            return Ok(LinkClick {
                action: "opened".into(),
                preview: None,
            });
        }
    }
    Ok(LinkClick {
        action: "preview".into(),
        preview: Some(preview),
    })
}

/// Open / Run against the fingerprint taken just now. Tests use this for
/// "Matt clicked Run on the preview he is looking at."
#[cfg_attr(not(test), allow(dead_code))]
pub fn open_link_at(raw: &str, approvals: &Path) -> Result<LinkClick, String> {
    let preview = preview_link(raw)?;
    let expect = LinkFingerprint::from_preview(&preview);
    open_link_checked(raw, approvals, &expect)
}

/// Open / Run only when `expect` still matches the live target. A mismatch
/// opens nothing and returns the new preview. Only this path records an
/// approval — ctl and MCP never do.
pub(crate) fn open_link_checked(
    raw: &str,
    approvals: &Path,
    expect: &LinkFingerprint,
) -> Result<LinkClick, String> {
    let preview = preview_link(raw)?;
    if preview.kind == "missing" || !fingerprint_eq(&preview, expect) {
        return Ok(LinkClick {
            action: "preview".into(),
            preview: Some(preview),
        });
    }
    launch(&preview)?;
    remember(&preview, approvals)?;
    Ok(LinkClick {
        action: "opened".into(),
        preview: None,
    })
}

fn fingerprint_eq(preview: &LinkPreview, expect: &LinkFingerprint) -> bool {
    preview.kind == expect.kind
        && preview.size == expect.size
        && preview.modified_secs == expect.modified_secs
        && preview.content_sha256 == expect.content_sha256
        && preview.plist_sha256 == expect.plist_sha256
}

/// orbittyctl and MCP `kanban open-link`. Documents, folders, images, and
/// https open. An executable is revealed unless `run` or an exact approval
/// still matches. Reads the approvals list and never writes it.
pub fn ctl_open_at(raw: &str, run: bool, approvals: &Path) -> Result<String, String> {
    let preview = preview_link(raw)?;
    if preview.kind == "missing" {
        return Err("file not found".into());
    }
    let file = load_approvals(approvals)?;
    let do_open = preview.kind != "executable" || run || approval_matches(&preview, &file);
    if do_open {
        launch(&preview)?;
        Ok("open".into())
    } else {
        reveal(&preview)?;
        Ok("reveal".into())
    }
}

pub fn approval_matches(preview: &LinkPreview, file: &LinkApprovalFile) -> bool {
    let Some(row) = file.approvals.iter().find(|a| a.target == preview.target) else {
        return false;
    };
    if preview.kind == "executable" {
        return row.kind == "executable"
            && row.size.is_some()
            && row.size == preview.size
            && row.modified_secs == preview.modified_secs
            && preview.content_sha256.is_some()
            && row.content_sha256 == preview.content_sha256
            && row.plist_sha256 == preview.plist_sha256;
    }
    true
}

pub fn load_approvals(path: &Path) -> Result<LinkApprovalFile, String> {
    match std::fs::read_to_string(path) {
        Ok(text) if text.trim().is_empty() => Ok(LinkApprovalFile::default()),
        Ok(text) => {
            serde_json::from_str(&text).map_err(|e| format!("parse {}: {e}", path.display()))
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(LinkApprovalFile::default()),
        Err(e) => Err(format!("read {}: {e}", path.display())),
    }
}

pub fn remove_approval(path: &Path, target: &str) -> Result<(), String> {
    let mut file = load_approvals(path)?;
    let target = resolve_approval_key(target)?;
    file.approvals.retain(|a| a.target != target);
    save_approvals(path, &file)
}

pub fn clear_approvals(path: &Path) -> Result<(), String> {
    save_approvals(path, &LinkApprovalFile::default())
}

fn resolve_approval_key(raw: &str) -> Result<String, String> {
    let preview = preview_link(raw).unwrap_or(LinkPreview {
        kind: "document".into(),
        target: raw.to_string(),
        name: raw.to_string(),
        host: None,
        size: None,
        modified: None,
        modified_secs: None,
        excerpt: None,
        warning: None,
        thumbnail: None,
        content_sha256: None,
        plist_sha256: None,
    });
    if preview.target.is_empty() {
        Ok(raw.to_string())
    } else {
        Ok(preview.target)
    }
}

fn remember(preview: &LinkPreview, path: &Path) -> Result<(), String> {
    let mut file = load_approvals(path)?;
    file.note = APPROVAL_NOTE.to_string();
    file.approvals.retain(|a| a.target != preview.target);
    file.approvals.push(LinkApproval {
        target: preview.target.clone(),
        kind: preview.kind.clone(),
        size: preview.size,
        modified_secs: preview.modified_secs,
        content_sha256: preview.content_sha256.clone(),
        plist_sha256: preview.plist_sha256.clone(),
        approved_at: chrono::Local::now().format("%Y-%m-%d %H:%M").to_string(),
    });
    save_approvals(path, &file)
}

fn save_approvals(path: &Path, file: &LinkApprovalFile) -> Result<(), String> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).map_err(|e| format!("create {}: {e}", dir.display()))?;
    }
    let text = serde_json::to_string_pretty(file).map_err(|e| format!("serialize: {e}"))?;
    let tmp = path.with_extension("json.tmp");
    std::fs::write(&tmp, text).map_err(|e| format!("write {}: {e}", tmp.display()))?;
    std::fs::rename(&tmp, path).map_err(|e| format!("rename to {}: {e}", path.display()))?;
    Ok(())
}

fn preview_path(path: &Path) -> LinkPreview {
    if !path.exists() {
        return bare_preview("missing", path, None);
    }
    let resolved = path.canonicalize().unwrap_or_else(|_| path.to_path_buf());
    let meta = match std::fs::metadata(&resolved) {
        Ok(m) => m,
        Err(_) => return bare_preview("missing", &resolved, None),
    };
    if resolved.is_dir() {
        if is_bundle_dir(&resolved) {
            return bundle_preview(&resolved);
        }
        let (modified_secs, modified) = modified_pair(&meta);
        return file_preview(
            "folder",
            &resolved,
            Some(meta.len()),
            modified,
            modified_secs,
            None,
            None,
        );
    }
    let (modified_secs, modified) = modified_pair(&meta);
    let size = Some(meta.len());
    if ext_in(&resolved, EXEC_EXT) || has_exec_bit(&meta) {
        let mut preview = file_preview(
            "executable",
            &resolved,
            size,
            modified,
            modified_secs,
            Some(RUN_WARNING.to_string()),
            None,
        );
        preview.content_sha256 = hash_file_capped(&resolved, meta.len());
        return preview;
    }
    if ext_in(&resolved, IMAGE_EXT) {
        let thumb = thumbnail_data_url(&resolved, meta.len());
        return file_preview(
            "image",
            &resolved,
            size,
            modified,
            modified_secs,
            None,
            thumb,
        );
    }
    let excerpt = if ext_in(&resolved, TEXT_EXT) {
        read_excerpt(&resolved)
    } else {
        None
    };
    file_preview(
        "document",
        &resolved,
        size,
        modified,
        modified_secs,
        None,
        excerpt,
    )
}

fn modified_pair(meta: &std::fs::Metadata) -> (Option<u64>, Option<String>) {
    match meta.modified().ok() {
        Some(t) => {
            let secs = t
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_secs())
                .unwrap_or(0);
            let shown = chrono::DateTime::<chrono::Local>::from(t)
                .format("%Y-%m-%d %H:%M")
                .to_string();
            (Some(secs), Some(shown))
        }
        None => (None, None),
    }
}

/// Bundle fingerprint is the main executable plus Info.plist, never the
/// bundle directory's own size or mtime.
fn bundle_preview(bundle: &Path) -> LinkPreview {
    let plist_path = bundle.join("Contents").join("Info.plist");
    let plist_sha256 = hash_plist(&plist_path);
    let exe = bundle_main_executable(bundle, &plist_path);
    let (size, modified, modified_secs, content_sha256) = match exe {
        Some(path) => match std::fs::metadata(&path) {
            Ok(meta) if meta.is_file() => {
                let (modified_secs, modified) = modified_pair(&meta);
                let sha = hash_file_capped(&path, meta.len());
                (Some(meta.len()), modified, modified_secs, sha)
            }
            _ => (None, None, None, None),
        },
        None => (None, None, None, None),
    };
    let mut preview = file_preview(
        "executable",
        bundle,
        size,
        modified,
        modified_secs,
        Some(RUN_WARNING.to_string()),
        None,
    );
    // No main-binary hash, or an Info.plist we could not hash, means the
    // next click always previews.
    preview.content_sha256 = content_sha256;
    preview.plist_sha256 = plist_sha256;
    if plist_path.is_file() && preview.plist_sha256.is_none() {
        preview.content_sha256 = None;
    }
    if preview.content_sha256.is_none() {
        preview.plist_sha256 = None;
    }
    preview
}

fn hash_plist(path: &Path) -> Option<String> {
    let meta = std::fs::metadata(path).ok()?;
    if !meta.is_file() {
        return None;
    }
    hash_file_capped(path, meta.len())
}

fn bundle_main_executable(bundle: &Path, plist_path: &Path) -> Option<PathBuf> {
    let bytes = std::fs::read(plist_path).ok()?;
    let name = cfbundle_executable(&bytes)?;
    if !exe_name_ok(&name) {
        return None;
    }
    Some(bundle.join("Contents").join("MacOS").join(name))
}

fn exe_name_ok(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 255
        && !name.contains('/')
        && !name.contains('\\')
        && !name.contains('\0')
        && name != "."
        && name != ".."
}

pub(crate) fn cfbundle_executable(bytes: &[u8]) -> Option<String> {
    if bytes.starts_with(b"bplist00") {
        return bplist_dict_string(bytes, "CFBundleExecutable");
    }
    xml_cfbundle_executable(bytes)
}

fn xml_cfbundle_executable(bytes: &[u8]) -> Option<String> {
    let text = std::str::from_utf8(bytes).ok()?;
    let key_at = text.find("CFBundleExecutable")?;
    let rest = &text[key_at..];
    let tag = "<string>";
    let start = rest.find(tag)? + tag.len();
    let end = start + rest[start..].find("</string>")?;
    let name = decode_xml_basic(rest[start..end].trim());
    if name.is_empty() {
        None
    } else {
        Some(name)
    }
}

fn decode_xml_basic(s: &str) -> String {
    s.replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&apos;", "'")
}

fn bplist_dict_string(data: &[u8], want: &str) -> Option<String> {
    if data.len() < 40 || &data[..8] != b"bplist00" {
        return None;
    }
    let t = data.len() - 32;
    let offset_size = data[t + 6] as usize;
    let ref_size = data[t + 7] as usize;
    if !(1..=8).contains(&offset_size) || !(1..=8).contains(&ref_size) {
        return None;
    }
    let num_objects = be_int(&data[t + 8..t + 16])?;
    let top = be_int(&data[t + 16..t + 24])?;
    let table_off = be_int(&data[t + 24..t + 32])?;
    if num_objects == 0 || num_objects > 100_000 || top >= num_objects {
        return None;
    }
    let mut offsets = Vec::with_capacity(num_objects);
    for i in 0..num_objects {
        let start = table_off.checked_add(i.checked_mul(offset_size)?)?;
        let end = start.checked_add(offset_size)?;
        if end > data.len() {
            return None;
        }
        offsets.push(be_int(&data[start..end])?);
    }
    let off = *offsets.get(top)?;
    if off >= data.len() || data[off] >> 4 != 0xD {
        return None;
    }
    let mut i = off + 1;
    let n = read_count(data, &mut i, data[off] & 0x0f)?;
    if n > 10_000 {
        return None;
    }
    let mut keys = Vec::with_capacity(n);
    let mut vals = Vec::with_capacity(n);
    for _ in 0..n {
        keys.push(read_ref(data, &mut i, ref_size)?);
    }
    for _ in 0..n {
        vals.push(read_ref(data, &mut i, ref_size)?);
    }
    for (k, v) in keys.into_iter().zip(vals) {
        let koff = *offsets.get(k)?;
        let voff = *offsets.get(v)?;
        let Some(ks) = parse_bplist_string(data, koff) else {
            continue;
        };
        if ks == want {
            return parse_bplist_string(data, voff);
        }
    }
    None
}

fn read_count(data: &[u8], i: &mut usize, low: u8) -> Option<usize> {
    if low != 0x0f {
        return Some(low as usize);
    }
    if *i >= data.len() {
        return None;
    }
    let marker = data[*i];
    *i += 1;
    if marker >> 4 != 0x1 {
        return None;
    }
    let nbytes = 1usize << (marker & 0x0f);
    if nbytes == 0 || nbytes > 8 || *i + nbytes > data.len() {
        return None;
    }
    let n = be_int(&data[*i..*i + nbytes])?;
    *i += nbytes;
    Some(n)
}

fn read_ref(data: &[u8], i: &mut usize, ref_size: usize) -> Option<usize> {
    if *i + ref_size > data.len() {
        return None;
    }
    let n = be_int(&data[*i..*i + ref_size])?;
    *i += ref_size;
    Some(n)
}

fn parse_bplist_string(data: &[u8], off: usize) -> Option<String> {
    if off >= data.len() {
        return None;
    }
    let marker = data[off];
    let mut i = off + 1;
    let n = read_count(data, &mut i, marker & 0x0f)?;
    match marker >> 4 {
        0x5 => {
            if i + n > data.len() {
                return None;
            }
            String::from_utf8(data[i..i + n].to_vec()).ok()
        }
        0x6 => {
            let bytes = n.checked_mul(2)?;
            if i + bytes > data.len() {
                return None;
            }
            let units: Vec<u16> = data[i..i + bytes]
                .chunks_exact(2)
                .map(|c| u16::from_be_bytes([c[0], c[1]]))
                .collect();
            String::from_utf16(&units).ok()
        }
        _ => None,
    }
}

fn be_int(b: &[u8]) -> Option<usize> {
    if b.is_empty() || b.len() > 8 {
        return None;
    }
    let mut n: u64 = 0;
    for byte in b {
        n = n.checked_shl(8)?.checked_add(u64::from(*byte))?;
    }
    usize::try_from(n).ok()
}

fn hash_file_capped(path: &Path, len: u64) -> Option<String> {
    if len > HASH_CAP {
        return None;
    }
    let mut f = std::fs::File::open(path).ok()?;
    let mut hasher = Sha256::new();
    let mut buf = [0u8; 64 * 1024];
    let mut left = len;
    while left > 0 {
        let n = f.read(&mut buf).ok()?;
        if n == 0 {
            break;
        }
        let n = n.min(left as usize);
        hasher.update(&buf[..n]);
        left -= n as u64;
    }
    if left != 0 {
        return None;
    }
    Some(format!("{:x}", hasher.finalize()))
}

fn bare_preview(kind: &str, path: &Path, excerpt: Option<String>) -> LinkPreview {
    let name = path
        .file_name()
        .map(|s| s.to_string_lossy().into_owned())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| path.display().to_string());
    LinkPreview {
        kind: kind.into(),
        target: path.display().to_string(),
        name,
        host: None,
        size: None,
        modified: None,
        modified_secs: None,
        excerpt,
        warning: None,
        thumbnail: None,
        content_sha256: None,
        plist_sha256: None,
    }
}

fn file_preview(
    kind: &str,
    path: &Path,
    size: Option<u64>,
    modified: Option<String>,
    modified_secs: Option<u64>,
    warning: Option<String>,
    extra: Option<String>,
) -> LinkPreview {
    let name = path
        .file_name()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_else(|| path.display().to_string());
    let (excerpt, thumbnail) = if kind == "image" {
        (None, extra)
    } else {
        (extra, None)
    };
    let warning = if kind == "executable" {
        Some(warning.unwrap_or_else(|| RUN_WARNING.to_string()))
    } else {
        warning
    };
    LinkPreview {
        kind: kind.into(),
        target: path.display().to_string(),
        name,
        host: None,
        size,
        modified,
        modified_secs,
        excerpt,
        warning,
        thumbnail,
        content_sha256: None,
        plist_sha256: None,
    }
}

fn is_bundle_dir(path: &Path) -> bool {
    if !path.is_dir() {
        return false;
    }
    if ext_in(path, EXEC_EXT) {
        return true;
    }
    path.join("Contents").join("Info.plist").is_file()
}

fn ext_in(path: &Path, list: &[&str]) -> bool {
    let Some(ext) = path.extension().and_then(|e| e.to_str()) else {
        return false;
    };
    list.iter().any(|e| e.eq_ignore_ascii_case(ext))
}

fn has_exec_bit(meta: &std::fs::Metadata) -> bool {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        meta.permissions().mode() & 0o111 != 0
    }
    #[cfg(not(unix))]
    {
        let _ = meta;
        false
    }
}

fn read_excerpt(path: &Path) -> Option<String> {
    let mut f = std::fs::File::open(path).ok()?;
    let mut buf = vec![0u8; EXCERPT_BYTES];
    let n = f.read(&mut buf).ok()?;
    buf.truncate(n);
    if buf.contains(&0) {
        return None;
    }
    let text = String::from_utf8_lossy(&buf).into_owned();
    let lines: Vec<&str> = text.lines().take(EXCERPT_LINES).collect();
    if lines.is_empty() {
        return None;
    }
    Some(lines.join("\n"))
}

fn thumbnail_data_url(path: &Path, size: u64) -> Option<String> {
    if size == 0 || size > THUMB_MAX {
        return None;
    }
    let mime = match path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_ascii_lowercase()
        .as_str()
    {
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "webp" => "image/webp",
        "bmp" => "image/bmp",
        _ => return None,
    };
    let bytes = std::fs::read(path).ok()?;
    Some(format!("data:{mime};base64,{}", b64(&bytes)))
}

fn b64(data: &[u8]) -> String {
    const T: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(data.len().div_ceil(3) * 4);
    for chunk in data.chunks(3) {
        let mut n = (chunk[0] as u32) << 16;
        if chunk.len() > 1 {
            n |= (chunk[1] as u32) << 8;
        }
        if chunk.len() > 2 {
            n |= chunk[2] as u32;
        }
        out.push(T[((n >> 18) & 63) as usize] as char);
        out.push(T[((n >> 12) & 63) as usize] as char);
        if chunk.len() > 1 {
            out.push(T[((n >> 6) & 63) as usize] as char);
        } else {
            out.push('=');
        }
        if chunk.len() > 2 {
            out.push(T[(n & 63) as usize] as char);
        } else {
            out.push('=');
        }
    }
    out
}

pub(crate) fn launch(preview: &LinkPreview) -> Result<(), String> {
    if preview.kind == "missing"
        || (preview.kind != "https" && !Path::new(&preview.target).exists())
    {
        return Err("file not found".into());
    }
    spawn_open(&[preview.target.as_str()])
}

pub(crate) fn reveal(preview: &LinkPreview) -> Result<(), String> {
    if preview.kind == "https" {
        return Err("reveal is for files".into());
    }
    if preview.kind == "missing" || !Path::new(&preview.target).exists() {
        return Err("file not found".into());
    }
    spawn_open(&["-R", preview.target.as_str()])
}

fn spawn_open(args: &[&str]) -> Result<(), String> {
    #[cfg(test)]
    {
        use super::tests::OPEN_LOG;
        let hooked = OPEN_LOG.with(|log| {
            let mut slot = log.borrow_mut();
            if let Some(v) = slot.as_mut() {
                v.push(args.iter().map(|s| (*s).to_string()).collect());
                true
            } else {
                false
            }
        });
        if hooked {
            return Ok(());
        }
    }
    #[cfg(target_os = "macos")]
    {
        Command::new("open")
            .args(args)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|e| format!("open: {e}"))?;
        Ok(())
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = args;
        Err("open is wired for macOS only".into())
    }
}

