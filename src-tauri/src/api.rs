// Local automation HTTP API (axum) for ShardX Launcher.
// 127.0.0.1:<api_port>; every endpoint except /health requires Bearer JWT (HS256).

use std::sync::{OnceLock, RwLock};

use axum::{
    extract::{Path, Query, Request},
    http::{header::AUTHORIZATION, StatusCode},
    middleware::{self, Next},
    response::{IntoResponse, Response},
    routing::{delete, get, patch, post},
    Json, Router,
};
use serde::Deserialize;
use serde_json::{json, Value};

// ---- HS256 secret (process-global so live rotation invalidates old tokens) ----

fn secret_cell() -> &'static RwLock<String> {
    static SECRET: OnceLock<RwLock<String>> = OnceLock::new();
    SECRET.get_or_init(|| RwLock::new(String::new()))
}

/// Install/replace the signing secret.
pub fn set_secret(s: &str) {
    if let Ok(mut g) = secret_cell().write() {
        *g = s.to_string();
    }
}

fn read_secret() -> String {
    secret_cell().read().map(|g| g.clone()).unwrap_or_default()
}

// ---- JWT ----

#[derive(serde::Serialize, serde::Deserialize)]
struct Claims {
    sub: String,
    iat: u64,
    exp: u64,
}

fn unix_now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

fn mint(secret: &str, ttl_secs: u64) -> Result<String, String> {
    use jsonwebtoken::{encode, Algorithm, EncodingKey, Header};
    let now = unix_now();
    let claims = Claims {
        sub: "shardx-api".into(),
        iat: now,
        exp: now.saturating_add(ttl_secs),
    };
    encode(
        &Header::new(Algorithm::HS256),
        &claims,
        &EncodingKey::from_secret(secret.as_bytes()),
    )
    .map_err(|e| e.to_string())
}

fn verify(secret: &str, token: &str) -> bool {
    use jsonwebtoken::{decode, Algorithm, DecodingKey, Validation};
    decode::<Claims>(
        token,
        &DecodingKey::from_secret(secret.as_bytes()),
        &Validation::new(Algorithm::HS256),
    )
    .is_ok()
}

/// 10-year token shown in Settings UI.
pub fn long_lived_token(secret: &str) -> Result<String, String> {
    mint(secret, 60 * 60 * 24 * 365 * 10)
}

// ---- error type ----

struct ApiError(StatusCode, String);

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        (self.0, Json(json!({ "error": self.1 }))).into_response()
    }
}

fn err(code: StatusCode, msg: impl Into<String>) -> ApiError {
    ApiError(code, msg.into())
}

type ApiResult = Result<Json<Value>, ApiError>;

// ---- auth middleware ----

async fn auth(req: Request, next: Next) -> Result<Response, StatusCode> {
    let secret = read_secret();
    let ok = req
        .headers()
        .get(AUTHORIZATION)
        .and_then(|h| h.to_str().ok())
        .and_then(|h| {
            h.strip_prefix("Bearer ")
                .or_else(|| h.strip_prefix("bearer "))
        })
        .map(|t| verify(&secret, t.trim()))
        .unwrap_or(false);
    if ok {
        Ok(next.run(req).await)
    } else {
        Err(StatusCode::UNAUTHORIZED)
    }
}

// ---- handlers ----

async fn health() -> Json<Value> {
    Json(json!({
        "ok": true,
        "name": "shardx-launcher",
        "version": env!("CARGO_PKG_VERSION"),
    }))
}

