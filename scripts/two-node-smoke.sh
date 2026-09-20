#!/usr/bin/env bash

set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$repo_root"

colloq_bin="${COLLOQ_BIN:-target/debug/colloq}"
if [[ ! -x "$colloq_bin" ]]; then
  echo "Colloq binary not found at $colloq_bin; run cargo build --locked first" >&2
  exit 1
fi
if ! command -v jq >/dev/null 2>&1; then
  echo "jq is required for the two-node smoke test" >&2
  exit 1
fi

work_dir="$(mktemp -d "${TMPDIR:-/tmp}/colloq-two-node.XXXXXX")"
active_pid=""

cleanup() {
  if [[ -n "$active_pid" ]]; then
    kill "$active_pid" >/dev/null 2>&1 || true
  fi
  rm -rf "$work_dir"
}
trap cleanup EXIT INT TERM

fail() {
  echo "two-node smoke test failed: $*" >&2
  exit 1
}

wait_for_ticket() {
  local ticket="$1"
  local process_id="$2"
  local attempt
  for attempt in $(seq 1 100); do
    if [[ -s "$ticket" ]] && jq -e '.colloq_endpoint == "0.1.0" and (.addresses | length > 0)' "$ticket" >/dev/null; then
      return 0
    fi
    if ! kill -0 "$process_id" >/dev/null 2>&1; then
      return 1
    fi
    sleep 0.1
  done
  return 1
}

assert_private_mode() {
  local path="$1"
  local mode
  if stat -f '%Lp' "$path" >/dev/null 2>&1; then
    mode="$(stat -f '%Lp' "$path")"
  else
    mode="$(stat -c '%a' "$path")"
  fi
  [[ "$mode" == "600" ]] || fail "$path has mode $mode, expected 600"
}

trust_dir="$work_dir/trust"
"$colloq_bin" bootstrap-two-node --out "$trust_dir" >"$work_dir/bootstrap.log"

assert_private_mode "$trust_dir/server.colloqnode.json"
assert_private_mode "$trust_dir/client.colloqnode.json"
jq -e '.rules | length == 2' "$trust_dir/server.authorization.json" >/dev/null
jq -e '.rules | length == 2' "$trust_dir/client.authorization.json" >/dev/null

generate_ticket="$work_dir/generate-server.colloqendpoint.json"
generate_server_report="$work_dir/generate-server-report.json"
generate_client_report="$work_dir/generate-client-report.json"

"$colloq_bin" serve-iroh \
  --identity "$trust_dir/server.colloqnode.json" \
  --policy "$trust_dir/server.authorization.json" \
  --listen 127.0.0.1:0 \
  --ticket-out "$generate_ticket" \
  --report-out "$generate_server_report" \
  --tokens 4 >"$work_dir/generate-server.log" 2>&1 &
generate_server_pid=$!
active_pid="$generate_server_pid"

wait_for_ticket "$generate_ticket" "$generate_server_pid" || {
  cat "$work_dir/generate-server.log" >&2
  fail "Generate server did not publish a ticket"
}

"$colloq_bin" connect-iroh \
  --identity "$trust_dir/client.colloqnode.json" \
  --policy "$trust_dir/client.authorization.json" \
  --server "$generate_ticket" \
  --report-out "$generate_client_report" >"$work_dir/generate-client.log" 2>&1

if ! wait "$generate_server_pid"; then
  cat "$work_dir/generate-server.log" >&2
  fail "Generate server exited unsuccessfully"
fi
active_pid=""

"$colloq_bin" verify-session \
  --client "$generate_client_report" \
  --server "$generate_server_report" >"$work_dir/generate-verification.json"
jq -e '.semantic_trace_equivalent and .outcome_equivalent and .client.completed and .server.completed' \
  "$work_dir/generate-verification.json" >/dev/null

server_draft="$work_dir/server.colloqdraft"
client_draft="$work_dir/client.colloqdraft"
"$colloq_bin" draft-create examples/generate.colloqconv.json --out "$server_draft" >"$work_dir/draft-create.log"
cp "$server_draft" "$client_draft"
"$colloq_bin" draft-patch "$server_draft" --pointer /annotations/server_observed --value true \
  >"$work_dir/draft-server-patch.log"
"$colloq_bin" draft-patch "$client_draft" --pointer /annotations/client_observed --value true \
  >"$work_dir/draft-client-patch.log"

draft_ticket="$work_dir/draft-server.colloqendpoint.json"
draft_server_report="$work_dir/draft-server-report.json"
draft_client_report="$work_dir/draft-client-report.json"

