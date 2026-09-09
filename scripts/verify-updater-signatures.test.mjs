import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { generateKeyPairSync, createHash, sign as signMessage, randomBytes } from "node:crypto";
import { mkdtemp, rm, writeFile } from "node:fs/promises";
import os from "node:os";
import path from "node:path";
import test from "node:test";
import { fileURLToPath } from "node:url";

const verifyScript = fileURLToPath(new URL("./verify-updater-signatures.mjs", import.meta.url));

/** minisign prefixes a 2-byte algorithm id and an 8-byte key id. */
const PREHASHED = Buffer.from("ED", "latin1");
const LEGACY = Buffer.from("Ed", "latin1");
const SPKI_ED25519_PREFIX_BYTES = 12;

function newSigningKey() {
  const { publicKey, privateKey } = generateKeyPairSync("ed25519");
  return {
    privateKey,
    rawPublic: publicKey.export({ format: "der", type: "spki" }).subarray(SPKI_ED25519_PREFIX_BYTES),
    keyId: randomBytes(8),
  };
}

/** The config stores the whole minisign public key file, base64 encoded. */
function shippedPubkeyField(key) {
  const blob = Buffer.concat([Buffer.from("Ed", "latin1"), key.keyId, key.rawPublic]);
  const file = `untrusted comment: minisign public key\n${blob.toString("base64")}\n`;
  return Buffer.from(file).toString("base64");
}

/** latest.json stores the whole .sig file, base64 encoded. */
function signatureField(key, bytes, { algorithm = PREHASHED, keyId = key.keyId } = {}) {
  const message = algorithm.equals(PREHASHED)
    ? createHash("blake2b512").update(bytes).digest()
    : bytes;
  const blob = Buffer.concat([algorithm, keyId, signMessage(null, message, key.privateKey)]);
  const file = `untrusted comment: signature\n${blob.toString("base64")}\ntrusted comment: test\n`;
  return Buffer.from(file).toString("base64");
}

async function withRelease(run) {
  const root = await mkdtemp(path.join(os.tmpdir(), "shardx-updater-verify-test-"));
  try {
    const key = newSigningKey();
    const dist = path.join(root, "dist");
    const configPath = path.join(root, "tauri.conf.json");
    const asset = "ShardX.Launcher_9.9.9_x64-setup.exe";
    const bytes = randomBytes(2048);

    await rm(dist, { recursive: true, force: true });
    await import("node:fs/promises").then((fs) => fs.mkdir(dist, { recursive: true }));
    await writeFile(path.join(dist, asset), bytes);
    await writeFile(
      configPath,
      JSON.stringify({ plugins: { updater: { pubkey: shippedPubkeyField(key) } } }),
    );

    const writeManifest = async (signature) => {
      await writeFile(
        path.join(dist, "latest.json"),
        JSON.stringify({
          version: "9.9.9",
          platforms: {
            "windows-x86_64": { signature, url: `https://example.invalid/${asset}` },
          },
        }),
      );
    };

    const verify = () =>
      spawnSync(process.execPath, [verifyScript, "--dist", dist, "--config", configPath], {
        encoding: "utf8",
      });

    await run({ key, dist, asset, bytes, writeManifest, verify });
  } finally {
    await rm(root, { recursive: true, force: true });
  }
}

test("a signature made by the shipping key verifies", async () => {
  await withRelease(async ({ key, bytes, writeManifest, verify }) => {
    await writeManifest(signatureField(key, bytes));

    const result = verify();
    assert.equal(result.status, 0, result.stdout + result.stderr);
    assert.match(result.stdout, /All 1 updater signatures verify/);
  });
});

test("a legacy non-prehashed signature verifies", async () => {
  await withRelease(async ({ key, bytes, writeManifest, verify }) => {
    await writeManifest(signatureField(key, bytes, { algorithm: LEGACY }));

    assert.equal(verify().status, 0);
  });
});

test("a tampered asset fails", async () => {
  await withRelease(async ({ key, dist, asset, bytes, writeManifest, verify }) => {
    await writeManifest(signatureField(key, bytes));
    const tampered = Buffer.from(bytes);
    tampered[64] ^= 0xff;
    await writeFile(path.join(dist, asset), tampered);

    const result = verify();
    assert.notEqual(result.status, 0);
    assert.match(result.stderr, /does not match the asset/);
  });
});

test("a signature from a different key fails", async () => {
  await withRelease(async ({ bytes, writeManifest, verify }) => {
    await writeManifest(signatureField(newSigningKey(), bytes));

    const result = verify();
    assert.notEqual(result.status, 0);
    assert.match(result.stderr, /but the app ships/);
  });
});

test("a signature carrying a foreign key id fails before verification", async () => {
  await withRelease(async ({ key, bytes, writeManifest, verify }) => {
    await writeManifest(signatureField(key, bytes, { keyId: randomBytes(8) }));

    const result = verify();
    assert.notEqual(result.status, 0);
    assert.match(result.stderr, /but the app ships/);
  });
});

test("an unknown signature algorithm fails instead of being trusted", async () => {
  await withRelease(async ({ key, bytes, writeManifest, verify }) => {
    await writeManifest(signatureField(key, bytes, { algorithm: Buffer.from("ZZ", "latin1") }));

    const result = verify();
    assert.notEqual(result.status, 0);
    assert.match(result.stderr, /unknown signature algorithm/);
  });
});

test("a manifest listing no platforms fails", async () => {
  await withRelease(async ({ dist, verify }) => {
    await writeFile(path.join(dist, "latest.json"), JSON.stringify({ version: "9.9.9", platforms: {} }));

    const result = verify();
    assert.notEqual(result.status, 0);
    assert.match(result.stderr, /no platforms/);
  });
});
