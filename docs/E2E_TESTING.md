# Launcher UI regression tests

The Launcher UI suite uses `@playwright/test` against a built Vite frontend.
It loads a test-only Tauri IPC mock before React renders, so tests never call the
real Automation API or read or modify local profiles.

## Run locally

```powershell
npm ci
npx playwright install chromium
npm run test:e2e
```

`npm run test:e2e` builds with Vite's `e2e` mode, starts a local preview server
on `127.0.0.1:4173`, and runs Chromium at the regression viewport of
`1296x839`. The suite also switches to a larger desktop viewport where needed.

## Fixture boundary

`src/e2e-mock.ts` contains only synthetic profiles, RFC 5737 network data, blank
credentials, and a placeholder token value. Do not add real tokens, cookies,
proxy credentials, profile paths, profile storage, or fingerprint payloads.
Test scenarios are selected with URL query parameters and are available only in
the `e2e` build mode.

## Covered regressions

- Startup/profile loading, inline failures, Retry, and single-flight reloads.
- Profile/proxy empty states, search shortcuts, responsive actions, and More.
- Settings dirty/restart state, sticky Save, MCP/Codex action hierarchy.
- Keyboard behavior for dialogs and `CSSelect`.
- Updater checking, no update, available, progress, consent, invalid signature,
  and offline failures.
- Accessible checkbox names, inline launch errors, and assertive error toasts.
- Dark/light theme behavior at narrow and large desktop viewports.

Playwright retains a trace and screenshot only when a test fails. CI uploads
`test-results/` for failed E2E runs and retains it for seven days.
