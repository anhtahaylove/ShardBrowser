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
use serde::{Deserialize, Serialize};
use serde_json::{json, Map, Value};

// ---- API listener status (actual runtime state, not just saved settings) ----

#[derive(Debug, Clone, Default, Serialize)]
pub struct ApiRuntimeStatus {
    /// Whether this process attempted to start the API from startup settings.
    pub enabled: bool,
    /// Port this process attempted to bind, when enabled.
    pub port: Option<u16>,
    /// True only after TcpListener::bind has succeeded and until serve exits.
    pub running: bool,
    /// Last bind/serve failure, safe to show in Settings.
    pub error: Option<String>,
}

fn runtime_status_cell() -> &'static RwLock<ApiRuntimeStatus> {
    static STATUS: OnceLock<RwLock<ApiRuntimeStatus>> = OnceLock::new();
    STATUS.get_or_init(|| RwLock::new(ApiRuntimeStatus::default()))
}

fn publish_runtime_status(status: ApiRuntimeStatus) {
    if let Ok(mut current) = runtime_status_cell().write() {
        *current = status;
    }
    crate::notify_store_changed("settings");
}

pub fn runtime_status() -> ApiRuntimeStatus {
    runtime_status_cell()
        .read()
        .map(|status| status.clone())
        .unwrap_or_default()
}

pub fn mark_disabled() {
    publish_runtime_status(ApiRuntimeStatus::default());
}

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

#[derive(Debug)]
struct ApiError(StatusCode, String);

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        (self.0, Json(json!({ "error": self.1 }))).into_response()
    }
}

fn err(code: StatusCode, msg: impl Into<String>) -> ApiError {
    ApiError(code, msg.into())
}

fn profile_api_error(error: anyhow::Error, fallback: StatusCode) -> ApiError {
    let status = match crate::profile::profile_error_kind(&error) {
        Some(crate::profile::ProfileErrorKind::Running | crate::profile::ProfileErrorKind::Busy) => {
            StatusCode::CONFLICT
        }
        Some(
            crate::profile::ProfileErrorKind::InvalidName
            | crate::profile::ProfileErrorKind::NameConflict,
        ) => StatusCode::BAD_REQUEST,
        None => fallback,
    };
    err(status, error.to_string())
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
        "capabilities": ["launch-instance-ownership-v1"],
    }))
}

#[derive(Debug, Deserialize)]
struct StartupConfigReq {
    enabled: bool,
    start_minimized: Option<bool>,
}

fn startup_app() -> Result<&'static tauri::AppHandle, ApiError> {
    crate::app_handle().ok_or_else(|| {
        err(
            StatusCode::SERVICE_UNAVAILABLE,
            "launcher startup manager is not initialized",
        )
    })
}

async fn get_startup() -> ApiResult {
    let status = crate::startup::status(startup_app()?)
        .map_err(|e| err(StatusCode::INTERNAL_SERVER_ERROR, e))?;
    Ok(Json(serde_json::to_value(status).unwrap_or(Value::Null)))
}

