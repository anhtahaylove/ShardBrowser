#!/usr/bin/env node
const API = (process.env.SHARDX_API || "http://127.0.0.1:40325").replace(/\/+$/, "");
const TOKEN = process.env.SHARDX_TOKEN || "";

if (typeof WebSocket !== "function") {
  throw new Error("Node.js with global WebSocket is required.");
}

async function api(path, { method = "GET", body } = {}) {
  const res = await fetch(API + path, {
    method,
    headers: {
      ...(TOKEN ? { Authorization: `Bearer ${TOKEN}` } : {}),
      ...(body !== undefined ? { "Content-Type": "application/json" } : {}),
    },
    body: body === undefined ? undefined : JSON.stringify(body),
  });
  const text = await res.text();
  const data = text ? JSON.parse(text) : null;
  if (!res.ok) throw new Error(`${method} ${path} -> ${data?.error || res.status}`);
  return data;
}

function openWs(url) {
  return new Promise((resolve, reject) => {
    const ws = new WebSocket(url);
    ws.addEventListener("open", () => resolve(ws), { once: true });
    ws.addEventListener("error", () => reject(new Error(`WebSocket failed: ${url}`)), { once: true });
  });
}

let nextId = 1;
function cdp(ws, method, params = {}) {
  const id = nextId++;
  return new Promise((resolve, reject) => {
    const timer = setTimeout(() => reject(new Error(`${method} timed out`)), 10_000);
    const onMessage = (event) => {
      const msg = JSON.parse(event.data);
      if (msg.id !== id) return;
      clearTimeout(timer);
      ws.removeEventListener("message", onMessage);
      if (msg.error) reject(new Error(`${method}: ${msg.error.message}`));
      else resolve(msg.result || {});
    };
    ws.addEventListener("message", onMessage);
    ws.send(JSON.stringify({ id, method, params }));
  });
}

async function canvasHash(page) {
  const expression = `
    (async () => {
      const c = document.createElement("canvas");
      c.width = 240; c.height = 80;
      const ctx = c.getContext("2d");
      ctx.textBaseline = "top";
      ctx.font = "16px Arial";
      ctx.fillStyle = "#f60";
      ctx.fillRect(100, 1, 62, 20);
      ctx.fillStyle = "#069";
      ctx.fillText("ShardX canvas smoke", 2, 15);
      ctx.strokeStyle = "rgba(120,120,0,0.7)";
      ctx.beginPath(); ctx.arc(70, 50, 30, 0, Math.PI * 2); ctx.stroke();
      const bytes = new TextEncoder().encode(c.toDataURL());
      const hash = await crypto.subtle.digest("SHA-256", bytes);
      return [...new Uint8Array(hash)].map((b) => b.toString(16).padStart(2, "0")).join("");
    })()
  `;
  const out = await cdp(page, "Runtime.evaluate", {
    expression,
    awaitPromise: true,
    returnByValue: true,
  });
  if (out.exceptionDetails) throw new Error("Canvas evaluation failed");
  return out.result.value;
}

async function hashForTempProfile(label) {
  const created = await api("/profiles/temporary", {
    method: "POST",
    body: {
      name: `canvas-smoke-${label}-${Date.now()}`,
      noise: { canvas: { enabled: true, seed: 0 } },
    },
  });
  const started = await api(`/profiles/${created.id}/start`, {
    method: "POST",
    body: { headless: true },
  });
  const cdpInfo = started.cdp;
  if (!cdpInfo?.http_url || !cdpInfo?.web_socket_debugger_url) {
    throw new Error("Profile started without a CDP endpoint");
  }

  const browser = await openWs(cdpInfo.web_socket_debugger_url);
  let page;
  try {
    const { targetId } = await cdp(browser, "Target.createTarget", { url: "about:blank" });
    const targets = await (await fetch(`${cdpInfo.http_url}/json/list`)).json();
    const target = targets.find((t) => t.id === targetId) || targets.find((t) => t.type === "page");
    if (!target?.webSocketDebuggerUrl) throw new Error("No page target WebSocket URL");
    page = await openWs(target.webSocketDebuggerUrl);
    return { id: created.id, hash: await canvasHash(page) };
  } finally {
    try { page?.close(); } catch {}
    try { browser.close(); } catch {}
    await api(`/profiles/${created.id}/stop`, { method: "POST" }).catch(() => {});
  }
}

const a = await hashForTempProfile("a");
const b = await hashForTempProfile("b");

console.log(JSON.stringify({ a: a.hash, b: b.hash, distinct: a.hash !== b.hash }, null, 2));
if (a.hash === b.hash) {
  throw new Error("Canvas hashes are identical; temporary profile canvas noise is not varying.");
}
