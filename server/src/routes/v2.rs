//! v2 team/fleet endpoints.
//!
//! These carry their own authorization: a v1 session token says who is
//! logged in, but a v2 operation additionally requires a signed record that
//! binds the action to a tenant, a server instance and a restore epoch. The
//! session and the record are checked independently — neither substitutes
//! for the other.

use axum::extract::State;
use axum::http::{HeaderMap, StatusCode};
use axum::response::IntoResponse;
use serde::Deserialize;
use sqlx::Row;

use crate::auth::AuthUser;
use crate::authz::{self, VerificationContext};
use crate::error::AppError;
use crate::idempotency::{self, OperationClaim, ReplayClaim, ReplayTable};
use crate::state::AppState;
use crate::util;

/// Hex-decode a fixed-width id from a request field.
fn parse_id16(hex: &str, field: &'static str) -> Result<[u8; 16], AppError> {
    let bytes = decode_hex(hex).ok_or_else(|| AppError::BadRequest(format!("{field}: not hex")))?;
    bytes
        .try_into()
        .map_err(|_| AppError::BadRequest(format!("{field}: must be 16 bytes")))
}

fn decode_hex(s: &str) -> Option<Vec<u8>> {
    if !s.len().is_multiple_of(2) {
        return None;
    }
    (0..s.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&s[i..i + 2], 16).ok())
        .collect()
}

/// Load the server-side facts a signed record is checked against.
///
/// Read from the database on every request rather than cached: the restore
/// epoch changes exactly when a restore happens, and a cached epoch would
/// keep honouring pre-restore records for the lifetime of the cache.
async fn verification_context(
    app: &AppState,
    tenant_id: [u8; 16],
    now_ms: u64,
) -> Result<VerificationContext, AppError> {
    // `v2_server_state` is a singleton: the instance id and restore epoch
    // describe the deployment, not a tenant. Reading it per request keeps the
    // epoch honest — a cached value would keep honouring pre-restore records.
    let row = sqlx::query(
        "SELECT server_instance_id, restore_epoch FROM v2_server_state WHERE singleton = 1",
    )
    .fetch_optional(&app.db)
    .await?
    .ok_or_else(|| AppError::BadRequest("server identity is not initialised".into()))?;

    let instance: Vec<u8> = row.get("server_instance_id");
    let epoch: i64 = row.get("restore_epoch");

    // Only keys this tenant currently trusts. A revoked issuer disappears
    // from this set, so its records stop verifying without any separate
    // revocation check on the hot path.
    let issuer_rows = sqlx::query(
        "SELECT signing_key_id, public_key FROM v2_tenant_issuers
          WHERE tenant_id = ? AND revoked_at IS NULL",
    )
    .bind(tenant_id.as_slice())
    .fetch_all(&app.db)
    .await?;

    let mut trusted = std::collections::HashMap::new();
    for r in issuer_rows {
        let id: Vec<u8> = r.get("signing_key_id");
        let pk: Vec<u8> = r.get("public_key");
        if let (Ok(id), Ok(pk)) = (<[u8; 32]>::try_from(id), <[u8; 32]>::try_from(pk)) {
            trusted.insert(id, pk);
        }
    }

    Ok(VerificationContext {
        tenant_id,
        server_instance_id: instance
            .try_into()
            .map_err(|_| AppError::BadRequest("corrupt server_instance_id".into()))?,
        restore_epoch: epoch as u64,
        now_ms,
        trusted_issuers: trusted,
    })
}

#[derive(Deserialize)]
pub struct PresentRecordReq {
    /// Tenant this operation targets, hex-encoded.
    pub tenant_id: String,
    /// Hex-encoded exact signed container bytes, verified as received.
    pub record_hex: String,
}

