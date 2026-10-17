#!/usr/bin/env bash
# End-to-end check of AI agent sign-in (OAuth for MCP clients) against a
# running dev server (`make dev`: Spin plus the outbox worker, mail
# captured). Plays the agent app with curl: discovery, self-registration,
# the Allow screen (as the signed-in taxpayer), PKCE code exchange, refresh
# rotation, and ending the connection.
#
#   BASE_URL=http://127.0.0.1:3008 bash scripts/verify_agents.sh
set -euo pipefail

BASE_URL="${BASE_URL:-http://127.0.0.1:3008}"
RUN="$(date +%s)$RANDOM"
PASSWORD="agents-e2e-password-${RUN}"
BODY="$(mktemp)"
HEADERS="$(mktemp)"
trap 'rm -f "$BODY" "$HEADERS"' EXIT

step() { printf '\n== %s\n' "$*"; }
fail() { echo "FAIL: $*" >&2; cat "$BODY" >&2 || true; exit 1; }

# call METHOD PATH TOKEN [JSON] -> prints HTTP status; body in $BODY
call() {
  local method="$1" path="$2" token="$3" data="${4:-}"
  local args=(-sS -m 60 -o "$BODY" -w '%{http_code}' -X "$method" "$BASE_URL$path"
    -H "authorization: Bearer $token" -H 'accept: application/json')
  if [[ -n "$data" ]]; then
    args+=(-H 'content-type: application/json' --data "$data")
  fi
  curl "${args[@]}"
}

expect() {
  local want="$1"; shift
  local got
  got="$(call "$@")"
  [[ "$got" == "$want" ]] || fail "$* -> HTTP $got, wanted $want"
}

# token_call FORM... -> HTTP status of POST /oauth/token; body in $BODY
token_call() {
  local args=(-sS -m 60 -o "$BODY" -w '%{http_code}' -X POST "$BASE_URL/oauth/token")
  for pair in "$@"; do args+=(--data-urlencode "$pair"); done
  curl "${args[@]}"
}

# mcp TOKEN JSON [EXTRA_CURL_ARGS...] -> HTTP status of POST /mcp; body in
# $BODY, headers in $HEADERS
mcp() {
  local token="$1" data="$2"; shift 2
  local args=(-sS -m 60 -o "$BODY" -D "$HEADERS" -w '%{http_code}' -X POST "$BASE_URL/mcp"
    -H 'content-type: application/json' -H 'accept: application/json, text/event-stream'
    --data "$data")
  [[ -n "$token" ]] && args+=(-H "authorization: Bearer $token")
  curl "${args[@]}" "$@"
}

captured_token() {
  local recipient="$1" kind="$2" mail=""
  for _ in $(seq 1 100); do
    mail="$(curl -sS -m 10 -f -G "$BASE_URL/api/auth/dev/mail/latest" \
      --data-urlencode "recipient=$recipient" --data-urlencode "kind=$kind" 2>/dev/null || true)"
    [[ -n "$mail" ]] && break
    sleep 0.2
  done
  [[ -n "$mail" ]] || fail "no $kind mail for $recipient"
  { jq -r '.action_url // empty' <<<"$mail"; jq -r '.body_text // empty' <<<"$mail"; } \
    | grep -Eo 'token=[A-Za-z0-9._~%-]+' | head -n1 | cut -d= -f2
}

step "the taxpayer signs up and registers two profiles; a stranger has one"
signup() {
  local email="$1"
  curl -sS -m 60 -o /dev/null -X POST "$BASE_URL/api/auth/password/register" \
    -H 'content-type: application/json' \
    --data "{\"email\":\"$email\",\"password\":\"$PASSWORD\",\"redirect_url\":\"/dashboard\"}"
  local token
  token="$(captured_token "$email" email-verification)"
  curl -sS -m 60 -f -X POST "$BASE_URL/api/auth/email/verify" \
    -H 'content-type: application/json' \
    --data "{\"token\":\"$token\",\"redirect_url\":\"/dashboard\"}"
}
OWNER_LOGIN="$(signup "owner-${RUN}@example.test")"
OWNER="$(jq -r '.access_token' <<<"$OWNER_LOGIN")"
OWNER_SESSION="$(jq -r '.session_id' <<<"$OWNER_LOGIN")"
STRANGER="$(signup "stranger-${RUN}@example.test" | jq -r '.access_token')"
COOKIE="wasi_auth_dev_session=$OWNER_SESSION"

