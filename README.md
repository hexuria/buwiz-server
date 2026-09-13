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
| `make db-migrate` | wasi-auth schema + Buwiz migrations (`0001`–`0004`) |
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

## Desktop auth — device flow (preferred)

GPUI desktop has no embedded browser we want to depend on. Do **not** rely on “browser redirect to a random https URL” as the primary path.

Public native client id: `buwiz-desktop` (no client secret).

1. Desktop calls `POST /auth/device/start`.
2. App shows the short `user_code` and an **Open browser** action pointing at `verification_uri` (`/login/device`).
3. User signs up or logs in on the web (email verification still required before tax-profile mutations).
4. Desktop polls `POST /auth/device/poll` until it receives **access + refresh** tokens (or `authorization_pending`).
5. Clients store the **refresh token in the OS keychain**. This server never logs refresh tokens.

| Endpoint | URL |
|----------|-----|
| Device start (primary) | `POST {AUTH_PUBLIC_BASE_URL}/auth/device/start` |
| Device poll (primary) | `POST {AUTH_PUBLIC_BASE_URL}/auth/device/poll` |
| Current user | `GET {AUTH_PUBLIC_BASE_URL}/me` |
| User verification page | `{AUTH_PUBLIC_BASE_URL}/login/device` |
| Metadata | `GET {AUTH_PUBLIC_BASE_URL}/.well-known/oauth-authorization-server` |

`POST /auth/device/start` → `{ device_code, user_code, verification_uri, interval }` (also `expires_in`, `verification_uri_complete`).

`POST /auth/device/poll` with `{ "device_code": "..." }` → access+refresh, or `{ "error": "authorization_pending" }`.

`GET /me` (Bearer or session cookie) → `{ user_id, email, orgs }`.

RFC 8628 aliases remain: `POST /oauth/device/code` and poll via `POST /oauth/token` with `grant_type=urn:ietf:params:oauth:grant-type:device_code`.

### Secondary: loopback PKCE

For clients that can bind a local port:

1. `GET /oauth/authorize?response_type=code&client_id=buwiz-desktop&redirect_uri=http://127.0.0.1:<port>/callback&code_challenge=...&code_challenge_method=S256&state=...`
2. User registers or logs in on the web, then authorizes.
3. Browser redirects to `http://127.0.0.1:<port>/callback?code=...&state=...`.
4. Desktop exchanges the code at `/oauth/token` (`grant_type=authorization_code`) with `code_verifier`. Refresh uses `grant_type=refresh_token`.

**Allowed `redirect_uri` values** (PKCE only; device flow does not use one):

- Loopback `http://` / `https://` on `127.0.0.1`, `localhost`, or `[::1]` (any port), typically `/callback`
- Optional custom scheme `buwiz://oauth/callback` (legacy `buwiz://auth/callback` still accepted)
- Extra comma-separated URIs in `DESKTOP_OAUTH_REDIRECT_URIS` / Spin variable `desktop_oauth_redirect_uris`

Scope issued: `openid profile email tax_profiles`.

```bash
curl -sS -X POST http://localhost:3008/auth/device/start \
  -H 'content-type: application/json' \
  -d '{"client_id":"buwiz-desktop"}'
```

---

## Tax profiles

Cloud subset of the desktop taxpayer profile. **`id` (UUID) is the server source of truth.** Clients must not treat TIN as the local identity key. Every mutating command loads the profile by UUID and asserts `account_id` matches the caller.

`tax_profiles.account_id` maps to wasi-auth `auth_users.user_id` (the sketch `accounts` table is not duplicated). Refresh tokens stay in `auth_refresh_tokens`. Device codes live in `buwiz_server.oauth_device_codes`; sessions in `auth_sessions`. Do not add product `devices` or `refresh_tokens` tables.

The server stores `tin_last4` for display, persisted `branch_code` (head office `00000`), and an HMAC of `{tin_root}|{branch_code}` as `tin_hash` (BYTEA; Spin binds hex via `decode(..., 'hex')`). Exclusive uniqueness is **only** on `RegisterTaxProfile` via that hash (`UNIQUE (tin_hash)` globally in V1, plus `UNIQUE (account_id, tin_hash)`). Drop the global unique later when `tax_profile_members` exists. Raw TIN is attested at register/claim time and is **not** the primary key.

`claim_status`: `owned` | `pending_claim` | `read_only`.

V1 commands (past-tense event mirrors). Keep them small.

