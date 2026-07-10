# Custom integration release

The fork release workflow builds the three Launcher platforms and packages the
matching `mcp/` source as `ShardX-MCP.tar.gz`. Launcher Settings downloads that
asset from:

```text
https://github.com/anhtahaylove/ShardBrowser/releases/download/v0.1.12/ShardX-MCP.tar.gz
```

The URL is compiled from `CARGO_PKG_VERSION`, so an older Launcher keeps using
the MCP source from its matching release instead of silently downloading a
future incompatible helper bundle. The release workflow rejects a tag that does
not exactly match `v<package version>`.

## Windows signing inputs

Configure these GitHub Actions repository secrets before creating a public tag:

- `WINDOWS_CERTIFICATE`: base64-encoded PFX containing the private key and a
  certificate with the Code Signing EKU.
- `WINDOWS_CERTIFICATE_PASSWORD`: password for that PFX.

The certificate must chain to a public trust root for public downloads. A
self-signed certificate can be useful for a controlled internal trust store,
but it does not make a generally trusted public installer.

The workflow imports the PFX only into the ephemeral Windows runner, passes its
thumbprint to the Tauri bundler, timestamps signatures, verifies every collected
`.exe` and `.msi` with `Get-AuthenticodeSignature`, then removes the imported
certificate and temporary files. Publication fails unless each Windows
signature reports `Valid`.

## Internal and public runs

- **Internal signed build:** run the Release workflow manually and leave
  `publish_release` disabled. Download the per-platform workflow artifacts.
- **Public release:** push a version tag such as `v0.1.12`, or manually run the
  workflow with `publish_release` enabled. The release receives native bundles,
  `ShardX-MCP.tar.gz`, `SHA256SUMS.txt`, and GitHub build-provenance
  attestations.

Do not create a release tag until the signing secrets are configured: the
Windows matrix job intentionally blocks unsigned public releases.
