import assert from "node:assert/strict";
import test from "node:test";
import { fileURLToPath } from "node:url";
import path from "node:path";

import { Client } from "@modelcontextprotocol/sdk/client/index.js";
import { StdioClientTransport } from "@modelcontextprotocol/sdk/client/stdio.js";

const dir = path.dirname(fileURLToPath(import.meta.url));

test("stdio server exposes the versioned ShardX tool contract", async () => {
  const transport = new StdioClientTransport({
    command: process.execPath,
    args: [path.join(dir, "index.js")],
    cwd: dir,
    stderr: "pipe",
  });
  const client = new Client({ name: "shardx-contract-test", version: "1.0.0" });

  try {
    await client.connect(transport);
    const { version } = client.getServerVersion() ?? {};
    const { tools } = await client.listTools();
    const names = new Set(tools.map((tool) => tool.name));

    assert.equal(version, "0.1.27");
    assert.equal(tools.length, 96);
    for (const name of [
      "health_check",
      "startup_status",
      "configure_startup",
      "find_profile_by_name",
      "ensure_profile_started",
      "safe_open_url",
      "devtools_context",
      "cleanup_stale_profile_processes",
      "challenge_status",
      "verification_checkpoint",
      "wait_for_human_verification",
    ]) {
      assert(names.has(name), `missing MCP tool: ${name}`);
    }
  } finally {
    await client.close();
  }
});
