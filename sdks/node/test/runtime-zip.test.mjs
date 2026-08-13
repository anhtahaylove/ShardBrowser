import assert from "node:assert/strict";
import { mkdtempSync, mkdirSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import test from "node:test";
import AdmZip from "adm-zip";

import { extractZipArchive } from "../dist/runtime.js";

function withTempDir(run) {
  const root = mkdtempSync(join(tmpdir(), "shardx-sdk-zip-"));
  try {
    run(root);
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
}

test("adm-zip 0.6 extracts and overwrites the nested Windows runtime layout", () => {
  withTempDir((root) => {
    const archive = join(root, "runtime.zip");
    const destination = join(root, "runtime");
    const executable = join(destination, "ShardX-Windows", "chrome.exe");
    mkdirSync(join(destination, "ShardX-Windows"), { recursive: true });
    writeFileSync(executable, "old-runtime");

    const zip = new AdmZip();
    zip.addFile("ShardX-Windows/chrome.exe", Buffer.from("new-runtime"));
    zip.addFile("ShardX-Windows/WidevineCdm/manifest.json", Buffer.from("{}"));
    zip.writeZip(archive);

    extractZipArchive(archive, destination);

    assert.equal(readFileSync(executable, "utf8"), "new-runtime");
    assert.equal(
      readFileSync(join(destination, "ShardX-Windows", "WidevineCdm", "manifest.json"), "utf8"),
      "{}",
    );
  });
});

test("adm-zip 0.6 preserves the fingerprint bundle directory and Unicode JSON", () => {
  withTempDir((root) => {
    const archive = join(root, "fingerprints.zip");
    const destination = join(root, "staging");
    const payload = JSON.stringify({ name: "Hồ sơ Việt Nam" });

    const zip = new AdmZip();
    zip.addFile("shardx-fingerprints/vietnam.json", Buffer.from(payload));
    zip.writeZip(archive);

    extractZipArchive(archive, destination);

    assert.equal(
      readFileSync(join(destination, "shardx-fingerprints", "vietnam.json"), "utf8"),
      payload,
    );
  });
});
