#!/usr/bin/env bash
# Runs the database tests (src/db_tests) against a migrated Postgres.
#
#   DATABASE_URL=postgres://user@127.0.0.1:5432/db scripts/test_db.sh [test-name-filter] [cargo test args]
#
# The database must already be migrated (`make db-migrate`). The keys below
# are throwaway test values; never point this at a database with real data.
set -euo pipefail
: "${DATABASE_URL:?set DATABASE_URL to a migrated test database}"
export AUTH_PUBLIC_BASE_URL="${AUTH_PUBLIC_BASE_URL:-http://127.0.0.1:3000}"
export AUTH_ROOT_KEY_BASE64="${AUTH_ROOT_KEY_BASE64:-dGVzdC1vbmx5LXJvb3Qta2V5LW5ldmVyLWluLXByb2Q=}"
export BUWIZ_REQUIRE_DATABASE=1
exec cargo test --lib --features ssr,postgres "${1:-db_tests}" "${@:2}"