async fn configure_startup(Json(body): Json<StartupConfigReq>) -> ApiResult {
    let app = startup_app()?;
    let mut configured = crate::settings::load()
        .map_err(|e| err(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    configured.launch_at_login = body.enabled;
    if let Some(start_minimized) = body.start_minimized {
        configured.start_minimized = start_minimized;
    }
    crate::startup::configure(app, &configured)
        .map_err(|e| err(StatusCode::INTERNAL_SERVER_ERROR, e))?;
    crate::notify_store_changed("settings");

    let status = crate::startup::status(app)
        .map_err(|e| err(StatusCode::INTERNAL_SERVER_ERROR, e))?;
    Ok(Json(serde_json::to_value(status).unwrap_or(Value::Null)))
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
                "running": r.is_some(),
                "pid": r.map(|x| x.pid),
                "cdp": r.and_then(|x| x.cdp.clone()),
                "verification": r.and_then(|x| x.verification.clone()),
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
    fingerprint: Value,
    launch: Option<Value>,
    /// Reserved for backward-compatible error reporting; not implemented.
    custom_fonts: Option<Value>,
}

fn reject_unavailable_custom_fonts(
    cfg: &Map<String, Value>,
    custom_fonts: &Option<Value>,
) -> Result<(), String> {
    if custom_fonts.is_some() || cfg.contains_key("custom_fonts") {
        return Err(
            "custom fonts are not available: browser-engine coherence has not been verified"
                .into(),
        );
    }
    Ok(())
}

fn validated_api_fingerprint(
    fingerprint: &Value,
    custom_fonts: &Option<Value>,
) -> Result<Map<String, Value>, String> {
    let mut cfg = fingerprint
        .as_object()
        .cloned()
        .ok_or_else(|| "`fingerprint` must be an object".to_string())?;
    cfg.remove("_meta");
    reject_unavailable_custom_fonts(&cfg, custom_fonts)?;
    Ok(cfg)
}

/// Persist verbatim (enrich=false); proxy_id binds, proxy string upserts+tests.
async fn persist_created(folder_override: Option<String>, body: CreateReq) -> ApiResult {
    let _claim = crate::profile::begin_profile_creation("create a profile")
        .map_err(|error| profile_api_error(error, StatusCode::INTERNAL_SERVER_ERROR))?;
    let mut cfg = validated_api_fingerprint(&body.fingerprint, &body.custom_fonts)
        .map_err(|e| err(StatusCode::BAD_REQUEST, e))?;
    if let Some(n) = body.name.as_ref() {
        cfg.insert("name".into(), json!(n));
    }
    if let Some(n) = body.notes.as_ref() {
        cfg.insert("notes".into(), json!(n));
    }
    let normalized_name = crate::profile::validate_profile_name_for_mutation(
        cfg.get("name").and_then(Value::as_str).unwrap_or_default(),
        None,
    )
    .map_err(|error| profile_api_error(error, StatusCode::BAD_REQUEST))?;
    cfg.insert("name".into(), json!(normalized_name));
    apply_temp_object_override(&mut cfg, "launch", body.launch)
        .map_err(|e| err(StatusCode::BAD_REQUEST, e))?;

    let folder = folder_override.or(body.folder).unwrap_or_default();
    let mut meta = json!({ "id": "", "folder": folder });
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

    let pm = crate::persist_profile_core_claimed(
        crate::main_window().as_ref(),
        Value::Object(cfg),
        false,
    )
    .map_err(|error| profile_api_error(error, StatusCode::BAD_REQUEST))?;
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
    /// Optional per-vector noise override, e.g.
    /// `{ "canvas": { "enabled": true, "seed": 0 } }`.
    noise: Option<Value>,
    /// Optional safe launch customizations, e.g.
    /// `{ "args": ["--mute-audio"], "extension_dirs": ["C:\\ext"] }`.
    launch: Option<Value>,
    /// Reserved for backward-compatible error reporting; not implemented.
    custom_fonts: Option<Value>,
    name: Option<String>,
    folder: Option<String>,
}

fn apply_temp_object_override(
    cfg: &mut Map<String, Value>,
    key: &str,
    value: Option<Value>,
) -> Result<(), String> {
    let Some(value) = value else {
        return Ok(());
    };
    if !value.is_object() {
        return Err(format!("`{key}` must be an object"));
    }
    cfg.insert(key.into(), value);
    Ok(())
}

fn apply_temp_noise_override(
    cfg: &mut Map<String, Value>,
    noise: Option<Value>,
) -> Result<(), String> {
    apply_temp_object_override(cfg, "noise", noise)
}

#[cfg(test)]
mod temp_profile_tests {
    use super::{apply_temp_noise_override, apply_temp_object_override};
    use serde_json::{json, Map, Value};

    #[test]
    fn temporary_profile_accepts_noise_override() {
        let mut cfg = Map::<String, Value>::new();

        apply_temp_noise_override(
            &mut cfg,
            Some(json!({ "canvas": { "enabled": true, "seed": 0 } })),
        )
        .unwrap();

        assert_eq!(cfg["noise"]["canvas"]["enabled"].as_bool(), Some(true));
    }

    #[test]
    fn temporary_profile_rejects_non_object_noise() {
        let mut cfg = Map::<String, Value>::new();

        assert!(apply_temp_noise_override(&mut cfg, Some(json!(true))).is_err());
    }

    #[test]
    fn temporary_profile_accepts_launch_object() {
        let mut cfg = Map::<String, Value>::new();

        apply_temp_object_override(&mut cfg, "launch", Some(json!({ "args": ["--mute-audio"] })))
            .unwrap();

        assert_eq!(cfg["launch"]["args"][0].as_str(), Some("--mute-audio"));
    }

    #[test]
    fn profile_creation_rejects_unverified_custom_fonts() {
        let mut cfg = Map::<String, Value>::new();
        cfg.insert("custom_fonts".into(), json!({ "mode": "append" }));

        assert!(super::reject_unavailable_custom_fonts(&cfg, &None).is_err());
        assert!(super::reject_unavailable_custom_fonts(
            &Map::new(),
            &Some(json!({ "mode": "append" })),
        )
        .is_err());
    }

    #[test]
    fn api_fingerprint_validation_rejects_custom_fonts_on_edit() {
        let fingerprint = json!({
            "_meta": { "id": "fixture" },
            "custom_fonts": { "mode": "append" }
        });

        assert!(super::validated_api_fingerprint(&fingerprint, &None).is_err());
        let accepted = super::validated_api_fingerprint(&json!({ "name": "fixture" }), &None)
            .unwrap();
        assert!(!accepted.contains_key("_meta"));
    }

    #[test]
    fn profile_validation_errors_map_to_actionable_http_statuses() {
        let invalid = crate::profile::normalize_profile_name("bad/name").unwrap_err();
        assert_eq!(
            super::profile_api_error(invalid, axum::http::StatusCode::INTERNAL_SERVER_ERROR).0,
            axum::http::StatusCode::BAD_REQUEST
        );

        let claim = crate::profile::begin_user_mutation(
            ["api-profile-busy-status-test"],
            "edit this profile",
        )
        .unwrap();
        let busy = crate::profile::begin_user_mutation(
            ["api-profile-busy-status-test"],
            "edit this profile",
        )
        .unwrap_err();
        assert_eq!(
            super::profile_api_error(busy, axum::http::StatusCode::INTERNAL_SERVER_ERROR).0,
            axum::http::StatusCode::CONFLICT
        );
        drop(claim);
    }

    fn operation_block<'a>(spec: &'a str, start: &str, end: &str) -> &'a str {
        let start_index = spec.find(start).expect("OpenAPI operation start");
        let rest = &spec[start_index..];
        let end_index = rest.find(end).expect("OpenAPI operation end");
        &rest[..end_index]
    }

    #[test]
    fn openapi_documents_profile_validation_and_lifecycle_conflicts() {
        let spec = include_str!("../../openapi.yaml").replace("\r\n", "\n");
        for (start, end) in [
            ("  /profiles:\n", "  /profiles/temporary:\n"),
            ("  /profiles/temporary:\n", "  /profiles/{id}:\n"),
            ("  /profiles/{id}:\n", "  /profiles/{id}/start:\n"),
            ("  /profiles/{id}/start:\n", "  /profiles/{id}/stop:\n"),
            ("  /folders/{folder}:\n", "  /folders/{folder}/profiles:\n"),
            ("  /folders/{folder}/profiles:\n", "  /fingerprints:\n"),
        ] {
            let operation = operation_block(&spec, start, end);
            assert!(operation.contains("\"409\":"), "missing 409 for {start}");
        }

        for (start, end) in [
            ("  /profiles:\n", "  /profiles/temporary:\n"),
            ("  /profiles/temporary:\n", "  /profiles/{id}:\n"),
            ("  /profiles/{id}:\n", "  /profiles/{id}/start:\n"),
            ("  /folders/{folder}/profiles:\n", "  /fingerprints:\n"),
        ] {
            let operation = operation_block(&spec, start, end);
            assert!(operation.contains("\"400\":"), "missing 400 for {start}");
        }
    }

    #[test]
    fn openapi_documents_exact_launch_instance_ownership_without_exposing_running_tokens() {
        let spec = include_str!("../../openapi.yaml").replace("\r\n", "\n");
        let start = operation_block(
            &spec,
            "  /profiles/{id}/start:\n",
            "  /profiles/{id}/stop:\n",
        );
        assert!(start.contains("launch_instance_token:"));

        let health = operation_block(&spec, "  /health:\n", "  /startup:\n");
        assert!(health.contains("capabilities:"));
        assert!(health.contains("launch-instance-ownership-v1"));

        let pid_guard = operation_block(
            &spec,
            "  /profiles/{id}/stop-if-pid/{expected_pid}:\n",
            "  /profiles/{id}/stop-if-launch-instance:\n",
        );
        assert!(pid_guard.contains("legacy PID guard"));
        assert!(pid_guard.contains("cannot distinguish a"));
        assert!(pid_guard.contains("deprecated: true"));
        assert!(pid_guard.contains("\"410\":"));

        let launch_guard = operation_block(
            &spec,
            "  /profiles/{id}/stop-if-launch-instance:\n",
            "  /profiles/{id}/cookies:\n",
        );
        assert!(launch_guard.contains("requestBody:"));
        assert!(launch_guard.contains("required: [expected_pid, launch_instance_token]"));
        assert!(launch_guard.contains("\"400\":"));
        assert!(launch_guard.contains("\"409\":"));

        let running = operation_block(
            &spec,
            "    RunningProfile:\n",
            "    LibraryEntry:\n",
        );
        assert!(!running.contains("launch_instance_token"));
    }

    #[tokio::test(flavor = "current_thread")]
    async fn legacy_pid_stop_is_fail_closed() {
        let error = super::stop_profile_if_pid(axum::extract::Path(("profile-1".into(), 111)))
            .await
            .expect_err("legacy PID-only ownership must be disabled");

        assert_eq!(error.0, axum::http::StatusCode::GONE);
    }
}

