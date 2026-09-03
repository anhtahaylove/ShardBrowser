//! Persistence for tenant root key grants.
//!
//! A grant is how a device obtains the tenant root key: the key sealed to that
//! device's HPKE public key, wrapped so the server stores it without being able
//! to open it. Until this module existed the grant endpoint verified a record
//! and then discarded it, so the table held nothing and a second device had no
//! way to obtain the key.
//!
//! Every value written here is read from the *signed* field map. A grant that
//! took its subject or its wrapped key from the request body would let a caller
//! present someone else's signature and redirect the custody it authorises —
//! the same mistake the upload commit path made before it was fixed.

use crate::authz::VerifiedRecord;
use crate::error::AppError;
use sqlx::{Row, SqlitePool};

/// The capability a root custody grant must carry.
const GRANT_CAPABILITY_ROOT_CUSTODY: &str = "root-custody";

/// A grant record's fields, all read from the signed container.
pub struct GrantRow {
    pub grant_variant: String,
    pub root_key_id: [u8; 32],
    pub root_generation: u64,
    pub grant_capability: String,
    pub subject_account_id: [u8; 16],
    pub subject_device_id: [u8; 16],
    pub subject_signing_key_id: [u8; 32],
    pub recipient_hpke_key_id: [u8; 32],
    pub subject_device_approval_replay_id: [u8; 16],
    pub hpke_suite_id: u64,
    pub hpke_mode_id: u64,
    pub hpke_kem_id: u64,
    pub hpke_kdf_id: u64,
    pub hpke_aead_id: u64,
    pub hpke_info_bytes: Vec<u8>,
    pub hpke_encapped_key_bytes: Vec<u8>,
    pub hpke_wrapped_trk_bytes: Vec<u8>,
}

/// Read every grant field out of the verified record.
///
/// A missing or wrong-typed field is a bad request rather than a panic: the
/// signature proves the issuer wrote these bytes, not that they wrote a record
/// this server version understands.
pub fn grant_row_from_record(record: &VerifiedRecord) -> Result<GrantRow, AppError> {
    fn missing(name: &str) -> AppError {
        AppError::BadRequest(format!("grant record: missing or invalid `{name}`"))
    }

    let grant_capability = record
        .signed_text("grant_capability")
        .ok_or_else(|| missing("grant_capability"))?;

    // A grant that does not claim root custody must not be stored as one.
    if grant_capability != GRANT_CAPABILITY_ROOT_CUSTODY {
        return Err(AppError::BadRequest(format!(
            "grant record: capability `{grant_capability}` is not root custody"
        )));
    }

    let grant_variant = record
        .signed_text("grant_variant")
        .ok_or_else(|| missing("grant_variant"))?;

    // The column has a CHECK constraint on these two values; rejecting here
    // names the problem instead of surfacing a constraint violation.
    if grant_variant != "FirstRootSelfGrant" && grant_variant != "CustodianIssued" {
        return Err(AppError::BadRequest(format!(
            "grant record: unknown grant_variant `{grant_variant}`"
        )));
    }

    Ok(GrantRow {
        grant_variant,
        root_key_id: record
            .signed_hash32("root_key_id")
            .ok_or_else(|| missing("root_key_id"))?,
        root_generation: record
            .signed_uint("root_generation")
            .ok_or_else(|| missing("root_generation"))?,
        grant_capability,
        subject_account_id: record
            .signed_id16("subject_account_id")
            .ok_or_else(|| missing("subject_account_id"))?,
        subject_device_id: record
            .signed_id16("subject_device_id")
            .ok_or_else(|| missing("subject_device_id"))?,
        subject_signing_key_id: record
            .signed_hash32("subject_signing_key_id")
            .ok_or_else(|| missing("subject_signing_key_id"))?,
        recipient_hpke_key_id: record
            .signed_hash32("recipient_hpke_key_id")
            .ok_or_else(|| missing("recipient_hpke_key_id"))?,
        subject_device_approval_replay_id: record
            .signed_id16("subject_device_approval_replay_id")
            .ok_or_else(|| missing("subject_device_approval_replay_id"))?,
        hpke_suite_id: record
            .signed_uint("hpke_suite_id")
            .ok_or_else(|| missing("hpke_suite_id"))?,
        hpke_mode_id: record
            .signed_uint("hpke_mode_id")
            .ok_or_else(|| missing("hpke_mode_id"))?,
        hpke_kem_id: record
            .signed_uint("hpke_kem_id")
            .ok_or_else(|| missing("hpke_kem_id"))?,
        hpke_kdf_id: record
            .signed_uint("hpke_kdf_id")
            .ok_or_else(|| missing("hpke_kdf_id"))?,
        hpke_aead_id: record
            .signed_uint("hpke_aead_id")
            .ok_or_else(|| missing("hpke_aead_id"))?,
        hpke_info_bytes: record
            .signed_bytes("hpke_info_bytes")
            .ok_or_else(|| missing("hpke_info_bytes"))?,
        hpke_encapped_key_bytes: record
            .signed_bytes("hpke_encapped_key_bytes")
            .ok_or_else(|| missing("hpke_encapped_key_bytes"))?,
        hpke_wrapped_trk_bytes: record
            .signed_bytes("hpke_wrapped_trk_bytes")
            .ok_or_else(|| missing("hpke_wrapped_trk_bytes"))?,
    })
}

