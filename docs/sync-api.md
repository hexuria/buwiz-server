# Filing-history sync API (intended shape)

This document is the **extension point** for syncing BIR filing history between **hexuria/headless-bir** (or the Buwiz desktop app) and this cloud control plane. v1 does **not** implement the protocol.

Durable source of truth for tax-profile **ownership** is already Postgres (`buwiz_server.tax_profiles`). Filing rows would be a separate aggregate keyed by the same registration unit (`tin_root` + `branch_code`).

## Goals

- Desktop / headless-bir can push a **checkpointed** filing list after a local scrape.
- Cloud never stores IMAP passwords, mailbox OAuth refresh tokens, or the desktop PIN/TOTP.
- Exclusive tax-profile ownership gates who may push or pull (`holder` or verified owner).
- Redis may **wake** subscribers after a sync commit; clients then GET the projection.

## Sketch (not implemented)

```
POST /api/sync/filing-history
Authorization: Bearer <access_token>
{
  "tin_root": "123456789",
  "branch_code": "00000",
  "expected_revision": 4,
  "source": "headless-bir",
  "checkpoint": "2026-09-01T00:00:00Z",
  "filings": [
    {
      "form_type": "2550Q",
      "period": "2026Q2",
      "rdo_code": "039",
      "status": "filed",
      "reference": "optional-bir-ref",
      "filed_at": "2026-07-15T08:00:00Z"
    }
  ]
}

GET /api/sync/filing-history?tin_root=123456789&branch_code=00000&after=...
```

### Rules to encode later

- Require verified email + tax-profile holder (or reclaim-capable verified owner).
- Optimistic concurrency on `expected_revision` of the filing-history stream (same pattern as tax profiles).
- Idempotency key per `(tin_root, branch_code, source, checkpoint)`.
- Reject payloads that include secrets (`imap_*`, `totp_*`, `pin`, mailbox tokens).

ORUS verification remains independent: syncing history does not prove TIN ownership.
