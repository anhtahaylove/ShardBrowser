use crate::store;
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use std::sync::{Mutex, OnceLock};

/// Launcher-side view of a profile (wraps raw FingerprintConfig JSON).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProfileMeta {
    pub id: String,
    pub name: String,
    pub notes: String,
    pub proxy_id: Option<String>,
    pub last_launched_at: Option<String>,
    pub created_at: Option<String>,
    pub pinned: bool,
    pub folder: String,
    /// Accumulated runtime across every launch; UI shows this plus the
    /// current-session uptime when the profile is running.
    #[serde(default)]
    pub total_runtime_ms: u64,
}

/// On-disk `<profiles_dir>/<id>.json`: FingerprintConfig + `_meta` envelope.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct StoredProfile {
    #[serde(rename = "_meta", default)]
    pub meta: StoredMeta,
    /// Verbatim FingerprintConfig payload (round-trip, not parsed).
    #[serde(flatten)]
    pub config: serde_json::Map<String, serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct StoredMeta {
    #[serde(default)]
    pub id: String,
    #[serde(default)]
    pub proxy_id: Option<String>,
    #[serde(default)]
    pub last_launched_at: Option<String>,
    /// "@<unix_secs>" creation marker.
    #[serde(default)]
    pub created_at: Option<String>,
    #[serde(default)]
    pub pinned: bool,
    /// Empty = unfiled (All tab).
    #[serde(default)]
    pub folder: String,
    /// Cumulative engine uptime in milliseconds; bumped by the Tracker
    /// when the child exits.  Persists across launcher restarts.
    #[serde(default)]
    pub total_runtime_ms: u64,
    /// Source library fingerprint id; MUST round-trip — drives the editor GPU select.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub gpu_preset_id: Option<String>,
    /// Inline proxy from temporary profile API; not in proxy store.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub inline_proxy: Option<crate::proxy::ProxyEntry>,
    /// Hidden from listings; auto-deleted on close.
    #[serde(default, skip_serializing_if = "is_false")]
    pub temporary: bool,
}

fn is_false(b: &bool) -> bool {
    !*b
}

pub const MAX_PROFILE_NAME_CHARS: usize = 128;
const CREATE_PROFILE_CLAIM: &str = "\0create-profile";
const GLOBAL_FOLDER_CLAIM: &str = "\0folder-mutation";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProfileErrorKind {
    Running,
    Busy,
    InvalidName,
    NameConflict,
}

#[derive(Debug)]
pub struct ProfileMutationError {
    kind: ProfileErrorKind,
    message: String,
}

impl std::fmt::Display for ProfileMutationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for ProfileMutationError {}

fn profile_error(kind: ProfileErrorKind, message: impl Into<String>) -> anyhow::Error {
    ProfileMutationError {
        kind,
        message: message.into(),
    }
    .into()
}

pub fn profile_error_kind(error: &anyhow::Error) -> Option<ProfileErrorKind> {
    error
        .downcast_ref::<ProfileMutationError>()
        .map(|error| error.kind)
}

pub fn normalize_profile_name(name: &str) -> Result<String> {
    if name.chars().any(char::is_control) {
        return Err(profile_error(
            ProfileErrorKind::InvalidName,
            "Profile name cannot contain control characters",
        ));
    }
    if name
        .chars()
        .last()
        .is_some_and(|character| character.is_whitespace() || character == '.')
    {
        return Err(profile_error(
            ProfileErrorKind::InvalidName,
            "Profile name cannot end with a dot or space",
        ));
    }

    let normalized = name.trim();
    if normalized.is_empty() {
        return Err(profile_error(
            ProfileErrorKind::InvalidName,
            "Profile name is required",
        ));
    }
    if normalized.chars().count() > MAX_PROFILE_NAME_CHARS {
        return Err(profile_error(
            ProfileErrorKind::InvalidName,
            format!("Profile name must be at most {MAX_PROFILE_NAME_CHARS} characters"),
        ));
    }
    if normalized.contains(['/', '\\']) {
        return Err(profile_error(
            ProfileErrorKind::InvalidName,
            "Profile name cannot contain path separators",
        ));
    }
    if normalized == "." || normalized == ".." {
        return Err(profile_error(
            ProfileErrorKind::InvalidName,
            "Profile name cannot be '.' or '..'",
        ));
    }
    if normalized.chars().any(|ch| matches!(ch, '<' | '>' | ':' | '"' | '|' | '?' | '*')) {
        return Err(profile_error(
            ProfileErrorKind::InvalidName,
            "Profile name contains characters reserved by Windows",
        ));
    }

    let device_stem = normalized
        .split('.')
        .next()
        .unwrap_or(normalized)
        .to_ascii_uppercase();
    let reserved = matches!(device_stem.as_str(), "CON" | "PRN" | "AUX" | "NUL")
        || (device_stem.len() == 4
            && (device_stem.starts_with("COM") || device_stem.starts_with("LPT"))
            && matches!(device_stem.as_bytes()[3], b'1'..=b'9'));
    if reserved {
        return Err(profile_error(
            ProfileErrorKind::InvalidName,
            "Profile name is reserved by Windows",
        ));
    }

    Ok(normalized.to_string())
}