/// Present a signed device approval.
///
/// Verification and consumption are separate steps on purpose: verification
/// is a pure function of bytes and context, while consumption mutates state
/// and can lose a race. Both must succeed.
pub async fn present_device_approval(
    State(app): State<AppState>,
    user: AuthUser,
    headers: HeaderMap,
    axum::Json(req): axum::Json<PresentRecordReq>,
) -> Result<impl IntoResponse, AppError> {
    let tenant_id = parse_id16(&req.tenant_id, "tenant_id")?;
    let record_bytes = decode_hex(&req.record_hex)
        .ok_or_else(|| AppError::BadRequest("record_hex: not hex".into()))?;

    let now_ms = chrono::Utc::now().timestamp_millis().max(0) as u64;
    let ctx = verification_context(&app, tenant_id, now_ms).await?;

    let verified = authz::verify_record(&record_bytes, authz::DOMAIN_DEVICE_APPROVAL, &ctx)
        .map_err(|e| AppError::BadRequest(e.to_string()))?;

    // Consume after verification: an invalid record must not burn a replay id,
    // or an attacker could disable a legitimate record by submitting a
    // corrupted copy of it first.
    match idempotency::consume_replay_id(
        &app.db,
        &tenant_id,
        ReplayTable::DeviceApprovals,
        &verified,
        &util::now_rfc3339(),
    )
    .await?
    {
        ReplayClaim::Fresh => {}
        ReplayClaim::AlreadyUsed => {
            return Err(AppError::Conflict(
                "authorization record already used".into(),
            ))
        }
    }

    crate::audit::log(
        &app.db,
        Some(&user.id),
        "v2.device_approval.accepted",
        None,
        &hex(&verified.signed_container_hash),
    )
    .await;

    let _ = headers;
    Ok((
        StatusCode::CREATED,
        axum::Json(serde_json::json!({
            "accepted": true,
            "signed_container_hash": hex(&verified.signed_container_hash),
        })),
    ))
}

#[derive(Deserialize)]
pub struct CompleteOpReq {
    pub tenant_id: String,
    pub idempotency_key: String,
    pub status_code: u16,
    /// Hex-encoded exact response bytes to replay on retry.
    pub response_hex: String,
}

/// Record an operation's outcome so subsequent retries replay it verbatim.
pub async fn complete_idempotent_operation(
    State(app): State<AppState>,
    _user: AuthUser,
    axum::Json(req): axum::Json<CompleteOpReq>,
) -> Result<impl IntoResponse, AppError> {
    let tenant_id = parse_id16(&req.tenant_id, "tenant_id")?;
    let key = parse_id16(&req.idempotency_key, "idempotency_key")?;
    let body = decode_hex(&req.response_hex)
        .ok_or_else(|| AppError::BadRequest("response_hex: not hex".into()))?;

    let now_ms = chrono::Utc::now().timestamp_millis().max(0) as u64;
    let ctx = verification_context(&app, tenant_id, now_ms).await?;

    idempotency::complete_operation(
        &app.db,
        &tenant_id,
        &key,
        &ctx.server_instance_id,
        ctx.restore_epoch,
        req.status_code,
        &body,
        &util::now_rfc3339(),
    )
    .await?;

    Ok((
        StatusCode::OK,
        axum::Json(serde_json::json!({ "recorded": true })),
    ))
}

/// Present a signed capability grant.
///
/// Same two-step shape as a device approval — verify, then consume — but
/// under its own domain, so a grant and an approval can never be substituted
/// for one another even if they share a replay id.
pub async fn present_capability_grant(
    State(app): State<AppState>,
    user: AuthUser,
    axum::Json(req): axum::Json<PresentRecordReq>,
) -> Result<impl IntoResponse, AppError> {
    let tenant_id = parse_id16(&req.tenant_id, "tenant_id")?;
    let record_bytes = decode_hex(&req.record_hex)
        .ok_or_else(|| AppError::BadRequest("record_hex: not hex".into()))?;

    let now_ms = chrono::Utc::now().timestamp_millis().max(0) as u64;
    let ctx = verification_context(&app, tenant_id, now_ms).await?;

    let verified = authz::verify_record(&record_bytes, authz::DOMAIN_CAPABILITY_GRANT, &ctx)
        .map_err(|e| AppError::BadRequest(e.to_string()))?;

    match idempotency::consume_replay_id(
        &app.db,
        &tenant_id,
        ReplayTable::CapabilityGrants,
        &verified,
        &util::now_rfc3339(),
    )
    .await?
    {
        ReplayClaim::Fresh => {}
        ReplayClaim::AlreadyUsed => {
            return Err(AppError::Conflict(
                "authorization record already used".into(),
            ))
        }
    }

    crate::audit::log(
        &app.db,
        Some(&user.id),
        "v2.capability_grant.accepted",
        None,
        &hex(&verified.signed_container_hash),
    )
    .await;

    Ok((
        StatusCode::CREATED,
        axum::Json(serde_json::json!({
            "accepted": true,
            "signed_container_hash": hex(&verified.signed_container_hash),
        })),
    ))
}

