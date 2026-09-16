//! User sprite packs: local folder/zip/image
//! import, pack listing for the Gear → Packs UI, and frame sources for the
//! habitat. Law: LIVESPRITE_SPRITE_PACKS_LOCK_v0.md — missing/unclear
//! license warns but never blocks, user packs stay on the user's machine.
//! The website-download path (itch/Kenney directory) is
//! gone — a sprite exists by generation (chars.rs) or by local import,
//! never by fetching from a website.
//!
//! Layout:
//! ```text
//! packs/<slug>/
//!   manifest.json   optional { name, license, frames?: ["a.png"],
//!                              sheet?: {file, frameW, frameH, count} }
//!   *.png
//! ```

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use tauri::Manager;

const README: &str = "Orbitty — sprite packs
==========================

Your imported sprite packs live here. One subfolder per pack:

    packs/
      my-pack/
        manifest.json   optional, e.g. { \"name\": \"My Pack\", \"license\": \"CC0\" }
        *.png           sprite sheets / frames

Manage packs from the rail: gear (⚙) → Packs → Import folder / Import zip,
or the Being editor's Use my image on any walker. Without a picked pack,
each walker is one of the built-in Season 1 bitty crew — no download needed.

Rules (locked in LIVESPRITE_SPRITE_PACKS_LOCK_v0.md):

  * Local import only: pick an image, a folder, or a zip you downloaded.
    Orbitty never fetches sprite art from a website.
  * Only add packs you have the right to use (CC0 or your own art).
    Missing or unclear license: the app warns, it does not block.
  * Your packs stay on your machine. Orbitty does not host or
    redistribute them.

App data: ~/Library/Application Support/space.orbitty.mac/packs/
";

// manifest

#[derive(Deserialize, Default, Clone, Debug)]
#[serde(rename_all = "camelCase", default)]
pub struct SheetSpec {
    pub file: String,
    pub frame_w: u32,
    pub frame_h: u32,
    pub count: u32,
}

#[derive(Deserialize, Default, Clone, Debug)]
#[serde(rename_all = "camelCase", default)]
struct Manifest {
    name: String,
    license: String,
    frames: Vec<String>,
    /// Optional idle cycle (catalog installs curate one when the pack has
    /// idle-named art next to the walk cycle).
    idle_frames: Vec<String>,
    /// Optional bust set (head+neck+collar)
    /// next to idle_* — the companion's Head view. Never derived from the
    /// idle frames; absent = the pack has no Head art.
    bust_frames: Vec<String>,
    sheet: Option<SheetSpec>,
    /// "habitat" packs (tiles/buildings) install but stay out of the Sprite
    /// picker and the habitat's walker art (ORBITTY_PLANET_TOWN).
    role: String,
    /// Companion bodies: playback cadence. 0 = engine default (140 ms).
    #[serde(default)]
    frame_ms: u32,
    /// Companion bodies: skip nearest-neighbor (3D bakes aren't 32px bitties).
    #[serde(default)]
    smooth: bool,
}

fn read_manifest(dir: &Path) -> Manifest {
    let Ok(text) = std::fs::read_to_string(dir.join("manifest.json")) else {
        return Manifest::default();
    };
    serde_json::from_str(&text).unwrap_or_default()
}

/// One art source: `Frames` are individual PNGs; a `Sheet` is sliced
/// row-major into `count` frames of frameW×frameH.
#[derive(Clone, Debug)]
pub enum ArtSource {
    Frames(Vec<PathBuf>),
    Sheet { file: PathBuf, spec: SheetSpec },
}

impl ArtSource {
    pub fn frame_count(&self) -> usize {
        match self {
            ArtSource::Frames(v) => v.len(),
            ArtSource::Sheet { spec, .. } => spec.count as usize,
        }
    }
}

/// What the habitat draws for one pack: the walk cycle, plus an optional
/// idle cycle for paused walkers (ORBITTY_PLANET_PLACE_GROW).
#[derive(Clone, Debug)]
pub struct PackArt {
    pub walk: ArtSource,
    pub idle: Option<ArtSource>,
    pub frame_ms: u32,
    pub smooth: bool,
}

/// Path-component needles that never make walker art (ORBITTY_PLANET_GROW):
/// tilesets, atlases, and menu/title screens — the SunnyLand glitch was a
/// tileset squashed into a walker frame. `sunnyland`/`sunny-land`/`phaser`
/// kill the whole-pack offenders at any depth (ORBITTY_PLANET_TOWN).
const SKIP_PARTS: [&str; 11] = [
    "atlas",
    "tileset",
    "environment",
    "collision",
    "title-screen",
    "press-enter",
    "credits",
    "loading",
    "sunnyland",
    "sunny-land",
    "phaser",
];

fn path_skipped(dir: &Path, p: &Path) -> bool {
    p.strip_prefix(dir)
        .unwrap_or(p)
        .components()
        .filter_map(|c| c.as_os_str().to_str().map(|s| s.to_lowercase()))
        .any(|part| SKIP_PARTS.iter().any(|k| part.contains(k)))
}

/// Pixel dimensions without a decoder: PNG IHDR, GIF logical-screen
/// descriptor, WebP VP8/VP8L/VP8X chunk headers. Anything unreadable is
/// None — such a file is never treated as a strip.
fn image_size(p: &Path) -> Option<(u32, u32)> {
    let bytes = std::fs::read(p).ok()?;
    let b = bytes.as_slice();
    let be32 = |o: usize| u32::from_be_bytes([b[o], b[o + 1], b[o + 2], b[o + 3]]);
    let le16 = |o: usize| u32::from(b[o + 1]) << 8 | u32::from(b[o]);
    if b.len() >= 24 && b.starts_with(b"\x89PNG\r\n\x1a\n") {
        return Some((be32(16), be32(20)));
    }
    if b.len() >= 10 && (b.starts_with(b"GIF87a") || b.starts_with(b"GIF89a")) {
        return Some((le16(6), le16(8)));
    }
    if b.len() >= 30 && b.starts_with(b"RIFF") && &b[8..12] == b"WEBP" {
        let fourcc = &b[12..16];
        if fourcc == b"VP8X" {
            // 24-bit minus-one canvas size at bytes 24..30.
            let w = 1 + u32::from(b[24]) + (u32::from(b[25]) << 8) + (u32::from(b[26]) << 16);
            let h = 1 + u32::from(b[27]) + (u32::from(b[28]) << 8) + (u32::from(b[29]) << 16);
            return Some((w, h));
        }
        if fourcc == b"VP8 " && b.len() >= 30 {
            // Lossy bitstream: frame tag (3) + start code (3) then u16s.
            let w = u32::from(b[26]) | (u32::from(b[27]) & 0x3f) << 8;
            let h = u32::from(b[28]) | (u32::from(b[29]) & 0x3f) << 8;
            return Some((w, h));
        }
        if fourcc == b"VP8L" && b.len() >= 25 {
            // Lossless: 1-byte signature then 14-bit packed width/height.
            let bits = u32::from(b[21])
                | u32::from(b[22]) << 8
                | u32::from(b[23]) << 16
                | u32::from(b[24]) << 24;
            return Some(((bits & 0x3fff) + 1, ((bits >> 14) & 0x3fff) + 1));
        }
    }
    None
}

/// Horizontal strip of square frames (the classic itch layout — Hero
/// Knight's Run.png is 1440×180 = 8×180): width an exact multiple of
/// height, at least 2 columns, and a sane sprite-cell height.
fn strip_spec(p: &Path) -> Option<SheetSpec> {
    let (w, h) = image_size(p)?;
    if h >= 16 && h <= 256 && w % h == 0 && w / h >= 2 {
        Some(SheetSpec {
            file: p.file_name()?.to_str()?.to_string(),
            frame_w: h,
            frame_h: h,
            count: w / h,
        })
    } else {
        None
    }
}

/// Walk/run-named art beats idle-named, which beats everything else.
fn cycle_rank(p: &Path) -> u8 {
    let name = p
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("")
        .to_lowercase();
    if name.contains("walk") || name.contains("run") {
        0
    } else if name.contains("idle") {
        1
    } else {
        2
    }
}

/// Character key for multi-character packs (ORBITTY_PLANET_TOWN): filename
/// tokens minus animation words, trailing digits, and single letters, so
/// `character_robot_walk0.png` and `character_robot_idle.png` share the key
/// `character-robot`. Empty key = the pack names no distinct characters.
fn character_key(p: &Path) -> String {
    let stem = p
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("")
        .to_lowercase();
    let mut toks = Vec::new();
    for tok in stem.split(|c: char| !c.is_ascii_alphanumeric()) {
        let tok = tok.trim_end_matches(|c: char| c.is_ascii_digit());
        if tok.is_empty() || matches!(tok, "walk" | "run" | "idle") {
            continue;
        }
        // `walk_a` / `walk_1a` style suffixes: drop single letters.
        if tok.chars().count() == 1 && tok.chars().next().is_some_and(|c| c.is_ascii_alphabetic()) {
            continue;
        }
        toks.push(tok);
    }
    toks.join("-")
}

/// Largest single-character group out of a rank-sorted image set, so a
/// multi-character pack (Kenney toon/platformer ship 4–6) becomes ONE
/// coherent walk cycle instead of a morphing slideshow. Deterministic:
/// biggest group, ties broken by key. Empty keys (no character in the
/// names) keep the whole set — the pre-TOWN behavior.
fn best_character_group(group: &[PathBuf]) -> Vec<PathBuf> {
    let mut by_key: std::collections::BTreeMap<String, Vec<PathBuf>> =
        std::collections::BTreeMap::new();
    for p in group {
        by_key.entry(character_key(p)).or_default().push(p.clone());
    }
    if by_key.len() <= 1 {
        return group.to_vec();
    }
    by_key
        .into_iter()
        .max_by(|(ka, a), (kb, b)| a.len().cmp(&b.len()).then_with(|| kb.cmp(ka)))
        .map(|(_, v)| v)
        .unwrap_or_else(|| group.to_vec())
}

/// One rank group → art source: a strip first image becomes a Sheet (never
/// one squashed frame); otherwise the group is individual frames (≤8).
fn group_source(dir: &Path, group: &[PathBuf]) -> Option<ArtSource> {
    let first = group.first()?;
    if let Some(mut spec) = strip_spec(first) {
        spec.file = first
            .strip_prefix(dir)
            .unwrap_or(first)
            .to_string_lossy()
            .into_owned();
        return Some(ArtSource::Sheet {
            file: first.clone(),
            spec,
        });
    }
    let mut frames = group.to_vec();
    frames.truncate(8);
    Some(ArtSource::Frames(frames))
}

/// Resolve a pack dir to its art source: explicit manifest frames first,
/// then a manifest sheet, then a recursive scan for images (itch packs like
/// Hero Knight nest theirs at `Sprites/*.png`). The scan skips tileset /
/// atlas / menu art, wants a run/walk (then idle) cycle, and auto-slices
/// horizontal strips. A pack with no walk-cycle art after filters resolves
/// to None — the picker shows it disabled (frameCount 0).
fn art_for(dir: &Path, manifest: &Manifest) -> Option<PackArt> {
    if manifest.role.eq_ignore_ascii_case("habitat") {
        return None;
    }
    let frames: Vec<PathBuf> = manifest
        .frames
        .iter()
        .map(|f| dir.join(f))
        .filter(|p| p.is_file())
        .collect();
    if !frames.is_empty() {
        let idle: Vec<PathBuf> = manifest
            .idle_frames
            .iter()
            .map(|f| dir.join(f))
            .filter(|p| p.is_file())
            .collect();
        return Some(PackArt {
            walk: ArtSource::Frames(frames),
            idle: if idle.is_empty() {
                None
            } else {
                Some(ArtSource::Frames(idle))
            },
            frame_ms: manifest.frame_ms,
            smooth: manifest.smooth,
        });
    }
    if let Some(sheet) = &manifest.sheet {
        let file = dir.join(&sheet.file);
        if sheet.frame_w > 0 && sheet.frame_h > 0 && sheet.count > 0 && file.is_file() {
            return Some(PackArt {
                walk: ArtSource::Sheet {
                    file,
                    spec: sheet.clone(),
                },
                idle: None,
                frame_ms: manifest.frame_ms,
                smooth: manifest.smooth,
            });
        }
    }
    let mut imgs = Vec::new();
    collect_images(dir, 0, &mut imgs);
    imgs.retain(|p| !path_skipped(dir, p));
    imgs.sort_by(|a, b| (cycle_rank(a), a.as_os_str()).cmp(&(cycle_rank(b), b.as_os_str())));
    let best = cycle_rank(imgs.first()?);
    if best == 2 {
        // A lone PNG (meme, portrait, "Trollface.png") is a still walker.
        // Do not require walk/run/idle in the filename. One image only —
        // a folder of random unnamed PNGs should not morph into a cycle.
        let one = imgs.into_iter().take(1).collect::<Vec<_>>();
        let walk = group_source(dir, &one)?;
        return Some(PackArt {
            walk,
            idle: None,
            frame_ms: 0,
            smooth: false,
        });
    }
    let group: Vec<PathBuf> = imgs
        .iter()
        .filter(|p| cycle_rank(p) == best)
        .cloned()
        .collect();
    // Multi-character packs: one character's cycle, never a morphing mix.
    let group = best_character_group(&group);
    let walk = group_source(dir, &group)?;
    // Idle-only packs already ARE the idle cycle; when run/walk won, an
    // idle group (if present) becomes the paused-walker art — same
    // character when the pack names them.
    let idle = if best == 0 {
        let key = character_key(group.first()?);
        let idle_group: Vec<PathBuf> = imgs
            .iter()
            .filter(|p| cycle_rank(p) == 1)
            .filter(|p| key.is_empty() || character_key(p) == key)
            .cloned()
            .collect();
        let idle_group = if idle_group.is_empty() && !key.is_empty() {
            imgs.iter()
                .filter(|p| cycle_rank(p) == 1)
                .cloned()
                .collect()
        } else {
            idle_group
        };
        group_source(dir, &idle_group)
    } else {
        None
    };
    Some(PackArt {
        walk,
        idle,
        frame_ms: 0,
        smooth: false,
    })
}

/// png/gif/webp/jpg — NSImage decodes all of these from bytes.
fn is_frame_image(p: &Path) -> bool {
    p.extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| matches!(e.to_ascii_lowercase().as_str(), "png" | "gif" | "webp" | "jpg" | "jpeg"))
}

