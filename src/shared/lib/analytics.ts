import { getVersion } from "@tauri-apps/api/app";

// Launcher-window analytics only: never the panels, never a profile's browser.

/** GA4 Web data stream. A measurement id is public by design. */
const MEASUREMENT_ID = "G-PLHRZ45072";

/** Reported as the page: GA4 would otherwise log `tauri://localhost`. */
const APP_URL = "https://launcher.proxyshard.com/";

const CLIENT_ID_KEY = "shardx-analytics-client-id";

/** One id per install. Not gtag's `_ga` cookie — it does not survive a
 *  restart on a custom-scheme origin, so every launch would be a new user. */
function clientId(): string {
  try {
    const stored = localStorage.getItem(CLIENT_ID_KEY);
    if (stored) return stored;
    const fresh = randomId();
    localStorage.setItem(CLIENT_ID_KEY, fresh);
    return fresh;
  } catch {
    // Private mode or blocked storage: still count the session, just without
    // being able to recognise it next time.
    return randomId();
  }
}

/// `crypto.randomUUID` needs a secure context, which a custom-scheme window is
/// not guaranteed to be. Falling back keeps a blocked call from taking the
/// whole module down with an unhandled rejection.
function randomId(): string {
  try {
    if (typeof crypto?.randomUUID === "function") return crypto.randomUUID();
  } catch { /* not a secure context */ }
  return `${Date.now().toString(36)}-${Math.random().toString(36).slice(2, 12)}`;
}

function hostOs(): string {
  const ua = navigator.userAgent;
  if (/Windows/i.test(ua)) return "Windows";
  if (/Macintosh|Mac OS X/i.test(ua)) return "macOS";
  if (/Linux|X11|CrOS/i.test(ua)) return "Linux";
  return "unknown";
}

let started = false;

/** Why the last init did what it did; readable as `__shardxAnalytics`. */
type Status = { sent: boolean; reason: string; client_id?: string; version?: string };

function report(status: Status): void {
  (window as any).__shardxAnalytics = status;
  if (status.sent) console.info("[analytics] reporting", status);
  else console.info("[analytics] not reporting:", status.reason);
}

/** Loads gtag once, unless analytics are off or no measurement id is set. */
export async function initAnalytics(): Promise<void> {
  if (started) return;
  started = true;
  if (!MEASUREMENT_ID) { report({ sent: false, reason: "no measurement id set" }); return; }

  try {
    await load();
  } catch (e) {
    // Never an unhandled rejection: analytics that can break the window on
    // its way up is worse than analytics that is missing.
    report({ sent: false, reason: `failed: ${String(e)}` });
  }
}

async function load(): Promise<void> {
  const version = await getVersion().catch(() => "dev");
  const dev = import.meta.env.DEV;

  const w = window as any;
  w.dataLayer = w.dataLayer || [];
  w.gtag = function gtag() { w.dataLayer.push(arguments); };
  w.gtag("js", new Date());
  // No advertising in this app, so nothing here should feed one.
  w.gtag("consent", "default", {
    ad_storage: "denied",
    ad_user_data: "denied",
    ad_personalization: "denied",
  });
  const cid = clientId();
  w.gtag("config", MEASUREMENT_ID, {
    client_id: cid,
    app_version: version,
    os: hostOs(),
    // Kept apart from real installs so a dev run can be filtered out of the
    // numbers. It still shows in Realtime, which is where it gets checked.
    env: dev ? "dev" : "prod",
    page_location: APP_URL,
    page_title: "ShardX Launcher",
    send_page_view: true,
  });

  const el = document.createElement("script");
  el.async = true;
  el.src = `https://www.googletagmanager.com/gtag/js?id=${MEASUREMENT_ID}`;
  el.onerror = () =>
    report({ sent: false, reason: "googletagmanager.com did not load — blocked or offline" });
  document.head.appendChild(el);

  report({ sent: true, reason: "gtag requested", client_id: cid, version });
}

/** One event; a no-op when gtag never loaded, so call sites need no guard. */
export function track(name: string, params: Record<string, unknown> = {}): void {
  const gtag = (window as any).gtag;
  if (typeof gtag === "function") gtag("event", name, params);
}
