#!/usr/bin/env bash
# Print the buwiz-server deploy plan. Does not change DNS and does not deploy
# unless BUWIZ_DEPLOY_CONFIRM=deploy-buwiz-server (a human must set that).
set -euo pipefail

root="$(cd "$(dirname "$0")/.." && pwd)"
cd "$root"

echo "buwiz-server deploy plan (dry run unless BUWIZ_DEPLOY_CONFIRM is set)"
echo
echo "DNS — set these by hand after the app exists. This script does not."
echo "  account.buwiz.com  CNAME  <server-app>.fermyon.app"
echo "  api.buwiz.com      CNAME  <server-app>.fermyon.app"
echo "  www.buwiz.com      CNAME  <site-app>.fermyon.app     (hexuria/buwiz.com)"
echo "  buwiz.com          ALIAS  <site-app>.fermyon.app"
echo
echo "Manifest: spin.production.toml.example"
echo "Public URL: https://account.buwiz.com"
echo "API URL:    https://api.buwiz.com"
echo
echo "Refusing to read or write any database. Apply migrations yourself"
echo "against a new database before the first request. See DEPLOY.md."

if [[ "${BUWIZ_DEPLOY_CONFIRM:-}" != "deploy-buwiz-server" ]]; then
  echo
  echo "Dry run only. Export BUWIZ_DEPLOY_CONFIRM=deploy-buwiz-server to run:"
  echo "  spin cloud deploy --from spin.production.toml.example"
  exit 0
fi

if ! command -v spin >/dev/null 2>&1; then
  echo "spin is not on PATH" >&2
  exit 1
fi

exec spin cloud deploy --from spin.production.toml.example "$@"