pub fn validate_profile_name_for_mutation(
    name: &str,
    profile_id: Option<&str>,
) -> Result<String> {
    let existing_name = profile_id
        .filter(|id| !id.is_empty())
        .and_then(|id| load_raw(id).ok())
        .map(|stored| stored_profile_name(&stored).to_string());
    if !should_validate_profile_name(
        profile_id.is_none_or(str::is_empty),
        existing_name.as_deref(),
        name,
    ) {
        return Ok(name.to_string());
    }

    let normalized = normalize_profile_name(name)?;
    ensure_profile_name_available(&normalized, profile_id.filter(|id| !id.is_empty()))?;
    Ok(normalized)
}

fn profile_names_collide(left: &str, right: &str) -> bool {
    left.to_lowercase() == right.to_lowercase()
}

fn should_validate_profile_name(
    is_new: bool,
    existing_name: Option<&str>,
    candidate: &str,
) -> bool {
    is_new || existing_name.map(|name| name != candidate).unwrap_or(true)
}

fn stored_profile_name(stored: &StoredProfile) -> &str {
    stored
        .config
        .get("name")
        .and_then(|value| value.as_str())
        .unwrap_or("")
}

fn existing_profile_names() -> Result<Vec<(String, String)>> {
    let mut names = Vec::new();
    for entry in fs::read_dir(store::profiles_dir()?)? {
        let entry = entry?;
        if entry.path().extension().and_then(|value| value.to_str()) != Some("json") {
            continue;
        }
        let Ok(body) = fs::read_to_string(entry.path()) else {
            continue;
        };
        let Ok(stored): std::result::Result<StoredProfile, _> = serde_json::from_str(&body)
        else {
            continue;
        };
        let name = stored_profile_name(&stored).to_string();
        names.push((stored.meta.id, name));
    }
    Ok(names)
}

fn ensure_profile_name_available(name: &str, exclude_id: Option<&str>) -> Result<()> {
    for (id, existing_name) in existing_profile_names()? {
        if exclude_id == Some(id.as_str()) {
            continue;
        }
        if profile_names_collide(name, &existing_name) {
            return Err(profile_error(
                ProfileErrorKind::NameConflict,
                format!("Another profile already uses the name '{name}'"),
            ));
        }
    }
    Ok(())
}

pub fn prepare_profile_name_for_save(stored: &mut StoredProfile) -> Result<()> {
    let candidate = stored_profile_name(stored).to_string();
    let profile_id = (!stored.meta.id.is_empty()).then_some(stored.meta.id.as_str());
    let normalized = validate_profile_name_for_mutation(&candidate, profile_id)?;
    if normalized != candidate {
        stored
            .config
            .insert("name".into(), serde_json::Value::String(normalized));
    }
    Ok(())
}