/// Recurse depth ≤4; resource-fork cruft and tooling dirs never count.
fn collect_images(dir: &Path, depth: usize, out: &mut Vec<PathBuf>) {
    if depth > 4 {
        return;
    }
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.filter_map(|e| e.ok()) {
        let p = entry.path();
        if p.is_dir() {
            let skip = p
                .file_name()
                .and_then(|n| n.to_str())
                .is_some_and(|n| matches!(n, "__MACOSX" | ".idea" | "node_modules"));
            if !skip {
                collect_images(&p, depth + 1, out);
            }
        } else if is_frame_image(&p) {
            out.push(p);
        }
    }
}

// paths

fn packs_dir_path(app: &tauri::AppHandle) -> Result<PathBuf, String> {
    let dir = app
        .path()
        .app_data_dir()
        .map_err(|e| format!("app data dir: {e}"))?;
    let packs = dir.join("packs");
    std::fs::create_dir_all(&packs).map_err(|e| format!("create {}: {e}", packs.display()))?;
    Ok(packs)
}

/// Folder/zip name → slug: lowercase, non-alphanumeric runs become one dash.
fn slugify(name: &str) -> String {
    let mut out = String::with_capacity(name.len());
    let mut dash = false;
    for c in name.chars().flat_map(|c| c.to_lowercase()) {
        if c.is_ascii_alphanumeric() {
            out.push(c);
            dash = false;
        } else if !dash && !out.is_empty() {
            out.push('-');
            dash = true;
        }
    }
    let out = out.trim_end_matches('-').to_string();
    if out.is_empty() { "pack".to_string() } else { out }
}

