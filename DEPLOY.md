# Deploy buwiz-server

This stops at a ready-to-deploy state. Nothing here changes production DNS,
deploys the app, or touches a production database. `scripts/deploy.sh` prints
the plan and exits unless a human sets `BUWIZ_DEPLOY_CONFIRM`.

`codeitlikemiley/tox` was not readable from this environment (GitHub returned
"repository not found"), so this follows the Spin manifest already in this
repo rather than a pipeline that lives in that monorepo. Do not move the
other goldcoders.dev Spin services.

The tree in [hexuria/buwiz.com](https://github.com/hexuria/buwiz.com) is a
placeholder, not an extraction of the existing buwiz.com Spin app. That app
still lives in `codeitlikemiley/tox` and was not copied. Do not deploy the
placeholder as the production marketing site, and do not point production
DNS at it, until the tox app is extracted into that repository.

## Layout

See [docs/domains.md](docs/domains.md).

- `account.buwiz.com` and `api.buwiz.com` are two hostnames on **one**
  buwiz-server Spin app.
- `buwiz.com` / `www.buwiz.com` are the marketing app in
  [hexuria/buwiz.com](https://github.com/hexuria/buwiz.com). Deploy that repo
  separately. Its `DEPLOY.md` lists its own records.

## DNS (do not apply until approved)

Point both API names at the same buwiz-server app, and the marketing names at
the marketing app. Fermyon Cloud custom domains use a CNAME to the app's
`*.fermyon.app` hostname. An apex (`buwiz.com`) cannot be a standard CNAME;
use the registrar's ALIAS / ANAME / CNAME-flattening record, or an A/AAAA
record if the platform publishes addresses. Replace `<server-app>` and
`<site-app>` with the names Fermyon assigns after the first deploy.

| Name | Type | Value |
|------|------|--------|
| `account.buwiz.com` | CNAME | `<server-app>.fermyon.app` |
| `api.buwiz.com` | CNAME | `<server-app>.fermyon.app` |
| `www.buwiz.com` | CNAME | `<site-app>.fermyon.app` |
| `buwiz.com` | ALIAS / ANAME | `<site-app>.fermyon.app` |

TLS is the platform certificate for those names. Add each hostname in the
Fermyon Cloud app's custom domains before expecting HTTPS to succeed.

SpinKube alternative (only if the cluster is already the target): build an
OCI image with `spin registry push` and apply a `SpinApp` that sets the same
variables as `spin.production.toml.example`. Do not apply that manifest to
the cluster that serves the other goldcoders.dev apps without a separate
namespace and an explicit approval. No kubeconfig is used by `scripts/deploy.sh`.

## Database and Redis

Create a **new** Postgres database and a **new** Redis instance for this
app. Do not point `database_url` or `redis_url` at an existing production
database. Apply migrations with the `migrate` feature against that new
database before the first request:

```bash
DATABASE_URL='postgres://…' cargo run --locked --no-default-features --features migrate --bin wasi-auth-migrate -- apply
```

Postgres must provide `pgcrypto`. Redis is wake-only pub/sub; set
`maxmemory-policy noeviction`.

## Secrets a human must set

Copy `spin.production.toml.example` only as a starting manifest. Pass secrets
with `spin cloud deploy --variable …` (or the platform's variable store). Do
not commit them.

Required before the app can boot in production mode:

| Variable | Notes |
|----------|--------|
| `database_url` | New Postgres URL. Not an existing production database. |
| `redis_url` | New Redis URL. |
| `auth_root_key_base64` | 32-byte key, base64. Fresh. |
| `auth_trusted_ingress_key_base64` | Fresh. Required by the example manifest. |
| `auth_jwt_secret` or the JWT key pair (`auth_jwt_private_key_der_base64`, `auth_jwt_public_jwks_json`, `auth_jwt_kid`) | Fresh signing key. Issuer stays `https://account.buwiz.com`. |
| `auth_csrf_secret` | Fresh. |
| `auth_vault_key_base64` | Fresh. |
| `auth_outbox_key_base64` | Fresh. |
| `auth_recovery_code_pepper_base64` | Fresh. |

Already defaulted in `spin.production.toml.example` (confirm, do not invent
new hostnames):

| Variable | Value |
|----------|--------|
| `auth_public_base_url` | `https://account.buwiz.com` |
| `auth_api_base_url` | `https://api.buwiz.com` |
| `auth_marketing_base_url` | `https://buwiz.com` |
| `auth_cors_origins` | `https://buwiz.com,https://www.buwiz.com` |
| `auth_passkey_rp_id` | `account.buwiz.com` |
| `auth_passkey_origin` | `https://account.buwiz.com` |
| `auth_jwt_issuer` | `https://account.buwiz.com` |
| Google / Facebook / Apple redirect URIs | `https://account.buwiz.com/api/auth/oauth/<provider>/callback` |

Mail (pick one; local capture is not a production setting):

- `mail-http` plus the provider URL and API key the outbox worker expects
  (`AUTH_MAIL_PRODUCT_NAME=Buwiz`), or
- leave mail unset and accept that verification email will not send.

Social login stays disabled until each provider's client id and secret are
set and the redirect URI above is registered at that provider. The desktop
app does not need them: it uses the device flow and has no client secret.

`auth_production_mode` stays `true`. `auth_dev_tools` stays off. Never set
`AUTH_DEV_TOOLS` or the development auto-verify flag on this deployment.

## Command

```bash
scripts/deploy.sh
```

That is a dry run. A deploy happens only when the operator exports
`BUWIZ_DEPLOY_CONFIRM=deploy-buwiz-server` and a Fermyon Cloud token. This
repository's automation does not set that variable.
