# Sync API: tax years, drafts and filings

The server keeps a tax profile's tax years, form drafts and filing history for backup and multi-device sync. It never reads a draft's contents and **never files with BIR**. The installed buwiz-forms app publishes, then reports what happened here.

Every request is keyed by the profile's UUID, never by a raw TIN. It goes through the same gatekeeper as the profile itself: the owner, the firm holding a draft, or a workspace with an access pass (README, "Who may do what"). A caller with no standing on the profile gets `404`. A caller whose standing lacks the step's permission gets `403`. An archived profile is read-only (`409`). Rules: `src/domain/records.rs`. Store: `src/store/records.rs`. Tables: migrations `0004` and `0010`.

The cloud never stores IMAP passwords, mailbox OAuth tokens, `profile_pin_hash`, `totp_secret` or BIR credentials. A draft whose payload carries such a key, at any depth, is refused. Effective-dated COR ledgers are not modeled, and per-year forms are chosen by hand.

## Tax years

| Request | Needs | Does |
|---|---|---|
| `GET /api/tax-profiles/{id}/years` | `years.view` | Every year, newest first, with its forms and overrides |
| `PUT /api/tax-profiles/{id}/years/{year}` | `years.manage` | Create or update a year. `{ overrides, forms }`: overrides are per-year values (null inherits the profile's), and `forms` replaces the year's set (`null` keeps it) |
| `POST /api/tax-profiles/{id}/years/{year}/clone` | `years.manage` | `{ from_year }`: creates the year if missing and copies `from_year`'s forms only if it has none yet |

A form is `{ form_code, frequency, active }`:
- `form_code` is 3 to 12 letters and digits, uppercased (`1701Q`);
- `frequency` is `monthly`, `quarterly`, `annual` or `one_time`;
- a year has at most 100 forms, each listed once.

Years are 2000 to 2100.

## Drafts

| Request | Needs | Does |
|---|---|---|
| `GET /api/tax-profiles/{id}/drafts[?tax_year=2026]` | `drafts.view` | Draft summaries (no payload), newest first |
| `GET /api/tax-profiles/{id}/drafts/{draft}` | `drafts.view` | One draft with its payload |
| `PUT /api/tax-profiles/{id}/drafts` | `drafts.save` for a new draft, `drafts.update` for an existing one | Save `{ form_code, tax_year, period_key, payload, base_rev }` |
| `POST /api/tax-profiles/{id}/drafts/{draft}/review` | `drafts.review` | Mark the current revision reviewed |

- **Key:** a draft is keyed by form, year and period. `period_key` starts with the year and is at most 16 letters, digits and dashes (`2026`, `2026-Q1`, `2026-03`).
- **Payload:** a JSON object of at most 200 KB.
- **Saving is optimistic.** A new draft sends no `base_rev` (or `0`) and becomes revision 1. A change sends the revision it started from and gets the next one. If another device saved first, the server answers `409` with the current revision; reload and merge, nothing is overwritten.
- **Reviews:** every save clears the review mark, so "reviewed" always means this revision.

## Filings

| Request | Needs | Does |
|---|---|---|
| `GET /api/tax-profiles/{id}/filings` | `filings.view` | Filing history, newest first |
| `POST /api/tax-profiles/{id}/filings` | `filings.publish` | `{ draft_id }`: queue that draft for the caller's app to send. The form must be active in that year. Only one filing per form and period can be on its way at a time |
| `POST /api/tax-profiles/{id}/filings/{filing}/submitted` | `filings.publish` | The app sent it: `{ xml_sha256 }` |
| `POST /api/tax-profiles/{id}/filings/{filing}/confirmed` | `filings.publish` | BIR confirmed it: `{ receipt_ref }` |
| `POST /api/tax-profiles/{id}/filings/{filing}/failed` | `filings.publish` | Sending failed: `{ error_summary }`. To retry, queue a new filing |
| `POST /api/tax-profiles/{id}/filings/{filing}/paid` | `filings.publish` | The tax due is paid |

The path is queued → submitted → confirmed → paid. A filing can fail while queued or submitted. Any other step is a `409`, and so is a step that loses a race with another device.

For a firm member, `filings.publish` already folds in both sides: the owner switched publishing on for the firm's pass, **and** the firm put this member on its publishers list.

## Transport limits on today's runtime

Request bodies can't stream on the current Spin/wasmtime (`waitable cannot be used synchronously`). The browser shell moves bodies of up to 6000 characters into a header, which is why Form 2303 uploads go in 4 KiB parts. Native clients such as the desktop app send real bodies, and `server_fn_http.rs` warns those may fail until the runtime can read incoming streams. Draft payloads are typically larger than 6000 characters, so check desktop draft sync against the deployed runtime before relying on it.

## Not built yet

- Batch history import from headless BIR (`POST /api/sync/filing-history`, idempotent per `(profile_id, source, checkpoint)`).
- MCP tools for years, drafts and filings (next PR). Agents never get `filings.publish`.
- TSP or BIR SFTP relay: out of scope; see [extensions.md](extensions.md).

Syncing history never proves TIN ownership; that is the Form 2303 claim (README).