/// packs/<slug>, suffixed -2/-3/… when the plain slug is taken.
fn fresh_pack_dir(packs: &Path, slug: &str) -> PathBuf {
    let first = packs.join(slug);
    if !first.exists() {
        return first;
    }
    for n in 2..1000 {
        let candidate = packs.join(format!("{slug}-{n}"));
        if !candidate.exists() {
            return candidate;
        }
    }
    first // pathological; overwrite target of copy_tree merge instead of failing
}

fn valid_pack_id(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= 128
        && id
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
}

// commands

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct PackInfo {
    pub id: String,
    pub name: String,
    pub license: Option<String>,
    /// No manifest license (or empty): warn, never block.
    pub warn_license: bool,
    pub frame_count: usize,
    pub path: String,
    /// Manifest role: "" = walker art, "habitat" = tiles/buildings (installs
    /// but stays out of the Sprite picker).
    pub role: String,
}

#[tauri::command]
pub fn list_packs(app: tauri::AppHandle) -> Result<Vec<PackInfo>, String> {
    let packs = packs_dir_path(&app)?;
    let mut out = Vec::new();
    let entries = std::fs::read_dir(&packs).map_err(|e| format!("read {}: {e}", packs.display()))?;
    for entry in entries.filter_map(|e| e.ok()) {
        let dir = entry.path();
        if !dir.is_dir() {
            continue;
        }
        let Some(id) = dir.file_name().and_then(|n| n.to_str()).map(String::from) else {
            continue;
        };
        let manifest = read_manifest(&dir);
        let frame_count = art_for(&dir, &manifest)
            .map(|a| a.walk.frame_count())
            .unwrap_or(0);
        let license = manifest.license.trim().to_string();
        out.push(PackInfo {
            name: if manifest.name.trim().is_empty() {
                id.clone()
            } else {
                manifest.name.trim().to_string()
            },
            warn_license: license.is_empty(),
            license: if license.is_empty() { None } else { Some(license) },
            frame_count,
            path: dir.to_string_lossy().into_owned(),
            role: manifest.role.trim().to_string(),
            id,
        });
    }
    out.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
    Ok(out)
}

