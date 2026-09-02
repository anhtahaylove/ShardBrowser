const INTERSTITIAL_TITLE = /just a moment|attention required|ch(?:ờ|o) một ch(?:ú|u)t|thực hiện xác minh bảo mật/i;
const INTERSTITIAL_BODY = /verify you are human|verifying you are human|xác minh bạn là con người|thực hiện xác minh bảo mật/i;
const CLOUDFLARE_MARKER = /cloudflare/i;

function headerValue(headers, name) {
  const wanted = name.toLowerCase();
  return Object.entries(headers || {}).find(([key]) => key.toLowerCase() === wanted)?.[1] || "";
}

export function classifyCloudflareChallenge({ headers, title, bodyText, turnstileVisible = false } = {}) {
  if (String(headerValue(headers, "cf-mitigated")).toLowerCase() === "challenge") {
    return { detected: true, provider: "cloudflare", kind: "interstitial", confidence: "high", signal: "cf-mitigated" };
  }
  const body = String(bodyText || "");
  const looksLikeInterstitial = INTERSTITIAL_TITLE.test(String(title || "")) || INTERSTITIAL_BODY.test(body);
  if (looksLikeInterstitial && CLOUDFLARE_MARKER.test(body)) {
    return { detected: true, provider: "cloudflare", kind: "interstitial", confidence: "medium", signal: "page_content" };
  }
  if (turnstileVisible) {
    return { detected: true, provider: "cloudflare", kind: "turnstile", confidence: "medium", signal: "visible_widget" };
  }
  return { detected: false, provider: null, kind: null, confidence: null, signal: null };
}

export async function waitForChallengeClear(
  initialChallenge,
  check,
  {
    timeoutMs = 120000,
    intervalMs = 1000,
    now = Date.now,
    sleep = (ms) => new Promise((resolve) => setTimeout(resolve, ms)),
    isClosed = () => false,
  } = {},
) {
  const startedAt = now();
  const deadline = startedAt + timeoutMs;
  let challenge = initialChallenge;
  const waited = !!challenge?.detected;

  while (challenge?.detected && !isClosed() && now() < deadline) {
    await sleep(Math.min(intervalMs, Math.max(1, deadline - now())));
    challenge = await check();
  }

  return {
    challenge,
    waited,
    timed_out: !!challenge?.detected && now() >= deadline,
    elapsed_ms: now() - startedAt,
  };
}
