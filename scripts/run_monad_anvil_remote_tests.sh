#!/usr/bin/env bash

set -euo pipefail

usage() {
  cat <<'EOF'
Usage: scripts/run_monad_anvil_remote_tests.sh [options] [-- <pytest selection args...>]

Start `anvil --monad`, wait for the local RPC to become ready, then run:

  UV_CACHE_DIR=/tmp/uv-cache uv run --no-sync execute remote \
    --fork=MONAD_EIGHT \
    --rpc-endpoint=http://127.0.0.1:8545 \
    --rpc-seed-key=0xac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80 \
    --chain-id=31337 \
    --no-html \
    tests -q

Default selection:
  tests

Options:
  --anvil-bin PATH       Anvil binary to run (default: anvil)
  --rpc-endpoint URL     RPC endpoint to wait for and test against
                         (default: http://127.0.0.1:8545)
  --fork FORK            EEST fork to execute (default: MONAD_EIGHT)
  --chain-id ID          Chain id expected from RPC (default: 31337)
  --rpc-seed-key KEY     Seed key for execute remote
  --uv-cache-dir DIR     UV cache directory (default: /tmp/uv-cache)
  --startup-timeout SEC  Seconds to wait for Anvil RPC readiness (default: 30)
  --anvil-log FILE       File to write Anvil logs to (default: temp file)
  --keep-anvil           Leave the started Anvil process running on exit
  --reuse-existing       Do not start Anvil; reuse the existing RPC endpoint
  -h, --help             Show this help

Examples:
  scripts/run_monad_anvil_remote_tests.sh
  scripts/run_monad_anvil_remote_tests.sh -- tests/osaka/eip7951_p256verify_precompiles
  scripts/run_monad_anvil_remote_tests.sh -- -m blockchain_test tests/monad_eight
EOF
}

repo_root="$(
  cd "$(dirname "${BASH_SOURCE[0]}")/.." >/dev/null 2>&1
  pwd
)"

anvil_bin="anvil"
rpc_endpoint="http://127.0.0.1:8545"
fork="MONAD_EIGHT"
chain_id=31337
rpc_seed_key="0xac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80"
uv_cache_dir="/tmp/uv-cache"
startup_timeout=30
anvil_log=""
keep_anvil=false
reuse_existing=false
selection=("tests")

while [[ $# -gt 0 ]]; do
  case "$1" in
    --anvil-bin)
      anvil_bin="${2:?missing value for --anvil-bin}"
      shift 2
      ;;
    --rpc-endpoint)
      rpc_endpoint="${2:?missing value for --rpc-endpoint}"
      shift 2
      ;;
    --fork)
      fork="${2:?missing value for --fork}"
      shift 2
      ;;
    --chain-id)
      chain_id="${2:?missing value for --chain-id}"
      shift 2
      ;;
    --rpc-seed-key)
      rpc_seed_key="${2:?missing value for --rpc-seed-key}"
      shift 2
      ;;
    --uv-cache-dir)
      uv_cache_dir="${2:?missing value for --uv-cache-dir}"
      shift 2
      ;;
    --startup-timeout)
      startup_timeout="${2:?missing value for --startup-timeout}"
      shift 2
      ;;
    --anvil-log)
      anvil_log="${2:?missing value for --anvil-log}"
      shift 2
      ;;
    --keep-anvil)
      keep_anvil=true
      shift
      ;;
    --reuse-existing)
      reuse_existing=true
      shift
      ;;
    -h|--help)
      usage
      exit 0
      ;;
    --)
      shift
      if [[ $# -gt 0 ]]; then
        selection=("$@")
      fi
      break
      ;;
    *)
      selection=("$@")
      break
      ;;
  esac
done

rpc_chain_id_hex="$(printf '0x%x' "$chain_id")"
anvil_pid=""
anvil_log_temp=false

rpc_ready() {
  local response
  response="$(
    curl \
      --silent \
      --show-error \
      --header "Content-Type: application/json" \
      --data '{"jsonrpc":"2.0","id":1,"method":"eth_chainId","params":[]}' \
      "$rpc_endpoint" 2>/dev/null || true
  )"
  [[ "$response" == *"\"result\":\"$rpc_chain_id_hex\""* ]]
}

cleanup() {
  local status=$?

  if [[ -n "$anvil_pid" && "$keep_anvil" == false ]]; then
    kill "$anvil_pid" >/dev/null 2>&1 || true
    wait "$anvil_pid" >/dev/null 2>&1 || true
  fi

  if [[ "$anvil_log_temp" == true && "$status" -eq 0 && "$keep_anvil" == false ]]; then
    rm -f "$anvil_log"
  fi

  exit "$status"
}

trap cleanup EXIT INT TERM

mkdir -p "$uv_cache_dir"

if [[ -z "$anvil_log" ]]; then
  anvil_log="$(mktemp "${TMPDIR:-/tmp}/monad-anvil.XXXXXX")"
  anvil_log_temp=true
fi

if [[ "$reuse_existing" == true ]]; then
  if ! rpc_ready; then
    echo "RPC endpoint is not ready: $rpc_endpoint" >&2
    exit 1
  fi
else
  if rpc_ready; then
    echo "RPC endpoint is already responding at $rpc_endpoint." >&2
    echo "Stop the existing node or rerun with --reuse-existing." >&2
    exit 1
  fi

  if [[ "$anvil_bin" == */* ]]; then
    if [[ ! -x "$anvil_bin" ]]; then
      echo "Anvil binary is not executable: $anvil_bin" >&2
      exit 1
    fi
  elif ! command -v "$anvil_bin" >/dev/null 2>&1; then
    echo "Anvil binary not found in PATH: $anvil_bin" >&2
    exit 1
  fi

  "$anvil_bin" --monad >"$anvil_log" 2>&1 &
  anvil_pid=$!

  deadline=$((SECONDS + startup_timeout))
  until rpc_ready; do
    if ! kill -0 "$anvil_pid" >/dev/null 2>&1; then
      echo "Anvil exited before RPC became ready. Log: $anvil_log" >&2
      tail -n 40 "$anvil_log" >&2 || true
      exit 1
    fi
    if (( SECONDS >= deadline )); then
      echo "Timed out waiting for Anvil RPC at $rpc_endpoint. Log: $anvil_log" >&2
      tail -n 40 "$anvil_log" >&2 || true
      exit 1
    fi
    sleep 1
  done
fi

echo "Repo root: $repo_root"
echo "RPC endpoint: $rpc_endpoint"
echo "Fork: $fork"
echo "Chain ID: $chain_id"
echo "Anvil log: $anvil_log"
echo "Selection: ${selection[*]}"

cd "$repo_root"

UV_CACHE_DIR="$uv_cache_dir" \
  uv run --no-sync execute remote \
  --fork="$fork" \
  --rpc-endpoint="$rpc_endpoint" \
  --rpc-seed-key="$rpc_seed_key" \
  --chain-id="$chain_id" \
  --no-html \
  "${selection[@]}" \
  -q