| Command | Event |
|---------|--------|
| `RegisterTaxProfile` | `TaxProfileRegistered` |
| `UpdateTaxProfileIdentity` | `TaxProfileIdentityUpdated` |
| `ArchiveTaxProfile` / `RestoreTaxProfile` | `TaxProfileArchived` / `TaxProfileRestored` |
| `CloneProfileYear` | `ProfileYearCloned` (copy prior `per_year_forms` only if dest empty) |
| `UpdateProfileYear` | `ProfileYearUpdated` (null = inherit) |
| `SetYearForms` / `ActivateYearForm` / `DeactivateYearForm` | year-forms events |
| `UpsertFormDraft` / `MarkDraftSaved` | `FormDraftUpserted` / `DraftSaved` |
| `EnqueueFiling` | `FilingQueued` |
| `MarkFilingSubmitted` / `ConfirmFilingFromReceipt` / `FailFiling` / `MarkFilingPaid` | filing events |
| `RegisterDevice` / `RevokeDevice` / `IssueDesktopSession` | device-session events (no PIN/TOTP) |

Queries: `GetTaxProfile`, `ListTaxProfilesForAccount`, `GetProfileYear`, `ListYearForms`, `GetFormDraft`, `ListDraftsForYear`, `ListFilings`, `GetFilingByPeriod`.

Claim/reclaim/transfer remain as extra company-managed commands (TIN attestation + ORUS proof). They are not TIN-as-SoT.

V1.1 stubs only: `SaveFormTemplate` / `ApplyFormTemplate`.

Do **not** port COR/OCR/effective-date ledgers, inference flag updates, or IMAP/OAuth mailbox secrets.

Rules:

1. Only one active holder per hashed taxpayer registration unit.
2. Personal hold is exclusive to that user (and their agents).
3. A company may hold it until the verified owner claims or reclaims (`pending_claim`).
4. The verified owner can reclaim at any time.
5. Multi-device identity uses last-write-wins via `updated_at` (PATCH must echo the last seen `updated_at`). Never silent-merge two different TINs into one row.
6. Concurrent registers of the same hashed identity fail with 409.

Email must be **verified** before register/claim/reclaim/transfer/patch/archive.

Cloud field subset for sync: `full_name`, `rdo_code`, `address`, `taxpayer_type`, plus contact/classification fields. **Never** sync `profile_pin_hash`, `totp_secret`, IMAP passwords, or mailbox OAuth tokens.

| REST | |
|------|--|
| `GET /tax-profiles` | `ListTaxProfilesForAccount` (`GET /api/tax-profiles` alias) |
| `POST /tax-profiles` | `RegisterTaxProfile` (TIN attested; UUID returned) |
| `GET /tax-profiles/{id}` | `GetTaxProfile` |
| `PATCH /tax-profiles/{id}` | `UpdateTaxProfileIdentity` (`updated_at` LWW) |
| `POST /tax-profiles/{id}/archive` | `ArchiveTaxProfile` |
| `POST /tax-profiles/{id}/restore` | `RestoreTaxProfile` |
| `POST /tax-profiles/claim` | Claim from company hold (ORUS proof; TIN attested) |
| `POST /tax-profiles/reclaim` | Take back control |
| `POST /tax-profiles/transfer` | Transfer to an org you belong to (`{ id, organization_id }`) |

UI: `/tax-profiles` (HTML). Desktop `GET /tax-profiles` should send `Authorization: Bearer` or `Accept: application/json` so it is not treated as the page. `GET /api/tax-profiles` is always JSON.

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
- Full HTTP for year forms / drafts / filings — canonical tables exist (`profile_years`, `per_year_forms`, `form_drafts`, `filings`); command names are typed. See [docs/sync-api.md](docs/sync-api.md)
- BIR SFTP / **TSP submission relay** ([hexuria/buwiz-forms#44](https://github.com/hexuria/buwiz-forms/issues/44)) — not implemented on purpose
- Copying desktop profile encryption / email OAuth secrets to cloud

---

## Layout

```
src/domain/           tax profile aggregate, hashed TIN identity, V1 command stubs, ORUS port, desktop redirect policy
src/application/       verified-email gate + commands
src/store/            Postgres events/projection + Redis wake + OAuth codes
src/desktop_oauth.rs  device flow + PKCE (`RegisterDevice` / `IssueDesktopSession`)
src/app/              Leptos UI (auth, tax profiles, WebMCP)
migrations/postgres/  0001 app storage, 0002 tax profiles + OAuth, 0003 hashed TIN, 0004 canonical V1 read models
docs/                 webmcp, sync API, extensions
```