/// Companion-window body roster: only packs with `role: companion`.
/// Planet walkers stay out of this list.
#[tauri::command]
pub fn list_companion_packs(app: tauri::AppHandle) -> Result<Vec<PackInfo>, String> {
    let mut out = list_packs(app)?;
    out.retain(|p| p.role.eq_ignore_ascii_case("companion") && p.frame_count > 0);
    Ok(out)
}

#[tauri::command]
pub fn packs_dir(app: tauri::AppHandle) -> Result<String, String> {
    Ok(packs_dir_path(&app)?.to_string_lossy().into_owned())
}

#[tauri::command]
pub fn remove_pack(app: tauri::AppHandle, id: String) -> Result<(), String> {
    if !valid_pack_id(&id) {
        return Err(format!("bad pack id: {id:?}"));
    }
    let dir = packs_dir_path(&app)?.join(&id);
    if !dir.exists() {
        return Ok(()); // already gone — remove is idempotent
    }
    std::fs::remove_dir_all(&dir).map_err(|e| format!("remove {}: {e}", dir.display()))?;
    crate::habitat::reload_pack_frames();
    Ok(())
}

/// NSOpenPanel must run on the main thread; commands arrive on a worker.
/// Returns the chosen path, or None on cancel. `exts` restricts file picks
/// (empty = any file); ignored for directory picks.
#[cfg(target_os = "macos")]
fn pick_path(
    app: &tauri::AppHandle,
    title: &str,
    directories: bool,
    exts: &[&str],
) -> Result<Option<PathBuf>, String> {
    let (tx, rx) = std::sync::mpsc::channel();
    let title = title.to_string();
    let exts: Vec<String> = exts.iter().map(|s| s.to_string()).collect();
    app.run_on_main_thread(move || {
        let picked = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| unsafe {
            pick_path_panel(&title, directories, &exts)
        }))
        .unwrap_or(None);
        let _ = tx.send(picked);
    })
    .map_err(|e| e.to_string())?;
    rx.recv_timeout(std::time::Duration::from_secs(300))
        .map_err(|_| "open panel: no answer from main thread".to_string())
}