pub fn prepare_import_batch(profiles: &mut [StoredProfile]) -> Result<()> {
    let existing = existing_profile_names()?;
    let mut batch_names: Vec<String> = Vec::with_capacity(profiles.len());
    for stored in profiles.iter_mut() {
        stored.meta.id.clear();
        let normalized = normalize_profile_name(stored_profile_name(stored))?;
        if existing
            .iter()
            .any(|(_, name)| profile_names_collide(&normalized, name))
            || batch_names
                .iter()
                .any(|name| profile_names_collide(&normalized, name))
        {
            return Err(profile_error(
                ProfileErrorKind::NameConflict,
                format!("Another profile already uses the name '{normalized}'"),
            ));
        }
        stored
            .config
            .insert("name".into(), serde_json::Value::String(normalized.clone()));
        batch_names.push(normalized);
    }
    Ok(())
}

#[derive(Debug, Clone, Copy)]
enum ProfileClaimKind {
    Launch,
    Mutation,
}

fn lifecycle_claims() -> &'static Mutex<HashMap<String, ProfileClaimKind>> {
    static CLAIMS: OnceLock<Mutex<HashMap<String, ProfileClaimKind>>> = OnceLock::new();
    CLAIMS.get_or_init(|| Mutex::new(HashMap::new()))
}

#[derive(Debug)]
pub struct ProfileClaimGuard {
    keys: Vec<String>,
}

impl Drop for ProfileClaimGuard {
    fn drop(&mut self) {
        if let Ok(mut claims) = lifecycle_claims().lock() {
            for key in &self.keys {
                claims.remove(key);
            }
        }
    }
}

fn begin_claim<I, S>(
    profile_ids: I,
    action: &str,
    kind: ProfileClaimKind,
) -> Result<ProfileClaimGuard>
where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
{
    let mut keys: Vec<String> = profile_ids
        .into_iter()
        .map(|id| id.as_ref().to_string())
        .collect();
    keys.sort();
    keys.dedup();

    let mut claims = lifecycle_claims()
        .lock()
        .map_err(|_| profile_error(ProfileErrorKind::Busy, "Profile lifecycle lock is unavailable"))?;
    if claims.contains_key(GLOBAL_FOLDER_CLAIM) {
        return Err(profile_error(
            ProfileErrorKind::Busy,
            format!("Profiles are being reorganized; retry {action}"),
        ));
    }
    let requests_name_namespace = keys.iter().any(|key| key == CREATE_PROFILE_CLAIM);
    if matches!(kind, ProfileClaimKind::Mutation) {
        let name_namespace_busy = if requests_name_namespace {
            claims
                .iter()
                .any(|(key, claim)| key != CREATE_PROFILE_CLAIM && matches!(claim, ProfileClaimKind::Mutation))
        } else {
            claims.contains_key(CREATE_PROFILE_CLAIM)
        };
        if name_namespace_busy {
            return Err(profile_error(
                ProfileErrorKind::Busy,
                format!("Profile names are being modified; retry {action}"),
            ));
        }
    }
    for key in &keys {
        if claims.contains_key(key) {
            return Err(profile_error(
                ProfileErrorKind::Busy,
                format!("Profile is being launched or modified; retry {action}"),
            ));
        }
        if key != CREATE_PROFILE_CLAIM && crate::process::Tracker::shared().is_running(key) {
            return Err(profile_error(
                ProfileErrorKind::Running,
                format!("Stop the running browser before you {action}"),
            ));
        }
    }
    for key in &keys {
        claims.insert(key.clone(), kind);
    }
    drop(claims);
    Ok(ProfileClaimGuard { keys })
}

pub fn begin_user_mutation<I, S>(profile_ids: I, action: &str) -> Result<ProfileClaimGuard>
where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
{
    begin_claim(profile_ids, action, ProfileClaimKind::Mutation)
}

pub fn begin_profile_creation(action: &str) -> Result<ProfileClaimGuard> {
    begin_claim(
        [CREATE_PROFILE_CLAIM],
        action,
        ProfileClaimKind::Mutation,
    )
}

pub fn begin_clone_mutation(profile_id: &str) -> Result<ProfileClaimGuard> {
    begin_claim(
        [profile_id, CREATE_PROFILE_CLAIM],
        "clone this profile",
        ProfileClaimKind::Mutation,
    )
}

pub fn begin_profile_launch(profile_id: &str) -> Result<ProfileClaimGuard> {
    begin_claim(
        [profile_id],
        "launch this profile",
        ProfileClaimKind::Launch,
    )
}

