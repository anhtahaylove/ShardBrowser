import { spawn } from "node:child_process";
import { createHash, randomUUID } from "node:crypto";
import { mkdir, readFile, rename, rm, writeFile } from "node:fs/promises";
import os from "node:os";
import path from "node:path";

const CHECKPOINT_VERSION = 1;
const CHECKPOINT_MAX_AGE_MS = 24 * 60 * 60 * 1000;
const DEFAULT_DIR = path.join(os.tmpdir(), "shardx-mcp-verification-checkpoints");

function checkpointPath(profileId, dir = DEFAULT_DIR) {
  const key = createHash("sha256").update(profileId).digest("hex");
  return path.join(dir, `${key}.json`);
}

function publicCheckpoint(checkpoint) {
  if (!checkpoint) return null;
  return {
    required: true,
    provider: checkpoint.provider,
    kind: checkpoint.kind,
    confidence: checkpoint.confidence,
    operation: checkpoint.operation,
    detected_at: checkpoint.detected_at,
    last_seen_at: checkpoint.last_seen_at,
  };
}

export async function readVerificationCheckpoint(profileId, { dir = DEFAULT_DIR, now = Date.now } = {}) {
  const file = checkpointPath(profileId, dir);
  try {
    const checkpoint = JSON.parse(await readFile(file, "utf8"));
    const lastSeen = Date.parse(checkpoint.last_seen_at || checkpoint.detected_at || "");
    if (
      checkpoint.version !== CHECKPOINT_VERSION ||
      checkpoint.profile_id !== profileId ||
      !checkpoint.required ||
      !Number.isFinite(lastSeen) ||
      now() - lastSeen > CHECKPOINT_MAX_AGE_MS
    ) {
      await rm(file, { force: true });
      return null;
    }
    return publicCheckpoint(checkpoint);
  } catch (error) {
    if (error?.code !== "ENOENT") await rm(file, { force: true }).catch(() => {});
    return null;
  }
}

export async function saveVerificationCheckpoint(
  profileId,
  challenge,
  { operation = "challenge_check", dir = DEFAULT_DIR, now = Date.now } = {},
) {
  const existing = await readVerificationCheckpoint(profileId, { dir, now });
  const timestamp = new Date(now()).toISOString();
  const checkpoint = {
    version: CHECKPOINT_VERSION,
    profile_id: profileId,
    required: true,
    provider: challenge.provider || "cloudflare",
    kind: challenge.kind || "interstitial",
    confidence: challenge.confidence || "unknown",
    operation,
    detected_at: existing?.detected_at || timestamp,
    last_seen_at: timestamp,
  };

  await mkdir(dir, { recursive: true });
  const file = checkpointPath(profileId, dir);
  const temporary = `${file}.${randomUUID()}.tmp`;
  try {
    await writeFile(temporary, `${JSON.stringify(checkpoint, null, 2)}\n`, {
      encoding: "utf8",
      mode: 0o600,
    });
    await rename(temporary, file);
  } finally {
    await rm(temporary, { force: true });
  }
  return { checkpoint: publicCheckpoint(checkpoint), created: !existing };
}

export async function clearVerificationCheckpoint(profileId, { dir = DEFAULT_DIR } = {}) {
  const file = checkpointPath(profileId, dir);
  const existed = await readFile(file, "utf8").then(() => true).catch(() => false);
  await rm(file, { force: true });
  return existed;
}

export function notifyVerificationRequired() {
  if (process.platform !== "win32") return false;

  const script = [
    "[Windows.UI.Notifications.ToastNotificationManager, Windows.UI.Notifications, ContentType = WindowsRuntime] > $null",
    "[Windows.Data.Xml.Dom.XmlDocument, Windows.Data.Xml.Dom.XmlDocument, ContentType = WindowsRuntime] > $null",
    "$xml = New-Object Windows.Data.Xml.Dom.XmlDocument",
    "$xml.LoadXml('<toast><visual><binding template=\"ToastGeneric\"><text>ShardX verification required</text><text>Open the visible browser profile to complete Cloudflare verification.</text></binding></visual></toast>')",
    "$toast = [Windows.UI.Notifications.ToastNotification]::new($xml)",
    "[Windows.UI.Notifications.ToastNotificationManager]::CreateToastNotifier('ShardX Launcher').Show($toast)",
  ].join("\r\n");
  const encoded = Buffer.from(script, "utf16le").toString("base64");
  const powershell = path.join(
    process.env.SystemRoot || "C:\\Windows",
    "System32",
    "WindowsPowerShell",
    "v1.0",
    "powershell.exe",
  );

  try {
    const child = spawn(
      powershell,
      ["-NoProfile", "-NonInteractive", "-WindowStyle", "Hidden", "-EncodedCommand", encoded],
      { detached: true, stdio: "ignore", windowsHide: true },
    );
    child.on("error", () => {});
    child.unref();
    return true;
  } catch {
    return false;
  }
}