#[cfg(target_os = "macos")]
unsafe fn pick_path_panel(title: &str, directories: bool, exts: &[String]) -> Option<PathBuf> {
    use core_foundation::base::TCFType;
    use objc::runtime::Object;
    use objc::{class, msg_send, sel, sel_impl};

    let panel: *mut Object = msg_send![class!(NSOpenPanel), openPanel];
    if panel.is_null() {
        return None;
    }
    let cf = core_foundation::string::CFString::new(title);
    let _: () = msg_send![panel, setTitle: cf.as_concrete_TypeRef() as *mut Object];
    let _: () = msg_send![panel, setCanChooseFiles: !directories];
    let _: () = msg_send![panel, setCanChooseDirectories: directories];
    let _: () = msg_send![panel, setAllowsMultipleSelection: false];
    if !directories && !exts.is_empty() {
        let cf_exts: Vec<core_foundation::string::CFString> =
            exts.iter().map(|e| core_foundation::string::CFString::new(e)).collect();
        let objs: Vec<*mut Object> = cf_exts
            .iter()
            .map(|e| e.as_concrete_TypeRef() as *mut Object)
            .collect();
        let types: *mut Object =
            msg_send![class!(NSArray), arrayWithObjects: objs.as_ptr() count: objs.len()];
        let _: () = msg_send![panel, setAllowedFileTypes: types];
    }
    let response: isize = msg_send![panel, runModal];
    if response != 1 {
        return None; // NSModalResponseOK
    }
    let url: *mut Object = msg_send![panel, URL];
    if url.is_null() {
        return None;
    }
    let path_obj: *mut Object = msg_send![url, path];
    if path_obj.is_null() {
        return None;
    }
    let utf8: *const std::os::raw::c_char = msg_send![path_obj, UTF8String];
    if utf8.is_null() {
        return None;
    }
    Some(PathBuf::from(
        std::ffi::CStr::from_ptr(utf8).to_string_lossy().into_owned(),
    ))
}

#[cfg(not(target_os = "macos"))]
fn pick_path(
    _app: &tauri::AppHandle,
    _title: &str,
    _directories: bool,
    _exts: &[&str],
) -> Result<Option<PathBuf>, String> {
    Ok(None)
}

