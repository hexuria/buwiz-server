# How buwiz-server fits with the other repos

buwiz-server keeps verified accounts, cloud copies of tax profiles, and who
may work on them (firms through access passes, AI agents through MCP). It
**never files with BIR**: filing stays on the taxpayer's or firm's installed
buwiz-forms app, which owns the tunnel / egress reroute to the BIR SFTP path.

```
                 ┌──────────────────────────┐
  browser ──────▶│ buwiz-server (this repo)  │◀────── AI agents (MCP)
                 │ Spin + Leptos + Postgres  │        Claude Code, Codex, Grok…
                 └───────────┬──────────────┘
          device flow / PKCE │  REST: tax profiles, passes, permissions
                 ┌───────────▼──────────────┐
                 │ buwiz-forms desktop app   │──────▶ BIR (SFTP, only from here)
                 └──────────────────────────┘
```

## Repositories

| Repo | Role | Used here as |
|------|------|--------------|
| [hexuria/buwiz-forms](https://github.com/hexuria/buwiz-forms) | Desktop app: forms, local tax profiles, local PIN/admin model, filing to BIR | Client of this server (Settings → Account) |
| [hexuria/buwiz.com](https://github.com/hexuria/buwiz.com) | Marketing site | Links to the desktop releases and to `https://account.buwiz.com/login` |
| [codeitlikemiley/wasi-auth](https://github.com/codeitlikemiley/wasi-auth) | Identity: accounts, email verification, sessions, MFA, passkeys, organizations and roles, mail outbox | Crate `wasi-auth = "=0.1.0-rc.5"`, plus the native `wasi-auth-outbox-worker` that `make dev` installs |
| [codeitlikemiley/ddd-cqrs-es](https://github.com/codeitlikemiley/ddd-cqrs-es) | Event-sourcing kit and the `examples/fullstack-app` template this repo was made from | Git dependency (pinned rev in `Cargo.toml`); its Spin Postgres adapter runs every query |
| [codeitlikemiley/spin-rust-sdk](https://github.com/codeitlikemiley/spin-rust-sdk) | Spin SDK with blocking Postgres imports | Vendored in `vendor/spin-sdk` (see `vendor/README.md`) |
| [hexuria/buwiz-ui-redesign](https://github.com/hexuria/buwiz-ui-redesign) | UI spec and prototype for the desktop app | Reference only |

Versions are pinned in README, "Version pins".

## buwiz-forms (desktop app)

**Status:** Settings → Account signs in with the device flow below. The
default server is `https://api.buwiz.com` (`BUWIZ_SERVER_URL` or the saved
setting overrides it). Filing does not wait on this server. Hostnames:
[domains.md](domains.md).

1. **Sign in** with the first-party client `buwiz-desktop`:
   - device flow (what the app uses): `POST /auth/device/start`, open
     `verification_uri_complete`, the user approves at `/login/device`, then
     poll `POST /auth/device/poll`;
   - loopback PKCE remains available on the server (`/oauth/authorize` then
     `/oauth/token`) for a client that can bind a local port.

   The app stores the refresh token in the OS keychain and in a mode-0600
   file beside the local database. Details: README, "Desktop auth".
2. **List and read profiles:** `GET /api/tax-profiles` and
   `GET /api/tax-profiles/{id}` with `Authorization: Bearer …`. Each profile
   carries `access` (`owner`, `holding_org` for a firm's own draft, `pass`) and
   `permissions`, the caller's own rights. Without `?organization_id=` the
   list holds the caller's own profiles; profiles a workspace reaches
   through a pass appear with `?organization_id=<workspace>`.
3. **Before filing with BIR**, the app must check that `permissions` contains
   `filings.publish`. The server only records this right; the app enforces
   it, because only the app can reach BIR. For a firm member this already
   folds in both sides: the owner's switch on the firm's pass and the firm's
   own publishers list, so the app checks this one list and nothing else.
4. **Sync** of tax years, drafts and filings: see [sync-api.md](sync-api.md).
   Drafts save at a base revision (409 on a stale one). Filings are reported
   by the app after it sends: queued → submitted → confirmed → paid.
5. **Never send** local secrets: the profile PIN, TOTP secret, IMAP passwords
   or mailbox OAuth tokens stay on the device.

The desktop app's local admin model (*Just me* or *help desk*, one PIN per
taxpayer, Settings › Computer setup) is separate from server accounts: a PIN
on the computer is not a server password, and a server account does not
unlock the computer.

## AI agents (MCP)

Any MCP client that supports Streamable HTTP with OAuth can connect. The
client registers itself, the taxpayer allows it on some of their own
profiles, and it gets read-only tools. README, "AI agents" and "MCP
endpoint", has the full flow.

```bash
# Claude Code
claude mcp add --transport http buwiz http://localhost:3008/mcp
# then run /mcp in Claude Code and press Allow in the browser
```

Other clients (Codex, Grok, Cursor…): add `http://localhost:3008/mcp` (or
your public origin + `/mcp`) as a Streamable HTTP server. They find the sign-in
through `/.well-known/oauth-protected-resource/mcp` and register themselves.

The taxpayer sees and ends connections under **Tax profiles → Connected
agents**. An agent never gets more than its taxpayer has, and never publishes
to BIR.

## wasi-auth

- Identity tables live in the `auth_*` schema; app tables live in
  `buwiz_server.*`. `make db-migrate` applies both.
- Verification, password-reset and invitation mail go through the outbox. The
  native worker delivers it: captured locally, or Resend / HTTP in production
  (README, "Email").
- Agent tokens are this app's own (`bwa_` / `bwr_`, in `buwiz_server.agent_*`),
  not wasi-auth tokens, and wasi-auth refuses them, so an agent can never act
  as the taxpayer.

## ddd-cqrs-es and the Spin SDK

- Tax profiles are an event-sourced aggregate (`src/domain/tax_profile.rs`)
  stored in `buwiz_server.tax_profile_events`, with projection tables for
  reads.
- The adapter binds every string parameter as text. Write uuid / json /
  timestamp parameters with a cast (`?1::uuid`); `postgres_placeholders`
  routes them through text.
- The vendored SDK uses blocking `spin:postgres@4.2` calls. A Spin host that
  doesn't match hangs on queries that return no rows (README,
  "Troubleshooting").
