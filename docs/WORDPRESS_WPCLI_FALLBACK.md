# WP-CLI fallback for WordPress plugin tasks

## Purpose

When a WordPress administration page is blocked by a Cloudflare verification
handoff, browser automation should pause instead of interacting with the
challenge. For plugin administration on a site the operator is authorized to
manage, WP-CLI over SSH can be an explicit fallback that does not depend on the
`wp-admin` browser session.

This is an integration design, not an enabled remote-execution feature in the
current Launcher. ShardX continues to use the visible-browser verification
handoff unless the operator separately configures and explicitly invokes a
future WP-CLI action.

## Required operator setup

- SSH access to the WordPress host with host-key verification enabled.
- A dedicated, least-privilege operating-system account that can run WP-CLI for
  the intended WordPress installation.
- WP-CLI installed on the host and a fixed WordPress root path.
- Authentication through Windows OpenSSH and `ssh-agent`, or a protected key
  file. Secrets must not be stored in Launcher settings, MCP arguments, logs,
  documentation, or task output.
- A tested backup/rollback process before plugin updates.

## Allowlisted action contract

A future helper should accept structured fields rather than an arbitrary shell
command:

```json
{
  "site": "configured-site-alias",
  "action": "list | status | activate | deactivate | update",
  "plugin": "plugin-slug",
  "dry_run": true
}
```

Rules:

1. `action` is an enum. Do not expose generic `wp`, SSH, shell, PHP, SQL, eval,
   install-from-URL, file-edit, or delete commands.
2. Require a plugin slug matching `^[a-z0-9][a-z0-9-]{0,99}$` for every
   mutating action.
3. Default to `dry_run: true`. Resolve and display the configured host alias,
   WordPress root, current plugin state, and intended action before mutation.
4. Execute Windows OpenSSH directly with an argument array; do not interpolate
   user values into `cmd.exe`, PowerShell, or a remote shell string.
5. Pin the remote host key, set connection/command timeouts, cap output size,
   and return sanitized stdout/stderr without environment variables or paths
   containing credentials.
6. Run one mutation at a time. Re-query plugin state afterward and report the
   observed result rather than assuming success from exit code alone.

The remote command mapping should remain fixed and reviewable:

| Action | WP-CLI operation |
| --- | --- |
| list | `wp --path=<configured-root> plugin list --format=json` |
| status | `wp --path=<configured-root> plugin status <slug>` |
| activate | `wp --path=<configured-root> plugin activate <slug>` |
| deactivate | `wp --path=<configured-root> plugin deactivate <slug>` |
| update | `wp --path=<configured-root> plugin update <slug>` |

`<configured-root>` and the SSH destination come from a local named-site
configuration controlled by the operator, never from an MCP call.

## Handoff flow

1. ShardX detects Cloudflare, records a verification checkpoint, reports
   `Verification required` to Launcher, and sends one Windows notification.
2. The browser task pauses. It never clicks or attempts to solve the challenge.
3. For a requested plugin-management operation only, the operator or agent may
   explicitly choose the configured WP-CLI fallback.
4. The helper performs a read-only preflight, then either returns the dry-run
   plan or executes one allowlisted action.
5. The helper verifies plugin state and records an audit result containing only
   site alias, action, plugin slug, timestamps, exit status, and sanitized
   output.
6. Browser automation remains checkpointed until human verification clears or
   the browser task is intentionally abandoned.

WP-CLI is not a generic Cloudflare bypass and must not be used to access other
browser-only pages or to discover an origin address behind the WAF.

## Suggested implementation boundary

If implemented later, keep this in a separate MCP module and tool such as
`wordpress_plugin_action`. Do not add it to Launcher core, browser launch
arguments, profile storage, fingerprint logic, or the Cloudflare detector. The
first implementation should support only `list` and `status`; add one mutating
action at a time with targeted tests for argument validation, dry-run behavior,
timeouts, output redaction, and post-action state verification.