async fn list_profiles() -> ApiResult {
    let metas = crate::profile::list_all().map_err(|e| err(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    let running = crate::process::Tracker::shared().running();
    let by_id: std::collections::HashMap<String, crate::process::RunningProfile> =
        running.into_iter().map(|r| (r.profile_id.clone(), r)).collect();
    let out: Vec<Value> = metas
        .into_iter()
        .map(|m| {
            let r = by_id.get(&m.id);
            json!({
                "id": m.id,
                "name": m.name,
                "notes": m.notes,
                "proxy_id": m.proxy_id,
                "last_launched_at": m.last_launched_at,
                "created_at": m.created_at,
                "pinned": m.pinned,
                "folder": m.folder,
                "color": m.color,
                "extensions": m.extensions,
                "running": r.is_some(),
                "pid": r.map(|x| x.pid),
                "cdp": r.and_then(|x| x.cdp.clone()),
            })
        })
        .collect();
    Ok(Json(json!(out)))
}

async fn get_profile(Path(id): Path<String>) -> ApiResult {
    let stored = crate::profile::load_raw(&id)
        .map_err(|e| err(StatusCode::NOT_FOUND, e.to_string()))?;
    let mut val = serde_json::to_value(stored)
        .map_err(|e| err(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    if let Some(cdp) = crate::process::Tracker::shared().cdp(&id) {
        if let Some(obj) = val.as_object_mut() {
            obj.insert("running".into(), json!(true));
            obj.insert("cdp".into(), serde_json::to_value(cdp).unwrap_or(Value::Null));
        }
    }
    Ok(Json(val))
}

// ---- get-new-fingerprint ----

/// Uniquified fingerprint without persisting; create-profile stores verbatim.
async fn new_fingerprint() -> ApiResult {
    new_fingerprint_impl(None).await
}

async fn new_fingerprint_for(Path(platform): Path<String>) -> ApiResult {
    new_fingerprint_impl(Some(platform)).await
}

async fn new_fingerprint_impl(platform: Option<String>) -> ApiResult {
    let fid = random_fingerprint_for(platform.as_deref())?;
    let mut cfg = crate::build_fingerprint_config(crate::main_window().as_ref(), &fid)
        .map_err(|e| err(StatusCode::BAD_REQUEST, e))?;
    cfg.remove("_meta");
    Ok(Json(json!({ "fingerprint": cfg })))
}

// ---- create-profile ----

#[derive(Deserialize)]
struct CreateReq {
    name: Option<String>,
    notes: Option<String>,
    proxy_id: Option<String>,
    /// Proxy string: added to store + full-tested, bound by id.
    proxy: Option<String>,
    folder: Option<String>,
    /// Icon and omnibox-pill accent, `#rrggbb`. Omitted = derived from the name.
    color: Option<String>,
    /// Extension-library ids to load at launch (`GET /extensions`).
    extensions: Option<Vec<String>>,
    fingerprint: Value,
}

/// Persist verbatim (enrich=false); proxy_id binds, proxy string upserts+tests.
async fn persist_created(folder_override: Option<String>, body: CreateReq) -> ApiResult {
    let mut cfg = body
        .fingerprint
        .as_object()
        .cloned()
        .ok_or_else(|| err(StatusCode::BAD_REQUEST, "`fingerprint` must be an object"))?;
    cfg.remove("_meta");
    if let Some(n) = body.name.as_ref() {
        cfg.insert("name".into(), json!(n));
    }
    if let Some(n) = body.notes.as_ref() {
        cfg.insert("notes".into(), json!(n));
    }

    let folder = folder_override.or(body.folder).unwrap_or_default();
    let mut meta = json!({ "id": "", "folder": folder });
    if let Some(c) = body.color.as_ref().filter(|c| !c.is_empty()) {
        meta["color"] = json!(c);
    }
    if let Some(ids) = body.extensions.as_ref() {
        validate_extension_ids(ids)?;
        meta["extensions"] = json!(ids);
    }
    if let Some(pid) = body.proxy_id.as_ref() {
        meta["proxy_id"] = json!(pid);
    } else if let Some(pstr) = body.proxy.as_ref() {
        let entry = crate::proxy::parse_single(pstr)
            .ok_or_else(|| err(StatusCode::BAD_REQUEST, format!("unparseable proxy: {pstr}")))?;
        let stored = crate::proxy::upsert_dedup(entry)
            .map_err(|e| err(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
        // Best-effort full test (UDP + geo); launch re-probes UDP live anyway.
        let _ = crate::proxy::full_test(&stored).await;
        meta["proxy_id"] = json!(stored.id);
        crate::notify_store_changed("proxies");
    }
    crate::ensure_default_noise(&mut cfg);
    cfg.insert("_meta".into(), meta);

    let pm = crate::save_profile_core(crate::main_window().as_ref(), Value::Object(cfg), false)
        .map_err(|e| err(StatusCode::BAD_REQUEST, e))?;
    crate::notify_store_changed("profiles");
    Ok(Json(serde_json::to_value(pm).unwrap_or(Value::Null)))
}

async fn create_profile(Json(body): Json<CreateReq>) -> ApiResult {
    persist_created(None, body).await
}

async fn create_profile_in_folder(Path(folder): Path<String>, Json(body): Json<CreateReq>) -> ApiResult {
    persist_created(Some(folder), body).await
}

// ---- temporary profiles ----

#[derive(Deserialize)]
struct TempReq {
    fingerprint_id: Option<String>,
    platform: Option<String>,
    /// Inline proxy (not stored).
    proxy: Option<String>,
    name: Option<String>,
    folder: Option<String>,
    /// Per vector: `{"canvas": true}` or a full block. Omitted vectors stay off.
    noise: Option<Value>,
}

/// Temporary profile (hidden, auto-deleted on close); pair with /start.
async fn create_temporary(Json(body): Json<TempReq>) -> ApiResult {
    let fid = match body.fingerprint_id {
        Some(f) => f,
        None => random_fingerprint_for(body.platform.as_deref())?,
    };
    let mut cfg = crate::build_fingerprint_config(crate::main_window().as_ref(), &fid)
        .map_err(|e| err(StatusCode::BAD_REQUEST, e))?;
    cfg.remove("_meta");
    if let Some(n) = body.name.as_ref() {
        cfg.insert("name".into(), json!(n));
    }
    crate::ensure_default_noise(&mut cfg);
    if let Some(n) = body.noise.as_ref() {
        apply_noise_overrides(&mut cfg, n)
            .map_err(|e| err(StatusCode::BAD_REQUEST, e))?;
    }
    let mut meta = json!({ "id": "", "folder": body.folder.unwrap_or_default(), "temporary": true });
    if let Some(pstr) = body.proxy.as_ref() {
        let entry = crate::proxy::parse_single(pstr)
            .ok_or_else(|| err(StatusCode::BAD_REQUEST, format!("unparseable proxy: {pstr}")))?;
        meta["inline_proxy"] = serde_json::to_value(entry).unwrap_or(Value::Null);
    }
    cfg.insert("_meta".into(), meta);

    let pm = crate::save_profile_core(crate::main_window().as_ref(), Value::Object(cfg), false)
        .map_err(|e| err(StatusCode::BAD_REQUEST, e))?;
    Ok(Json(json!({
        "id": pm.id,
        "name": pm.name,
        "fingerprint_id": fid,
        "temporary": true,
        "proxy_inline": body.proxy.is_some(),
    })))
}

/// Merge a caller's noise request into the config's block. Merged, not
/// replaced: `{"webgl": true}` must not zero its intensity.
fn apply_noise_overrides(
    cfg: &mut serde_json::Map<String, Value>,
    req: &Value,
) -> Result<(), String> {
    const VECTORS: &[&str] = &["canvas", "webgl", "audio", "client_rects", "sensors", "fonts"];
    let req = req.as_object().ok_or("`noise` must be an object")?;
    let noise = cfg
        .get_mut("noise")
        .and_then(|n| n.as_object_mut())
        .ok_or("profile has no noise block")?;

    for (key, val) in req {
        let key = key.as_str();
        if !VECTORS.contains(&key) {
            return Err(format!("unknown noise vector `{key}`"));
        }
        let slot = noise
            .entry(key.to_string())
            .or_insert_with(|| json!({ "enabled": false, "seed": 0 }));
        let Some(slot) = slot.as_object_mut() else { continue };
        match val {
            Value::Bool(on) => {
                slot.insert("enabled".into(), json!(on));
                // The defaults the UI writes for the two vectors that carry a
                // strength; turning one on with a zero strength is a no-op.
                if *on && key == "webgl" && slot.get("intensity").and_then(|v| v.as_f64()) == Some(0.0) {
                    slot.insert("intensity".into(), json!(0.0005));
                }
                if *on && key == "client_rects" && slot.get("max_offset").and_then(|v| v.as_f64()) == Some(0.0) {
                    slot.insert("max_offset".into(), json!(1));
                }
            }
            Value::Object(fields) => {
                for (k, v) in fields {
                    slot.insert(k.clone(), v.clone());
                }
            }
            _ => return Err(format!("`noise.{key}` must be a bool or an object")),
        }
    }
    Ok(())
}

/// Into the trash, like the UI's delete. Temporary profiles are torn down by
/// the Tracker and never reach it.
async fn delete_profile(Path(id): Path<String>) -> ApiResult {
    crate::trash::move_to_trash(&id)
        .map_err(|e| err(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    crate::notify_store_changed("profiles");
    Ok(Json(json!({ "deleted": true, "id": id, "trashed": true })))
}

#[derive(Deserialize)]
struct EditReq {
    name: Option<String>,
    notes: Option<String>,
    /// "" unfiles.
    folder: Option<String>,
    /// "" unbinds.
    proxy_id: Option<String>,
    /// Proxy string: stored + tested, then bound.
    proxy: Option<String>,
    /// `#rrggbb`; "" goes back to the name-derived colour.
    color: Option<String>,
    /// Replaces the whole list; `[]` loads none.
    extensions: Option<Vec<String>>,
    /// Replace stored fingerprint verbatim.
    fingerprint: Option<Value>,
}

/// Reject unknown ids rather than writing a profile that launches without them.
fn validate_extension_ids(ids: &[String]) -> Result<(), ApiError> {
    let known: std::collections::HashSet<String> = crate::extensions::list()
        .map_err(|e| err(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
        .into_iter()
        .map(|e| e.id)
        .collect();
    let missing: Vec<&str> = ids
        .iter()
        .filter(|id| !known.contains(*id))
        .map(|s| s.as_str())
        .collect();
    if missing.is_empty() {
        Ok(())
    } else {
        Err(err(
            StatusCode::BAD_REQUEST,
            format!("no such extension: {}", missing.join(", ")),
        ))
    }
}

/// Edit profile; only provided fields change. Returns the updated profile.
async fn edit_profile(Path(id): Path<String>, Json(body): Json<EditReq>) -> ApiResult {
    let mut stored = crate::profile::load_raw(&id)
        .map_err(|e| err(StatusCode::NOT_FOUND, e.to_string()))?;

    if let Some(fp) = body.fingerprint {
        let mut cfg = fp
            .as_object()
            .cloned()
            .ok_or_else(|| err(StatusCode::BAD_REQUEST, "`fingerprint` must be an object"))?;
        cfg.remove("_meta");
        stored.config = cfg;
    }
    if let Some(n) = body.name.as_ref() {
        stored.config.insert("name".into(), json!(n));
    }
    if let Some(n) = body.notes.as_ref() {
        stored.config.insert("notes".into(), json!(n));
    }
    if let Some(pid) = body.proxy_id.as_ref() {
        stored.meta.proxy_id = if pid.is_empty() { None } else { Some(pid.clone()) };
        stored.meta.inline_proxy = None;
    } else if let Some(pstr) = body.proxy.as_ref() {
        let entry = crate::proxy::parse_single(pstr)
            .ok_or_else(|| err(StatusCode::BAD_REQUEST, format!("unparseable proxy: {pstr}")))?;
        let s = crate::proxy::upsert_dedup(entry)
            .map_err(|e| err(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
        let _ = crate::proxy::full_test(&s).await;
        stored.meta.proxy_id = Some(s.id);
        stored.meta.inline_proxy = None;
        crate::notify_store_changed("proxies");
    }
    if let Some(c) = body.color.as_ref() {
        // "" is how a caller asks for the derived colour back.
        stored.meta.color = (!c.is_empty()).then(|| c.clone());
    }
    if let Some(ids) = body.extensions.as_ref() {
        validate_extension_ids(ids)?;
        stored.meta.extensions = ids.clone();
    }

    crate::profile::save_raw(&mut stored)
        .map_err(|e| err(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    // set_folder handles unfile; save_raw keeps the existing folder when empty.
    if let Some(f) = body.folder.as_ref() {
        crate::profile::set_folder(&id, f)
            .map_err(|e| err(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    }

    let updated = crate::profile::load_raw(&id)
        .map_err(|e| err(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    crate::notify_store_changed("profiles");
    Ok(Json(serde_json::to_value(updated).unwrap_or(Value::Null)))
}

#[derive(Deserialize)]
struct RenameFolderReq {
    name: String,
}

async fn rename_folder_ep(Path(folder): Path<String>, Json(body): Json<RenameFolderReq>) -> ApiResult {
    let n = crate::profile::rename_folder(&folder, &body.name)
        .map_err(|e| err(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    crate::notify_store_changed("profiles");
    Ok(Json(json!({ "renamed_to": body.name, "profiles": n })))
}

#[derive(Deserialize)]
struct DeleteFolderQuery {
    /// true → delete profiles; false (default) → unfile.
    #[serde(default)]
    delete_profiles: bool,
}

async fn delete_folder_ep(Path(folder): Path<String>, Query(q): Query<DeleteFolderQuery>) -> ApiResult {
    let n = crate::profile::delete_folder(&folder, q.delete_profiles)
        .map_err(|e| err(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    crate::notify_store_changed("profiles");
    Ok(Json(json!({
        "deleted_folder": folder,
        "delete_profiles": q.delete_profiles,
        "profiles": n,
    })))
}

#[derive(Deserialize, Default)]
struct StartReq {
    #[serde(default)]
    headless: bool,
}

/// Launch with CDP; body `{ "headless": true }` opt-in.
async fn start_profile(Path(id): Path<String>, body: Option<Json<StartReq>>) -> ApiResult {
    let headless = body.map(|Json(b)| b.headless).unwrap_or(false);
    let outcome = crate::launch::launch_profile(&id, true, headless)
        .await
        .map_err(|e| err(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    Ok(Json(json!({
        "profile_id": id,
        "pid": outcome.pid,
        "headless": headless,
        "cdp": outcome.cdp,
    })))
}

async fn stop_profile(Path(id): Path<String>) -> ApiResult {
    let stopped = crate::process::Tracker::shared()
        .kill(&id)
        .await
        .map_err(|e| err(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    Ok(Json(json!({ "profile_id": id, "stopped": stopped })))
}

async fn export_cookies(Path(id): Path<String>) -> ApiResult {
    let cookies = crate::cookies::export(&id)
        .map_err(|e| err(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    Ok(Json(json!({ "cookies": cookies })))
}

#[derive(Deserialize)]
struct ImportCookiesReq {
    cookies: Vec<crate::cookies::Cookie>,
}

async fn import_cookies(Path(id): Path<String>, Json(body): Json<ImportCookiesReq>) -> ApiResult {
    // Running browser would clobber imports on exit.
    if crate::is_profile_running(&id) {
        return Err(err(
            StatusCode::CONFLICT,
            "stop the profile before importing cookies",
        ));
    }
    let n = crate::cookies::import(&id, &body.cookies)
        .map_err(|e| err(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    Ok(Json(json!({ "imported": n })))
}

async fn list_running() -> Json<Value> {
    Json(json!(crate::process::Tracker::shared().running()))
}

async fn list_fingerprints() -> ApiResult {
    let all = crate::fingerprints::list_all()
        .map_err(|e| err(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    let out: Vec<Value> = all
        .into_iter()
        .map(|e| {
            json!({
                "id": e.id,
                "label": e.label,
                "platform": e.platform,
                "chrome": e.chrome,
                "gpu": e.gpu,
                "builtin": e.builtin,
            })
        })
        .collect();
    Ok(Json(json!(out)))
}

#[derive(Deserialize)]
struct AddProxyReq {
    /// "scheme://user:pass@host:port" or "host:port:user:pass"; wins over fields.
    proxy: Option<String>,
    /// socks5 | http | https (default socks5).
    kind: Option<String>,
    host: Option<String>,
    port: Option<u16>,
    username: Option<String>,
    password: Option<String>,
    name: Option<String>,
    country: Option<String>,
    notes: Option<String>,
}

/// Add proxy (deduped by endpoint); returns summary.
async fn add_proxy(Json(body): Json<AddProxyReq>) -> ApiResult {
    let mut entry = if let Some(s) = body.proxy.as_ref() {
        crate::proxy::parse_single(s)
            .ok_or_else(|| err(StatusCode::BAD_REQUEST, format!("unparseable proxy: {s}")))?
    } else {
        let host = body
            .host
            .clone()
            .ok_or_else(|| err(StatusCode::BAD_REQUEST, "`proxy` string or host+port required"))?;
        let port = body
            .port
            .ok_or_else(|| err(StatusCode::BAD_REQUEST, "`port` required"))?;
        let kind = match body.kind.as_deref() {
            Some("http") => crate::proxy::ProxyKind::Http,
            Some("https") => crate::proxy::ProxyKind::Https,
            _ => crate::proxy::ProxyKind::Socks5,
        };
        crate::proxy::ProxyEntry {
            id: String::new(),
            name: String::new(),
            kind,
            host,
            port,
            username: body.username.clone().unwrap_or_default(),
            password: body.password.clone().unwrap_or_default(),
            country: String::new(),
            notes: String::new(),
        }
    };
    // metadata overrides (applied to parsed entries too).
    if let Some(n) = body.name.filter(|s| !s.is_empty()) {
        entry.name = n;
    }
    if let Some(c) = body.country {
        entry.country = c;
    }
    if let Some(nt) = body.notes {
        entry.notes = nt;
    }
    let stored = crate::proxy::upsert_dedup(entry)
        .map_err(|e| err(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    crate::notify_store_changed("proxies");
    Ok(Json(json!({
        "id": stored.id,
        "name": stored.name,
        "kind": stored.kind,
        "host": stored.host,
        "port": stored.port,
        "country": stored.country,
    })))
}

async fn delete_proxy(Path(id): Path<String>) -> ApiResult {
    crate::proxy::delete(&id).map_err(|e| err(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    crate::notify_store_changed("proxies");
    Ok(Json(json!({ "deleted": true, "id": id })))
}

async fn list_proxies() -> ApiResult {
    let list = crate::proxy::list().map_err(|e| err(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    // Credentials never exposed over API.
    let out: Vec<Value> = list
        .into_iter()
        .map(|p| {
            json!({
                "id": p.id,
                "name": p.name,
                "kind": p.kind,
                "host": p.host,
                "port": p.port,
                "country": p.country,
            })
        })
        .collect();
    Ok(Json(json!(out)))
}

// ---- extensions ----

fn extension_json(e: &crate::extensions::ExtensionEntry, with_icon: bool) -> Value {
    json!({
        "id": e.id,
        "name": e.name,
        "version": e.version,
        "description": e.description,
        "size_bytes": e.size_bytes,
        "added_at": e.added_at,
        // Data URLs run to hundreds of kilobytes each; a listing of thirty
        // extensions is not the place for them.
        "icon": with_icon.then(|| e.icon.clone()),
    })
}

async fn list_extensions() -> ApiResult {
    let list = crate::extensions::list()
        .map_err(|e| err(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    let out: Vec<Value> = list.iter().map(|e| extension_json(e, false)).collect();
    Ok(Json(json!(out)))
}

#[derive(Deserialize)]
struct AddExtensionReq {
    /// Web Store page, a bare extension id, or a direct .crx / .zip link.
    url: Option<String>,
    /// Local .crx / .zip file, or an unpacked folder, on this machine.
    path: Option<String>,
}

/// Add to the library. The response carries the icon, since the caller has
/// just asked for exactly this one.
async fn add_extension(Json(body): Json<AddExtensionReq>) -> ApiResult {
    let entry = match (body.url.as_deref(), body.path.as_deref()) {
        (Some(u), _) if !u.is_empty() => crate::extensions::import_url(u)
            .await
            .map_err(|e| err(StatusCode::BAD_REQUEST, format!("{e:#}")))?,
        (_, Some(p)) if !p.is_empty() => {
            crate::extensions::import(std::path::Path::new(p))
                .map_err(|e| err(StatusCode::BAD_REQUEST, format!("{e:#}")))?
        }
        _ => return Err(err(StatusCode::BAD_REQUEST, "`url` or `path` required")),
    };
    crate::notify_store_changed("extensions");
    Ok(Json(extension_json(&entry, true)))
}

async fn delete_extension(Path(id): Path<String>) -> ApiResult {
    crate::extensions::delete(&id)
        .map_err(|e| err(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    crate::notify_store_changed("extensions");
    Ok(Json(json!({ "deleted": true, "id": id })))
}

// ---- bookmarks ----

async fn list_bookmarks() -> ApiResult {
    let list = crate::bookmarks::list()
        .map_err(|e| err(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    Ok(Json(serde_json::to_value(list).unwrap_or(Value::Null)))
}

#[derive(Deserialize)]
struct BookmarkReq {
    /// Present = update that bookmark; absent = a new one.
    id: Option<String>,
    url: String,
    /// Blank uses the address itself.
    title: Option<String>,
    /// Launcher folder; "" (or absent) means every profile.
    folder: Option<String>,
}

/// Upsert. Profiles pick the change up on their next launch, not immediately —
/// the bookmarks file is written just before the browser reads it.
async fn save_bookmark(Json(body): Json<BookmarkReq>) -> ApiResult {
    let saved = crate::bookmarks::save(crate::bookmarks::Bookmark {
        id: body.id.unwrap_or_default(),
        title: body.title.unwrap_or_default(),
        url: body.url,
        folder: body.folder.unwrap_or_default(),
    })
    .map_err(|e| err(StatusCode::BAD_REQUEST, e.to_string()))?;
    crate::notify_store_changed("bookmarks");
    Ok(Json(serde_json::to_value(saved).unwrap_or(Value::Null)))
}

async fn delete_bookmark(Path(id): Path<String>) -> ApiResult {
    crate::bookmarks::delete(&id)
        .map_err(|e| err(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    crate::notify_store_changed("bookmarks");
    Ok(Json(json!({ "deleted": true, "id": id })))
}

// ---- trash ----

async fn list_trash() -> ApiResult {
    let list = crate::trash::list()
        .map_err(|e| err(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    Ok(Json(serde_json::to_value(list).unwrap_or(Value::Null)))
}

/// Puts the profile back under its own id, so anything that referenced it
/// still points at it.
async fn restore_trash(Path(id): Path<String>) -> ApiResult {
    let meta = crate::trash::restore(&id)
        .map_err(|e| err(StatusCode::NOT_FOUND, e.to_string()))?;
    crate::notify_store_changed("profiles");
    Ok(Json(serde_json::to_value(meta).unwrap_or(Value::Null)))
}

/// Deletes the archive for good. There is nothing after this.
async fn purge_trash(Path(id): Path<String>) -> ApiResult {
    crate::trash::purge(&id)
        .map_err(|e| err(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    crate::notify_store_changed("trash");
    Ok(Json(json!({ "purged": true, "id": id })))
}

async fn list_folders() -> ApiResult {
    let metas = crate::profile::list_all().map_err(|e| err(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    let mut set = std::collections::BTreeSet::new();
    for m in metas {
        if !m.folder.is_empty() {
            set.insert(m.folder);
        }
    }
    Ok(Json(json!(set.into_iter().collect::<Vec<_>>())))
}

/// Normalize platform string to library tag vocabulary.
fn normalize_platform(p: &str) -> String {
    match p.trim().to_lowercase().as_str() {
        "windows" | "win" => "Windows".into(),
        "linux" => "Linux".into(),
        "mac" | "macos" | "osx" | "darwin" => "macOS".into(),
        other => other.to_string(),
    }
}

/// Random fingerprint id for platform (host OS when None); falls back to all.
fn random_fingerprint_for(platform: Option<&str>) -> Result<String, ApiError> {
    let want = platform
        .map(normalize_platform)
        .unwrap_or_else(crate::host_platform);
    let all = crate::fingerprints::list_all()
        .map_err(|e| err(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    if all.is_empty() {
        return Err(err(StatusCode::NOT_FOUND, "fingerprint library is empty"));
    }
    let matching: Vec<crate::fingerprints::LibraryEntry> = all
        .iter()
        .filter(|e| e.platform.eq_ignore_ascii_case(&want))
        .cloned()
        .collect();
    let pool = if matching.is_empty() { all } else { matching };
    let idx = (uuid::Uuid::new_v4().as_bytes()[0] as usize) % pool.len();
    Ok(pool[idx].id.clone())
}

// ---- server ----

pub async fn serve(secret: String, port: u16) {
    set_secret(&secret);

    let protected = Router::new()
        .route("/profiles", get(list_profiles).post(create_profile))
        .route("/profiles/temporary", post(create_temporary))
        .route("/profiles/:id", get(get_profile).patch(edit_profile).delete(delete_profile))
        .route("/profiles/:id/start", post(start_profile))
        .route("/profiles/:id/stop", post(stop_profile))
        .route("/profiles/:id/cookies", get(export_cookies).post(import_cookies))
        .route("/folders", get(list_folders))
        .route("/folders/:folder", patch(rename_folder_ep).delete(delete_folder_ep))
        .route("/folders/:folder/profiles", post(create_profile_in_folder))
        .route("/fingerprint/new", get(new_fingerprint))
        .route("/fingerprint/new/:platform", get(new_fingerprint_for))
        .route("/fingerprints", get(list_fingerprints))
        .route("/running", get(list_running))
        .route("/proxies", get(list_proxies).post(add_proxy))
        .route("/proxies/:id", delete(delete_proxy))
        .route("/extensions", get(list_extensions).post(add_extension))
        .route("/extensions/:id", delete(delete_extension))
        .route("/bookmarks", get(list_bookmarks).post(save_bookmark))
        .route("/bookmarks/:id", delete(delete_bookmark))
        .route("/trash", get(list_trash))
        .route("/trash/:id", delete(purge_trash))
        .route("/trash/:id/restore", post(restore_trash))
        .route_layer(middleware::from_fn(auth));

    let app = Router::new()
        .route("/health", get(health))
        .merge(protected);

    let addr = std::net::SocketAddr::from(([127, 0, 0, 1], port));
    match tokio::net::TcpListener::bind(addr).await {
        Ok(listener) => {
            eprintln!("[launcher] automation API listening on http://{addr}");
            if let Err(e) = axum::serve(listener, app).await {
                eprintln!("[launcher] API server error: {e}");
            }
        }
        Err(e) => eprintln!("[launcher] API bind {addr} failed: {e}"),
    }
}
