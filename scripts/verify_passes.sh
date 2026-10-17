#!/usr/bin/env bash
# End-to-end check of tax-profile access passes against a running dev server
# (`make dev`: Spin plus the outbox worker, mail captured). Accounts are
# verified through the captured email. Uses bearer tokens, like the desktop
# app.
#
#   BASE_URL=http://127.0.0.1:3008 bash scripts/verify_passes.sh
set -euo pipefail

BASE_URL="${BASE_URL:-http://127.0.0.1:3008}"
RUN="$(date +%s)$RANDOM"
PASSWORD="passes-e2e-password-${RUN}"
BODY="$(mktemp)"
trap 'rm -f "$BODY"' EXIT

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

# signup_and_login EMAIL -> access token. Signs up, then completes the
# captured verification email, which returns a session.
signup_and_login() {
  local email="$1"
  curl -sS -m 60 -o /dev/null -X POST "$BASE_URL/api/auth/password/register" \
    -H 'content-type: application/json' \
    --data "{\"email\":\"$email\",\"password\":\"$PASSWORD\",\"redirect_url\":\"/dashboard\"}"
  local token response
  token="$(captured_token "$email" email-verification)"
  response="$(curl -sS -m 60 -f -X POST "$BASE_URL/api/auth/email/verify" \
    -H 'content-type: application/json' \
    --data "{\"token\":\"$token\",\"redirect_url\":\"/dashboard\"}")"
  jq -e '.authenticated == true and (.access_token | length > 0)' <<<"$response" >/dev/null \
    || fail "verify $email: $response"
  jq -r '.access_token' <<<"$response"
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

step "accounts: owner, firm boss, firm clerk, outsider"
OWNER_EMAIL="owner-${RUN}@example.test"
BOSS_EMAIL="boss-${RUN}@example.test"
CLERK_EMAIL="clerk-${RUN}@example.test"
OUT_EMAIL="outsider-${RUN}@example.test"
OWNER="$(signup_and_login "$OWNER_EMAIL")"
BOSS="$(signup_and_login "$BOSS_EMAIL")"
CLERK="$(signup_and_login "$CLERK_EMAIL")"
OUTSIDER="$(signup_and_login "$OUT_EMAIL")"

step "the firm workspace, with the clerk as a plain member"
SLUG="abc-firm-${RUN}"
expect 200 POST /api/organizations "$BOSS" "{\"name\":\"ABC Firm ${RUN}\",\"slug\":\"$SLUG\"}"
FIRM="$(jq -r '.organization_id' "$BODY")"
expect 200 GET "/api/organizations/roles?organization_id=$FIRM" "$BOSS"
MEMBER_ROLE="$(jq -r '[.roles[] | select(.permissions | index("organization.update") | not)][0].role_id' "$BODY")"
[[ -n "$MEMBER_ROLE" && "$MEMBER_ROLE" != "null" ]] || fail "no role without organization.update"
expect 200 POST /api/organizations/invitations "$BOSS" \
  "{\"organization_id\":\"$FIRM\",\"email\":\"$CLERK_EMAIL\",\"role_id\":\"$MEMBER_ROLE\"}"
INVITE="$(captured_token "$CLERK_EMAIL" invitation)"
expect 200 POST /api/organizations/invitations/accept "$CLERK" "{\"token\":\"$INVITE\"}"
expect 200 POST /api/organizations "$OUTSIDER" "{\"name\":\"Other Firm ${RUN}\",\"slug\":\"other-${RUN}\"}"

step "owner registers a tax profile"
TIN="$(printf '%09d' $(( (RUN % 900000000) + 100000000 )))"
expect 200 POST /api/tax-profiles "$OWNER" \
  "{\"tin_root\":\"$TIN\",\"branch_code\":\"00000\",\"registered_name\":\"Ana Freelance\",\"rdo_code\":\"039\",\"taxpayer_type\":\"individual\"}"
PROFILE="$(jq -r '.id' "$BODY")"
jq -e '.access == "owner" and (.permissions | index("filings.publish"))' "$BODY" >/dev/null \
  || fail "owner view should say owner with every permission"

step "a pass cannot carry publishing inside the permission list"
expect 400 POST "/api/tax-profiles/$PROFILE/passes" "$OWNER" \
  "{\"organization_slug\":\"$SLUG\",\"permissions\":[\"filings.publish\"]}"

step "owner offers the Safe preset (the default) to the firm"
expect 200 POST "/api/tax-profiles/$PROFILE/passes" "$OWNER" "{\"organization_slug\":\"$SLUG\"}"
PASS="$(jq -r --arg p "$PROFILE" '[.passes[] | select(.profile_id == $p)][0].pass_id' "$BODY")"
jq -e --arg id "$PASS" '.passes[] | select(.pass_id == $id)
  | .status == "offered" and .preset == "safe" and .can_publish == false and .my_side == "owner"' \
  "$BODY" >/dev/null || fail "offer should be a Safe offer"
jq -e '[.presets[] | select(.is_default)][0].id == "safe"' "$BODY" >/dev/null || fail "Safe is the default"
expect 409 POST "/api/tax-profiles/$PROFILE/passes" "$OWNER" "{\"organization_slug\":\"$SLUG\"}"

step "nobody in the firm has access until it is accepted"
expect 403 GET "/api/tax-profiles/$PROFILE" "$CLERK"
expect 200 GET /api/tax-profiles/passes "$BOSS"
jq -e --arg id "$PASS" '.passes[] | select(.pass_id == $id) | .waiting_on_me == true and .my_side == "grantee"' \
  "$BODY" >/dev/null || fail "boss should see the offer waiting"

step "a plain member and an outsider cannot accept; the boss can"
expect 403 POST "/api/tax-profiles/$PROFILE/passes/$PASS/accept" "$CLERK"
expect 403 POST "/api/tax-profiles/$PROFILE/passes/$PASS/accept" "$OUTSIDER"
expect 200 POST "/api/tax-profiles/$PROFILE/passes/$PASS/accept" "$BOSS"

step "any member uses the pass: Safe sees but cannot edit details"
expect 200 GET "/api/tax-profiles/$PROFILE" "$CLERK"
jq -e '.access == "pass" and (.permissions | index("drafts.save")) and (.permissions | index("profile.edit") | not) and (.permissions | index("filings.publish") | not)' \
  "$BODY" >/dev/null || fail "clerk should see a Safe pass"
expect 403 PATCH "/api/tax-profiles/$PROFILE" "$CLERK" '{"display_name":"Clerk rename"}'
expect 403 POST "/api/tax-profiles/$PROFILE/archive" "$CLERK"
expect 403 GET "/api/tax-profiles/$PROFILE" "$OUTSIDER"
expect 200 GET "/api/tax-profiles?organization_id=$FIRM" "$CLERK"
jq -e --arg p "$PROFILE" '[.profiles[] | select(.id == $p)][0].access == "pass"' "$BODY" >/dev/null \
  || fail "the firm's list should include the profile with pass access"

step "only the owner shares"
expect 403 POST "/api/tax-profiles/$PROFILE/passes" "$BOSS" "{\"organization_slug\":\"other-${RUN}\"}"
expect 403 PATCH "/api/tax-profiles/$PROFILE/passes/$PASS" "$BOSS" '{"preset":"full"}'

step "owner widens it to Full and switches on publishing"
expect 200 PATCH "/api/tax-profiles/$PROFILE/passes/$PASS" "$OWNER" '{"preset":"full","allow_publish":true}'
jq -e --arg id "$PASS" '.passes[] | select(.pass_id == $id) | .can_publish == true and .status == "active"' \
  "$BODY" >/dev/null || fail "pass should now publish"
expect 200 PATCH "/api/tax-profiles/$PROFILE" "$CLERK" '{"display_name":"Ana (via firm)"}'
jq -e '.display_name == "Ana (via firm)" and (.permissions | index("filings.publish"))' "$BODY" >/dev/null \
  || fail "clerk edit should land and report publish"

step "owner revokes: access ends at once"
expect 200 POST "/api/tax-profiles/$PROFILE/passes/$PASS/end" "$OWNER"
expect 403 GET "/api/tax-profiles/$PROFILE" "$CLERK"
expect 200 GET "/api/tax-profiles?organization_id=$FIRM" "$CLERK"
jq -e --arg p "$PROFILE" '[.profiles[] | select(.id == $p)] | length == 0' "$BODY" >/dev/null \
  || fail "revoked profile should leave the firm's list"
expect 409 POST "/api/tax-profiles/$PROFILE/passes/$PASS/accept" "$BOSS"

step "the firm asks with the TIN; the owner approves"
expect 404 POST /api/tax-profiles/passes/request "$BOSS" \
  "{\"organization_id\":\"$FIRM\",\"tin_root\":\"999999999\",\"branch_code\":\"00000\",\"preset\":\"read_only\"}"
expect 403 POST /api/tax-profiles/passes/request "$CLERK" \
  "{\"organization_id\":\"$FIRM\",\"tin_root\":\"$TIN\",\"branch_code\":\"00000\",\"preset\":\"read_only\"}"
expect 200 POST /api/tax-profiles/passes/request "$BOSS" \
  "{\"organization_id\":\"$FIRM\",\"tin_root\":\"$TIN\",\"branch_code\":\"00000\",\"preset\":\"read_only\"}"
REQ="$(jq -r --arg p "$PROFILE" '[.passes[] | select(.profile_id == $p and .status == "requested")][0].pass_id' "$BODY")"
expect 200 GET /api/tax-profiles/passes "$OWNER"
jq -e --arg id "$REQ" '.passes[] | select(.pass_id == $id) | .waiting_on_me == true and .my_side == "owner" and .preset == "read_only"' \
  "$BODY" >/dev/null || fail "owner should see the request waiting"
expect 403 POST "/api/tax-profiles/$PROFILE/passes/$REQ/approve" "$BOSS" '{}'
expect 200 POST "/api/tax-profiles/$PROFILE/passes/$REQ/approve" "$OWNER" '{"ends_on":"2099-12-31"}'
jq -e --arg id "$REQ" '.passes[] | select(.pass_id == $id) | .status == "active" and (.expires_at | startswith("2099-12-31T16:00"))' \
  "$BODY" >/dev/null || fail "approved pass should be active until 2099-12-31 (Manila)"
expect 200 GET "/api/tax-profiles/$PROFILE" "$CLERK"
jq -e '.access == "pass" and (.permissions | index("drafts.save") | not)' "$BODY" >/dev/null \
  || fail "read-only pass should not save drafts"

step "the firm gives the pass back"
expect 200 POST "/api/tax-profiles/$PROFILE/passes/$REQ/end" "$BOSS"
expect 403 GET "/api/tax-profiles/$PROFILE" "$CLERK"

step "an end date in the past is refused"
expect 400 POST "/api/tax-profiles/$PROFILE/passes" "$OWNER" \
  "{\"organization_slug\":\"$SLUG\",\"ends_on\":\"2001-01-01\"}"

echo
echo "PASS: access passes end to end (profile $PROFILE)"
echo "PROFILE=$PROFILE"