fn begin_folder_mutation(folder: &str, action: &str) -> Result<ProfileClaimGuard> {
    let mut claims = lifecycle_claims()
        .lock()
        .map_err(|_| profile_error(ProfileErrorKind::Busy, "Profile lifecycle lock is unavailable"))?;
    if !claims.is_empty() {
        return Err(profile_error(
            ProfileErrorKind::Busy,
            format!("Profiles are being launched or modified; retry {action}"),
        ));
    }
    claims.insert(GLOBAL_FOLDER_CLAIM.to_string(), ProfileClaimKind::Mutation);
    drop(claims);

    let guard = ProfileClaimGuard {
        keys: vec![GLOBAL_FOLDER_CLAIM.to_string()],
    };
    let profile_ids = profile_ids_in_folder(folder)?;
    for profile_id in &profile_ids {
        if crate::process::Tracker::shared().is_running(profile_id) {
            return Err(profile_error(
                ProfileErrorKind::Running,
                format!("Stop every running browser before you {action}"),
            ));
        }
    }
    Ok(guard)
}

fn path_for(id: &str) -> Result<PathBuf> {
    if id.contains(['/', '\\', '.']) {
        anyhow::bail!("invalid profile id");
    }
    Ok(store::profiles_dir()?.join(format!("{id}.json")))
}

pub fn list_all() -> Result<Vec<ProfileMeta>> {
    let dir = store::profiles_dir()?;
    let mut out = Vec::new();
    for entry in fs::read_dir(&dir)? {
        let entry = entry?;
        if entry.path().extension().and_then(|s| s.to_str()) != Some("json") {
            continue;
        }
        let path = entry.path();
        let body = fs::read_to_string(&path)?;
        let Ok(mut stored): std::result::Result<StoredProfile, _> = serde_json::from_str(&body) else {
            continue;
        };
        // Hide ephemeral profiles.
        if stored.meta.temporary {
            continue;
        }
        // Backfill legacy profiles' created_at from file mtime, then persist.
        if stored.meta.created_at.is_none() {
            let mtime = entry
                .metadata()
                .ok()
                .and_then(|m| m.modified().ok())
                .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                .map(|d| format!("@{}", d.as_secs()));
            if let Some(ts) = mtime {
                stored.meta.created_at = Some(ts);
                if let Ok(_claim) =
                    begin_user_mutation([&stored.meta.id], "backfill profile metadata")
                {
                    if let Ok(body) = serde_json::to_string_pretty(&stored) {
                        let _ = fs::write(&path, body);
                    }
                }
            }
        }
        let name = stored
            .config
            .get("name")
            .and_then(|v| v.as_str())
            .unwrap_or("(unnamed)")
            .to_string();
        let notes = stored
            .config
            .get("notes")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        out.push(ProfileMeta {
            id: stored.meta.id,
            name,
            notes,
            proxy_id: stored.meta.proxy_id,
            last_launched_at: stored.meta.last_launched_at,
            created_at: stored.meta.created_at,
            pinned: stored.meta.pinned,
            folder: stored.meta.folder,
            total_runtime_ms: stored.meta.total_runtime_ms,
        });
    }
    // Pinned first, then newest-first by created_at; name fallback for same-second ties.
    out.sort_by(|a, b| {
        match (a.pinned, b.pinned) {
            (true, false) => return std::cmp::Ordering::Less,
            (false, true) => return std::cmp::Ordering::Greater,
            _ => {}
        }
        match (&b.created_at, &a.created_at) {
            (Some(bv), Some(av)) => bv.cmp(av),
            (Some(_), None) => std::cmp::Ordering::Less,
            (None, Some(_)) => std::cmp::Ordering::Greater,
            (None, None) => a.name.cmp(&b.name),
        }
    });
    Ok(out)
}

