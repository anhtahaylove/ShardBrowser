# v1 authorization audit

Checked every v1 route for the flaw fixed in #21, where eight `/v2/` fleet
handlers accepted a `tenant_id` from the request body or path and never
confirmed the caller belonged to it. Any authenticated user could lease,
upload to or download from any tenant.

**No v1 route has the equivalent flaw.**

## Method

Two questions per route:

1. Does it read an owner, tenant or account identifier from the request, rather
   than deriving it from the authenticated session?
2. If it names a resource by id, is access to that resource checked, or is
   authentication treated as sufficient?

## Result

| Area | Routes | Guard |
| --- | --- | --- |
| `envs` | get/update | `load_accessible` (ACL) |
| `envs` | create/delete | `require_admin` |
| `envs` | list | ACL-filtered in SQL, bound to `user.id` |
| `envs/:id/*` locks | checkout/lease/checkin/status/download | `load_accessible` |
| `envs/:id/*` locks | force-unlock | `require_admin` |
| `envs/:id/*` locks | release | constrained by `owner_user_id` — see below |
| `folders` | create/update/delete | `require_admin` |
| `folders` | list | ACL-filtered in SQL, bound to `user.id` |
| `proxies` | list/create/delete | `require_admin` |
| `acl` | grant/revoke | `require_admin` |
| `users` | list/create/delete/set-role/reset-password | `require_admin` |
| `audit` | list | `require_admin` |
| `me`, `me/password`, `auth/*` | — | scoped to the session's own user |

The two `list` routes take no guard at the top of the handler because they do
not name a resource: each branches on `user.is_admin()` and otherwise runs a
recursive-CTE query whose ACL lookup is bound to `user.id`. A non-admin sees
only what has been granted, so there is nothing for a caller to assert.

### `release` has no ACL check, and is correct anyway

`locks::release` is the one handler that names an environment without calling
`load_accessible`. It is safe for a structural reason rather than a check:

```sql
DELETE FROM locks
 WHERE env_id = ? AND owner_user_id = ? AND owner_client_id = ? AND lock_token = ?
```

The delete is constrained by `owner_user_id`, so a caller can only release a
lock they hold, and must present the matching client id and token. Adding an
ACL lookup would change nothing about what the statement can affect.

Recording it here because it looks like the v2 flaw at a glance, and a future
reader should not have to re-derive why it is not.

## Why v1 avoided the flaw and v2 did not

v1 derives identity from the session: `AuthUser` gives a `user.id`, and every
ownership decision follows from it. Nothing in v1 asks the client which tenant
it belongs to, because v1 has no tenants.

v2 introduced tenant-scoped resources but kept `AuthUser` as-is, so `tenant_id`
had to arrive from somewhere — and it arrived from the request. At each
individual call site "the user is authenticated" looked like enough. It was not,
and the shape of the mistake was invisible until all thirteen routes were listed
side by side.

The lesson worth keeping: when an identifier that scopes authorization comes
from the caller, an authentication check is not an authorization check.

## Verification

- `require_tenant_member` appears 11 times in `server/src/routes/v2.rs`,
  covering all eight fleet handlers.
- `server/tests/v2_e2e.rs` contains the cross-tenant regression: a user with no
  account in the target tenant receives `201 Created` before the fix and is
  refused after it.