/// Store a verified grant.
///
/// The subject device must exist in this tenant. The foreign key enforces that
/// too, but checking first turns a constraint violation naming a table and
/// column into a message an operator can act on.
#[allow(clippy::too_many_arguments)]
pub async fn insert_grant(
    pool: &SqlitePool,
    tenant_id: &[u8; 16],
    server_instance_id: &[u8; 16],
    restore_epoch: u64,
    record: &VerifiedRecord,
    exact_signed_container_bytes: &[u8],
    row: &GrantRow,
    now: &str,
) -> Result<(), AppError> {
    let device_exists =
        sqlx::query("SELECT 1 FROM v2_devices WHERE tenant_id = ? AND id = ? LIMIT 1")
            .bind(tenant_id.as_slice())
            .bind(row.subject_device_id.as_slice())
            .fetch_optional(pool)
            .await?
            .is_some();

    if !device_exists {
        return Err(AppError::BadRequest(
            "grant record: subject device is not enrolled in this tenant".into(),
        ));
    }

    sqlx::query(
        "INSERT INTO v2_tenant_root_key_grants (
             tenant_id, replay_id, payload_domain, grant_variant,
             root_key_id, root_generation, grant_capability,
             subject_account_id, subject_device_id, subject_signing_key_id,
             recipient_hpke_key_id, subject_device_approval_replay_id,
             hpke_suite_id, hpke_mode_id, hpke_kem_id, hpke_kdf_id, hpke_aead_id,
             hpke_info_bytes, hpke_encapped_key_bytes, hpke_wrapped_trk_bytes,
             server_instance_id, restore_epoch,
             signature_bytes, issuer_signing_key_id,
             signed_container_hash, exact_signed_container_bytes,
             exact_signed_container_bytes_sha256, created_at
         ) VALUES (
             ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?,
             ?, ?, ?, ?, ?, ?, ?, ?
         )",
    )
    .bind(tenant_id.as_slice())
    .bind(record.replay_id.as_slice())
    .bind(&record.domain)
    .bind(&row.grant_variant)
    .bind(row.root_key_id.as_slice())
    .bind(row.root_generation as i64)
    .bind(&row.grant_capability)
    .bind(row.subject_account_id.as_slice())
    .bind(row.subject_device_id.as_slice())
    .bind(row.subject_signing_key_id.as_slice())
    .bind(row.recipient_hpke_key_id.as_slice())
    .bind(row.subject_device_approval_replay_id.as_slice())
    .bind(row.hpke_suite_id as i64)
    .bind(row.hpke_mode_id as i64)
    .bind(row.hpke_kem_id as i64)
    .bind(row.hpke_kdf_id as i64)
    .bind(row.hpke_aead_id as i64)
    .bind(row.hpke_info_bytes.as_slice())
    .bind(row.hpke_encapped_key_bytes.as_slice())
    .bind(row.hpke_wrapped_trk_bytes.as_slice())
    .bind(server_instance_id.as_slice())
    .bind(restore_epoch as i64)
    .bind(record.signature_bytes.as_slice())
    .bind(record.issuer_signing_key_id.as_slice())
    .bind(record.signed_container_hash.as_slice())
    .bind(exact_signed_container_bytes)
    .bind(record.exact_bytes_sha256.as_slice())
    .bind(now)
    .execute(pool)
    .await?;

    Ok(())
}

/// A grant as handed back to a device that is collecting its root key.
pub struct StoredGrant {
    pub grant_variant: String,
    pub root_key_id: Vec<u8>,
    pub root_generation: i64,
    pub recipient_hpke_key_id: Vec<u8>,
    pub hpke_info_bytes: Vec<u8>,
    pub hpke_encapped_key_bytes: Vec<u8>,
    pub hpke_wrapped_trk_bytes: Vec<u8>,
    pub exact_signed_container_bytes: Vec<u8>,
    pub created_at: String,
}

/// Fetch the grants issued to one device, newest root generation first.
///
/// Revoked grants are excluded. Revocation blocks a device that has not yet
/// collected the key; it cannot retract a key already copied, which is why
/// excluding a device means rotating the generation rather than revoking alone.
pub async fn grants_for_device(
    pool: &SqlitePool,
    tenant_id: &[u8; 16],
    device_id: &[u8; 16],
) -> Result<Vec<StoredGrant>, AppError> {
    let rows = sqlx::query(
        "SELECT grant_variant, root_key_id, root_generation, recipient_hpke_key_id,
                hpke_info_bytes, hpke_encapped_key_bytes, hpke_wrapped_trk_bytes,
                exact_signed_container_bytes, created_at
           FROM v2_tenant_root_key_grants
          WHERE tenant_id = ? AND subject_device_id = ? AND revoked_at IS NULL
          ORDER BY root_generation DESC, created_at DESC",
    )
    .bind(tenant_id.as_slice())
    .bind(device_id.as_slice())
    .fetch_all(pool)
    .await?;

    Ok(rows
        .into_iter()
        .map(|r| StoredGrant {
            grant_variant: r.get("grant_variant"),
            root_key_id: r.get("root_key_id"),
            root_generation: r.get("root_generation"),
            recipient_hpke_key_id: r.get("recipient_hpke_key_id"),
            hpke_info_bytes: r.get("hpke_info_bytes"),
            hpke_encapped_key_bytes: r.get("hpke_encapped_key_bytes"),
            hpke_wrapped_trk_bytes: r.get("hpke_wrapped_trk_bytes"),
            exact_signed_container_bytes: r.get("exact_signed_container_bytes"),
            created_at: r.get("created_at"),
        })
        .collect())
}
