# Sync entities (headless-bir / Grok Bot)

v1 ships **stable server ids and canonical read-model tables**. Year / draft / filing HTTP is not implemented yet; command and event names are typed in `src/domain/v1_sync.rs`.

Cloud never stores IMAP passwords, mailbox OAuth tokens, `profile_pin_hash`, or `totp_secret`.

Effective-dated COR version ledgers are **not** modeled. Per-year forms are **Manual-only**.

Identity mapping: `accounts` → `auth_users`; refresh tokens → `auth_refresh_tokens`; devices → `oauth_device_codes` + `auth_sessions`. `tax_profiles.account_id` is `auth_users.user_id`.

## 1. TaxProfile (implemented)

UUID `id` is the source of truth. Exclusive uniqueness is `tin_hash` on `RegisterTaxProfile` only, never raw TIN.

| REST | Command |
|------|---------|
| `GET /tax-profiles` | `ListTaxProfilesForAccount` |
| `POST /tax-profiles` | `RegisterTaxProfile` |
| `GET /tax-profiles/{id}` | `GetTaxProfile` |
| `PATCH /tax-profiles/{id}` | `UpdateTaxProfileIdentity` |
| `POST /tax-profiles/{id}/archive` | `ArchiveTaxProfile` |
| `POST /tax-profiles/{id}/restore` | `RestoreTaxProfile` |

See the product README. Redis may wake subscribers after a profile commit; clients then GET the projection.

## 2. ProfileYear + per_year_forms (tables; commands stubbed)

`buwiz_server.profile_years` (no effective_from/until) and `buwiz_server.per_year_forms`.

Commands: `CloneProfileYear` (copy prior year’s forms only if dest empty), `UpdateProfileYear` (null = inherit), `SetYearForms` (replace active set), `ActivateYearForm` / `DeactivateYearForm`.

Queries: `GetProfileYear`, `ListYearForms`.

`source` is Manual in v1. No HTTP routes yet.

## 3. FormDraft (canonical table)

`buwiz_server.form_drafts` — PK `id`. Fields: `form_code`, `tax_year`, `period_key`, `payload`, `rev`, `saved_once`.

Commands: `UpsertFormDraft` (bump rev; `saved_once=true` on success), `MarkDraftSaved`.

Queries: `GetFormDraft`, `ListDraftsForYear`.

## 4. Filing (canonical table)

`buwiz_server.filings`.

Status path: **queued → submitted → confirmed → paid** (`EnqueueFiling` → `FilingQueued`, plus submit/confirm/fail/paid).

Authz for enqueue: caller owns the profile (`account_id`); form is active in the year set.

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

- Require verified email + tax-profile `account_id` match (UUID load, never raw TIN as SoT).
- Key all sync by **profile UUID**, never raw TIN.
- Idempotency per `(profile_id, source, checkpoint)`.
- Reject payloads that include secrets (`imap_*`, `totp_*`, `pin`, mailbox tokens, `profile_pin_hash`).

ORUS verification remains independent: syncing history does not prove TIN ownership.

TSP / BIR SFTP relay is out of scope. See [extensions.md](extensions.md).
