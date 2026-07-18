# Custom integration release

The fork release workflow builds the three Launcher platforms and packages the
matching `mcp/` source as `ShardX-MCP.tar.gz`. Launcher Settings downloads that
asset from:

```text
https://github.com/anhtahaylove/ShardBrowser/releases/download/v<version>/ShardX-MCP.tar.gz
```

The URL is compiled from `CARGO_PKG_VERSION`, so an older Launcher keeps using
the MCP source from its matching release instead of silently downloading a
future incompatible helper bundle. The release workflow rejects a tag that does
not exactly match `v<package version>`.

## Updater signing inputs

Configure these GitHub Actions repository secrets before creating a release:

- `TAURI_SIGNING_PRIVATE_KEY`: private key used only for Tauri updater
  artifacts.
- `TAURI_SIGNING_PRIVATE_KEY_PASSWORD`: password for that updater key.

The workflow signs a disposable fixture and verifies it with the exact public
key embedded in `src-tauri/tauri.conf.json` before native builds start. Each
platform updater bundle then receives a matching `.sig`, and `latest.json`
publishes those signatures for the in-app updater.

This follows the Youwee-style release model: Windows installers and portable
executables are intentionally **not Authenticode-signed**. Windows can show
SmartScreen or **Unknown Publisher** during a manual install. A `.exe.sig` or
`.msi.sig` is a Tauri updater signature; it verifies update bytes inside the
app and does not establish a trusted Windows publisher.

## Internal and public runs

- **Internal updater-signed build:** run the Release workflow manually and leave
  `publish_release` disabled. Select the branch or ref that contains this
  custom workflow, pass the matching tag such as `v0.1.23`, and download the
  per-platform workflow artifacts. The workflow rejects manual runs when the
  requested tag does not match the package versions, and artifact names include
  the tag to avoid confusing multiple internal builds.
- **Public release:** push a version tag such as `v0.1.23`, or manually run the
  workflow with `publish_release` enabled. The release receives native bundles,
  `ShardX-MCP.tar.gz`, `SHA256SUMS.txt`, and GitHub build-provenance
  attestations.

Do not create a release tag until the updater signing secrets are configured.
The workflow intentionally blocks artifacts that are missing updater
signatures, but it does not require a PFX or Authenticode certificate.

When the custom release workflow is still only on a feature branch, run it from
that branch explicitly, for example:

```powershell
gh workflow run Release --repo anhtahaylove/ShardBrowser --ref codex/custom-integration-v0.1.12 -f tag=v0.1.23 -f publish_release=false
```

The internal workflow artifacts are enough to test updater-signed Launcher installation.
Settings still downloads MCP from the public fork release URL compiled into that
Launcher version, so the end-to-end Download MCP button is only fully available
after the matching public release asset exists.
