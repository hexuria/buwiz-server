# Sync entities (headless-bir / Grok Bot)

v1 ships **stable server ids and stub tables**. The HTTP sync protocol is not implemented yet.

Cloud never stores IMAP passwords, mailbox OAuth tokens, `profile_pin_hash`, or `totp_secret`.

Effective-dated COR version ledgers are **not** modeled. Per-year forms are **Manual-only**.

## 1. TaxProfile (implemented)

UUID `id` is the source of truth. Exclusive uniqueness is the hashed TIN identity, not raw TIN.

| REST | |
|------|--|
| `GET/POST /tax-profiles` | list / create |
| `GET/PATCH /tax-profiles/{id}` | fetch / metadata LWW |

See the product README. Redis may wake subscribers after a profile commit; clients then GET the projection.

## 2. PerYearFormsSet (stub table)

`buwiz_server.per_year_forms_sets`

```
{
  "id": "<uuid>",
  "profile_id": "<tax profile uuid>",
  "tax_year": 2026,
  "entries": [
    { "form_code": "2550Q", "frequency": "quarterly", "active": true, "source": "Manual" }
  ]
}
```

`source` is Manual in v1. No HTTP routes yet.

## 3. FormDraft (stub table)

`buwiz_server.form_drafts`

Stable `draft_id`. Fields: `form_code`, `period`, `status`, `payload_json`. Full draft sync is **not** implemented.

## 4. Filing / submission job (stub tables)

`buwiz_server.filing_jobs` + append-only `filing_job_events`.

Status path: **Draft → Queued → Submitted → Confirmed → Paid**, plus `receipt_match_keys`.

Metadata only. **Not** BIR credentials.

## Intended later HTTP (not implemented)

```
POST /api/sync/filing-history
Authorization: Bearer <access_token>
{
  "profile_id": "<uuid>",
  "source": "headless-bir",
  "checkpoint": "2026-09-01T00:00:00Z",
  "filings": []
}

GET /api/sync/filing-history?profile_id=...&after=...
```

### Rules to encode later

- Require verified email + tax-profile holder (or reclaim-capable verified owner).
- Key all sync by **profile UUID**, never raw TIN.
- Idempotency per `(profile_id, source, checkpoint)`.
- Reject payloads that include secrets (`imap_*`, `totp_*`, `pin`, mailbox tokens, `profile_pin_hash`).

ORUS verification remains independent: syncing history does not prove TIN ownership.

TSP / BIR SFTP relay is out of scope. See [extensions.md](extensions.md).