/// Temporary profile (hidden, auto-deleted on close); pair with /start.
async fn create_temporary(Json(body): Json<TempReq>) -> ApiResult {
    let _claim = crate::profile::begin_profile_creation("create a temporary profile")
        .map_err(|error| profile_api_error(error, StatusCode::INTERNAL_SERVER_ERROR))?;
    let fid = match body.fingerprint_id {
        Some(f) => f,
        None => random_fingerprint_for(body.platform.as_deref())?,
    };
    let mut cfg = crate::build_fingerprint_config(crate::main_window().as_ref(), &fid)
        .map_err(|e| err(StatusCode::BAD_REQUEST, e))?;
    cfg.remove("_meta");
    reject_unavailable_custom_fonts(&cfg, &body.custom_fonts)
        .map_err(|e| err(StatusCode::BAD_REQUEST, e))?;
    if let Some(n) = body.name.as_ref() {
        cfg.insert("name".into(), json!(n));
    }
    let normalized_name = crate::profile::validate_profile_name_for_mutation(
        cfg.get("name").and_then(Value::as_str).unwrap_or_default(),
        None,
    )
    .map_err(|error| profile_api_error(error, StatusCode::BAD_REQUEST))?;
    cfg.insert("name".into(), json!(normalized_name));
    apply_temp_noise_override(&mut cfg, body.noise)
        .map_err(|e| err(StatusCode::BAD_REQUEST, e))?;
    apply_temp_object_override(&mut cfg, "launch", body.launch)
        .map_err(|e| err(StatusCode::BAD_REQUEST, e))?;
    let mut meta = json!({ "id": "", "folder": body.folder.unwrap_or_default(), "temporary": true });
    if let Some(pstr) = body.proxy.as_ref() {
        let entry = crate::proxy::parse_single(pstr)
            .ok_or_else(|| err(StatusCode::BAD_REQUEST, format!("unparseable proxy: {pstr}")))?;
        meta["inline_proxy"] = serde_json::to_value(entry).unwrap_or(Value::Null);
    }
    cfg.insert("_meta".into(), meta);

    let pm = crate::persist_profile_core_claimed(
        crate::main_window().as_ref(),
        Value::Object(cfg),
        false,
    )
    .map_err(|error| profile_api_error(error, StatusCode::BAD_REQUEST))?;
    Ok(Json(json!({
        "id": pm.id,
        "name": pm.name,
        "fingerprint_id": fid,
        "temporary": true,
        "proxy_inline": body.proxy.is_some(),
    })))
}

