#!/usr/bin/env bash

set -euo pipefail

usage() {
  cat <<'EOF'
Usage: scripts/run_monad_revm_state_tests.sh [options] [-- <fill selection args...>]

Build `monad-statetest`, fill `MONAD_EIGHT` state fixtures, then consume them
directly with `monad-revm`.

Default selection:
  tests

That means "run all supported tests on monad-revm" currently resolves to:
  1. fill all sources under `tests/`
  2. restrict to `-m state_test`
  3. fill for `--fork MONAD_EIGHT`
  4. consume the resulting fixtures with `monad-statetest`

Options:
  --fixtures-dir DIR   Keep filled fixtures in DIR instead of a temp dir
  --keep-fixtures      Do not delete the temp fixture dir on exit
  --uv-cache-dir DIR   Set UV_CACHE_DIR (default: /tmp/uv-cache)
  --debug              Build and use target/debug/monad-statetest
  --binary PATH        Use an explicit monad-statetest binary path
  --no-build           Skip cargo build
  -h, --help           Show this help

Examples:
  scripts/run_monad_revm_state_tests.sh
  scripts/run_monad_revm_state_tests.sh -- tests/osaka/eip7951_p256verify_precompiles
  scripts/run_monad_revm_state_tests.sh -- tests/prague/eip7702_set_code_tx/test_set_code_txs.py -k test_self_sponsored_set_code
EOF
}

repo_root="$(
  cd "$(dirname "${BASH_SOURCE[0]}")/.." >/dev/null 2>&1
  pwd
)"

fixtures_dir=""
fixtures_dir_provided=false
keep_fixtures=false
uv_cache_dir="/tmp/uv-cache"
profile="release"
build_binary=true
binary=""
fill_selection=("tests")

while [[ $# -gt 0 ]]; do
  case "$1" in
    --fixtures-dir)
      fixtures_dir="${2:?missing value for --fixtures-dir}"
      fixtures_dir_provided=true
      shift 2
      ;;
    --keep-fixtures)
      keep_fixtures=true
      shift
      ;;
    --uv-cache-dir)
      uv_cache_dir="${2:?missing value for --uv-cache-dir}"
      shift 2
      ;;
    --debug)
      profile="debug"
      shift
      ;;
    --binary)
      binary="${2:?missing value for --binary}"
      shift 2
      ;;
    --no-build)
      build_binary=false
      shift
      ;;
    -h|--help)
      usage
      exit 0
      ;;
    --)
      shift
      if [[ $# -gt 0 ]]; then
        fill_selection=("$@")
      fi
      break
      ;;
    *)
      fill_selection=("$@")
      break
      ;;
  esac
done

if [[ -z "$binary" ]]; then
  binary="$repo_root/tools/monad-statetest/target/$profile/monad-statetest"
fi

if [[ -z "$fixtures_dir" ]]; then
  fixtures_dir="$(mktemp -d "${TMPDIR:-/tmp}/monad-revm-fixtures.XXXXXX")"
fi

cleanup() {
  if [[ "$keep_fixtures" == false && "$fixtures_dir_provided" == false ]]; then
    rm -rf "$fixtures_dir"
  fi
}

trap cleanup EXIT

mkdir -p "$uv_cache_dir"
mkdir -p "$fixtures_dir"

cd "$repo_root"

if [[ "$build_binary" == true ]]; then
  cargo build "--$profile" --manifest-path tools/monad-statetest/Cargo.toml
fi

if [[ ! -x "$binary" ]]; then
  echo "monad-statetest binary not found or not executable: $binary" >&2
  exit 1
fi

echo "Repo root: $repo_root"
echo "Monad fork: MONAD_EIGHT"
echo "Fixture dir: $fixtures_dir"
echo "Binary: $binary"
echo "Fill selection: ${fill_selection[*]}"

UV_CACHE_DIR="$uv_cache_dir" \
  uv run --no-sync fill \
  --fork MONAD_EIGHT \
  --clean \
  --no-html \
  --single-fixture-per-file \
  --output "$fixtures_dir" \
  -m state_test \
  -q \
  "${fill_selection[@]}"

UV_CACHE_DIR="$uv_cache_dir" \
  uv run --no-sync consume direct \
  --input "$fixtures_dir" \
  -m state_test \
  --bin "$binary" \
  -q

if [[ "$keep_fixtures" == true || "$fixtures_dir_provided" == true ]]; then
  echo "Fixtures kept at: $fixtures_dir"
fi
