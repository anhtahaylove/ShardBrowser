# Release signing readiness

Public `v*` tag releases and manual runs with `publish_release=true` must use a
trusted Windows Authenticode certificate. Manual internal builds may use the
same PFX secret format with an internal self-signed certificate, but those runs
do not publish a public release.

Required secrets:

- `WINDOWS_CERTIFICATE`: base64-encoded PFX.
- `WINDOWS_CERTIFICATE_PASSWORD`: PFX password.
- `TAURI_SIGNING_PRIVATE_KEY`: Tauri updater signing private key.
- `TAURI_SIGNING_PRIVATE_KEY_PASSWORD`: Tauri updater signing key password.

The Authenticode certificate and Tauri updater key serve different trust
boundaries. Authenticode establishes the Windows publisher and trusted chain;
the updater key verifies downloaded update bytes before installation. Neither
key can replace the other.

Before native builds start, the workflow signs a disposable fixture with the
updater private-key secret and verifies it using the exact public key shipped
in `src-tauri/tauri.conf.json`. A rotated or mismatched key therefore stops the
release before any artifact can be published.

Optional repo variable:

- `EXPECTED_WINDOWS_CERTIFICATE_SUBJECT`: exact expected Windows signer subject.

Public release gates reject self-signed certificates and require a private key,
Code Signing EKU, a trusted certificate chain, and at least 30 days remaining
validity. Windows release assets must share the same signer/thumbprint, include
an Authenticode timestamp, pass `Get-AuthenticodeSignature` as `Valid`, and pass
`signtool verify /pa /all /v`.

The workflow records cleanup handles before certificate validation and removes
the temporary PFX, generated Tauri signing config, and every imported
certificate in an `always()` step, including expiry, subject, chain, or build
failure paths.

`latest.json` deterministically selects the single Windows NSIS updater bundle,
the Apple Silicon app archive, and the Linux x86_64 AppImage. Duplicate or
missing updater bundles fail the release. The manifest, updater `.sig` files,
and all release assets are included in checksums and GitHub build-provenance
attestation.

## First updater-capable release

Installed `v0.1.21` builds do not contain the Tauri updater plugin, so they
cannot self-update to the first updater-capable release. Users must manually
install that first trusted release from GitHub once. Later releases can use the
in-app Download, signature verification, and Install and restart flow without
replacing profile data, settings, the Automation API token, or the MCP folder.

## Internal dry run

A manual workflow run with `publish_release=false` may use the existing
self-signed internal PFX. It builds and verifies artifacts but does not create a
public GitHub Release. Public tag runs and `publish_release=true` reject a
self-signed certificate and require a CA-issued code-signing certificate whose
chain is trusted by Windows.
