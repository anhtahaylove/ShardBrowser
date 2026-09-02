# Custom fonts feasibility gate

Status: RFC / release gate for issue #9.
Recommendation for `v0.1.22`: do not expose or claim custom-font support.

## Scope

Issue #9 asks for per-profile font folders, explicit font IDs/names, random
font selection from folders, and append/replace modes so profiles do not all
present the same system font set.

This document gates that request at the browser-engine boundary. Launcher-only
manifest plumbing is not enough to make the feature true.

## Proven locally

- Existing persisted profile data can retain a legacy `custom_fonts` object for
  forward compatibility; profile edits do not intentionally erase that stored
  value.
- The Launcher does not write a custom-font manifest, pass an unproven engine
  switch, or include `custom_fonts` in the runtime fingerprint payload.
- The profile editor does not expose controls that would imply the bundled
  engine supports custom font folders.
- Automation API create, temporary-create, and fingerprint-replacement edits
  reject new `custom_fonts` input with a clear coherence-gate error.
- MCP schemas and default summaries do not expose custom-font paths or options.
- Rust regression tests lock the inert launch behavior and API rejection;
  Playwright locks that ordinary Launcher edits preserve sanitized legacy
  metadata while keeping the unsupported controls hidden.

## Unproven / not release-ready

- The bundled browser engine is closed source, and available string scanning did
  not prove that it consumes `--shardx-custom-fonts-manifest`.
- No verified engine contract exists for append vs replace semantics.
- No verified coherence exists across:
  - `document.fonts`
  - CSS font matching and fallback
  - rendered glyph availability
  - canvas text measurement and pixels
  - WebGL/font-adjacent fingerprint surfaces
  - font enumeration APIs or JS-visible probes
  - per-profile isolation
- Therefore a Launcher UI that promises custom fonts would be a fake feature
  unless the engine contract and coherence tests pass.

## Minimum engine contract

Before enabling public UI/docs, the engine must explicitly support the manifest
switch and document these behaviors:

- Accept `--shardx-custom-fonts-manifest=<absolute path>`.
- Read a JSON manifest with `mode`, `dirs`, `names`, and `random_count`.
- Treat `off` as no custom font mutation.
- Treat `append` as adding eligible manifest fonts to the profile-visible font
  set without removing normal profile/system fonts.
- Treat `replace` as exposing only the selected manifest fonts plus any minimal
  engine-required fallback fonts needed for stable rendering.
- Resolve `names` against actual font family/full names in the allowed dirs.
- Select `random_count` deterministically per profile or from a documented seed
  source so restarts do not silently reshuffle one profile.
- Apply the same selected font set to CSS, `document.fonts`, canvas, WebGL-
  adjacent observable output, and enumeration/fingerprint surfaces.
- Keep selection and caches scoped to the profile user-data-dir.
- Fail closed: invalid manifests, unreadable dirs, unsupported font files, or
  malformed names must not fall back to leaking an unrelated host-wide set while
  claiming replace mode.

## Security and path validation

Launcher-side validation should stay conservative:

- Only absolute, canonical, existing directories.
- No commas in directory values because paths are also used in comma-separated
  launch contexts elsewhere.
- No control characters in names or paths.
- Cap list/string sizes and `random_count`.
- Do not accept remote paths, URLs, glob patterns, or inline font bytes.
- Do not copy or log font file contents, profile storage, cookies, proxy
  credentials, tokens, or fingerprint payloads.
- The manifest path must live under the target profile user-data-dir and should
  be regenerated per launch.

Engine-side validation must repeat trust-boundary checks. The engine cannot
trust the Launcher manifest because users can edit files under the profile dir.

## Compatibility and migration

- Preserve legacy `custom_fonts` profile data. Do not delete or rewrite it in a
  migration just because the feature is gated.
- Keep `mode: "off"` as the default.
- If UI is hidden/disabled for `v0.1.22`, stored data should remain inert and
  round-trip through profile edits where possible.
- If engine support later ships with a changed manifest shape, add a versioned
  manifest reader or explicit migration instead of silently reinterpreting old
  profile data.

## Release gate

Do not expose, advertise, or document custom-font support as working until all
of these pass:

1. Engine owner confirms the manifest switch is implemented for the bundled
   browser build used in the release.
2. Launcher writes the expected manifest and switch for a temporary fixture
   profile only.
3. Coherence tests pass for append and replace modes across:
   - `document.fonts`
   - CSS font resolution
   - glyph rendering
   - canvas text metrics/pixels
   - WebGL/font-adjacent fingerprint observations
   - font enumeration/probe behavior
4. Isolation tests prove two temporary profiles with different manifests expose
   different selected sets without cross-profile bleed.
5. Negative tests prove invalid dirs, malformed manifests, missing fonts, and
   unsupported font files fail safely.
6. No tests mutate real user profiles or use real profile storage.

## Safe test matrix

Use only temporary fixture profiles and temporary fixture font directories.
Never use the canonical profile or real user profile dirs.

| Case | Fixture | Expected result |
| --- | --- | --- |
| Off | no manifest or `mode: "off"` | baseline profile font behavior unchanged |
| Append by name | one known fixture font + name | baseline plus fixture font visible and renderable |
| Replace by name | one known fixture font + name | only allowed set plus required fallbacks visible |
| Append random | dir with N fixture fonts, `random_count < N` | deterministic per-profile selected subset |
| Isolation | profile A font A, profile B font B | no selected-set bleed between profiles |
| Bad dir | missing/non-directory path | Launcher/API rejects before launch |
| Bad manifest | malformed JSON under temp profile | engine fails closed |
| Unsupported file | non-font file in dir | ignored or rejected without widening font set |

Suggested probes:

- JS: `document.fonts.check()` and enumerated `FontFaceSet` behavior.
- DOM/CSS: render known text with the fixture family and compare fallback.
- Canvas: compare `measureText()` and pixel hash against baseline.
- Glyph: use a fixture font with a distinctive private-use or uncommon glyph.
- WebGL/fingerprint: compare only reduced pass/fail observations; do not store
  full fingerprint payloads.

## Recommendation for v0.1.22

Ship `v0.1.22` with custom fonts treated as not ready:

- Do not claim issue #9 as fixed.
- Do not market custom fonts in release notes.
- Hide or explicitly disable any user-facing control that implies support until
  the engine contract is confirmed.
- It is acceptable to preserve inert stored `custom_fonts` data for forward
  compatibility, provided the release notes do not present it as a working
  browser fingerprint feature.
