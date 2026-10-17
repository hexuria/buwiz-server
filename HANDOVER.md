# Handover — buwiz-server

The previous version of this file described the `ddd-cqrs-es` template repository this app was generated from (its modularization and Tailwind work). That history now lives in the template repository; this file describes this repository.

## What this server is for

- Verified accounts (wasi-auth: email verification, MFA, passkeys, sessions, organizations).
- Backup and multi-device sync of tax profiles for the buwiz-forms desktop app (device-code and loopback PKCE login, see README).
- Sharing a tax profile with firms (organizations) and, next, with AI agents over MCP.

It never submits returns to BIR. Publishing goes through the installed buwiz-forms app, which owns the tunnel / egress reroute. A server-side "publish to BIR" permission is only a right that app checks.

## Access rule (current)

One gatekeeper: `authorize` in `src/application/tax_profile.rs`, re-checked in the domain by `TaxProfile::access_for` (`src/domain/tax_profile.rs`). Control follows the holder; the registering account grants nothing afterwards. Table in README, "Who may do what".

## Plan in progress

1. Only the current holder can transfer (takeover fix).
2. One gatekeeper for every read and write; docs match the code.
3. Owner keeps the profile; firms get revocable passes with fine-grained permissions (presets Read-only / Safe / Full; publish-to-BIR is a separate permission). Done: `src/domain/pass.rs`, pass commands in `src/domain/tax_profile.rs`, migration `0006`, REST + Tax profiles page, `scripts/verify_passes.sh`.
4. Agent login: OAuth dynamic client registration + Allow screen; each agent connection has its own tokens, bounded by the taxpayer's live access, and ends from Tax profiles → Connected agents. Done: `src/domain/agent.rs`, `src/desktop_oauth/agent.rs`, `src/store/agent.rs`, migration `0007`, `scripts/verify_agents.sh`.
5. `POST /mcp` with read-only tools first.

## Verify

```bash
cargo test --lib      # domain rules, no Spin needed
make check            # wasm32-wasip2 build
make grpc-check
make db-up && make db-migrate && make smoke   # needs Docker
bash scripts/verify_passes.sh                 # passes end to end; needs `make dev` (Spin + outbox worker, captured mail)
bash scripts/verify_agents.sh                 # agent OAuth end to end; same needs
```
