import assert from "node:assert/strict";
import test from "node:test";

import { classifyCloudflareChallenge } from "./challenge.js";

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