/// Delete leftover temporary profiles after a crash; returns count.
pub fn purge_temporary() -> Result<usize> {
    let dir = store::profiles_dir()?;
    let mut n = 0;
    for entry in fs::read_dir(&dir)? {
        let entry = entry?;
        if entry.path().extension().and_then(|s| s.to_str()) != Some("json") {
            continue;
        }
        let Ok(body) = fs::read_to_string(entry.path()) else { continue; };
        let Ok(stored): std::result::Result<StoredProfile, _> = serde_json::from_str(&body) else {
            continue;
        };
        if stored.meta.temporary && !stored.meta.id.is_empty() {
            let _ = delete(&stored.meta.id);
            n += 1;
        }
    }
    Ok(n)
}

pub fn load_raw(id: &str) -> Result<StoredProfile> {
    let path = path_for(id)?;
    let body = fs::read_to_string(&path).with_context(|| format!("read {}", path.display()))?;
    let stored: StoredProfile = serde_json::from_str(&body)?;
    Ok(stored)
}

/// Deterministic non-zero 32-bit seed from the profile id + noise slot (FNV-1a).
/// Same id + slot always yields the same seed (stable fingerprint across
/// launches/edits); different ids yield different seeds (unique per profile).
fn derive_noise_seed(id: &str, slot: &str) -> u32 {
    let s = format!("{id}::{slot}");
    let mut h: u32 = 2166136261;
    for b in s.bytes() {
        h ^= b as u32;
        h = h.wrapping_mul(16777619);
    }
    // 0 is the "derive automatically" sentinel — never hand it back as a value.
    if h == 0 {
        1
    } else {
        h
    }
}

/// Replace every auto-sentinel noise seed (`seed == 0` or absent) with a
/// stable per-profile value derived from the final profile id.  The UI can't
/// know the id at create time, so it sends `seed: 0` for every vector; without
/// this every freshly-created profile would otherwise share one placeholder
/// seed and produce an identical canvas/audio/WebGL fingerprint.
fn fill_noise_seeds(config: &mut serde_json::Map<String, serde_json::Value>, id: &str) {
    let Some(noise) = config.get_mut("noise").and_then(|n| n.as_object_mut()) else {
        return;
    };
    for (slot, block) in noise.iter_mut() {
        let Some(obj) = block.as_object_mut() else {
            continue;
        };
        let needs = obj
            .get("seed")
            .and_then(|v| v.as_u64())
            .map(|n| n == 0)
            .unwrap_or(true);
        if needs {
            obj.insert("seed".into(), serde_json::Value::from(derive_noise_seed(id, slot)));
        }
    }
}

/// Reset every noise seed back to the auto sentinel so the next `save_raw`
/// re-derives them from a fresh id.  Used when cloning so the copy doesn't
/// inherit the source's canvas/audio/WebGL fingerprint.
fn clear_noise_seeds(config: &mut serde_json::Map<String, serde_json::Value>) {
    let Some(noise) = config.get_mut("noise").and_then(|n| n.as_object_mut()) else {
        return;
    };
    for (_, block) in noise.iter_mut() {
        if let Some(obj) = block.as_object_mut() {
            obj.insert("seed".into(), serde_json::Value::from(0u32));
        }
    }
}

pub fn save_raw(stored: &mut StoredProfile) -> Result<()> {
    let is_new = stored.meta.id.is_empty();
    if is_new {
        stored.meta.id = uuid::Uuid::new_v4().to_string();
    }
    // Carry created_at/pinned/folder/last_launched_at through edits.
    // pinned and folder are owned by set_pin/set_folder respectively.
    if !is_new {
        if let Ok(existing) = load_raw(&stored.meta.id) {
            if stored.meta.created_at.is_none() {
                stored.meta.created_at = existing.meta.created_at;
            }
            stored.meta.pinned = existing.meta.pinned;
            if stored.meta.folder.is_empty() {
                stored.meta.folder = existing.meta.folder;
            }
            if stored.meta.last_launched_at.is_none() {
                stored.meta.last_launched_at = existing.meta.last_launched_at;
            }
            // total_runtime_ms is owned by the Tracker — every save (edit /
            // proxy bind / folder move) carries the existing counter through.
            if stored.meta.total_runtime_ms == 0 {
                stored.meta.total_runtime_ms = existing.meta.total_runtime_ms;
            }
        }
    }
    if stored.meta.created_at.is_none() {
        stored.meta.created_at = Some(chrono_now_iso());
    }
    // The id is now final (freshly minted for new profiles, carried through for
    // edits) — derive per-profile noise seeds from it so each profile gets a
    // unique-but-stable fingerprint instead of sharing the UI's placeholder.
    fill_noise_seeds(&mut stored.config, &stored.meta.id);
    let path = path_for(&stored.meta.id)?;
    let body = serde_json::to_string_pretty(stored)?;
    fs::write(path, body)?;
    Ok(())
}

