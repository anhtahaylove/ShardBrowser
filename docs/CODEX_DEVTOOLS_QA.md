# Codex ShardBrowser + Chrome DevTools QA

Use this when a Codex task needs both a ShardX anti-detect profile and
DevTools-grade console/network/performance evidence.

## Current attach audit

- `shardbrowser` MCP starts the chosen ShardX profile and returns CDP data with
  `devtools_context`.
- Local `chrome_devtools` is currently registered as an isolated Google Chrome
  runner, so it audits its own Chrome instance, not a ShardX profile.
- `chrome-devtools-mcp@1.5.0` supports `--browserUrl` and `--wsEndpoint`, but
  the exposed Codex tools do not include a runtime attach/connect call. Pick the
  browser in the MCP client config before startup.
- Keep this integration at the MCP/client layer. Do not wire
  `chrome_devtools` into Launcher core or browser fingerprint code.

## Workflow

1. Check Launcher auth:
   ```text
   mcp__shardbrowser.health_check
   ```
2. Open the target page in the intended profile:
   ```text
   mcp__shardbrowser.safe_open_url({
     "profile_query": "VN Automation 001 - No Proxy",
     "exact": true,
     "url": "https://example.com/"
   })
   ```
3. Get the live CDP handoff:
   ```text
   mcp__shardbrowser.devtools_context({
     "profile_query": "VN Automation 001 - No Proxy",
     "exact": true
   })
   ```
   If the profile is already running without CDP, stop and restart it through
   ShardBrowser MCP or the Automation API; a running browser process cannot get
   a CDP port retrofitted in place.
4. For ShardX-only checks, continue with `shardbrowser` tools:
   `browser_aria_snapshot`, `browser_screenshot`, `browser_capture_start`,
   `browser_capture_stop`, and `browser_set_network_conditions`.
5. For Chrome DevTools MCP checks against the ShardX page, register a second
   MCP entry from the returned CDP URL, then restart Codex:
   ```powershell
   codex mcp add shardbrowser-devtools -- cmd /c npx -y chrome-devtools-mcp@1.5.0 --browserUrl http://127.0.0.1:<cdp-port> --no-usage-statistics --no-performance-crux --redactNetworkHeaders
   ```
6. After restart, use `chrome_devtools`/the configured DevTools namespace for
   `take_snapshot`, `list_console_messages`, `list_network_requests`,
   `performance_start_trace` / `performance_stop_trace`, and
   `lighthouse_audit`.

## End-to-end prompt goal

```text
Use ShardBrowser MCP to health_check, open the requested URL in the exact
profile, call devtools_context, and report the CDP http_url plus current page
title/url. If chrome_devtools is still configured as isolated Chrome, do not
claim it audited the ShardX profile; either run ShardBrowser-native screenshot,
a11y, and network checks, or give the exact Codex MCP add/repair command needed
to register chrome-devtools-mcp with the returned --browserUrl. Never print
SHARDX_TOKEN, cookies, fingerprint payloads, or proxy credentials.
```
