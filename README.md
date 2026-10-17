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

### Prerequisites

| Tool | Version | Install |
|------|---------|---------|
| Rust | ≥ 1.93 with the `wasm32-wasip2` target | `rustup target add wasm32-wasip2` |
| cargo-leptos | ≥ 0.3.7 | `cargo install --locked cargo-leptos --version '^0.3.7'` |
| wasm-tools | any | `cargo install --locked wasm-tools` |
| Spin | a host whose `spin:postgres@4.2` blocking `query` returns empty row sets (see Troubleshooting) | [Spin install](https://spinframework.dev/install) |
| Docker | for Postgres 17 (and Redis) | Docker Desktop / OrbStack |
| jq, curl | for the verify scripts | Homebrew |

`bash scripts/verify_toolchain.sh` checks Rust, cargo-leptos, the target and wasm-tools.

### Start

```bash
cp .env.example .env     # works as is: commented lines fall back to the Makefile defaults
make db-up               # Postgres on 127.0.0.1:54329, Redis on 127.0.0.1:6379
make dev transport=both  # migrations, then Spin + the mail outbox worker
# open http://localhost:3008  (localhost, not 127.0.0.1, so passkeys and cookies work)
```

- `make dev` applies every migration (`0001`–`0007`, idempotent), builds, then runs **Spin** and the native **`wasi-auth-outbox-worker`**. `make spin` serves the app without delivering verification mail.
- The first `make` writes random dev keys to `.auth-root-key` and `.auth-outbox-key` (gitignored, mode 600). Keep them: deleting them signs everyone out and makes sealed mail unreadable.
- Mail is **captured** locally. After you register, press **Verify now** (see below).
- Guest logs: `make dev SPIN_UP_BUILD_FLAG="--build -e RUST_LOG=info,buwiz_server=debug"`.

### First steps in the app

1. Register, then **Verify now**.
2. **Tax profiles** → add a profile (TIN, branch, RDO).
3. Share it with a firm: create a workspace (Organizations), then **Give access** on Tax profiles (passes, below).
4. Connect an AI agent: `claude mcp add --transport http buwiz http://localhost:3008/mcp`, then `/mcp` in Claude Code and **Allow** (AI agents, below).
5. The desktop app signs in with the device flow (Desktop auth, below).

### Make targets

| Target | Purpose |
|--------|---------|
| `make db-up` / `make db-down` | Start / stop Postgres + Redis (Docker Compose) |
| `make db-migrate` | wasi-auth schema + Buwiz migrations `0001`–`0007` |
| `make db-verify` | Check every migration is applied and unchanged |
| `make dev` | Migrations, Spin and the outbox worker (register / verify work) |
| `make spin` | Spin only (no mail delivery) |
| `make check` / `make grpc-check` | `wasm32-wasip2` compile, REST / gRPC transport |
| `make smoke` | REST + web route checks against `BASE_URL` |
| `cargo test --lib` | Domain tests, no Spin needed |
| `cargo test --lib --features ssr,postgres` | Also the OAuth, MCP, REST-route and SQL-placeholder tests |

Password minimum length is **15** (wasi-auth default in this stack).

### Check it works end to end

With `make dev` running:

```bash
bash scripts/verify_passes.sh   # access passes: offer, accept, use, revoke (4 accounts)
bash scripts/verify_agents.sh   # agent OAuth: register, Allow, PKCE, refresh, /mcp tools, End
```

Both sign up throwaway `@example.test` accounts through captured mail and use only the API.

### Troubleshooting

| Symptom | Cause | Fix |
|---------|-------|-----|
| Sign-in for an unknown email, email verification or `POST /auth/device/poll` with a wrong code never answers; a 401 after a token lookup hangs | Spin host and the vendored `spin-sdk` (blocking `spin:postgres@4.2`, see `vendor/README.md`) disagree: **queries that return zero rows never complete**. Seen with Spin `4.1.0-pre0` on macOS. Queries with rows work. | Use the Spin build the vendored SDK targets. App code is not at fault; endpoints whose queries return rows (for example `/mcp` with a connected agent) still work. |
| `make db-up` fails: port 6379 in use | Another Redis (e.g. Homebrew) holds 6379 | Redis is optional (wake only): `docker compose up -d --wait postgres`, then `make dev -o db-up REDIS_URL=`. |
| `cargo-leptos` fails to read `.env` | An unquoted value with spaces (old `.env` copies had `AUTH_*_SCOPES=openid email profile`) | Comment those lines out; the Makefile defaults are the same. |
| `POSTGRES_URL is required` or Spin refuses to start without a root key | A blank `POSTGRES_URL=` / `AUTH_ROOT_KEY_BASE64=` in `.env` overrides the Makefile default | Delete or comment out the blank line. |
| `WrongType { postgres: Uuid, rust: String }` | A query bound a uuid/json/timestamp parameter as text | Fixed in `postgres_placeholders` (`$1::uuid` becomes `$1::text::uuid`). New SQL should cast parameters the same way (`?1::uuid`). |
| Spin ignores Ctrl-C | It waits for hung requests (see the first row) | Send SIGINT again, then SIGKILL the `spin up` process. |

How this server fits with the desktop app and the other repos: [docs/integrations.md](docs/integrations.md).

### Development authentication (cloud / local default)

`make dev` defaults to **capture mail** so you can register and sign in without Resend.

| Flag | Default | Role |
|------|---------|------|
| `AUTH_MAIL_TRANSPORT` | `capture` | Local outbox. The worker acks mail without calling a provider. |
| `AUTH_DEV_TOOLS` | `true` | Shows **Verify now**, **Copy link**, and **Skip verification (dev)** after register / on pending login. |
| `AUTH_DEV_AUTO_VERIFY` | `false` | If `true`, register consumes the captured token and signs you in. |
| `AUTH_PRODUCTION_MODE` | `false` | When `true`, Spin refuses `AUTH_DEV_TOOLS`, `AUTH_DEV_AUTO_VERIFY`, and capture mail. `spin.production.toml.example` keeps these off and uses `mail-http`. |

After register the success panel is **not** “check your real inbox” as the only path. **Verify now** opens the captured token on **this host** (same-origin `/verify-email?token=…`), so a Cloud Agent / remote client is not sent to `http://localhost:3008` from an email app.

Production-shaped builds compile without `mail-capture` (`AUTH_MAIL_TRANSPORT=http` / `resend` → `mail-http`) and the capture/skip controls are absent.

### Email: capture vs Resend

| Mode | When |
|------|--------|
| **capture** (default) | Local and Cloud Agent. Outbox stores the message. Use **Verify now** / **Skip verification (dev)** when `AUTH_DEV_TOOLS=true`. |
| **resend** | Real inbox. Set `AUTH_MAIL_TRANSPORT=resend`, `AUTH_RESEND_API_KEY`, and `AUTH_RESEND_FROM` (verified domain). The API key stays on the **native worker**, never in Spin. |
| **http** | Production worker + HTTP mailer (`spin.production.toml.example`). |

```bash
# Real inbox — AUTH_PUBLIC_BASE_URL must be reachable from the mail client
AUTH_MAIL_TRANSPORT=resend \
AUTH_PUBLIC_BASE_URL='https://your-tunnel.example' \
AUTH_RESEND_API_KEY=re_... \
AUTH_RESEND_FROM='Buwiz <auth@your-verified-domain.example>' \
make dev transport=both listen=0.0.0.0:3008
```

Never assume `http://localhost:3008` in a Resend message will open this VM. Use a tunnel or public origin. Never commit Resend API keys.

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

## AI agents (OAuth for MCP clients)

Claude Code, Codex, Grok and other MCP clients sign in with standard OAuth and get their **own** tokens. They are never the taxpayer's session: ending an agent does not sign the taxpayer out, and an agent token is refused everywhere a user token is expected (REST, gRPC, `/api/auth/token/verify`).

1. The client finds the server: `GET /.well-known/oauth-protected-resource` (RFC 9728; also `/.well-known/oauth-protected-resource/mcp`) names the resource (`{base}/mcp`) and this server as its authorization server. `GET /.well-known/oauth-authorization-server` lists `registration_endpoint`.
2. It registers itself: `POST /oauth/register` (RFC 7591), public clients only (`token_endpoint_auth_method: none`). The answer carries a `client_id` starting with `agent_`. Registering grants nothing. Redirect URIs: `https://`, loopback `http://` (`127.0.0.1`, `localhost`, `[::1]`; any port at use time), or an app's own scheme; never `javascript:`, `data:`, `file:` or a fragment. Registrations are capped at 500 an hour, and apps never allowed are dropped after 7 days.
3. It opens `GET /oauth/authorize?response_type=code&client_id=agent_…&redirect_uri=…&code_challenge=…&code_challenge_method=S256&state=…[&resource={base}/mcp]`. The signed-in taxpayer sees the **Allow** screen:
   - which of **their own** profiles it may reach (held personally, or verified as theirs while a firm holds them);
   - what it may do: Read-only (selected), Safe or Full, the same presets as access passes;
   - the app's name is what it called itself, and the screen says so.
4. It exchanges the code at `POST /oauth/token` (`grant_type=authorization_code`, `code_verifier`). Access tokens (`bwa_…`) last 1 hour. Refresh tokens (`bwr_…`) last 30 days and are replaced on every refresh. Reusing an old refresh token ends every token of that connection (RFC 9700).

What an agent may do on a profile is the connection's permissions **narrowed by the taxpayer's access at that moment**: if a firm now holds the profile, the agent can only read; if the profile moved to someone else, nothing. An agent can never publish to BIR, share, archive or delete. Allowing the same app again replaces its earlier connection.

The connection ends when the taxpayer presses **End** under Tax profiles → Connected agents, or on a password change or any other security change on the account (`auth_users.security_revision`), or when the account is disabled.

| REST | |
|------|--|
| `GET /api/agents` | The caller's connected agents, with profiles and preset |
| `POST /api/agents/{connection}/revoke` | End one (its tokens stop on the next call) |

Agent tokens are accepted only at `POST /mcp`.

---

## MCP endpoint

`POST /mcp` is MCP over Streamable HTTP, kept to what Spin does well:
- **Stateless, JSON answers only:** no `Mcp-Session-Id`, no server-sent events. `GET` and `DELETE` get 405.
- **Protocol versions:** 2025-11-25, 2025-06-18 and 2025-03-26. An unknown `MCP-Protocol-Version` header gets 400.
- **Sign-in:** without a valid agent token the answer is 401 with `WWW-Authenticate: Bearer resource_metadata="{base}/.well-known/oauth-protected-resource/mcp"`, which starts the client's OAuth flow above.
- **Origin:** a request with an `Origin` other than this server is refused, to block DNS rebinding.

Tools, all read-only (`readOnlyHint`):

| Tool | Needs | Returns |
|------|-------|---------|
| `list_tax_profiles` | — | The connected profiles with what the agent may do on each right now. A profile it can no longer reach shows only its id. |
| `get_tax_profile` | `profile.view` | Registration details: name, TIN last 4, branch, RDO, address, contact, taxpayer type, classification, VAT, who holds it. No internal user or organization ids. |
| `list_tax_years` | `years.view` | Each tax year, newest first: its forms (code, frequency, active) and per-year registration values. |
| `list_form_drafts` | `drafts.view` | Draft summaries, newest first, optionally for one `tax_year`: form, year, period, revision, reviewed or not. No contents. |
| `get_form_draft` | `drafts.view` | One draft (`profile_id`, `draft_id`) with its contents. |
| `list_filings` | `filings.view` | Filing history, newest first: status, BIR receipt reference, failure summary, paid date. |

Every call re-checks the token, the connection and the taxpayer's live access. A profile the agent may not see comes back as a tool error (`isError: true`) so the model can explain it. Who saved or reviewed a draft (internal user ids) is left out. An archived profile is out of an agent's reach until it is restored. There is no tool that writes, and none that files with BIR: agents never get `filings.publish`.

Try it with Claude Code: `claude mcp add --transport http buwiz http://localhost:3008/mcp`, then `/mcp` to sign in.

---

## Tax profiles

Cloud subset of the desktop taxpayer profile. **`id` (UUID) is the server source of truth.** Clients must not treat TIN as the local identity key. Every read and write loads the profile by UUID and goes through one gatekeeper (`authorize` in `src/application/tax_profile.rs`, re-checked by `TaxProfile::access_for` in the domain). See **Who may do what** below.

`tax_profiles.account_id` maps to wasi-auth `auth_users.user_id` (the sketch `accounts` table is not duplicated). Refresh tokens stay in `auth_refresh_tokens`. Device codes live in `buwiz_server.oauth_device_codes`; sessions in `auth_sessions`. Do not add product `devices` or `refresh_tokens` tables.

The server stores `tin_last4` for display, persisted `branch_code` (head office `00000`), and an HMAC of `{tin_root}|{branch_code}` as `tin_hash` (BYTEA; Spin binds hex via `decode(..., 'hex')`). Exclusive uniqueness is **only** on `RegisterTaxProfile` via that hash (`UNIQUE (tin_hash)` globally in V1, plus `UNIQUE (account_id, tin_hash)`). Sharing never copies the row: firms reach a profile through access passes and AI agents through agent connections, so the global unique stays. Raw TIN is attested at register/claim time and is **not** the primary key.

`claim_status`: `owned` | `pending_claim` | `read_only`.

Tax profile commands (event-sourced; past-tense event mirrors). Keep them small.

| Command | Event |
|---------|--------|
| `RegisterTaxProfile` | `TaxProfileRegistered` |
| `UpdateTaxProfileIdentity` | `TaxProfileIdentityUpdated` |
| `ArchiveTaxProfile` / `RestoreTaxProfile` | `TaxProfileArchived` / `TaxProfileRestored` |
| `OfferPass` / `RequestPass` / `AcceptPass` / `ApprovePass` / `ChangePass` / `EndPass` | `PassOffered` / `PassRequested` / `PassActivated` / `PassChanged` / `PassEnded` |
| `InviteOwner` / `WithdrawOwnerInvite` / `AcceptOwnerInvite` | `OwnerInvited` / `OwnerInviteWithdrawn` / `OwnerInviteAccepted` |
| `RegisterDevice` / `RevokeDevice` / `IssueDesktopSession` | device-session events (no PIN/TOTP) |

Tax years, drafts and filings are plain rows, not event streams. The rules are in `src/domain/records.rs`, and the endpoints are in [docs/sync-api.md](docs/sync-api.md).

Claim and reclaim remain for a taxpayer to take a firm's draft or a profile someone else registered. Moving a profile to a firm is gone: firms work through passes.

**Ownership claims with Form 2303.** ORUS has no production adapter, so the working proof is a Form 2303 (Certificate of Registration) that Buwiz staff review. The flow:

1. The claimant starts a claim with the TIN and branch. The TIN goes in the request body, never in a URL.
2. They upload the file in 4 KiB parts. Request bodies can't stream on the current runtime, so each part has to fit the browser shell's 6000-character header shim. Each part is encrypted when it arrives.
3. After the last part, the server screens the whole file (`domain::provenance`):
   - it must be a JPEG, PNG or PDF of at most 5 MB;
   - a file that names an IPTC AI source type (`trainedAlgorithmicMedia`, in C2PA or XMP) is refused with "upload a clean scan", counted, and never stored.
4. The file is stored encrypted: AES-256-GCM, with a key derived from the server key and the claim id as associated data.
5. Staff (`system.user.manage` plus a step-up, never the claimant) download it from `/admin/ownership-claims` and approve or reject it.
6. Approval runs the profile's own claim or reclaim (`ProofMethod::Form2303Review`) and closes the claim in the same transaction, so every pass and invite ends.
7. Every way a claim closes deletes the file, and keeps the decision, reviewer, note and SHA-256.

Form 2303 is used **only** to prove ownership, never to choose forms. The `c2pa` crate doesn't build for `wasm32-wasip2` on stable, so the screen reads the marks without verifying signatures. It only ever refuses, so a forged mark hurts nobody but its uploader. No mark found proves nothing: metadata can be stripped, and SynthID has no public detector. That's why staff review every claim.


Do **not** port COR/OCR/effective-date ledgers, inference flag updates, or IMAP/OAuth mailbox secrets. The only Form 2303 the server accepts is an ownership-claim upload (above). It is never read for form selection and is deleted once the claim is decided.

Rules:

1. **The taxpayer always wins.** The owner can always publish, edit, see everything and end any firm's or agent's access. No firm can lock an owner out.
2. One owner per hashed taxpayer registration unit. Firms and agents only ever work through passes the owner gives and can end.
3. A profile a firm creates is a **draft** nobody owns (`pending_claim`) until a taxpayer claims it; meanwhile the firm's members work on it with the Full preset, without publishing or sharing.
4. A proven owner (claim or reclaim) starts clean: every open pass ends.
5. Multi-device identity uses last-write-wins via `updated_at` (PATCH must echo the last seen `updated_at`). Never silent-merge two different TINs into one row.
6. Concurrent registers of the same hashed identity fail with 409.

### Who may do what

The owner decides, always. Registering a profile grants nothing by itself (`account_id` is history only); a firm reaches someone else's profile only with an **access pass**.

| Caller | View | Edit | Publish to BIR | Archive / restore | Share (passes) |
|---|---|---|---|---|---|
| The owner | yes | yes | yes, always | yes | yes |
| Member of a workspace with an active pass | `profile.view` | `profile.edit` | only if the owner switched on `filings.publish` **and** the workspace picked this member as a publisher | no | no |
| Member of the firm that created a draft nobody owns yet | yes | yes | no | yes | no |
| Anyone else, including the original registrant after a claim | no | no | no | no | no |

**Firm drafts and owner invites.** A firm that creates a profile for a client holds it as a draft. It invites the taxpayer by email (`InviteOwner`) and shares the link itself; no new email kinds are sent. The link is `/invites/{id}#t={secret}`; only the secret's SHA-256 is stored, and the secret sits in the fragment so it never reaches server logs. Only a signed-in account whose **verified** email is the invited one can accept. Accepting makes that person the owner and gives the firm an active Full pass without publishing, which the owner can change or end like any other. A newer invite replaces the open one, a claim withdraws it, and an invite lasts 1 to 14 days. Downside accepted: a firm could invite an address it controls; the real taxpayer still takes the profile with a claim.

**Publishers.** A workspace keeps its own list of members who may publish to BIR (`buwiz_server.firm_publishers`; wasi-auth's role catalog is fixed, so this is not a role permission). Changing the list needs `organization.update` and a step-up (AAL2). A member's `permissions` include `filings.publish` only when a working pass for that workspace has it and the member is on the list.

Profiles moved to a firm under the old rules replay as: the owner keeps the profile, and the firm holds a Full pass (no publishing) the owner can change or end. After deploying this, run `POST /api/auth/storage/tax-profiles/reproject` once (storage admin) so the lists match.

#### Access passes

A pass lets a firm's workspace work on someone else's profile. Any member of that workspace can use it.

- **Start:** the owner offers one (the workspace's owner or admin, i.e. `organization.update`, accepts), or the workspace asks with the TIN and branch and the owner approves.
- **Permissions:** `profile.view`, `profile.edit`, `years.view`, `years.manage`, `drafts.view`, `drafts.save`, `drafts.update`, `drafts.review`, `filings.view`. Write permissions bring their read permission with them.
- **Presets:** Read-only (the four `*.view`), **Safe** (Read-only + save, update and review drafts; selected when the screen opens), Full (all nine).
- **`filings.publish`** is never in a preset and never in the permission list: only `allow_publish: true` turns it on, and the screen asks "Are you sure?" first. The server only records it; the grantee's installed buwiz-forms app checks it before sending to BIR.
- **End date:** none unless the owner sets one (`ends_on`, the last day in Philippine time). A pass past its end date stops working at once. The next pass action on that profile records it in the history as `tax_profile.pass_expired`, dated at the end date, with no actor. The pass is then closed: it can't be changed back to life, and the firm can be offered a new one. Ending a pass that has already expired just records the expiry.
- **Changing:** the owner changes an offered or working pass from Tax profiles → Open passes → **Change**: permissions, publishing (asks first) and the last day (empty removes it).
- **Ending:** the owner revokes or declines; the workspace declines an offer, withdraws a request, or gives the pass back. A claim or reclaim ends every open pass.
- Sharing, ending passes, archiving and deleting stay with the owner; no pass grants them.
- Notices are in-app (the Tax profiles page, "Waiting for you"); there is no grant email.

- An **archived** profile is frozen for grantees: its passes give no access and cannot be accepted, approved or changed until it is restored. The owner can still end them.
- A workspace that asked with a TIN sees the profile's name only after the owner approves.
- Sharing a pass does not move the profile's `updated_at`, so the desktop's last-write-wins PATCH is not disturbed.

The server is for backup, multi-device sync and sharing access. It never submits returns to BIR: publishing goes through the installed buwiz-forms app, which owns the tunnel / egress reroute.

Email must be **verified** before register/claim/reclaim/patch/archive.

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
| `POST /tax-profiles/reclaim` | Prove ownership of a profile someone else registered (ORUS) |
| `GET /tax-profiles/passes` | Open passes on your profiles or held by your workspaces, plus the permission catalog and presets |
| `POST /tax-profiles/{id}/passes` | `OfferPass` (`{ organization_slug, preset \| permissions, allow_publish, ends_on }`) |
| `POST /tax-profiles/passes/request` | `RequestPass` (`{ organization_id, tin_root, branch_code, preset }`) |
| `POST /tax-profiles/{id}/passes/{pass}/accept` | `AcceptPass` (workspace, `organization.update`) |
| `POST /tax-profiles/{id}/passes/{pass}/approve` | `ApprovePass` (owner; `{}` keeps what was asked for) |
| `PATCH /tax-profiles/{id}/passes/{pass}` | `ChangePass` (owner; publishing stays on only while `allow_publish` is sent; omit `ends_on` to keep the end date, `""` to remove it) |
| `POST /tax-profiles/{id}/passes/{pass}/end` | `EndPass` (either side) |
| `POST /tax-profiles/{id}/invites` | `InviteOwner` (holding workspace, `organization.update`; `{ email, days? }`; returns the link once) |
| `POST /tax-profiles/{id}/invites/{invite}/withdraw` | `WithdrawOwnerInvite` (holding workspace) |
| `POST /tax-profiles/invites/{invite}/preview` | What the invite is for (`{ token }`) |
| `POST /tax-profiles/invites/{invite}/accept` | `AcceptOwnerInvite` (`{ token }`; caller's verified email must match) |
| `GET /tax-profiles/ownership-claims` | Your Form 2303 claims |
| `POST /tax-profiles/ownership-claims` | Start a claim (`{ tin_root, branch_code }`) |
| `POST /tax-profiles/ownership-claims/{claim}/file-parts` | One part of the file (`{ index, total, data }`, base64 of at most 4 KiB, in order; the last part screens and stores it) |
| `POST /tax-profiles/ownership-claims/{claim}/withdraw` | Withdraw an open claim (the file is deleted) |
| `GET /api/admin/ownership-claims` | Staff: claims waiting for review (`system.user.manage`, step-up) |
| `GET /api/admin/ownership-claims/{claim}/file` | Staff: download the decrypted file |
| `POST /api/admin/ownership-claims/{claim}/approve` / `reject` | Staff: decide (`{ note }`, required to reject) |
| `GET /api/organizations/{org}/publishers` | Members the workspace lets publish to BIR (any member) |
| `POST /api/organizations/{org}/publishers` | Add one by sign-in email (`organization.update`, step-up) |
| `POST /api/organizations/{org}/publishers/{user}/remove` | Remove one (`organization.update`, step-up) |

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

Browser agents can drive register/login via **declarative HTML attributes** (and an optional hydrate-only `document.modelContext.registerTool`). This is separate from the server's `POST /mcp` endpoint.

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
| `AUTH_PUBLIC_BASE_URL` | Derived from `make … listen=` unless set. For Resend, this **must** be a URL the inbox can open (tunnel/public). Localhost only works on the same machine. |
| `AUTH_DEV_TOOLS` | Capture-mail UI + skip-verify (default `true` locally; forbidden in production) |
| `AUTH_DEV_AUTO_VERIFY` | Auto-complete verification after register (default `false`; forbidden in production) |
| `POSTGRES_URL` | Default `postgres://wasi_auth:wasi_auth_dev@127.0.0.1:54329/wasi_auth` |
| `REDIS_URL` | Empty skips wake publishes |
| `REDIS_CHANNEL` | Default `buwiz-tax-profiles` |
| `ORUS_FAKE_VERIFIER` | Local TIN proof |
| `WEBMCP_ENABLED` | Imperative WebMCP registration |
| `DESKTOP_OAUTH_REDIRECT_URIS` | Extra native redirect URIs |

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
src/domain/           tax profile aggregate (+ access passes), pass permissions and dates, agent rules,
                      hashed TIN identity, V1 sync commands (no HTTP yet), ORUS port, desktop redirect policy
src/application/      the gatekeeper (`authorize`), tax profile + pass services, agent connections
src/store/            Postgres events/projection, pass and agent tables, Redis wake, OAuth codes
src/desktop_oauth.rs  device flow + PKCE for the desktop app; desktop_oauth/agent.rs: agent OAuth
src/mcp/              POST /mcp (protocol.rs: JSON-RPC and the tool list)
src/app/              Leptos UI (auth, tax profiles, passes, connected agents, WebMCP)
migrations/postgres/  0001 app storage, 0002 tax profiles + OAuth, 0003 hashed TIN, 0004 V1 read models,
                      0005 device interval, 0006 access passes, 0007 agent connections
docs/                 integrations (other repos, MCP clients), sync API, webmcp, extensions
```
