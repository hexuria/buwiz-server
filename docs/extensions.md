# Extension points (stubs)

Buwiz-server v1 ships accounts + exclusive tax-profile ownership. Adjacent products stay out of this binary.

## ORUS TIN verification

Trait: `src/domain/orus.rs` (`TinOwnershipVerifier`).

| Impl | Config |
|------|--------|
| `UnconfiguredOrusVerifier` | Default. Claim/reclaim return a configuration error. |
| `FakeOrusVerifier` | `ORUS_FAKE_VERIFIER=true` / Spin `orus_fake_verifier`. Always issues `ProofMethod::Fake`. |

Production adapter (live ORUS session, document capture, branch matching) is not in this repo. Keep proof material out of Redis; store only what Postgres already has (`verification_status`, `verified_owner_user_id`).

## Filing-history sync (headless-bir)

See [sync-api.md](sync-api.md). No HTTP routes yet.

## TSP / BIR SFTP relay

Related product intent: [hexuria/buwiz-forms#44](https://github.com/hexuria/buwiz-forms/issues/44).

**Do not implement BIR SFTP in this service.** A future TSP submission relay would:

- Accept an already-authorized tax profile (holder check).
- Hand a package to a dedicated worker / buwiz-forms pipeline.
- Never take BIR mailbox credentials from the desktop encryption store.

Leave a comment or ticket, not a half-wired SFTP client.
