import assert from "node:assert/strict";
import { mkdtemp, readFile, readdir, rm } from "node:fs/promises";
import os from "node:os";
import path from "node:path";
import test from "node:test";

import {
  clearVerificationCheckpoint,
  readVerificationCheckpoint,
  saveVerificationCheckpoint,
} from "./verification-checkpoint.js";

const challenge = {
  provider: "cloudflare",
  kind: "turnstile",
  confidence: "high",
};

test("persists one privacy-minimal verification checkpoint until it clears", async (t) => {
  const dir = await mkdtemp(path.join(os.tmpdir(), "shardx-checkpoint-test-"));
  t.after(() => rm(dir, { recursive: true, force: true }));
  let clock = Date.parse("2026-07-17T00:00:00.000Z");
  const now = () => clock;

  const first = await saveVerificationCheckpoint("profile-1", challenge, {
    dir,
    now,
    operation: "safe_open_url",
  });
  assert.equal(first.created, true);
  assert.deepEqual(await readVerificationCheckpoint("profile-1", { dir, now }), first.checkpoint);

  clock += 1000;
  const repeated = await saveVerificationCheckpoint("profile-1", challenge, {
    dir,
    now,
    operation: "challenge_status",
  });
  assert.equal(repeated.created, false);
  assert.equal(repeated.checkpoint.detected_at, first.checkpoint.detected_at);
  assert.equal(repeated.checkpoint.last_seen_at, "2026-07-17T00:00:01.000Z");

  const [file] = await readdir(dir);
  const persisted = JSON.parse(await readFile(path.join(dir, file), "utf8"));
  assert.equal(persisted.profile_id, "profile-1");
  assert.equal("url" in persisted, false);
  assert.equal("profile_name" in persisted, false);
  assert.equal("token" in persisted, false);
  assert.equal(await clearVerificationCheckpoint("profile-1", { dir }), true);
  assert.equal(await readVerificationCheckpoint("profile-1", { dir, now }), null);
});

test("expires a stale checkpoint", async (t) => {
  const dir = await mkdtemp(path.join(os.tmpdir(), "shardx-checkpoint-expiry-"));
  t.after(() => rm(dir, { recursive: true, force: true }));
  let clock = Date.parse("2026-07-17T00:00:00.000Z");
  const now = () => clock;

  await saveVerificationCheckpoint("profile-2", challenge, { dir, now });
  clock += 25 * 60 * 60 * 1000;
  assert.equal(await readVerificationCheckpoint("profile-2", { dir, now }), null);
});
