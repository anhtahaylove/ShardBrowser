# ShardX Team Server core port

This branch ports only the self-hosted team-server core from
`chovizzz/ShardBrowser#2`:

- user login with Argon2 password hashes and bearer tokens
- admin/member roles
- folder/env/proxy CRUD with ACL filtering
- exclusive checkout locks with `lock_token` + lease renewal
- opaque snapshot upload/download with retention GC and SHA-256 metadata
- login throttling, token invalidation on password change, audit logging

Out of scope for this branch:

- Launcher Team UI / remote workspace commands
- portable cookie/Login Data/Web Data pack/unpack
- browser engine, fingerprint, profile isolation, proxy launch behavior

Snapshot bytes are intentionally opaque here. The portable snapshot safety layer
is ported separately so it can be reviewed without mixing it into server auth
and lock behavior.
