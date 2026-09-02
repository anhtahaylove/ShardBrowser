//! End-to-end checks of the v2 control-plane endpoints.
//!
//! These drive the real server binary over HTTP. Unit tests already cover the
//! verification and replay logic in isolation; what they cannot show is that
//! the wiring in front of that logic is correct — that the route actually
//! calls the verifier, that a rejection becomes a non-2xx response, and that
//! a tenant boundary holds when the request arrives over the network rather
//! than as a direct function call.

use std::process::{Child, Command};
use std::time::Duration;

use serde_json::{json, Value};
use sqlx::sqlite::SqlitePoolOptions;
use sqlx::Executor;

use shared::canonical as c;
use shared::signing::{build_signed_container, identity_key_id, Ed25519SigningKey};

fn base(port: u16) -> String {
    format!("http://127.0.0.1:{port}")
}

struct ServerGuard(Child);
impl Drop for ServerGuard {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

fn client() -> reqwest::Client {
    reqwest::Client::builder()
        .no_proxy()
        .timeout(Duration::from_secs(30))
        .build()
        .unwrap()
}

async fn wait_health(c: &reqwest::Client, port: u16) {
    for _ in 0..60 {
        if let Ok(r) = c.get(format!("{}/health", base(port))).send().await {
            if r.status().is_success() {
                return;
            }
        }
        tokio::time::sleep(Duration::from_millis(200)).await;
    }
    panic!("server did not become healthy on port {port}");
}

fn spawn_server(data: &std::path::Path, port: u16) -> ServerGuard {
    let _ = std::fs::remove_dir_all(data);
    let bin = env!("CARGO_BIN_EXE_shardx-team-server");
    let child = Command::new(bin)
        .env("SHARDX_BIND", format!("127.0.0.1:{port}"))
        .env("SHARDX_DATA_DIR", data)
        .env("SHARDX_TOKEN_SECRET", "e2e-v2-secret")
        .env("SHARDX_ADMIN_USER", "admin")
        .env("SHARDX_ADMIN_PASS", "secret")
        .spawn()
        .expect("spawn server binary");
    ServerGuard(child)
}

async fn token(c: &reqwest::Client, port: u16, user: &str, pass: &str) -> String {
    let v: Value = c
        .post(format!("{}/auth/login", base(port)))
        .json(&json!({ "username": user, "password": pass }))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    v["token"].as_str().unwrap().to_string()
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

const INSTANCE: [u8; 16] = [7u8; 16];
const TENANT_A: [u8; 16] = [0xAAu8; 16];
const TENANT_B: [u8; 16] = [0xBBu8; 16];

/// Seed the v2 tables the endpoints read: server identity, two tenants, an
/// issuer per tenant, and a v2 account linked to the v1 admin user.
///
/// Done directly against the database because there is no provisioning API
/// yet; the point of these tests is the request path, not provisioning.
async fn seed(
    data: &std::path::Path,
    user_id: &str,
    key_a: &Ed25519SigningKey,
    key_b: &Ed25519SigningKey,
) {
    let db = data.join("shardx.db");
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect(&format!(
            "sqlite://{}",
            db.to_string_lossy().replace('\\', "/")
        ))
        .await
        .expect("open server db");

    pool.execute("PRAGMA foreign_keys = ON").await.unwrap();

    sqlx::query(
        "INSERT OR REPLACE INTO v2_server_state
             (singleton, server_instance_id, restore_epoch, external_record_sha256, updated_at)
         VALUES (1, ?, 0, ?, '2026-01-01T00:00:00Z')",
    )
    .bind(INSTANCE.as_slice())
    .bind([0u8; 32].as_slice())
    .execute(&pool)
    .await
    .unwrap();

    for (tid, sk) in [(TENANT_A, key_a), (TENANT_B, key_b)] {
        // The issuer is keyed by its derived identity id — the same value the
        // record carries — while `public_key` holds the raw verifying key the
        // signature is actually checked against.
        let vk = sk.verifying_key().to_bytes();
        let key_id = identity_key_id(&sk.verifying_key());
        sqlx::query(
            "INSERT INTO v2_tenants (id, slug, status, active_root_generation, created_at)
             VALUES (?, ?, 'active', 1, '2026-01-01T00:00:00Z')",
        )
        .bind(tid.as_slice())
        .bind(format!("tenant-{}", hex(&tid[..1])))
        .execute(&pool)
        .await
        .unwrap();

        sqlx::query(
            "INSERT INTO v2_tenant_issuers
                 (tenant_id, signing_key_id, public_key, added_at)
             VALUES (?, ?, ?, '2026-01-01T00:00:00Z')",
        )
        .bind(tid.as_slice())
        .bind(key_id.as_slice())
        .bind(vk.as_slice())
        .execute(&pool)
        .await
        .unwrap();

        sqlx::query(
            "INSERT INTO v2_accounts
                 (id, tenant_id, username, pw_hash, legacy_user_id, status, created_at)
             VALUES (?, ?, 'admin', 'x', ?, 'active', '2026-01-01T00:00:00Z')",
        )
        .bind([1u8; 16].as_slice())
        .bind(tid.as_slice())
        .bind(user_id)
        .execute(&pool)
        .await
        .unwrap();
    }

    pool.close().await;
}

/// Build a signed device-approval record for a tenant.
fn approval_record(
    sk: &Ed25519SigningKey,
    tenant: [u8; 16],
    replay_id: [u8; 16],
    domain: &str,
) -> Vec<u8> {
    let fields = vec![
        ("container_domain", c::Value::Text(domain.to_string())),
        ("container_version", c::Value::Uint(1)),
        ("tenant_id", c::Value::Bytes(tenant.to_vec())),
        ("replay_id", c::Value::Bytes(replay_id.to_vec())),
        ("subject_account_id", c::Value::Bytes(vec![3u8; 16])),
        ("subject_device_id", c::Value::Bytes(vec![4u8; 16])),
        ("approved_use", c::Value::Text("profile.sync".to_string())),
        ("issued_at_ms", c::Value::Uint(0)),
        ("not_before_ms", c::Value::Uint(0)),
        // Far enough out that wall-clock time in CI cannot expire it, but
        // still a real bounded window rather than "never expires".
        ("not_after_ms", c::Value::Uint(4_102_444_800_000)),
        ("server_instance_id", c::Value::Bytes(INSTANCE.to_vec())),
        ("restore_epoch", c::Value::Uint(0)),
        (
            "issuer_signing_key_id",
            c::Value::Bytes(identity_key_id(&sk.verifying_key()).to_vec()),
        ),
    ];
    build_signed_container(sk, fields).exact_bytes
}

async fn admin_user_id(c: &reqwest::Client, port: u16, tok: &str) -> String {
    let v: Value = c
        .get(format!("{}/users", base(port)))
        .bearer_auth(tok)
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    v.as_array()
        .unwrap()
        .iter()
        .find(|u| u["username"] == "admin")
        .expect("admin user")["id"]
        .as_str()
        .unwrap()
        .to_string()
}

#[tokio::test]
async fn v2_authorization_holds_over_http() {
    let port = 38111u16;
    let data = std::env::temp_dir().join(format!("shardx-e2e-v2-{}", std::process::id()));
    let _guard = spawn_server(&data, port);
    let cl = client();
    wait_health(&cl, port).await;

    let admin = token(&cl, port, "admin", "secret").await;
    let user_id = admin_user_id(&cl, port, &admin).await;

    let sk_a = Ed25519SigningKey::from_bytes(&[11u8; 32]);
    let sk_b = Ed25519SigningKey::from_bytes(&[22u8; 32]);
    seed(&data, &user_id, &sk_a, &sk_b).await;

    let present = |body: Value| {
        let cl = cl.clone();
        let admin = admin.clone();
        async move {
            cl.post(format!("{}/v2/device-approvals", base(port)))
                .bearer_auth(&admin)
                .json(&body)
                .send()
                .await
                .unwrap()
        }
    };

    // A well-formed record for tenant A is accepted.
    let rec = approval_record(
        &sk_a,
        TENANT_A,
        [1u8; 16],
        "shardx.authorization.device-approval.v2",
    );
    let r = present(json!({
        "tenant_id": hex(&TENANT_A),
        "record_hex": hex(&rec),
    }))
    .await;
    let status = r.status();
    let body = r.text().await.unwrap_or_default();
    assert_eq!(status, 201, "valid record should be accepted: {body}");

    // The same record a second time is a replay: refused, not silently re-run.
    let r = present(json!({
        "tenant_id": hex(&TENANT_A),
        "record_hex": hex(&rec),
    }))
    .await;
    assert_eq!(r.status(), 409, "replayed record must be refused");

    // Tenant isolation over HTTP: a record signed by tenant A's issuer,
    // presented against tenant B, must fail. B does not trust that key.
    let cross = approval_record(
        &sk_a,
        TENANT_B,
        [2u8; 16],
        "shardx.authorization.device-approval.v2",
    );
    let r = present(json!({
        "tenant_id": hex(&TENANT_B),
        "record_hex": hex(&cross),
    }))
    .await;
    assert!(
        !r.status().is_success(),
        "a record signed by another tenant's issuer must not verify, got {}",
        r.status()
    );

    // Tenant confusion, the case a trusted-issuer check alone cannot catch:
    // a record that binds tenant A, presented as tenant B, signed by B's own
    // issuer. The issuer IS trusted for B, the signature IS valid, and the
    // domain IS right — only the tenant binding separates the two. Without
    // it, B could replay A's authorizations under its own key.
    let confused = approval_record(
        &sk_b,
        TENANT_A,
        [7u8; 16],
        "shardx.authorization.device-approval.v2",
    );
    let r = present(json!({
        "tenant_id": hex(&TENANT_B),
        "record_hex": hex(&confused),
    }))
    .await;
    assert!(
        !r.status().is_success(),
        "a record bound to another tenant must not verify even when signed by \
         a trusted issuer of the presenting tenant, got {}",
        r.status()
    );

    // Domain confusion: a capability grant presented to the device-approval
    // endpoint must be refused even though it is correctly signed.
    let wrong_domain = approval_record(
        &sk_a,
        TENANT_A,
        [3u8; 16],
        "shardx.authorization.capability-grant.v2",
    );
    let r = present(json!({
        "tenant_id": hex(&TENANT_A),
        "record_hex": hex(&wrong_domain),
    }))
    .await;
    assert!(
        !r.status().is_success(),
        "a record from another domain must not be accepted, got {}",
        r.status()
    );

    // A single flipped byte in the signed container must not verify.
    let mut tampered = approval_record(
        &sk_a,
        TENANT_A,
        [4u8; 16],
        "shardx.authorization.device-approval.v2",
    );
    let last = tampered.len() - 1;
    tampered[last] ^= 0x01;
    let r = present(json!({
        "tenant_id": hex(&TENANT_A),
        "record_hex": hex(&tampered),
    }))
    .await;
    assert!(
        !r.status().is_success(),
        "a tampered record must not verify, got {}",
        r.status()
    );

    // Unauthenticated requests never reach the verifier.
    let r = cl
        .post(format!("{}/v2/device-approvals", base(port)))
        .json(&json!({ "tenant_id": hex(&TENANT_A), "record_hex": hex(&rec) }))
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), 401, "v2 endpoints require a session");

