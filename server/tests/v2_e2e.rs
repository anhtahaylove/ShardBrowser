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
