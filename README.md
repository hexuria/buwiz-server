# buwiz-server

Philippine tax SaaS backend and web app for **Buwiz**: verified accounts, exclusive cloud tax-profile ownership, and a desktop-friendly login URL.

This is the first working slice on [hexuria/buwiz-server](https://github.com/hexuria/buwiz-server). It is a Spin + Leptos fullstack app scaffolded from [codeitlikemiley/ddd-cqrs-es](https://github.com/codeitlikemiley/ddd-cqrs-es) `examples/fullstack-app` (revision in `.template-revision`). Identity stays in **wasi-auth**; tax profiles are a product aggregate on Postgres.

| | |
|--|--|
| Runtime | Fermyon Spin (`wasm32-wasip2`) + Leptos islands |
| Auth | wasi-auth sessions, email verification, orgs/RBAC, mail outbox |
| Database | PostgreSQL (durable events + read models) |
| Realtime | Redis pub/sub **wake only** — clients replay from Postgres |
| Default listen | `http://localhost:3008` |

Desktop IMAP secrets, local PIN, and TOTP are **not** stored on this server.

---

## Run locally

```bash
cp .env.example .env
# .env may leave AUTH_ROOT_KEY_BASE64 / AUTH_OUTBOX_KEY_BASE64 empty;
# `make` writes them to .auth-root-key and .auth-outbox-key if needed.

make db-up          # Postgres :54329 and Redis :6379
make dev transport=both
# open http://localhost:3008  (prefer localhost over 127.0.0.1 for passkeys)
```

`make dev` starts **Spin** and the native **`wasi-auth-outbox-worker`**. `make spin` serves the app but does **not** deliver verification email.

| Target | Purpose |
|--------|---------|
| `make db-up` | Postgres + Redis |
| `make db-migrate` | wasi-auth schema + Buwiz migrations (`0001`, `0002`) |
| `make dev` | Spin + outbox worker (register / verify) |
| `make check` | `wasm32-wasip2` compile |
| `make smoke` | REST + web route checks against `BASE_URL` |
| `cargo test --lib` | Domain tests (exclusive ownership) without Spin |

Password minimum length is **15** (wasi-auth default in this stack).

### Email: capture vs Resend

| Mode | When |
|------|--------|
| **capture** (default) | Local. Outbox stores the message. After register, open `/verify-email` / resend and use **Open captured verification link** when `AUTH_DEV_TOOLS=true`. |
| **resend** | Set `AUTH_MAIL_TRANSPORT=resend`, `AUTH_RESEND_API_KEY`, and `AUTH_RESEND_FROM` (verified domain). The API key stays on the **native worker**, never in Spin. |

```bash
# Real inbox
AUTH_MAIL_TRANSPORT=resend \
AUTH_RESEND_API_KEY=re_... \
AUTH_RESEND_FROM='Buwiz <auth@your-verified-domain.example>' \
make dev transport=both
```

The worker uses `AUTH_MAIL_PRODUCT_NAME=Buwiz` in templates.

---

## Desktop OAuth / device login

wasi-auth is an OAuth **client** (Google/Apple/Facebook). The Buwiz desktop app needs this service as a first-party **authorization server**.

Public native client id: `buwiz-desktop` (no client secret; PKCE S256).

| Endpoint | URL |
|----------|-----|
| Metadata | `GET {AUTH_PUBLIC_BASE_URL}/.well-known/oauth-authorization-server` |
| Authorize (PKCE) | `GET {AUTH_PUBLIC_BASE_URL}/oauth/authorize` |
| Token | `POST {AUTH_PUBLIC_BASE_URL}/oauth/token` |
| Device code | `POST {AUTH_PUBLIC_BASE_URL}/oauth/device/code` |
| User verification | `{AUTH_PUBLIC_BASE_URL}/login/device` |

### Authorization-code + PKCE

1. Desktop opens a browser (or embedded webview) at:

   `{AUTH_PUBLIC_BASE_URL}/oauth/authorize?response_type=code&client_id=buwiz-desktop&redirect_uri=buwiz://auth/callback&code_challenge=...&code_challenge_method=S256&state=...`

2. User registers or logs in on the web, then authorizes the desktop app.
3. Browser redirects to `redirect_uri?code=...&state=...`.
4. Desktop exchanges the code at `/oauth/token` (`grant_type=authorization_code`) with `code_verifier`. Refresh tokens use `grant_type=refresh_token`.

**Allowed `redirect_uri` values**

- `buwiz://auth/callback` (custom scheme)
- Loopback `http://` / `https://` on `127.0.0.1`, `localhost`, or `[::1]` (any port)
- Extra comma-separated URIs in `DESKTOP_OAUTH_REDIRECT_URIS` / Spin variable `desktop_oauth_redirect_uris`

### Device code (headless / TV-style)

```bash
curl -sS -X POST http://localhost:3008/oauth/device/code \
  -H 'content-type: application/json' \
  -d '{"client_id":"buwiz-desktop"}'
```

Poll `/oauth/token` with `grant_type=urn:ietf:params:oauth:grant-type:device_code` until the user finishes `/login/device`.

Scope issued: `openid profile email tax_profiles`.

---

## Tax profiles

Cloud subset of the desktop `TaxpayerProfile` (TIN + branch is the uniqueness key). A **personal** account or a **company/org** workspace can hold a profile. Rules:

1. Only one holder controls a TIN+branch at a time.
2. Personal hold is exclusive to that user (and their agents).
3. A company may hold it until the verified owner claims or reclaims.
4. The verified owner can reclaim at any time.
5. If the owner has no account yet, a company may create/hold; the owner claims later with TIN proof.
6. Concurrent claims fail safely (expected revision + unique `(tin_root, branch_code)`).

Email must be **verified** before create/claim/reclaim/transfer.

| REST | |
|------|--|
| `GET /api/tax-profiles` | List held profiles |
| `POST /api/tax-profiles` | Create |
| `POST /api/tax-profiles/claim` | Claim from company hold (ORUS proof) |
| `POST /api/tax-profiles/reclaim` | Take back control |
| `POST /api/tax-profiles/transfer` | Transfer to an org you belong to |

UI: `/tax-profiles`.

Claim/reclaim call `TinOwnershipVerifier`. Production ORUS is **not** implemented. Set `ORUS_FAKE_VERIFIER=true` for local proof. See [docs/extensions.md](docs/extensions.md).

---

## Redis wake

After a tax-profile commit, the guest publishes a JSON wake on `REDIS_CHANNEL` (default `buwiz-tax-profiles`):

```json
{ "kind": "tax_profile", "stream_id": "...", "revision": 1 }
```

Postgres remains the source of truth. A missing Redis URL, or a publish error, is fail-open (logged). Follow [ddd-cqrs-es Redis guidance](https://github.com/codeitlikemiley/ddd-cqrs-es/blob/main/docs/production/redis.md): subscribe, then reload the projection.

Compose Redis uses `--maxmemory-policy noeviction` so a future event-store experiment cannot evict stream keys.

---

## WebMCP

Browser agents can drive register/login via **declarative HTML attributes** (and an optional hydrate-only `document.modelContext.registerTool`). This is **not** a backend MCP server.

See [docs/webmcp.md](docs/webmcp.md). Flag: `WEBMCP_ENABLED` (default on in local Spin vars). Auth forms never use `toolautosubmit`.

---

## Env vars (product)

Copy `.env.example`. Secrets stay out of git (`.env`, `.auth-root-key`, `.auth-outbox-key`).

| Variable | Role |
|---------|------|
| `AUTH_ROOT_KEY_BASE64` | 32-byte root secret (required at runtime) |
| `AUTH_OUTBOX_KEY_BASE64` | Native worker mail sealing |
| `AUTH_MAIL_TRANSPORT` | `capture` \| `resend` \| `http` |
| `AUTH_RESEND_API_KEY` / `AUTH_RESEND_FROM` | Resend (worker only) |
| `AUTH_PUBLIC_BASE_URL` | Derived from `make … listen=` unless set |
| `POSTGRES_URL` | Default `postgres://wasi_auth:wasi_auth_dev@127.0.0.1:54329/wasi_auth` |
| `REDIS_URL` | Empty skips wake publishes |
| `REDIS_CHANNEL` | Default `buwiz-tax-profiles` |
| `ORUS_FAKE_VERIFIER` | Local TIN proof |
| `WEBMCP_ENABLED` | Imperative WebMCP registration |
| `DESKTOP_OAUTH_REDIRECT_URIS` | Extra native redirect URIs |
| `AUTH_DEV_TOOLS` | Capture-mail UI helpers |

---

## Version pins

| Crate | Pin | Why |
|-------|-----|-----|
| `wasi-auth` | `=0.1.0-rc.5` | crates.io |
| `leptos-wasi-runtime` | `=0.4.2-rc.1` | crates.io |
| `ddd_cqrs_es` | git `71ac721f48cf236244abc76c17d39e2cd58e1341` | crates.io does not publish `0.4.0-alpha.3` yet |
| `spin-sdk` | git `a02d330fe9357be2d18e6deef400511195ce6f7f` | audited wasip3 graph (crates.io patch) |

Rust **1.93.0**. Toolchain is gated by `scripts/verify_toolchain.sh`.

---

## What is stubbed (not in this slice)

- Live **ORUS** TIN verification — trait + `FakeOrusVerifier` only
- Filing-history sync with headless-bir — [docs/sync-api.md](docs/sync-api.md)
- BIR SFTP / TSP submission relay ([hexuria/buwiz-forms#44](https://github.com/hexuria/buwiz-forms/issues/44)) — not implemented on purpose
- Copying desktop profile encryption / email OAuth secrets to cloud

---

## Layout

```
src/domain/           tax profile aggregate, TIN, ORUS port, desktop redirect policy
src/application/       verified-email gate + commands
src/store/            Postgres events/projection + Redis wake + OAuth codes
src/desktop_oauth.rs  first-party AS
src/app/              Leptos UI (auth, tax profiles, WebMCP)
migrations/postgres/  0001 app storage, 0002 tax profiles + OAuth
docs/                 webmcp, sync API, extensions
```
