# Hostnames

One buwiz-server deployment answers two names. The marketing site is a
separate Spin app.

| Host | What it serves |
|------|----------------|
| `https://buwiz.com` and `https://www.buwiz.com` | Marketing site ([hexuria/buwiz.com](https://github.com/hexuria/buwiz.com)). Download links and a sign-in link. No accounts, no database. |
| `https://account.buwiz.com` | This server's Leptos UI: login, device approval, passkeys, tax profiles. `AUTH_PUBLIC_BASE_URL`. |
| `https://api.buwiz.com` | The same process, for bearer clients. Desktop default, MCP, `/oauth`, `/auth`, `/api`, `/me`. |

`account.buwiz.com` is the canonical browser origin because the session
cookie is host-only. Production sets `__Host-session` (`Secure`, `Path=/`,
`HttpOnly`, `SameSite=Lax`) and `__Host-` forbids a `Domain` attribute, so
the cookie is never sent to `buwiz.com` or `api.buwiz.com`. Sharing it with
`Domain=.buwiz.com` would also hand it to the marketing site. `SameSite=Lax`
would not carry it on a cross-site XHR anyway.

A document navigation that arrives on `api.buwiz.com` (no `Authorization`
header, not a JSON-only `Accept`, not an `/api`, `/auth`, `/oauth`, `/mcp`,
`/.well-known`, `/pkg`, `/me`, or `/health` path) is redirected to the same
path on `account.buwiz.com`. Passkeys therefore always see rpId
`account.buwiz.com`. Bearer calls stay on the API host.

## Variables

| Variable | Local default | Production |
|----------|---------------|------------|
| `AUTH_PUBLIC_BASE_URL` | `http://localhost:3008` | `https://account.buwiz.com` |
| `AUTH_API_BASE_URL` | empty (no redirect) | `https://api.buwiz.com` |
| `AUTH_CORS_ORIGINS` | empty | `https://buwiz.com,https://www.buwiz.com` |
| `AUTH_MARKETING_BASE_URL` | `https://buwiz.com` | `https://buwiz.com` |

CORS allows the account origin, the API origin when set, and
`AUTH_CORS_ORIGINS`. Social OAuth redirect URIs stay on the account origin
(`/api/auth/oauth/<provider>/callback`).

The desktop app's default server is `https://api.buwiz.com`
(`BUWIZ_SERVER_URL`, or Settings → Account). It signs in with the device
flow and stores a bearer refresh token. It does not use the session cookie.
Filing still runs on the computer when this server is unreachable.