/// Present a signed tenant root key grant.
///
/// The server verifies and files the grant but cannot read it: the payload is
/// HPKE-sealed to the recipient device's public key, so only that device can
/// recover the tenant root key. Storing it here is a delivery mechanism, not
/// an escrow.
pub async fn present_tenant_root_key_grant(
    State(app): State<AppState>,
    user: AuthUser,
    axum::Json(req): axum::Json<PresentRecordReq>,
) -> Result<impl IntoResponse, AppError> {
    let tenant_id = parse_id16(&req.tenant_id, "tenant_id")?;
    let record_bytes = decode_hex(&req.record_hex)
        .ok_or_else(|| AppError::BadRequest("record_hex: not hex".into()))?;

    let now_ms = chrono::Utc::now().timestamp_millis().max(0) as u64;
    let ctx = verification_context(&app, tenant_id, now_ms).await?;

    let verified = authz::verify_record(&record_bytes, authz::DOMAIN_TENANT_ROOT_KEY_GRANT, &ctx)
        .map_err(|e| AppError::BadRequest(e.to_string()))?;

    match idempotency::consume_replay_id(
        &app.db,
        &tenant_id,
        ReplayTable::TenantRootKeyGrants,
        &verified,
        &util::now_rfc3339(),
    )
    .await?
    {
        ReplayClaim::Fresh => {}
        ReplayClaim::AlreadyUsed => {
            return Err(AppError::Conflict(
                "authorization record already used".into(),
            ))
        }
    }

    crate::audit::log(
        &app.db,
        Some(&user.id),
        "v2.tenant_root_key_grant.accepted",
        None,
        &hex(&verified.signed_container_hash),
    )
    .await;

    Ok((
        StatusCode::CREATED,
        axum::Json(serde_json::json!({
            "accepted": true,
            "signed_container_hash": hex(&verified.signed_container_hash),
        })),
    ))
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

#[derive(Deserialize)]
pub struct IdempotentOpReq {
    pub tenant_id: String,
    pub idempotency_key: String,
    pub operation_kind: String,
    /// Opaque request body the operation acts on, hex-encoded.
    pub payload_hex: String,
}

/// Begin an idempotent operation, replaying a prior response when the same
/// key and body are retried.
pub async fn begin_idempotent_operation(
    State(app): State<AppState>,
    user: AuthUser,
    axum::Json(req): axum::Json<IdempotentOpReq>,
) -> Result<impl IntoResponse, AppError> {
    let tenant_id = parse_id16(&req.tenant_id, "tenant_id")?;
    let key = parse_id16(&req.idempotency_key, "idempotency_key")?;
    let payload = decode_hex(&req.payload_hex)
        .ok_or_else(|| AppError::BadRequest("payload_hex: not hex".into()))?;

    let now_ms = chrono::Utc::now().timestamp_millis().max(0) as u64;
    let ctx = verification_context(&app, tenant_id, now_ms).await?;

    // The request digest is what distinguishes a retry from a key collision.
    let request_sha = shared::canonical::sha256(&payload);

    // Account and device are derived from the authenticated session, never
    // from the request body: a caller must not be able to attribute an
    // operation to someone else.
    let account_id = account_id_for(&app, tenant_id, &user.id).await?;

    match idempotency::begin_operation(
        &app.db,
        &tenant_id,
        &key,
        &ctx.server_instance_id,
        ctx.restore_epoch,
        &account_id,
        &account_id,
        &req.operation_kind,
        &request_sha,
        &util::now_rfc3339(),
    )
    .await?
    {
        OperationClaim::Started => Ok((
            StatusCode::ACCEPTED,
            axum::Json(serde_json::json!({ "status": "started" })),
        )),
        OperationClaim::Completed {
            status_code,
            exact_response_bytes,
        } => Ok((
            StatusCode::from_u16(status_code).unwrap_or(StatusCode::OK),
            axum::Json(serde_json::json!({
                "status": "replayed",
                "response_hex": hex(&exact_response_bytes),
            })),
        )),
        OperationClaim::InFlight => Err(AppError::Conflict("operation already in flight".into())),
        OperationClaim::KeyReusedWithDifferentRequest => Err(AppError::Conflict(
            "idempotency key reused with a different request".into(),
        )),
    }
}

/// Map a v1 session user to a v2 account within a tenant.
async fn account_id_for(
    app: &AppState,
    tenant_id: [u8; 16],
    user_id: &str,
) -> Result<[u8; 16], AppError> {
    let row = sqlx::query("SELECT id FROM v2_accounts WHERE tenant_id = ? AND legacy_user_id = ?")
        .bind(tenant_id.as_slice())
        .bind(user_id)
        .fetch_optional(&app.db)
        .await?
        .ok_or(AppError::Forbidden)?;

    let id: Vec<u8> = row.get("id");
    id.try_into()
        .map_err(|_| AppError::BadRequest("corrupt account id".into()))
}
