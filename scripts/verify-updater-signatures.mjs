#!/usr/bin/env node
// Verifies every updater signature in latest.json against the public key the
// app ships, the same way the running launcher will before it installs.
//
// The release job already fails when a .sig is missing or empty, but an empty
// check cannot catch a signature produced by the wrong key or one that stopped
// matching its installer: both leave a well-formed file behind. A client that
// downloads such a release rejects the update, and the failure lands on users
// rather than on this pipeline.
import { readFileSync } from "node:fs";
import { join } from "node:path";
import { createHash, createPublicKey, verify as verifySignature } from "node:crypto";

const args = new Map();
for (let i = 2; i < process.argv.length; i += 2) {
  args.set(process.argv[i], process.argv[i + 1]);
}

const dist = args.get("--dist") ?? "dist";
const conf = args.get("--config") ?? "src-tauri/tauri.conf.json";

/** minisign packs a 2-byte algorithm id and an 8-byte key id ahead of the payload. */
const MINISIGN_HEADER = 10;
const ALGORITHM_PREHASHED = "ED";
const ALGORITHM_LEGACY = "Ed";

/** minisign keys and signatures are base64 on the second line, after a comment. */
function decodeMinisign(text, label) {
  const line = text
    .trim()
    .split(/\r?\n/)
    .find((entry) => entry && !entry.startsWith("untrusted comment:"));
  if (!line) {
    throw new Error(`${label} has no base64 payload.`);
  }
  const raw = Buffer.from(line, "base64");
  if (raw.length <= MINISIGN_HEADER) {
    throw new Error(`${label} is too short to be a minisign blob.`);
  }
  return {
    algorithm: raw.subarray(0, 2).toString("latin1"),
    keyId: raw.subarray(2, MINISIGN_HEADER).toString("hex"),
    payload: raw.subarray(MINISIGN_HEADER),
  };
}

/** Ed25519 verification needs the raw key wrapped in the SPKI prefix Node expects. */
const SPKI_ED25519_PREFIX = Buffer.from("302a300506032b6570032100", "hex");
function toEd25519Key(raw) {
  return createPublicKey({
    key: Buffer.concat([SPKI_ED25519_PREFIX, raw]),
    format: "der",
    type: "spki",
  });
}

const shippedPubkey = JSON.parse(readFileSync(conf, "utf8")).plugins?.updater?.pubkey;
if (!shippedPubkey) {
  throw new Error(`No updater public key in ${conf}.`);
}
const pub = decodeMinisign(Buffer.from(shippedPubkey, "base64").toString("utf8"), "Shipped public key");
const publicKey = toEd25519Key(pub.payload);

const manifest = JSON.parse(readFileSync(join(dist, "latest.json"), "utf8"));
const platforms = Object.entries(manifest.platforms ?? {});
if (platforms.length === 0) {
  throw new Error("latest.json lists no platforms.");
}

const failures = [];
for (const [platform, entry] of platforms) {
  const asset = decodeURIComponent(entry.url.split("/").pop());
  try {
    const sig = decodeMinisign(Buffer.from(entry.signature, "base64").toString("utf8"), `${platform} signature`);

    if (sig.keyId !== pub.keyId) {
      throw new Error(`signed by key ${sig.keyId}, but the app ships ${pub.keyId}`);
    }

    // "ED" signs a BLAKE2b-512 digest of the file; "Ed" signs the bytes.
    const bytes = readFileSync(join(dist, asset));
    let message;
    if (sig.algorithm === ALGORITHM_PREHASHED) {
      message = createHash("blake2b512").update(bytes).digest();
    } else if (sig.algorithm === ALGORITHM_LEGACY) {
      message = bytes;
    } else {
      throw new Error(`unknown signature algorithm ${JSON.stringify(sig.algorithm)}`);
    }

    if (!verifySignature(null, message, publicKey, sig.payload)) {
      throw new Error("signature does not match the asset");
    }
    console.log(`ok   ${platform}: ${asset}`);
  } catch (error) {
    failures.push(`${platform} (${asset}): ${error.message}`);
    console.log(`FAIL ${platform}: ${asset}`);
  }
}

if (failures.length > 0) {
  console.error(`\nUpdater signatures clients would reject:\n${failures.map((f) => `  - ${f}`).join("\n")}`);
  process.exit(1);
}

console.log(`\nAll ${platforms.length} updater signatures verify against key ${pub.keyId}.`);
