import { copyFile, mkdir, stat } from "node:fs/promises";
import path from "node:path";
import { fileURLToPath } from "node:url";

export const MCP_RELEASE_FILES = [
  "index.js",
  "challenge.js",
  "challenge.test.js",
  "contract.test.js",
  "safe-open-lifecycle.js",
  "safe-open-lifecycle.test.js",
  "verification-checkpoint.js",
  "verification-checkpoint.test.js",
  "package.json",
  "package-lock.json",
  "README.md",
];

const scriptDir = path.dirname(fileURLToPath(import.meta.url));
const repositoryRoot = path.resolve(scriptDir, "..");
const sourceRoot = path.join(repositoryRoot, "mcp");

export async function stageRelease(targetRoot) {
  await mkdir(targetRoot, { recursive: true });
  for (const file of MCP_RELEASE_FILES) {
    const source = path.join(sourceRoot, file);
    await stat(source);
    await copyFile(source, path.join(targetRoot, file));
  }
}

if (process.argv[1] && path.resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  const [target] = process.argv.slice(2);
  if (target === "--list") {
    process.stdout.write(`${MCP_RELEASE_FILES.join("\n")}\n`);
  } else if (target) {
    await stageRelease(path.resolve(target));
  } else {
    throw new Error("usage: node scripts/stage-mcp-release.mjs <target-directory>|--list");
  }
}
