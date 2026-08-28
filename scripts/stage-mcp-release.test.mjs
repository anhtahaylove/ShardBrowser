import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { mkdtemp, mkdir, readFile, rm, writeFile } from "node:fs/promises";
import os from "node:os";
import path from "node:path";
import test from "node:test";
import { fileURLToPath } from "node:url";

import {
  MCP_RELEASE_FILES,
  expectedArchiveEntries,
  stageRelease,
  verifyArchiveEntries,
  verifyReleaseDirectory,
} from "./stage-mcp-release.mjs";

const stageScript = fileURLToPath(new URL("./stage-mcp-release.mjs", import.meta.url));

async function withTempDirectory(run) {
  const root = await mkdtemp(path.join(os.tmpdir(), "shardx-mcp-release-test-"));
  try {
    await run(root);
  } finally {
    await rm(root, { recursive: true, force: true });
  }
}

test("stageRelease refuses a non-empty destination", async () => {
  await withTempDirectory(async (root) => {
    const target = path.join(root, "ShardX-MCP");
    await mkdir(target);
    await writeFile(path.join(target, "stale.txt"), "stale");

    await assert.rejects(stageRelease(target), /must be empty/);
    assert.equal(await readFile(path.join(target, "stale.txt"), "utf8"), "stale");
  });
});

test("verifyReleaseDirectory accepts only the exact regular-file manifest", async () => {
  await withTempDirectory(async (root) => {
    const target = path.join(root, "ShardX-MCP");
    await stageRelease(target);

    await verifyReleaseDirectory(target);
  });
});

test("verifyReleaseDirectory rejects missing, extra, and nested entries", async (t) => {
  await t.test("missing", async () => {
    await withTempDirectory(async (root) => {
      const target = path.join(root, "ShardX-MCP");
      await stageRelease(target);
      await rm(path.join(target, MCP_RELEASE_FILES[0]));
      await assert.rejects(verifyReleaseDirectory(target), /archive manifest mismatch/);
    });
  });

  await t.test("extra", async () => {
    await withTempDirectory(async (root) => {
      const target = path.join(root, "ShardX-MCP");
      await stageRelease(target);
      await writeFile(path.join(target, "extra.txt"), "extra");
      await assert.rejects(verifyReleaseDirectory(target), /archive manifest mismatch/);
    });
  });

  await t.test("nested", async () => {
    await withTempDirectory(async (root) => {
      const target = path.join(root, "ShardX-MCP");
      await stageRelease(target);
      await mkdir(path.join(target, "nested"));
      await writeFile(path.join(target, "nested", "extra.txt"), "extra");
      await assert.rejects(verifyReleaseDirectory(target), /archive manifest mismatch/);
    });
  });
});

test("verifyArchiveEntries enforces exact set equality", () => {
  const expected = expectedArchiveEntries("ShardX-MCP");
  assert.doesNotThrow(() => verifyArchiveEntries(expected, "ShardX-MCP"));
  assert.doesNotThrow(() =>
    verifyArchiveEntries(
      expected.map((entry) => `${entry}\r`),
      "ShardX-MCP",
    ),
  );
  assert.throws(
    () => verifyArchiveEntries(expected.slice(0, -1), "ShardX-MCP"),
    /archive manifest mismatch/,
  );
  assert.throws(
    () => verifyArchiveEntries([...expected, "ShardX-MCP/extra.txt"], "ShardX-MCP"),
    /archive manifest mismatch/,
  );
  assert.throws(
    () => verifyArchiveEntries([...expected, expected[1]], "ShardX-MCP"),
    /duplicate archive entry/,
  );
  assert.throws(
    () => verifyArchiveEntries([...expected, "../escape"], "ShardX-MCP"),
    /archive manifest mismatch/,
  );
  for (const unsafeRoot of [null, ".", "..", "/absolute", "nested/root", "nested\\root"]) {
    assert.throws(
      () => expectedArchiveEntries(unsafeRoot),
      /safe path segment/,
      `accepted unsafe archive root: ${unsafeRoot}`,
    );
  }
});

test("release staging CLI rejects incomplete or unknown option forms", async () => {
  await withTempDirectory(async (root) => {
    for (const args of [["--verify-directory"], ["--unknown"], ["--list", "unexpected"]]) {
      const result = spawnSync(process.execPath, [stageScript, ...args], {
        cwd: root,
        encoding: "utf8",
      });
      assert.notEqual(result.status, 0, `accepted malformed CLI args: ${args.join(" ")}`);
      assert.match(result.stderr, /usage:/);
    }
  });
});