#[tauri::command]
pub fn import_pack_folder(app: tauri::AppHandle) -> Result<Option<PackInfo>, String> {
    let Some(src) = pick_path(&app, "Import sprite pack folder", true, &[])? else {
        return Ok(None); // cancelled
    };
    if !src.is_dir() {
        return Err("not a folder".into());
    }
    let packs = packs_dir_path(&app)?;
    let name = src
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("pack");
    let dst = fresh_pack_dir(&packs, &slugify(name));
    copy_tree(&src, &dst).map_err(|e| format!("copy {}: {e}", src.display()))?;
    crate::habitat::reload_pack_frames();
    pack_info(&dst).map(Some)
}

#[tauri::command]
pub fn import_pack_zip(app: tauri::AppHandle) -> Result<Option<PackInfo>, String> {
    let Some(src) = pick_path(&app, "Import sprite pack zip", false, &["zip"])? else {
        return Ok(None); // cancelled
    };
    let packs = packs_dir_path(&app)?;
    let name = src
        .file_stem()
        .and_then(|n| n.to_str())
        .unwrap_or("pack");
    let dst = fresh_pack_dir(&packs, &slugify(name));
    std::fs::create_dir_all(&dst).map_err(|e| format!("create {}: {e}", dst.display()))?;
    if let Err(e) = unzip(&src, &dst) {
        let _ = std::fs::remove_dir_all(&dst); // never leave a half pack
        return Err(e);
    }
    crate::habitat::reload_pack_frames();
    pack_info(&dst).map(Some)
}

/// "Use my image…": pick one local image file; it lands
/// in packs/<slug>/ as a single-frame pack (a lone PNG/GIF/WebP/JPEG is a
/// still walker per `art_for`). Local files only — Orbitty never downloads
/// sprite art.
#[tauri::command]
pub fn import_pack_image(app: tauri::AppHandle) -> Result<Option<PackInfo>, String> {
    let Some(src) = pick_path(
        &app,
        "Use my image",
        false,
        &["png", "gif", "webp", "jpg", "jpeg"],
    )?
    else {
        return Ok(None); // cancelled
    };
    if !src.is_file() {
        return Err("not a file".into());
    }
    let packs = packs_dir_path(&app)?;
    let name = src
        .file_stem()
        .and_then(|n| n.to_str())
        .unwrap_or("image");
    let dst = fresh_pack_dir(&packs, &slugify(name));
    std::fs::create_dir_all(&dst).map_err(|e| format!("create {}: {e}", dst.display()))?;
    let file_name = src
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("image.png");
    std::fs::copy(&src, dst.join(file_name))
        .map_err(|e| format!("copy {}: {e}", src.display()))?;
    // Name-only manifest: no license claim — the picker warns (never
    // blocks) per the packs lock.
    let manifest_path = dst.join("manifest.json");
    if !manifest_path.exists() {
        if let Ok(text) = serde_json::to_string_pretty(&serde_json::json!({ "name": name })) {
            let _ = std::fs::write(&manifest_path, text);
        }
    }
    crate::habitat::reload_pack_frames();
    pack_info(&dst).map(Some)
}

fn pack_info(dir: &Path) -> Result<PackInfo, String> {
    let id = dir
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("pack")
        .to_string();
    let manifest = read_manifest(dir);
    let frame_count = art_for(dir, &manifest)
        .map(|a| a.walk.frame_count())
        .unwrap_or(0);
    let license = manifest.license.trim().to_string();
    Ok(PackInfo {
        name: if manifest.name.trim().is_empty() {
            id.clone()
        } else {
            manifest.name.trim().to_string()
        },
        warn_license: license.is_empty(),
        license: if license.is_empty() { None } else { Some(license) },
        frame_count,
        path: dir.to_string_lossy().into_owned(),
        role: manifest.role.trim().to_string(),
        id,
    })
}

fn copy_tree(src: &Path, dst: &Path) -> std::io::Result<()> {
    std::fs::create_dir_all(dst)?;
    for entry in std::fs::read_dir(src)? {
        let entry = entry?;
        let from = entry.path();
        let to = dst.join(entry.file_name());
        if from.is_dir() {
            copy_tree(&from, &to)?;
        } else {
            std::fs::copy(&from, &to)?;
        }
    }
    Ok(())
}