"$colloq_bin" draft-serve-iroh \
  --draft "$server_draft" \
  --identity "$trust_dir/server.colloqnode.json" \
  --policy "$trust_dir/server.authorization.json" \
  --listen 127.0.0.1:0 \
  --ticket-out "$draft_ticket" \
  --report-out "$draft_server_report" >"$work_dir/draft-server.log" 2>&1 &
draft_server_pid=$!
active_pid="$draft_server_pid"

wait_for_ticket "$draft_ticket" "$draft_server_pid" || {
  cat "$work_dir/draft-server.log" >&2
  fail "draft-sync server did not publish a ticket"
}

"$colloq_bin" draft-connect-iroh \
  --draft "$client_draft" \
  --identity "$trust_dir/client.colloqnode.json" \
  --policy "$trust_dir/client.authorization.json" \
  --server "$draft_ticket" \
  --report-out "$draft_client_report" >"$work_dir/draft-client.log" 2>&1

if ! wait "$draft_server_pid"; then
  cat "$work_dir/draft-server.log" >&2
  fail "draft-sync server exited unsuccessfully"
fi
active_pid=""

jq -e -s '
  .[0].completed and .[1].completed and
  .[0].conversation_identity == .[1].conversation_identity and
  .[0].plan_identity == .[1].plan_identity and
  .[0].heads == .[1].heads
' "$draft_client_report" "$draft_server_report" >/dev/null

"$colloq_bin" draft-promote "$server_draft" \
  --conversation-out "$work_dir/server-promoted.colloqconv.json" \
  --plan-out "$work_dir/server-promoted.colloqplan.json" >"$work_dir/server-promote.log"
"$colloq_bin" draft-promote "$client_draft" \
  --conversation-out "$work_dir/client-promoted.colloqconv.json" \
  --plan-out "$work_dir/client-promoted.colloqplan.json" >"$work_dir/client-promote.log"

cmp "$work_dir/server-promoted.colloqconv.json" "$work_dir/client-promoted.colloqconv.json" >/dev/null \
  || fail "promoted conversations diverged"
cmp "$work_dir/server-promoted.colloqplan.json" "$work_dir/client-promoted.colloqplan.json" >/dev/null \
  || fail "promoted plans diverged"
jq -e '.annotations.server_observed and .annotations.client_observed' \
  "$work_dir/client-promoted.colloqconv.json" >/dev/null

impostor_identity="$work_dir/impostor.colloqnode.json"
impostor_policy="$work_dir/impostor.authorization.json"
"$colloq_bin" node-init --out "$impostor_identity" >"$work_dir/impostor-init.log"
assert_private_mode "$impostor_identity"
server_identity="$(jq -r '.endpoint_identity' "$trust_dir/server.colloqnode.json")"
"$colloq_bin" policy-allow \
  --policy "$impostor_policy" \
  --peer "$server_identity" \
  --role server >"$work_dir/impostor-policy.log"

unauthorized_ticket="$work_dir/unauthorized-server.colloqendpoint.json"
"$colloq_bin" serve-iroh \
  --identity "$trust_dir/server.colloqnode.json" \
  --policy "$trust_dir/server.authorization.json" \
  --listen 127.0.0.1:0 \
  --ticket-out "$unauthorized_ticket" >"$work_dir/unauthorized-server.log" 2>&1 &
unauthorized_server_pid=$!
active_pid="$unauthorized_server_pid"

wait_for_ticket "$unauthorized_ticket" "$unauthorized_server_pid" || {
  cat "$work_dir/unauthorized-server.log" >&2
  fail "authorization test server did not publish a ticket"
}

if "$colloq_bin" connect-iroh \
  --identity "$impostor_identity" \
  --policy "$impostor_policy" \
  --server "$unauthorized_ticket" >"$work_dir/unauthorized-client.log" 2>&1; then
  fail "unauthorized client was accepted"
fi
if wait "$unauthorized_server_pid"; then
  fail "server reported success for an unauthorized client"
fi
active_pid=""
if ! grep -Eiq 'not authorized|unauthorized|identity mismatch' \
  "$work_dir/unauthorized-server.log" "$work_dir/unauthorized-client.log"; then
  cat "$work_dir/unauthorized-server.log" >&2
  cat "$work_dir/unauthorized-client.log" >&2
  fail "authorization failure was not explicit"
fi

echo "two-node smoke test passed: Generate agreement, draft convergence, and fail-closed authorization"
