# Custom integration release

The fork release workflow builds the three Launcher platforms and packages the
matching `mcp/` source as `ShardX-MCP.tar.gz`. Launcher Settings downloads that
asset from:

```text
https://github.com/anhtahaylove/ShardBrowser/releases/download/v0.1.16/ShardX-MCP.tar.gz
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

Prefer a certificate that chains to a public trust root for public downloads. A
self-signed certificate can sign custom builds, but Windows may show an
unknown-publisher prompt until the certificate is trusted locally.

Self-signed PFX builds verify by signer thumbprint without adding the certificate
to the runner trust stores. Publicly trusted certificates still verify with the
normal Windows `Valid` Authenticode status.

The workflow imports the PFX only into the ephemeral Windows runner, passes its
thumbprint to the Tauri bundler, timestamps signatures, verifies every collected
`.exe` and `.msi` with `Get-AuthenticodeSignature`, then removes the imported
certificate and temporary files. Publication fails unless each Windows
artifact is signed by the configured certificate. Publicly trusted certificates
must report `Valid`; self-signed certificates must match the configured signer
thumbprint.

## Internal and public runs

- **Internal signed build:** run the Release workflow manually and leave
  `publish_release` disabled. Select the branch or ref that contains this
  custom workflow, pass the matching tag such as `v0.1.16`, and download the
  per-platform workflow artifacts. The workflow rejects manual runs when the
  requested tag does not match the package versions, and artifact names include
  the tag to avoid confusing multiple internal builds.
- **Public release:** push a version tag such as `v0.1.16`, or manually run the
  workflow with `publish_release` enabled. The release receives native bundles,
  `ShardX-MCP.tar.gz`, `SHA256SUMS.txt`, and GitHub build-provenance
  attestations.

Do not create a release tag until the signing secrets are configured: the
Windows matrix job intentionally blocks unsigned public releases.

When the custom release workflow is still only on a feature branch, run it from
that branch explicitly, for example:

```powershell
gh workflow run Release --repo anhtahaylove/ShardBrowser --ref codex/custom-integration-v0.1.12 -f tag=v0.1.16 -f publish_release=false
```

The internal workflow artifacts are enough to test signed Launcher installation.
Settings still downloads MCP from the public fork release URL compiled into that
Launcher version, so the end-to-end Download MCP button is only fully available
after the matching public release asset exists.