pub fn delete(id: &str) -> Result<()> {
    let path = path_for(id)?;
    if path.exists() {
        fs::remove_file(path)?;
    }
    // Also wipe per-profile user-data-dir.
    let udd = store::user_data_root()?.join(id);
    if udd.exists() {
        let _ = fs::remove_dir_all(udd);
    }
    Ok(())
}

/// Add `ms` to the persisted total_runtime_ms counter.  Called by the
/// process Tracker when the engine exits — totals survive launcher restarts.
pub fn add_runtime(id: &str, ms: u64) -> Result<()> {
    let mut p = load_raw(id)?;
    p.meta.total_runtime_ms = p.meta.total_runtime_ms.saturating_add(ms);
    save_raw(&mut p)?;
    Ok(())
}

/// Touch last_launched_at; optionally switch bound proxy.
pub fn touch_launched(id: &str, proxy_id: Option<String>) -> Result<()> {
    let mut p = load_raw(id)?;
    p.meta.last_launched_at = Some(chrono_now_iso());
    if proxy_id.is_some() {
        p.meta.proxy_id = proxy_id;
    }
    save_raw(&mut p)?;
    Ok(())
}

pub fn clone_profile(id: &str) -> Result<ProfileMeta> {
    let mut src = load_raw(id)?;
    let old_name = src
        .config
        .get("name")
        .and_then(|v| v.as_str())
        .unwrap_or("profile")
        .to_string();
    let clone_name = next_available_clone_name(&old_name)?;
    src.meta.id.clear();
    src.meta.last_launched_at = None;
    src.meta.created_at = None;
    src.meta.pinned = false;
    src.config
        .insert("name".into(), serde_json::Value::String(clone_name.clone()));
    // Re-randomize CPU/RAM/platform_version so the copy doesn't collide on those axes.
    crate::randomize_platform_version(&mut src.config);
    crate::randomize_hardware(&mut src.config);
    // Same reasoning for the fingerprint noise: drop the source's seeds so
    // save_raw re-derives fresh ones from new_id, giving the copy its own
    // canvas/audio/WebGL fingerprint instead of a clone of the original's.
    clear_noise_seeds(&mut src.config);
    save_raw(&mut src)?;
    Ok(ProfileMeta {
        id: src.meta.id,
        name: clone_name,
        notes: src
            .config
            .get("notes")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string(),
        proxy_id: src.meta.proxy_id,
        last_launched_at: None,
        created_at: src.meta.created_at,
        pinned: false,
        folder: src.meta.folder,
        total_runtime_ms: 0,
    })
}

fn next_available_clone_name(source_name: &str) -> Result<String> {
    let source = source_name.trim();
    let source = if source.is_empty() { "Profile" } else { source };
    for suffix in 1..=10_000usize {
        let marker = if suffix == 1 {
            " (copy)".to_string()
        } else {
            format!(" (copy {suffix})")
        };
        let max_base = MAX_PROFILE_NAME_CHARS.saturating_sub(marker.chars().count());
        let base: String = source.chars().take(max_base).collect();
        let candidate = format!("{base}{marker}");
        let normalized = normalize_profile_name(&candidate)?;
        if ensure_profile_name_available(&normalized, None).is_ok() {
            return Ok(normalized);
        }
    }
    Err(profile_error(
        ProfileErrorKind::NameConflict,
        "Unable to allocate a unique name for the cloned profile",
    ))
}

/// Flip pin flag.
pub fn set_pin(id: &str, pinned: bool) -> Result<()> {
    let mut p = load_raw(id)?;
    p.meta.pinned = pinned;
    let path = path_for(&p.meta.id)?;
    let body = serde_json::to_string_pretty(&p)?;
    fs::write(path, body)?;
    Ok(())
}