/// Extract into `dst`, stripping a single wrapper directory when every
/// entry shares one root (the classic itch zip shape). `enclosed_name`
/// rejects `..`/absolute paths.
fn unzip(src: &Path, dst: &Path) -> Result<(), String> {
    let file = std::fs::File::open(src).map_err(|e| format!("open {}: {e}", src.display()))?;
    let mut archive =
        zip::ZipArchive::new(file).map_err(|e| format!("read zip {}: {e}", src.display()))?;

    let mut root: Option<String> = None;
    let mut single_root = true;
    for i in 0..archive.len() {
        let Ok(entry) = archive.by_index(i) else {
            single_root = false;
            break;
        };
        let name = entry.name().trim_start_matches("./");
        if name.is_empty() {
            continue;
        }
        let first = name.split('/').next().unwrap_or("");
        if first.is_empty() || first == "__MACOSX" {
            continue; // resource-fork cruft never decides the root
        }
        match &root {
            None => root = Some(first.to_string()),
            Some(r) if r == first => {}
            Some(_) => {
                single_root = false;
                break;
            }
        }
        // A root-level file means there is no wrapper directory.
        if !name.contains('/') {
            single_root = false;
            break;
        }
    }
    let strip = if single_root { root } else { None };

    for i in 0..archive.len() {
        let mut entry = archive
            .by_index(i)
            .map_err(|e| format!("zip entry {i}: {e}"))?;
        let Some(rel) = entry.enclosed_name() else {
            continue;
        };
        let mut rel = rel.to_path_buf();
        if let Some(root) = &strip {
            let Ok(rest) = rel.strip_prefix(root) else {
                continue;
            };
            rel = rest.to_path_buf();
        }
        if rel.as_os_str().is_empty()
            || rel.components().any(|c| c.as_os_str() == "__MACOSX")
        {
            continue;
        }
        let out = dst.join(&rel);
        if entry.is_dir() {
            std::fs::create_dir_all(&out).map_err(|e| format!("create {}: {e}", out.display()))?;
            continue;
        }
        if let Some(parent) = out.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| format!("create {}: {e}", parent.display()))?;
        }
        let mut w = std::fs::File::create(&out)
            .map_err(|e| format!("write {}: {e}", out.display()))?;
        std::io::copy(&mut entry, &mut w).map_err(|e| format!("extract {}: {e}", out.display()))?;
    }
    Ok(())
}

/// Art sources for every pack that has frames, keyed by pack folder id —
/// the habitat loads these into NSImages and the sprite picker addresses
/// them by id (walkers use a picked pack instead of the built-in templates).
pub fn pack_art(app: &tauri::AppHandle) -> Vec<(String, PackArt)> {
    let Ok(packs) = packs_dir_path(app) else {
        return Vec::new();
    };
    let Ok(entries) = std::fs::read_dir(&packs) else {
        return Vec::new();
    };
    let mut dirs: Vec<PathBuf> = entries
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.is_dir())
        .collect();
    dirs.sort();
    dirs.iter()
        .filter_map(|d| {
            let id = d.file_name()?.to_str()?.to_string();
            let manifest = read_manifest(d);
            let role = manifest.role.trim();
            if role == "habitat" || role.eq_ignore_ascii_case("companion") {
                return None; // tiles / companion bodies — never planet walkers
            }
            art_for(d, &manifest).map(|a| (id, a))
        })
        .collect()
}

/// Resolve one pack's art for the companion
/// window's "Use my image…" (the caller prefers the idle cycle; a lone
/// image is a still spirit). Unknown/gone id → None.
pub fn companion_pack_art(app: &tauri::AppHandle, id: &str) -> Option<PackArt> {
    if !valid_pack_id(id) {
        return None;
    }
    let dir = packs_dir_path(app).ok()?.join(id);
    if !dir.is_dir() {
        return None;
    }
    let manifest = read_manifest(&dir);
    art_for(&dir, &manifest)
}

/// A companion pack MAY carry a bust set
/// (head+neck+collar) next to its idle cycle: manifest `bustFrames` first,
/// else top-level `bust_*` images, sorted. Never synthesized from idle_* —
/// no bust art means an empty list, and the companion sits on Full with
/// Head disabled (there is no 128px crop fallback).
pub fn companion_pack_bust(app: &tauri::AppHandle, id: &str) -> Vec<PathBuf> {
    if !valid_pack_id(id) {
        return Vec::new();
    }
    let dir = match packs_dir_path(app) {
        Ok(p) => p.join(id),
        Err(_) => return Vec::new(),
    };
    if !dir.is_dir() {
        return Vec::new();
    }
    let manifest = read_manifest(&dir);
    let listed: Vec<PathBuf> = manifest
        .bust_frames
        .iter()
        .map(|f| dir.join(f))
        .filter(|p| p.is_file())
        .collect();
    if !listed.is_empty() {
        return listed;
    }
    let Ok(entries) = std::fs::read_dir(&dir) else {
        return Vec::new();
    };
    let mut scanned: Vec<PathBuf> = entries
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.is_file() && is_frame_image(p))
        .filter(|p| {
            p.file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("")
                .to_lowercase()
                .starts_with("bust_")
        })
        .collect();
    scanned.sort();
    scanned
}

