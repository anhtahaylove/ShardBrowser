# Snapshot safety port

This branch ports only the `shared/` snapshot safety crate from `chovizzz/ShardBrowser#2`.

Included:

- portable profile snapshot pack/unpack helpers
- cache/temp exclusion rules
- path traversal, Windows-reserved-name, symlink, depth, entry-count, and decompression-bomb guards
- atomic unpack staging/backup behavior
- os_crypt helpers and portable Cookies/Web Data/Login Data re-sealing tests

Out of scope:

- Team Server auth/lock API
- Launcher Team UI / remote commands
- browser engine, fingerprint, profile isolation, launch behavior

This keeps snapshot validation reviewable before wiring it into team-server checkin/checkout flows.