tin() { printf '%09d' $(( ((RUN + $1) % 900000000) + 100000000 )); }
expect 200 POST /api/tax-profiles "$OWNER" \
  "{\"tin_root\":\"$(tin 1)\",\"branch_code\":\"00000\",\"registered_name\":\"Ana Freelance\",\"rdo_code\":\"039\",\"taxpayer_type\":\"individual\"}"
PROFILE="$(jq -r '.id' "$BODY")"
expect 200 POST /api/tax-profiles "$OWNER" \
  "{\"tin_root\":\"$(tin 2)\",\"branch_code\":\"00000\",\"registered_name\":\"Ana Rentals\",\"rdo_code\":\"039\",\"taxpayer_type\":\"individual\"}"
OTHER_PROFILE="$(jq -r '.id' "$BODY")"
expect 200 POST /api/tax-profiles "$STRANGER" \
  "{\"tin_root\":\"$(tin 3)\",\"branch_code\":\"00000\",\"registered_name\":\"Not Ana Corp\",\"rdo_code\":\"039\",\"taxpayer_type\":\"individual\"}"
STRANGER_PROFILE="$(jq -r '.id' "$BODY")"

step "discovery: protected-resource and authorization-server metadata"
curl -sS -m 30 -f "$BASE_URL/.well-known/oauth-protected-resource" -o "$BODY" || fail "no RFC 9728 metadata"
jq -e --arg b "$BASE_URL" '.resource == ($b + "/mcp") and (.authorization_servers | index($b))' "$BODY" >/dev/null \
  || echo "note: resource/issuer differ from BASE_URL (AUTH_PUBLIC_BASE_URL?): $(cat "$BODY")"
curl -sS -m 30 -f "$BASE_URL/.well-known/oauth-authorization-server" -o "$BODY"
jq -e '.registration_endpoint | endswith("/oauth/register")' "$BODY" >/dev/null || fail "no registration_endpoint"

step "registration refuses unsafe redirects and accepts a loopback app"
status="$(curl -sS -m 30 -o "$BODY" -w '%{http_code}' -X POST "$BASE_URL/oauth/register" \
  -H 'content-type: application/json' \
  --data '{"client_name":"Evil","redirect_uris":["http://evil.example/cb"]}')"
[[ "$status" == 400 ]] || fail "non-loopback http redirect should be refused (got $status)"
REDIRECT="http://127.0.0.1:8765/callback"
status="$(curl -sS -m 30 -o "$BODY" -w '%{http_code}' -X POST "$BASE_URL/oauth/register" \
  -H 'content-type: application/json' \
  --data "{\"client_name\":\"Claude Code e2e\",\"redirect_uris\":[\"$REDIRECT\"],\"token_endpoint_auth_method\":\"none\"}")"
[[ "$status" == 201 ]] || fail "registration -> $status"
CLIENT="$(jq -r '.client_id' "$BODY")"
[[ "$CLIENT" == agent_* ]] || fail "client id should start with agent_"