/// Create `packs/` + README.txt under the app data dir. Never touches
/// anything else in the folder — user packs are the user's own.
pub fn ensure_packs_dir(app: &tauri::AppHandle) {
    let Ok(dir) = app.path().app_data_dir() else {
        return;
    };
    let packs = dir.join("packs");
    if std::fs::create_dir_all(&packs).is_err() {
        return;
    }
    let _ = std::fs::write(packs.join("README.txt"), README);
    seed_bible_packs(&packs, app);
}

fn seed_bible_packs(packs: &Path, app: &tauri::AppHandle) {
    // The Season 1 bitty crew (11 kinds × default +
    // 3 named palettes) is the default planet population. The old Imagine
    // guy/cat slugs stay seeded so pre-S1 claims that picked them still
    // resolve — they are just never the default anymore (chars::default_pack).
    const SLUGS: &[&str] = &[
        "orbitty-guy",
        "orbitty-cat-orange",
        "orbitty-cat-tuxedo",
        "orbitty-cat-void",
        "orbitty-cat-gray",
        "orbitty-cat-cream",
        "orbitty-cat-calico",
        "orbitty-cat-siamese",
        "orbitty-bitty-boy",
        "orbitty-bitty-boy-green",
        "orbitty-bitty-boy-red",
        "orbitty-bitty-boy-tan",
        "orbitty-bitty-girl",
        "orbitty-bitty-girl-blue",
        "orbitty-bitty-girl-brown",
        "orbitty-bitty-girl-green",
        "orbitty-bitty-man",
        "orbitty-bitty-man-brown",
        "orbitty-bitty-man-green",
        "orbitty-bitty-man-red",
        "orbitty-bitty-woman",
        "orbitty-bitty-woman-blue",
        "orbitty-bitty-woman-green",
        "orbitty-bitty-woman-red",
        "orbitty-bitty-old-man",
        "orbitty-bitty-old-man-blue",
        "orbitty-bitty-old-man-brown",
        "orbitty-bitty-old-man-burgundy",
        "orbitty-bitty-old-woman",
        "orbitty-bitty-old-woman-blue",
        "orbitty-bitty-old-woman-brown",
        "orbitty-bitty-old-woman-green",
        "orbitty-bitty-cat",
        "orbitty-bitty-cat-black",
        "orbitty-bitty-cat-calico",
        "orbitty-bitty-cat-gray",
        "orbitty-bitty-dog",
        "orbitty-bitty-dog-black",
        "orbitty-bitty-dog-spot",
        "orbitty-bitty-dog-white",
        "orbitty-bitty-cow",
        "orbitty-bitty-cow-black",
        "orbitty-bitty-cow-brown",
        "orbitty-bitty-cow-white",
        "orbitty-bitty-horse",
        "orbitty-bitty-horse-black",
        "orbitty-bitty-horse-palomino",
        "orbitty-bitty-horse-white",
        "orbitty-bitty-chicken",
        "orbitty-bitty-chicken-black",
        "orbitty-bitty-chicken-brown",
        "orbitty-bitty-chicken-white",
    ];
    let mut srcs: Vec<PathBuf> = Vec::new();
    if let Ok(res) = app.path().resource_dir() {
        srcs.push(res.join("packs"));
        srcs.push(res.join("imagine-walkers/packs"));
        srcs.push(res.join("bitty-crew/packs"));
    }
    srcs.push(PathBuf::from(
        "/Users/dude/goose artifacts/livesprite-tauri/brand/imagine-walkers/packs",
    ));
    srcs.push(PathBuf::from(
        "/Users/dude/goose artifacts/livesprite-tauri/brand/bitty-crew/packs",
    ));
    for slug in SLUGS {
        let dest = packs.join(slug);
        if dest.join("walk_0.png").exists() {
            continue;
        }
        for root in &srcs {
            let src = root.join(slug);
            if src.join("walk_0.png").exists() {
                let _ = copy_dir(&src, &dest);
                break;
            }
        }
    }
}

fn copy_dir(src: &Path, dest: &Path) -> std::io::Result<()> {
    std::fs::create_dir_all(dest)?;
    for ent in std::fs::read_dir(src)? {
        let ent = ent?;
        let to = dest.join(ent.file_name());
        if ent.path().is_dir() {
            copy_dir(&ent.path(), &to)?;
        } else {
            let _ = std::fs::copy(ent.path(), to);
        }
    }
    Ok(())
}