/// Assign folder tag (empty string clears).
pub fn set_folder(id: &str, folder: &str) -> Result<()> {
    let mut p = load_raw(id)?;
    p.meta.folder = folder.trim().to_string();
    let path = path_for(&p.meta.id)?;
    let body = serde_json::to_string_pretty(&p)?;
    fs::write(path, body)?;
    Ok(())
}

pub fn profile_ids_in_folder(name: &str) -> Result<Vec<String>> {
    let mut profile_ids = Vec::new();
    for entry in fs::read_dir(store::profiles_dir()?)? {
        let entry = entry?;
        if entry.path().extension().and_then(|value| value.to_str()) != Some("json") {
            continue;
        }
        let Ok(body) = fs::read_to_string(entry.path()) else {
            continue;
        };
        let Ok(stored): std::result::Result<StoredProfile, _> = serde_json::from_str(&body)
        else {
            continue;
        };
        if stored.meta.folder == name && !stored.meta.id.is_empty() {
            profile_ids.push(stored.meta.id);
        }
    }
    Ok(profile_ids)
}

/// Retag profiles from folder `old` to `new`; returns count.
pub fn rename_folder(old: &str, new: &str) -> Result<usize> {
    let _claim = begin_folder_mutation(old, "rename this folder")?;
    let dir = store::profiles_dir()?;
    let new = new.trim();
    let mut n = 0;
    for entry in fs::read_dir(&dir)? {
        let entry = entry?;
        if entry.path().extension().and_then(|s| s.to_str()) != Some("json") {
            continue;
        }
        let Ok(body) = fs::read_to_string(entry.path()) else { continue; };
        let Ok(mut stored): std::result::Result<StoredProfile, _> = serde_json::from_str(&body)
        else {
            continue;
        };
        if stored.meta.folder == old {
            stored.meta.folder = new.to_string();
            if let Ok(out) = serde_json::to_string_pretty(&stored) {
                let _ = fs::write(entry.path(), out);
            }
            n += 1;
        }
    }
    Ok(n)
}

/// Delete folder; `delete_profiles` true removes, false unfiles. Returns count.
pub fn delete_folder(name: &str, delete_profiles: bool) -> Result<usize> {
    let _claim = begin_folder_mutation(name, "delete this folder")?;
    let dir = store::profiles_dir()?;
    let mut n = 0;
    for entry in fs::read_dir(&dir)? {
        let entry = entry?;
        if entry.path().extension().and_then(|s| s.to_str()) != Some("json") {
            continue;
        }
        let Ok(body) = fs::read_to_string(entry.path()) else { continue; };
        let Ok(mut stored): std::result::Result<StoredProfile, _> = serde_json::from_str(&body)
        else {
            continue;
        };
        if stored.meta.folder == name {
            if delete_profiles {
                let _ = delete(&stored.meta.id);
            } else {
                stored.meta.folder = String::new();
                if let Ok(out) = serde_json::to_string_pretty(&stored) {
                    let _ = fs::write(entry.path(), out);
                }
            }
            n += 1;
        }
    }
    Ok(n)
}

/// Per-profile user-data-dir; created on first call.
pub fn user_data_dir(id: &str) -> Result<PathBuf> {
    if id.contains(['/', '\\', '.']) {
        anyhow::bail!("invalid profile id");
    }
    let p = store::user_data_root()?.join(id);
    std::fs::create_dir_all(&p)?;
    Ok(p)
}

