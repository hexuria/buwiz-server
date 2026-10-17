# Handover — buwiz-server

The previous version of this file described the `ddd-cqrs-es` template repository this app was generated from (its modularization and Tailwind work). That history now lives in the template repository; this file describes this repository.

## What this server is for

- Verified accounts (wasi-auth: email verification, MFA, passkeys, sessions, organizations).
- Backup and multi-device sync of tax profiles for the buwiz-forms desktop app (device-code and loopback PKCE login, see README).
- Sharing a tax profile with firms (organizations, through access passes) and with AI agents over MCP (read-only tools).

It never submits returns to BIR. Publishing goes through the installed buwiz-forms app, which owns the tunnel / egress reroute. A server-side "publish to BIR" permission is only a right that app checks.

## Access rule (current)

One gatekeeper: `authorize` in `src/application/tax_profile.rs`, re-checked in the domain by `TaxProfile::access_for` (`src/domain/tax_profile.rs`). Control follows the holder; the registering account grants nothing afterwards. Table in README, "Who may do what".

## Ground rule (2026-10-17)

The taxpayer always wins: the owner can always publish, edit and end any access; firms and agents only work through passes. No read-only owner, no "take it back". Moving a profile to a firm is gone; old moves replay as a Full pass for that firm (`legacy_move_pass_id`). Ownership disputes (someone registered your TIN first) are the one remaining hard case: Form 2303 upload, reviewed by Buwiz staff, only to prove ownership (next PRs). Decision page: https://claude.ai/artifact/HreFCAgWBxACoBhdqm4HYa

## Done (all merged to `main`)

1. Only the current holder can transfer (takeover fix).
2. One gatekeeper for every read and write; docs match the code.
3. Owner keeps the profile; firms get revocable passes with fine-grained permissions (presets Read-only / Safe / Full; publish-to-BIR is a separate permission). Done: `src/domain/pass.rs`, pass commands in `src/domain/tax_profile.rs`, migration `0006`, REST + Tax profiles page, `scripts/verify_passes.sh`.
4. Agent login: OAuth dynamic client registration + Allow screen; each agent connection has its own tokens, bounded by the taxpayer's live access, and ends from Tax profiles → Connected agents. Done: `src/domain/agent.rs`, `src/desktop_oauth/agent.rs`, `src/store/agent.rs`, migration `0007`, `scripts/verify_agents.sh`.
5. `POST /mcp` with read-only tools first. Done: `src/mcp/` (stateless, JSON answers), tools `list_tax_profiles` and `get_tax_profile`; `scripts/verify_agents.sh` covers it.

Plan item A3 ("tokens carry a scope that is never checked") is settled by design rather than a scope check: browser and desktop tokens are first-party and act as the user; agents get their own `bwa_`/`bwr_` tokens, which wasi-auth refuses everywhere a user token is expected, and which `/mcp` bounds by the connection's permissions and the taxpayer's live access.

## Next (agreed order, 2026-10-17)

1. Done: the taxpayer always wins, passes only (this change).
2. Done: firm drafts and owner invites bound to a verified email (the firm keeps a Full pass without publishing), and a workspace publishers list; publishing needs the owner's switch on the pass *and* the firm's pick. Migration `0008`, `src/domain/invite.rs`, Tax profiles page → "For your firm", `/invites/{id}`.
3. Done: ownership claims with Form 2303. Uploads arrive in 4 KiB encrypted parts. An AI-marked file is refused with "upload a clean scan". Staff review at `/admin/ownership-claims`, and the file is deleted when the claim closes (decision, reviewer and hash kept). Migration `0009`, `src/domain/{provenance,ownership_claim}.rs`. Form 2303 is never used to choose forms. Raise `PART_BYTES` once Spin can stream request bodies.
4. HTTP for tax years, drafts and filings (commands and tables exist; see [docs/sync-api.md](docs/sync-api.md)); then MCP read tools for them.
5. Notices on the firm's page, a change-pass screen, pass expiry recorded in history, agents for firm members (needs membership checks without a browser session).

- Desktop client: buwiz-forms does not call this server yet. The contract is in [docs/integrations.md](docs/integrations.md).
- Local end-to-end runs need a Spin host that matches the vendored SDK (README, "Troubleshooting").

## Verify

```bash
cargo test --lib      # domain rules, no Spin needed
cargo test --lib --features ssr,postgres   # plus OAuth, MCP, REST routes, SQL placeholders
make check            # wasm32-wasip2 build
make grpc-check
make db-up && make db-migrate && make smoke   # needs Docker
bash scripts/verify_passes.sh                 # passes end to end; needs `make dev` (Spin + outbox worker, captured mail)
bash scripts/verify_agents.sh                 # agent OAuth end to end; same needs
```