step "the Allow screen (signed in), with a different loopback port"
VERIFIER="$(head -c 48 /dev/urandom | base64 | tr '+/' '-_' | tr -d '=\n' | cut -c1-64)"
CHALLENGE="$(printf '%s' "$VERIFIER" | openssl dgst -sha256 -binary | base64 | tr '+/' '-_' | tr -d '=\n')"
USED_REDIRECT="http://127.0.0.1:9911/callback"
STATE="st-${RUN}"
AUTHZ="/oauth/authorize?response_type=code&client_id=$CLIENT&redirect_uri=$(jq -rn --arg v "$USED_REDIRECT" '$v|@uri')&code_challenge=$CHALLENGE&code_challenge_method=S256&state=$STATE"
status="$(curl -sS -m 30 -o "$BODY" -w '%{http_code}' "$BASE_URL$AUTHZ")"
[[ "$status" == 302 ]] || fail "signed out should be sent to log in (got $status)"
status="$(curl -sS -m 30 -o "$BODY" -w '%{http_code}' -H "cookie: $COOKIE" "$BASE_URL$AUTHZ")"
[[ "$status" == 200 ]] || fail "Allow screen -> $status"
grep -q 'Claude Code e2e' "$BODY" || fail "Allow screen should name the app"
grep -q "$PROFILE" "$BODY" && grep -q "$OTHER_PROFILE" "$BODY" || fail "both own profiles should be offered"
! grep -q "$STRANGER_PROFILE" "$BODY" || fail "a stranger's profile must not be offered"
CSRF="$(grep -Eo 'name="csrf" value="[^"]*"' "$BODY" | head -n1 | sed 's/.*value="//; s/"$//')"
[[ -n "$CSRF" ]] || fail "no csrf on the Allow screen"

allow() { # allow DECISION PROFILE... -> Location header of the answer
  local decision="$1"; shift
  local args=(-sS -m 30 -o /dev/null -D "$HEADERS" -X POST "$BASE_URL/oauth/authorize"
    -H "cookie: $COOKIE"
    --data-urlencode "client_id=$CLIENT" --data-urlencode "redirect_uri=$USED_REDIRECT"
    --data-urlencode "state=$STATE" --data-urlencode "code_challenge=$CHALLENGE"
    --data-urlencode "code_challenge_method=S256" --data-urlencode "csrf=$CSRF"
    --data-urlencode "preset=read_only" --data-urlencode "decision=$decision")
  for p in "$@"; do args+=(--data-urlencode "profile=$p"); done
  curl "${args[@]}"
  grep -i '^location:' "$HEADERS" | head -n1 | sed 's/^[Ll]ocation: *//' | tr -d '\r'
}

step "Deny sends access_denied back to the app"
LOCATION="$(allow deny "$PROFILE")"
[[ "$LOCATION" == "$USED_REDIRECT?error=access_denied&state=$STATE" ]] || fail "deny -> $LOCATION"

step "Allow on one profile, smuggling in the stranger's: only the own one lands"
LOCATION="$(allow allow "$PROFILE" "$STRANGER_PROFILE")"
[[ "$LOCATION" == "$USED_REDIRECT?code="* ]] || fail "allow -> $LOCATION"
CODE="$(sed -E 's/.*[?&]code=([^&]+).*/\1/' <<<"$LOCATION")"
[[ "$(sed -E 's/.*[?&]state=([^&]+).*/\1/' <<<"$LOCATION")" == "$STATE" ]] || fail "state not returned"

step "code exchange: wrong verifier refused, right one works once"
status="$(token_call grant_type=authorization_code "client_id=$CLIENT" "code=$CODE" \
  "redirect_uri=$USED_REDIRECT" "code_verifier=$(printf 'x%.0s' {1..50})")"
[[ "$status" == 400 ]] || fail "bad verifier -> $status"
# The bad attempt consumed the code: authorize again for a fresh one.
LOCATION="$(allow allow "$PROFILE")"
CODE="$(sed -E 's/.*[?&]code=([^&]+).*/\1/' <<<"$LOCATION")"
status="$(token_call grant_type=authorization_code "client_id=$CLIENT" "code=$CODE" \
  "redirect_uri=$USED_REDIRECT" "code_verifier=$VERIFIER")"
[[ "$status" == 200 ]] || fail "code exchange -> $status"
ACCESS="$(jq -r '.access_token' "$BODY")"
REFRESH="$(jq -r '.refresh_token' "$BODY")"
[[ "$ACCESS" == bwa_* && "$REFRESH" == bwr_* ]] || fail "agent tokens should be bwa_/bwr_"
status="$(token_call grant_type=authorization_code "client_id=$CLIENT" "code=$CODE" \
  "redirect_uri=$USED_REDIRECT" "code_verifier=$VERIFIER")"
[[ "$status" == 400 ]] || fail "a code works only once (got $status)"

step "MCP: no token -> 401 pointing at the metadata; foreign Origin -> 403; GET -> 405"
INIT='{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-06-18","capabilities":{},"clientInfo":{"name":"e2e","version":"0"}}}'
[[ "$(mcp "" "$INIT")" == 401 ]] || fail "no token should be 401"
grep -qi '^www-authenticate: Bearer resource_metadata=".*/.well-known/oauth-protected-resource/mcp"' "$HEADERS" \
  || fail "401 should name the resource metadata"
[[ "$(mcp "$ACCESS" "$INIT" -H 'origin: https://evil.example')" == 403 ]] || fail "foreign origin should be 403"
[[ "$(curl -sS -m 30 -o /dev/null -w '%{http_code}' -H "authorization: Bearer $ACCESS" "$BASE_URL/mcp")" == 405 ]] \
  || fail "GET /mcp should be 405"

step "MCP: initialize, initialized, tools/list"
[[ "$(mcp "$ACCESS" "$INIT")" == 200 ]] || fail "initialize"
jq -e '.result.protocolVersion == "2025-06-18" and .result.capabilities.tools' "$BODY" >/dev/null || fail "initialize result"
[[ "$(mcp "$ACCESS" '{"jsonrpc":"2.0","method":"notifications/initialized"}' -H 'mcp-protocol-version: 2025-06-18')" == 202 ]] \
  || fail "a notification should get 202"
[[ "$(mcp "$ACCESS" '{"jsonrpc":"2.0","id":2,"method":"tools/list"}' -H 'mcp-protocol-version: 1999-01-01')" == 400 ]] \
  || fail "unknown protocol version header should be 400"
[[ "$(mcp "$ACCESS" '{"jsonrpc":"2.0","id":2,"method":"tools/list"}' -H 'mcp-protocol-version: 2025-06-18')" == 200 ]] || fail "tools/list"
jq -e '[.result.tools[].name] == ["list_tax_profiles","get_tax_profile","list_tax_years","list_form_drafts","get_form_draft","list_filings"]
  and all(.result.tools[]; .annotations.readOnlyHint)' "$BODY" >/dev/null \
  || fail "six read-only tools"

step "MCP: list_tax_profiles shows only the connected profile, read-only"
mcp "$ACCESS" '{"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"list_tax_profiles","arguments":{}}}' >/dev/null
jq -e --arg p "$PROFILE" '.result.isError == false
  and ([.result.structuredContent.profiles[].profile_id] == [$p])
  and (.result.structuredContent.profiles[0].permissions | index("profile.view") and (index("profile.edit") | not) and (index("filings.publish") | not))' \
  "$BODY" >/dev/null || fail "list_tax_profiles"

step "MCP: get_tax_profile on it works; on anything else it is a tool error"
call_get() { mcp "$ACCESS" "{\"jsonrpc\":\"2.0\",\"id\":4,\"method\":\"tools/call\",\"params\":{\"name\":\"get_tax_profile\",\"arguments\":{\"profile_id\":\"$1\"}}}" >/dev/null; }
call_get "$PROFILE"
jq -e '.result.isError == false and .result.structuredContent.name == "Ana Freelance"
  and (.result.structuredContent | has("owner_user_id") | not)' "$BODY" >/dev/null || fail "get own connected profile"
for other in "$OTHER_PROFILE" "$STRANGER_PROFILE"; do
  call_get "$other"
  jq -e '.result.isError == true' "$BODY" >/dev/null || fail "get $other should be refused"
done
step "MCP: years, drafts and filings of the connected profile only"
call_tool() { mcp "$ACCESS" "{\"jsonrpc\":\"2.0\",\"id\":6,\"method\":\"tools/call\",\"params\":{\"name\":\"$1\",\"arguments\":$2}}" >/dev/null; }
call_tool list_tax_years "{\"profile_id\":\"$PROFILE\"}"
jq -e '.result.isError == false and (.result.structuredContent.years | type == "array")' "$BODY" >/dev/null || fail "list_tax_years"
call_tool list_form_drafts "{\"profile_id\":\"$PROFILE\",\"tax_year\":2026}"
jq -e '.result.isError == false and (.result.structuredContent.drafts | type == "array")' "$BODY" >/dev/null || fail "list_form_drafts"
call_tool list_filings "{\"profile_id\":\"$PROFILE\"}"
jq -e '.result.isError == false and (.result.structuredContent.filings | type == "array")' "$BODY" >/dev/null || fail "list_filings"
call_tool get_form_draft "{\"profile_id\":\"$PROFILE\"}"
jq -e '.result.isError == true' "$BODY" >/dev/null || fail "get_form_draft without draft_id should be a tool error"
for other in "$OTHER_PROFILE" "$STRANGER_PROFILE"; do
  for tool in list_tax_years list_form_drafts list_filings; do
    call_tool "$tool" "{\"profile_id\":\"$other\"}"
    jq -e '.result.isError == true' "$BODY" >/dev/null || fail "$tool on $other should be refused"
  done
done
mcp "$ACCESS" '{"jsonrpc":"2.0","id":5,"method":"tools/call","params":{"name":"file_return","arguments":{}}}' >/dev/null
jq -e '.error.code == -32602' "$BODY" >/dev/null || fail "unknown tool should be -32602"

step "an agent token is not the taxpayer: REST refuses it"
expect 401 GET /api/tax-profiles "$ACCESS"
expect 401 GET /api/agents "$ACCESS"

step "the taxpayer sees one connection (re-allowing replaced the first)"
expect 200 GET /api/agents "$OWNER"
jq -e --arg p "$PROFILE" --arg s "$STRANGER_PROFILE" '
  (.connections | length) == 1
  and .connections[0].client_name == "Claude Code e2e"
  and .connections[0].preset == "read_only"
  and ([.connections[0].profiles[].profile_id] == [$p])' "$BODY" >/dev/null \
  || fail "one read-only connection on the own profile only"

step "refresh needs the app's own client_id"
status="$(token_call grant_type=refresh_token "refresh_token=$REFRESH")"
[[ "$status" == 400 ]] || fail "refresh without client_id -> $status"
status="$(token_call grant_type=refresh_token "refresh_token=$REFRESH" "client_id=agent_someone_else")"
[[ "$status" == 400 ]] || fail "refresh for another app -> $status"

step "refresh rotates; reusing the old refresh token ends the whole connection"
status="$(token_call grant_type=refresh_token "refresh_token=$REFRESH" "client_id=$CLIENT")"
[[ "$status" == 200 ]] || fail "refresh -> $status"
NEW_REFRESH="$(jq -r '.refresh_token' "$BODY")"
status="$(token_call grant_type=refresh_token "refresh_token=$REFRESH" "client_id=$CLIENT")"
[[ "$status" == 400 ]] || fail "replayed refresh -> $status"
status="$(token_call grant_type=refresh_token "refresh_token=$NEW_REFRESH" "client_id=$CLIENT")"
[[ "$status" == 400 ]] || fail "after a replay the newer refresh token is dead too (got $status)"
[[ "$(mcp "$ACCESS" "$INIT")" == 401 ]] || fail "after a replay the access token is dead at /mcp too"
expect 200 GET /api/agents "$OWNER"
jq -e '.connections | length == 0' "$BODY" >/dev/null || fail "a replay ends the connection"

step "allow again; a stranger cannot end the connection; the taxpayer can, and the next call is 401"
LOCATION="$(allow allow "$PROFILE")"
CODE="$(sed -E 's/.*[?&]code=([^&]+).*/\1/' <<<"$LOCATION")"
status="$(token_call grant_type=authorization_code "client_id=$CLIENT" "code=$CODE" \
  "redirect_uri=$USED_REDIRECT" "code_verifier=$VERIFIER")"
[[ "$status" == 200 ]] || fail "second code exchange -> $status"
ACCESS2="$(jq -r '.access_token' "$BODY")"
[[ "$(mcp "$ACCESS2" "$INIT")" == 200 ]] || fail "the new connection works"
expect 200 GET /api/agents "$OWNER"
CONNECTION2="$(jq -r '.connections[0].connection_id' "$BODY")"
expect 404 POST "/api/agents/$CONNECTION2/revoke" "$STRANGER" '{}'
expect 200 POST "/api/agents/$CONNECTION2/revoke" "$OWNER" '{}'
jq -e '.connections | length == 0' "$BODY" >/dev/null || fail "connection should be gone"
[[ "$(mcp "$ACCESS2" "$INIT")" == 401 ]] || fail "after End the agent's next call is 401"
expect 200 GET /api/tax-profiles "$OWNER"   # the taxpayer is still signed in

echo
echo "OK: agent sign-in verified"