fn chrono_now_iso() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let s = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    format!("@{s}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn profile_name_validation_accepts_unicode_and_trims_leading_space() {
        assert_eq!(normalize_profile_name("  Việt Nam 001").unwrap(), "Việt Nam 001");
        assert_eq!(normalize_profile_name("Khách_hàng-01").unwrap(), "Khách_hàng-01");
        assert_eq!(normalize_profile_name(&"A".repeat(MAX_PROFILE_NAME_CHARS)).unwrap(), "A".repeat(MAX_PROFILE_NAME_CHARS));
    }

    #[test]
    fn profile_name_validation_rejects_unsafe_windows_names_and_boundaries() {
        let invalid = [
            "",
            "   ",
            ".",
            "..",
            "bad/name",
            "bad\\name",
            "bad\u{0000}name",
            "bad:name",
            "bad*name",
            "trailing.",
            "trailing ",
            "CON",
            "con.txt",
            "LPT9",
        ];
        for name in invalid {
            assert!(normalize_profile_name(name).is_err(), "expected invalid name: {name:?}");
        }
        assert!(normalize_profile_name(&"A".repeat(MAX_PROFILE_NAME_CHARS + 1)).is_err());
    }

    #[test]
    fn profile_name_collisions_are_case_insensitive_without_rejecting_legacy_noop_edits() {
        assert!(profile_names_collide("Automation", "automation"));
        assert!(profile_names_collide("HỒ SƠ", "hồ sơ"));
        assert!(!profile_names_collide("Automation 1", "Automation 2"));
        assert!(should_validate_profile_name(true, None, "new"));
        assert!(should_validate_profile_name(false, Some("old"), "new"));
        assert!(!should_validate_profile_name(false, Some("legacy/name"), "legacy/name"));
    }

    #[test]
    fn lifecycle_claims_serialize_launch_and_user_mutations() {
        let mutation = begin_user_mutation(["profile-claim-a"], "edit profile").unwrap();
        let launch_error = match begin_profile_launch("profile-claim-a") {
            Ok(_) => panic!("launch must not overlap a mutation"),
            Err(error) => error,
        };
        assert_eq!(profile_error_kind(&launch_error), Some(ProfileErrorKind::Busy));
        drop(mutation);

        let launch = begin_profile_launch("profile-claim-a").unwrap();
        let mutation_error = match begin_user_mutation(["profile-claim-a"], "delete profile") {
            Ok(_) => panic!("mutation must not overlap a launch reservation"),
            Err(error) => error,
        };
        assert_eq!(profile_error_kind(&mutation_error), Some(ProfileErrorKind::Busy));
        drop(launch);

        assert!(begin_user_mutation(["profile-claim-a"], "edit profile").is_ok());
    }

    #[test]
    fn lifecycle_claims_allow_unrelated_profiles_to_run_concurrently() {
        let first = begin_profile_launch("profile-claim-concurrent-a").unwrap();
        let second = begin_profile_launch("profile-claim-concurrent-b").unwrap();
        let edit = begin_user_mutation(["profile-claim-concurrent-c"], "edit profile").unwrap();
        drop((first, second, edit));
    }

    #[test]
    fn lifecycle_claims_serialize_name_changes_without_blocking_unrelated_launches() {
        let creation = begin_profile_creation("create profile").unwrap();
        let mutation_error = match begin_user_mutation(["profile-name-edit-a"], "rename profile") {
            Ok(_) => panic!("profile edit must not overlap name allocation"),
            Err(error) => error,
        };
        assert_eq!(profile_error_kind(&mutation_error), Some(ProfileErrorKind::Busy));
        let launch = begin_profile_launch("profile-name-launch-a").unwrap();
        drop((launch, creation));

        let mutation = begin_user_mutation(["profile-name-edit-b"], "rename profile").unwrap();
        let creation_error = match begin_profile_creation("create profile") {
            Ok(_) => panic!("name allocation must not overlap profile edit"),
            Err(error) => error,
        };
        assert_eq!(profile_error_kind(&creation_error), Some(ProfileErrorKind::Busy));
        drop(mutation);
    }

    #[test]
    fn running_profile_mutations_fail_closed_and_stopped_profiles_remain_mutable() {
        let profile_id = "profile-running-guard-test";
        let tracker = crate::process::Tracker::shared();
        tracker.set_running_for_test(profile_id, true);

        let error = match begin_user_mutation([profile_id], "edit this profile") {
            Ok(_) => panic!("running profile mutation must fail closed"),
            Err(error) => error,
        };
        assert_eq!(profile_error_kind(&error), Some(ProfileErrorKind::Running));

        tracker.set_running_for_test(profile_id, false);
        assert!(begin_user_mutation([profile_id], "edit this profile").is_ok());
    }
}