async fn delete_profile(Path(id): Path<String>) -> ApiResult {
    let _claim = crate::profile::begin_user_mutation([&id], "delete this profile")
        .map_err(|error| profile_api_error(error, StatusCode::INTERNAL_SERVER_ERROR))?;
    crate::profile::delete(&id)
        .map_err(|error| profile_api_error(error, StatusCode::INTERNAL_SERVER_ERROR))?;
    crate::notify_store_changed("profiles");
    Ok(Json(json!({ "deleted": true, "id": id })))
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
    /// Replace stored fingerprint verbatim.
    fingerprint: Option<Value>,
}

/// Edit profile; only provided fields change. Returns the updated profile.
async fn edit_profile(Path(id): Path<String>, Json(body): Json<EditReq>) -> ApiResult {
    let _claim = crate::profile::begin_user_mutation([&id], "modify this profile")
        .map_err(|error| profile_api_error(error, StatusCode::INTERNAL_SERVER_ERROR))?;
    let mut stored = crate::profile::load_raw(&id)
        .map_err(|e| err(StatusCode::NOT_FOUND, e.to_string()))?;

    if let Some(fp) = body.fingerprint {
        let cfg = validated_api_fingerprint(&fp, &None)
            .map_err(|e| err(StatusCode::BAD_REQUEST, e))?;
        stored.config = cfg;
    }
    if let Some(n) = body.name.as_ref() {
        stored.config.insert("name".into(), json!(n));
    }
    if let Some(n) = body.notes.as_ref() {
        stored.config.insert("notes".into(), json!(n));
    }
    crate::profile::prepare_profile_name_for_save(&mut stored)
        .map_err(|error| profile_api_error(error, StatusCode::INTERNAL_SERVER_ERROR))?;
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

    crate::profile::save_raw(&mut stored)
        .map_err(|error| profile_api_error(error, StatusCode::INTERNAL_SERVER_ERROR))?;
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
        .map_err(|error| profile_api_error(error, StatusCode::INTERNAL_SERVER_ERROR))?;
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
        .map_err(|error| profile_api_error(error, StatusCode::INTERNAL_SERVER_ERROR))?;
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
        .map_err(|error| profile_api_error(error, StatusCode::INTERNAL_SERVER_ERROR))?;
    Ok(Json(json!({
        "profile_id": id,
        "pid": outcome.pid,
        "launch_instance_token": outcome.launch_instance_token,
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

async fn stop_profile_if_pid(Path((_id, _expected_pid)): Path<(String, u32)>) -> ApiResult {
    Err(err(
        StatusCode::GONE,
        "legacy PID-only conditional stop is disabled; use stop-if-launch-instance",
    ))
}

#[derive(Deserialize)]
struct StopIfInstanceReq {
    expected_pid: u32,
    launch_instance_token: String,
}

async fn stop_profile_if_instance(
    Path(id): Path<String>,
    Json(body): Json<StopIfInstanceReq>,
) -> ApiResult {
    if body.expected_pid == 0 {
        return Err(err(StatusCode::BAD_REQUEST, "expected_pid must be positive"));
    }
    if uuid::Uuid::parse_str(&body.launch_instance_token).is_err() {
        return Err(err(
            StatusCode::BAD_REQUEST,
            "launch_instance_token must be a UUID",
        ));
    }

    let outcome = crate::process::Tracker::shared()
        .kill_if_instance(&id, body.expected_pid, &body.launch_instance_token)
        .await
        .map_err(|e| err(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    match outcome {
        crate::process::KillOutcome::Stopped { pid } => {
            Ok(Json(json!({ "profile_id": id, "stopped": true, "pid": pid })))
        }
        crate::process::KillOutcome::NotRunning => {
            Ok(Json(json!({ "profile_id": id, "stopped": false })))
        }
        crate::process::KillOutcome::PidMismatch {
            expected_pid,
            actual_pid,
        } => Err(err(
            StatusCode::CONFLICT,
            format!(
                "profile {id} process changed: expected pid {expected_pid}, running pid {actual_pid}"
            ),
        )),
        crate::process::KillOutcome::LaunchInstanceMismatch { pid } => Err(err(
            StatusCode::CONFLICT,
            format!("profile {id} launch instance changed while pid {pid} was reused"),
        )),
    }
}

#[derive(Deserialize)]
struct VerificationStatusReq {
    required: bool,
    kind: Option<String>,
}

fn verification_from_request(
    body: VerificationStatusReq,
) -> Result<Option<crate::process::VerificationStatus>, ApiError> {
    if !body.required {
        return Ok(None);
    }
    let kind = match body.kind.as_deref() {
        Some("interstitial") => "interstitial",
        Some("turnstile") => "turnstile",
        _ => {
            return Err(err(
                StatusCode::BAD_REQUEST,
                "kind must be interstitial or turnstile when verification is required",
            ));
        }
    };
    Ok(Some(crate::process::VerificationStatus {
        required: true,
        provider: "cloudflare".into(),
        kind: kind.into(),
        updated_at: unix_now(),
    }))
}

async fn report_verification_status(
    Path(id): Path<String>,
    Json(body): Json<VerificationStatusReq>,
) -> ApiResult {
    let verification = verification_from_request(body)?;

    if !crate::process::Tracker::shared().set_verification(&id, verification.clone()) {
        return Err(err(StatusCode::CONFLICT, "profile is not running"));
    }
    Ok(Json(json!({
        "profile_id": id,
        "verification": verification,
    })))
}

#[cfg(test)]
mod verification_status_tests {
    use super::{verification_from_request, VerificationStatusReq};
    use axum::http::StatusCode;

    #[test]
    fn clear_report_ignores_kind_and_returns_no_status() {
        let status = verification_from_request(VerificationStatusReq {
            required: false,
            kind: Some("unexpected".into()),
        })
        .unwrap();
        assert!(status.is_none());
    }

    #[test]
    fn required_report_accepts_only_known_cloudflare_kinds() {
        let status = verification_from_request(VerificationStatusReq {
            required: true,
            kind: Some("interstitial".into()),
        })
        .unwrap()
        .unwrap();
        assert!(status.required);
        assert_eq!(status.provider, "cloudflare");
        assert_eq!(status.kind, "interstitial");

        let error = verification_from_request(VerificationStatusReq {
            required: true,
            kind: Some("other".into()),
        })
        .unwrap_err();
        assert_eq!(error.0, StatusCode::BAD_REQUEST);
    }
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
    let _claim = crate::profile::begin_user_mutation([&id], "import cookies")
        .map_err(|error| profile_api_error(error, StatusCode::INTERNAL_SERVER_ERROR))?;
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

#[cfg(windows)]
fn disable_listener_inheritance(listener: &tokio::net::TcpListener) -> std::io::Result<()> {
    use std::os::windows::io::AsRawSocket;
    use windows_sys::Win32::Foundation::{SetHandleInformation, HANDLE, HANDLE_FLAG_INHERIT};

    let result =
        unsafe { SetHandleInformation(listener.as_raw_socket() as HANDLE, HANDLE_FLAG_INHERIT, 0) };
    if result == 0 {
        Err(std::io::Error::last_os_error())
    } else {
        Ok(())
    }
}

pub async fn serve(secret: String, port: u16) {
    set_secret(&secret);
    publish_runtime_status(ApiRuntimeStatus {
        enabled: true,
        port: Some(port),
        running: false,
        error: None,
    });

    let protected = Router::new()
        .route("/profiles", get(list_profiles).post(create_profile))
        .route("/profiles/temporary", post(create_temporary))
        .route("/profiles/:id", get(get_profile).patch(edit_profile).delete(delete_profile))
        .route("/profiles/:id/start", post(start_profile))
        .route("/profiles/:id/stop", post(stop_profile))
        .route(
            "/profiles/:id/stop-if-pid/:expected_pid",
            post(stop_profile_if_pid),
        )
        .route(
            "/profiles/:id/stop-if-launch-instance",
            post(stop_profile_if_instance),
        )
        .route(
            "/profiles/:id/verification-status",
            post(report_verification_status),
        )
        .route("/profiles/:id/cookies", get(export_cookies).post(import_cookies))
        .route("/folders", get(list_folders))
        .route("/folders/:folder", patch(rename_folder_ep).delete(delete_folder_ep))
        .route("/folders/:folder/profiles", post(create_profile_in_folder))
        .route("/fingerprint/new", get(new_fingerprint))
        .route("/fingerprint/new/:platform", get(new_fingerprint_for))
        .route("/fingerprints", get(list_fingerprints))
        .route("/running", get(list_running))
        .route("/startup", get(get_startup).put(configure_startup))
        .route("/proxies", get(list_proxies).post(add_proxy))
        .route("/proxies/:id", delete(delete_proxy))
        .route_layer(middleware::from_fn(auth));

    let app = Router::new()
        .route("/health", get(health))
        .merge(protected);

    let addr = std::net::SocketAddr::from(([127, 0, 0, 1], port));
    match tokio::net::TcpListener::bind(addr).await {
        Ok(listener) => {
            #[cfg(windows)]
            if let Err(e) = disable_listener_inheritance(&listener) {
                let message = format!("Could not disable API listener inheritance: {e}");
                publish_runtime_status(ApiRuntimeStatus {
                    enabled: true,
                    port: Some(port),
                    running: false,
                    error: Some(message.clone()),
                });
                eprintln!("[launcher] {message}");
                return;
            }
            publish_runtime_status(ApiRuntimeStatus {
                enabled: true,
                port: Some(port),
                running: true,
                error: None,
            });
            eprintln!("[launcher] automation API listening on http://{addr}");
            if let Err(e) = axum::serve(listener, app).await {
                let message = format!("API server stopped: {e}");
                publish_runtime_status(ApiRuntimeStatus {
                    enabled: true,
                    port: Some(port),
                    running: false,
                    error: Some(message.clone()),
                });
                eprintln!("[launcher] API server error: {e}");
            }
        }
        Err(e) => {
            let message = format!("Could not bind {addr}: {e}");
            publish_runtime_status(ApiRuntimeStatus {
                enabled: true,
                port: Some(port),
                running: false,
                error: Some(message),
            });
            eprintln!("[launcher] API bind {addr} failed: {e}");
        }
    }
}

#[cfg(all(test, windows))]
mod listener_handle_tests {
    use super::disable_listener_inheritance;
    use std::os::windows::io::AsRawSocket;
    use windows_sys::Win32::Foundation::{
        GetHandleInformation, SetHandleInformation, HANDLE, HANDLE_FLAG_INHERIT,
    };

    #[tokio::test]
    async fn listener_handle_is_not_inheritable() {
        let listener = tokio::net::TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, 0))
            .await
            .expect("bind test listener");
        let handle = listener.as_raw_socket() as HANDLE;

        let marked =
            unsafe { SetHandleInformation(handle, HANDLE_FLAG_INHERIT, HANDLE_FLAG_INHERIT) };
        assert_ne!(marked, 0, "mark listener inheritable for regression test");

        disable_listener_inheritance(&listener).expect("disable listener inheritance");

        let mut flags = 0;
        let read = unsafe { GetHandleInformation(handle, &mut flags) };
        assert_ne!(read, 0, "read listener handle flags");
        assert_eq!(flags & HANDLE_FLAG_INHERIT, 0);
    }
}
