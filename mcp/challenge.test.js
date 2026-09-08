import assert from "node:assert/strict";
import test from "node:test";

import { classifyCloudflareChallenge, waitForChallengeClear } from "./challenge.js";

test("detects Cloudflare's documented mitigation header", () => {
  assert.deepEqual(
    classifyCloudflareChallenge({ headers: { "CF-MitIGated": "challenge" } }),
    { detected: true, provider: "cloudflare", kind: "interstitial", confidence: "high", signal: "cf-mitigated" },
  );
});

test("detects the Vietnamese interstitial shown by Cloudflare", () => {
  assert.equal(
    classifyCloudflareChallenge({
      title: "Chờ một chút...",
      bodyText: "Thực hiện xác minh bảo mật bởi Cloudflare",
    }).detected,
    true,
  );
});

test("does not label generic human-verification copy as Cloudflare", () => {
  assert.equal(classifyCloudflareChallenge({ bodyText: "Verify you are human to continue" }).detected, false);
});

test("distinguishes a visible Turnstile widget from a clear page", () => {
  assert.equal(classifyCloudflareChallenge({ turnstileVisible: true }).kind, "turnstile");
  assert.equal(classifyCloudflareChallenge({ title: "WordPress" }).detected, false);
});

test("waits for a detected challenge and resumes when it clears", async () => {
  let clock = 0;
  const result = await waitForChallengeClear(
    { detected: true, kind: "interstitial" },
    async () => ({ detected: false, kind: null }),
    {
      timeoutMs: 5000,
      now: () => clock,
      sleep: async (ms) => { clock += ms; },
    },
  );

  assert.equal(result.waited, true);
  assert.equal(result.timed_out, false);
  assert.equal(result.challenge.detected, false);
});

test("reports a timeout when verification remains required", async () => {
  let clock = 0;
  const result = await waitForChallengeClear(
    { detected: true, kind: "turnstile" },
    async () => ({ detected: true, kind: "turnstile" }),
    {
      timeoutMs: 1000,
      intervalMs: 1000,
      now: () => clock,
      sleep: async (ms) => { clock += ms; },
    },
  );

  assert.equal(result.waited, true);
  assert.equal(result.timed_out, true);
  assert.equal(result.challenge.detected, true);
});