    let _ = sk_b;
}

#[tokio::test]
async fn v2_operations_replay_the_stored_response() {
    let port = 38112u16;
    let data = std::env::temp_dir().join(format!("shardx-e2e-v2op-{}", std::process::id()));
    let _guard = spawn_server(&data, port);
    let cl = client();
    wait_health(&cl, port).await;

    let admin = token(&cl, port, "admin", "secret").await;
    let user_id = admin_user_id(&cl, port, &admin).await;

    let sk_a = Ed25519SigningKey::from_bytes(&[11u8; 32]);
    let sk_b = Ed25519SigningKey::from_bytes(&[22u8; 32]);
    seed(&data, &user_id, &sk_a, &sk_b).await;

    let begin = |key: &str, payload: &str| {
        let cl = cl.clone();
        let admin = admin.clone();
        let body = json!({
            "tenant_id": hex(&TENANT_A),
            "idempotency_key": key,
            "operation_kind": "profile.push",
            "payload_hex": payload,
        });
        async move {
            cl.post(format!("{}/v2/operations", base(port)))
                .bearer_auth(&admin)
                .json(&body)
                .send()
                .await
                .unwrap()
        }
    };

    let key = hex(&[9u8; 16]);

    // First claim starts the operation.
    let r = begin(&key, "aabb").await;
    assert_eq!(r.status(), 202, "first claim should start the operation");

    // Same key while in flight is a conflict, not a second execution.
    let r = begin(&key, "aabb").await;
    assert_eq!(r.status(), 409, "in-flight duplicate must conflict");

    // Record the outcome, then retry: the stored response comes back verbatim.
    let r = cl
        .post(format!("{}/v2/operations/complete", base(port)))
        .bearer_auth(&admin)
        .json(&json!({
            "tenant_id": hex(&TENANT_A),
            "idempotency_key": key,
            "status_code": 200,
            "response_hex": "cafe",
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), 200, "completion should be recorded");

    let r = begin(&key, "aabb").await;
    assert_eq!(
        r.status(),
        200,
        "a completed operation replays its response"
    );
    let v: Value = r.json().await.unwrap();
    assert_eq!(v["status"], "replayed");
    assert_eq!(
        v["response_hex"], "cafe",
        "the replayed body must be the exact stored bytes"
    );

    // Same key, different request body: refused rather than answering a
    // question the caller did not ask.
    let r = begin(&key, "ccdd").await;
    assert_eq!(
        r.status(),
        409,
        "key reuse with a different request must conflict"
    );
}

/// A client needs the deployment identity before it can sign anything the
/// server will accept, so the endpoint must report the same values the
/// verifier reads — and must not hand them to an unauthenticated caller.
#[tokio::test]
async fn server_identity_reports_the_values_records_bind_to() {
    let port = 38113u16;
    let data = std::env::temp_dir().join(format!("shardx-e2e-v2id-{}", std::process::id()));
    let _guard = spawn_server(&data, port);
    let cl = client();
    wait_health(&cl, port).await;

    let admin = token(&cl, port, "admin", "secret").await;
    let user_id = admin_user_id(&cl, port, &admin).await;
    let sk_a = Ed25519SigningKey::from_bytes(&[11u8; 32]);
    let sk_b = Ed25519SigningKey::from_bytes(&[22u8; 32]);
    seed(&data, &user_id, &sk_a, &sk_b).await;

    let url = format!("http://127.0.0.1:{port}/v2/server-identity");

    // Deployment identity is not public: it is an input to authorization.
    let anon = cl.get(&url).send().await.expect("anon request");
    assert_eq!(anon.status(), 401, "identity must require a session");

    let res = cl
        .get(&url)
        .bearer_auth(&admin)
        .send()
        .await
        .expect("identity request");
    assert_eq!(res.status(), 200);
    let body: Value = res.json().await.expect("identity json");

    assert_eq!(
        body["server_instance_id"].as_str().expect("instance id"),
        hex(&INSTANCE),
        "must match the instance the verifier checks records against",
    );
    assert_eq!(
        body["restore_epoch"].as_i64().expect("restore epoch"),
        0,
        "must match the seeded epoch",
    );
}

/// The manifest the Launcher signs must be one the server accepts.
///
/// Client and server each used to describe this record separately, which is
/// exactly the kind of split that fails only during a real sync. Both sides
/// now build it from `shared::fleet_manifest`, and this test drives that
/// builder through the real HTTP commit path to prove the two agree.
#[tokio::test]
async fn a_client_built_manifest_is_accepted_by_the_commit_endpoint() {
    let port = 38114u16;
    let data = std::env::temp_dir().join(format!("shardx-e2e-manifest-{}", std::process::id()));
    let _guard = spawn_server(&data, port);
    let cl = client();
    wait_health(&cl, port).await;

    let admin = token(&cl, port, "admin", "secret").await;
    let user_id = admin_user_id(&cl, port, &admin).await;
    let sk_a = Ed25519SigningKey::from_bytes(&[11u8; 32]);
    let sk_b = Ed25519SigningKey::from_bytes(&[22u8; 32]);
    seed(&data, &user_id, &sk_a, &sk_b).await;

    let tenant_hex = hex(&TENANT_A);
    let profile = [0x51u8; 16];
    let profile_hex = hex(&profile);

    // The identity endpoint is the only way a client learns these; a manifest
    // signed against the wrong pair is refused.
    let identity: Value = cl
        .get(format!("{}/v2/server-identity", base(port)))
        .bearer_auth(&admin)
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let instance_hex = identity["server_instance_id"].as_str().unwrap().to_string();
    let restore_epoch = identity["restore_epoch"].as_i64().unwrap();
    assert_eq!(instance_hex, hex(&INSTANCE));

    let fleet = [0x53u8; 16];
    seed_profile(&data, &fleet, &profile).await;

    let container = b"sealed-container-bytes-opaque-to-the-server".to_vec();
    let container_sha = c::sha256(&container);
    let snapshot = [0x52u8; 16];
    let lease_id = hex(&[0x54u8; 16]);
    let session_id = hex(&[0x55u8; 16]);

    let lease_res = cl
        .post(format!("{}/v2/fleet/leases", base(port)))
        .bearer_auth(&admin)
        .json(&json!({
            "tenant_id": tenant_hex,
            "profile_id": profile_hex,
            "lease_id": lease_id,
            "account_id": hex(&[1u8; 16]),
            "device_id": hex(&[4u8; 16]),
            "ttl_seconds": 60,
        }))
        .send()
        .await
        .unwrap();
    let lease_status = lease_res.status();
    let lease_body = lease_res.text().await.unwrap();
    assert!(lease_status.is_success(), "lease ({lease_status}): {lease_body}");
    let lease: Value = serde_json::from_str(&lease_body).unwrap();
    let fencing = lease["fencing_token"].as_i64().unwrap();

    let intent = c::sha256(&c::encode(&c::m(vec![
        ("profile_id", c::t(&profile_hex)),
        ("snapshot_id", c::t(&hex(&snapshot))),
        ("container_sha256", c::b(&container_sha)),
    ])));

    let open = cl
        .post(format!("{}/v2/fleet/uploads", base(port)))
        .bearer_auth(&admin)
        .json(&json!({
            "tenant_id": tenant_hex,
            "profile_id": profile_hex,
            "session_id": session_id,
            "lease_id": lease_id,
            "fencing_token": fencing,
            "target_version": 1,
            "intent_hash": hex(&intent),
            "declared_size": container.len() as i64,
        }))
        .send()
        .await
        .unwrap();
    assert!(open.status().is_success(), "open upload: {}", open.text().await.unwrap());

    let chunk = cl
        .post(format!(
            "{}/v2/fleet/uploads/{tenant_hex}/{session_id}/chunk",
            base(port)
        ))
        .bearer_auth(&admin)
        .header("x-chunk-offset", "0")
        .header("content-type", "application/octet-stream")
        .body(container.clone())
        .send()
        .await
        .unwrap();
    assert!(chunk.status().is_success(), "chunk: {}", chunk.text().await.unwrap());

    let now_ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_millis() as u64;

    // Built by the same code path the Launcher uses.
    let manifest = shared::fleet_manifest::build_snapshot_manifest(
        &sk_a,
        &shared::fleet_manifest::ManifestFields {
            tenant_id: TENANT_A,
            server_instance_id: INSTANCE,
            restore_epoch: restore_epoch as u64,
            replay_id: [0x56u8; 16],
            profile_id: profile,
            snapshot_id: snapshot,
            fleet_id: fleet,
            base_version: 0,
            key_generation: 1,
            container_sha256: container_sha,
            not_before_ms: now_ms.saturating_sub(60_000),
            not_after_ms: now_ms + 300_000,
        },
    );

    let commit = cl
        .post(format!("{}/v2/fleet/uploads/commit", base(port)))
        .bearer_auth(&admin)
        .json(&json!({
            "tenant_id": tenant_hex,
            "profile_id": profile_hex,
            "session_id": session_id,
            "manifest_hex": hex(&manifest),
            "snapshot_id": hex(&snapshot),
            "fleet_id": hex(&fleet),
            "base_version": 0,
            "key_generation": 1,
            "container_sha256": hex(&container_sha),
            "author_account_id": hex(&[1u8; 16]),
            "author_device_id": hex(&[4u8; 16]),
        }))
        .send()
        .await
        .unwrap();
    let status = commit.status();
    let body = commit.text().await.unwrap();
    assert!(status.is_success(), "commit rejected ({status}): {body}");

    let published: Value = serde_json::from_str(&body).unwrap();
    assert_eq!(published["version"].as_i64(), Some(1));

}

/// A lease targets an existing profile, so create the fleet and profile rows
/// the way tenant provisioning would.
async fn seed_profile(data: &std::path::Path, fleet: &[u8; 16], profile: &[u8; 16]) {
    let url = format!("sqlite://{}/shardx.db", data.display().to_string().replace('\\', "/"));
    let pool = SqlitePoolOptions::new().connect(&url).await.unwrap();
    pool.execute("PRAGMA foreign_keys = ON").await.unwrap();

    sqlx::query(
        "INSERT INTO v2_fleets (id, tenant_id, name, status, created_at)
         VALUES (?, ?, 'fleet-1', 'active', '2026-01-01T00:00:00Z')",
    )
    .bind(fleet.as_slice())
    .bind(TENANT_A.as_slice())
    .execute(&pool)
    .await
    .unwrap();

    sqlx::query(
        "INSERT INTO v2_profiles
             (id, tenant_id, fleet_id, name, current_version, status, created_at, updated_at)
         VALUES (?, ?, ?, 'profile-1', 0, 'active', '2026-01-01T00:00:00Z', '2026-01-01T00:00:00Z')",
    )
    .bind(profile.as_slice())
    .bind(TENANT_A.as_slice())
    .bind(fleet.as_slice())
    .execute(&pool)
    .await
    .unwrap();

    pool.close().await;
}

/// A valid signature must not authorize different data.
///
/// The commit endpoint verifies the manifest and then reads several values
/// from the request body. If it trusts the body over the signed record, a
/// caller can attach a genuine manifest to another payload. This drives that
/// exact attack: same signed manifest, mismatched body.
#[tokio::test]
async fn a_body_that_contradicts_the_signed_manifest_is_refused() {
    let port = 38115u16;
    let data = std::env::temp_dir().join(format!("shardx-e2e-mismatch-{}", std::process::id()));
    let _guard = spawn_server(&data, port);
    let cl = client();
    wait_health(&cl, port).await;

    let admin = token(&cl, port, "admin", "secret").await;
    let user_id = admin_user_id(&cl, port, &admin).await;
    let sk_a = Ed25519SigningKey::from_bytes(&[11u8; 32]);
    let sk_b = Ed25519SigningKey::from_bytes(&[22u8; 32]);
    seed(&data, &user_id, &sk_a, &sk_b).await;

    let tenant_hex = hex(&TENANT_A);
    let profile = [0x61u8; 16];
    let profile_hex = hex(&profile);
    let fleet = [0x62u8; 16];
    seed_profile(&data, &fleet, &profile).await;

    let container = b"the-bytes-actually-staged".to_vec();
    let container_sha = c::sha256(&container);
    let snapshot = [0x63u8; 16];
    let lease_id = hex(&[0x64u8; 16]);
    let session_id = hex(&[0x65u8; 16]);

    let lease: Value = cl
        .post(format!("{}/v2/fleet/leases", base(port)))
        .bearer_auth(&admin)
        .json(&json!({
            "tenant_id": tenant_hex,
            "profile_id": profile_hex,
            "lease_id": lease_id,
            "account_id": hex(&[1u8; 16]),
            "device_id": hex(&[4u8; 16]),
            "ttl_seconds": 60,
        }))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let fencing = lease["fencing_token"].as_i64().unwrap();

    let intent = c::sha256(&c::encode(&c::m(vec![
        ("profile_id", c::t(&profile_hex)),
        ("snapshot_id", c::t(&hex(&snapshot))),
        ("container_sha256", c::b(&container_sha)),
    ])));

    cl.post(format!("{}/v2/fleet/uploads", base(port)))
        .bearer_auth(&admin)
        .json(&json!({
            "tenant_id": tenant_hex,
            "profile_id": profile_hex,
            "session_id": session_id,
            "lease_id": lease_id,
            "fencing_token": fencing,
            "target_version": 1,
            "intent_hash": hex(&intent),
            "declared_size": container.len() as i64,
        }))
        .send()
        .await
        .unwrap();

    cl.post(format!(
        "{}/v2/fleet/uploads/{tenant_hex}/{session_id}/chunk",
        base(port)
    ))
    .bearer_auth(&admin)
    .header("x-chunk-offset", "0")
    .header("content-type", "application/octet-stream")
    .body(container.clone())
    .send()
    .await
    .unwrap();

    let now_ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_millis() as u64;

    // A manifest that is genuinely signed, but for a *different* snapshot:
    // it commits to other bytes than the ones staged in this session.
    let other_container = b"bytes-from-some-other-snapshot".to_vec();
    let manifest = shared::fleet_manifest::build_snapshot_manifest(
        &sk_a,
        &shared::fleet_manifest::ManifestFields {
            tenant_id: TENANT_A,
            server_instance_id: INSTANCE,
            restore_epoch: 0,
            replay_id: [0x66u8; 16],
            profile_id: profile,
            snapshot_id: snapshot,
            fleet_id: fleet,
            base_version: 0,
            key_generation: 1,
            container_sha256: c::sha256(&other_container),
            not_before_ms: now_ms.saturating_sub(60_000),
            not_after_ms: now_ms + 300_000,
        },
    );

    // The body describes the staged bytes, so every server-side consistency
    // check on the staged content passes. Only comparing the body against the
    // signed manifest catches this.
    let forged = cl
        .post(format!("{}/v2/fleet/uploads/commit", base(port)))
        .bearer_auth(&admin)
        .json(&json!({
            "tenant_id": tenant_hex,
            "profile_id": profile_hex,
            "session_id": session_id,
            "manifest_hex": hex(&manifest),
            "snapshot_id": hex(&snapshot),
            "fleet_id": hex(&fleet),
            "base_version": 0,
            "key_generation": 1,
            "container_sha256": hex(&container_sha),
            "author_account_id": hex(&[1u8; 16]),
            "author_device_id": hex(&[4u8; 16]),
        }))
        .send()
        .await
        .unwrap();
    let st = forged.status();
    let body = forged.text().await.unwrap();
    assert_eq!(
        st,
        reqwest::StatusCode::BAD_REQUEST,
        "a manifest signed for other bytes must not publish the staged ones: {body}"
    );
    assert!(
        body.contains("signed manifest"),
        "refusal should name the manifest mismatch, got: {body}"
    );
}

/// A member with no v2 account in a tenant must not be able to lease that
/// tenant's profiles.
///
/// The lease handler reads `account_id` from the request body. Nothing tied
/// that value to the caller, so any authenticated user could name any account
/// in any tenant and check out its profiles. The signed-record endpoints are
/// safe because a signature covers their fields; the fleet endpoints carry no
/// signature, so the tenant boundary has to be enforced by the handler.
#[tokio::test]
async fn a_member_outside_the_tenant_cannot_lease_its_profiles() {
    let port = 38117u16;
    let data = std::env::temp_dir().join(format!("shardx-e2e-crosstenant-{}", std::process::id()));
    let _guard = spawn_server(&data, port);
    let cl = client();
    wait_health(&cl, port).await;

    let admin = token(&cl, port, "admin", "secret").await;
    let user_id = admin_user_id(&cl, port, &admin).await;
    let sk_a = Ed25519SigningKey::from_bytes(&[11u8; 32]);
    let sk_b = Ed25519SigningKey::from_bytes(&[22u8; 32]);
    seed(&data, &user_id, &sk_a, &sk_b).await;

    let fleet = [0x63u8; 16];
    let profile = [0x61u8; 16];
    seed_profile(&data, &fleet, &profile).await;

    // A second, legitimate user of the deployment — but seeded into no tenant,
    // so it holds no v2 account anywhere.
    let created = cl
        .post(format!("{}/users", base(port)))
        .bearer_auth(&admin)
        .json(&json!({ "username": "outsider", "password": "outsider-pass", "role": "member" }))
        .send()
        .await
        .unwrap();
    assert!(
        created.status().is_success(),
        "could not create the outsider account: {}",
        created.status()
    );
    let outsider = token(&cl, port, "outsider", "outsider-pass").await;

    // The account id the *admin* owns in tenant A, named by someone who has no
    // account in that tenant at all.
    let res = cl
        .post(format!("{}/v2/fleet/leases", base(port)))
        .bearer_auth(&outsider)
        .json(&json!({
            "tenant_id": hex(&TENANT_A),
            "profile_id": hex(&profile),
            "lease_id": hex(&[0x64u8; 16]),
            "account_id": hex(&[1u8; 16]),
            "device_id": hex(&[0x65u8; 16]),
            "ttl_seconds": 60,
        }))
        .send()
        .await
        .unwrap();

    assert_eq!(
        res.status(),
        reqwest::StatusCode::FORBIDDEN,
        "a caller with no account in the tenant leased one of its profiles"
    );
}
