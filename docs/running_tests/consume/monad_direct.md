# Running `consume direct` Against `monad-revm`

This document describes the first direct-consumer path for Monad state
fixtures in this repository.

## Goal

The goal of this integration is to validate Monad EVM behavior against EEST
fixtures without relying on:

- `execute remote`
- JSON-RPC
- Anvil
- a live node lifecycle

That keeps the feedback loop focused on the execution engine itself.

## What Was Added

Two pieces were added:

1. A Rust binary at `tools/monad-statetest/` that executes `MONAD_EIGHT`
   state fixtures directly with `monad-revm`.
2. A Python direct-consumer adapter that lets `execution-specs` call that
   binary through the existing `consume direct` flow.

## Execution Client Under Test

The execution client used by this `consume direct` path is
[`monad-revm`](https://github.com/category-labs/monad-revm).

That is the thing being validated here.

This branch does not use:

- `alloy-monad-evm` as the direct execution target
- Foundry / Anvil as the direct execution target

Those projects are important integration layers around Monad execution, but the
direct consumer in this branch is intentionally aimed at the execution engine
itself: `monad-revm`.

## Why The Binary Lives Here

The binary uses the real `monad-revm` engine, but it currently lives inside
`execution-specs` as a local integration crate because this repo is where the
direct-consumer harness already exists.

The crate depends on the sibling checkout at:

```text
../../../monad-revm/crates/monad-revm
```

That makes this branch immediately usable on your machine, while keeping the
scope of changes local to the harness work.

## Current Scope

Supported:

- `StateFixture`
- pure `MONAD_EIGHT` fixtures
- direct post-state validation
- direct logs-hash validation
- direct return-data validation
- expected-exception presence/absence validation

Not supported yet:

- `BlockchainFixture`
- mixed-fork fixture files
- exact mapping from Monad execution errors to every EEST exception enum
- portable builds in environments that do not also have a sibling
  `monad-revm` checkout

## How It Works

### 1. EEST Selects A Fixture

`consume direct` already collects fixture files and individual test cases.
When the selected consumer is `MonadStateFixtureConsumer`, it receives:

- the fixture file path
- the fixture name

### 2. The Python Adapter Narrows Scope

The adapter:

- loads only the requested fixture
- verifies that it is a pure `MONAD_EIGHT` state fixture
- writes a temporary single-fixture JSON file
- invokes `monad-statetest --json <temp-file>`

This is deliberate. It avoids file-level caching assumptions and avoids
accidentally invoking the Monad consumer on non-Monad fixtures grouped into the
same JSON file.

### 3. The Rust Tool Rewrites Fork Labels For Parsing

Upstream `revm` statetest helpers understand Ethereum fork names such as
`Prague`, not the EEST-specific `MONAD_EIGHT` string.

So the tool rewrites:

- `MONAD_EIGHT` -> `Prague`

This rewrite is only for deserialization. Actual execution still uses:

- `MonadCfgEnv`
- `MonadSpecId::MonadEight`
- `monad-revm`

So parsing stays compatible with upstream REVM helpers, while execution stays
Monad-native.

### 4. The Tool Executes With `monad-revm`

The runner is based on the same shape as upstream `revme statetest`:

- decode the state fixture into REVM statetest types
- build pre-state from `pre`
- build a block environment from `env`
- build a transaction from fixture transaction parts
- execute with `monad-revm`
- compute the resulting state root and logs root
- compare against the fixture expectations

## Why State Tests Were Chosen First

State tests are the shortest path to useful signal on the engine:

- they validate the EVM transition directly
- they are less noisy than RPC-based execution
- they are much cheaper to run than node-backed tests
- they map closely to the kind of "is the engine still spec-compatible?"
  question you asked

Blockchain fixtures are still valuable, but they require more than just a
transaction executor. They need:

- block import semantics
- multi-block sequencing
- invalid block handling
- receipts-root and header validation
- block-level environment transitions

That is a second phase, not something worth forcing into the first binary.

This is not primarily about consensus work.

`BlockchainFixture` would still require block-processing and block-import style
logic around the execution engine, even if no consensus client is involved.
`monad-revm` is the right engine to test, but this branch is using it as an
execution engine for `StateFixture` first, not as a full block-test harness.

## Build Instructions

From the `execution-specs` repo root:

```bash
cargo build --release --manifest-path tools/monad-statetest/Cargo.toml
```

The binary ends up at:

```bash
./tools/monad-statetest/target/release/monad-statetest
```

## Manual Smoke Run

If you already have a MONAD_EIGHT state fixture JSON:

```bash
./tools/monad-statetest/target/release/monad-statetest --json path/to/fixture.json
```

If you want an EIP-3155 trace on stderr:

```bash
./tools/monad-statetest/target/release/monad-statetest --json --trace path/to/fixture.json
```

## `consume direct` Usage

Once the binary is built:

```bash
UV_CACHE_DIR=/tmp/uv-cache uv run --no-sync consume direct \
  --input <fixtures-dir-or-index> \
  -m state_test \
  --bin ./tools/monad-statetest/target/release/monad-statetest
```

Important notes:

- Use `-m state_test` for now.
- Non-Monad state fixtures are skipped by the Python adapter.
- The current focus is `StateFixture` only.
- `BlockchainFixture` is not consumed by this binary yet.

## Validation Semantics

For successful executions, the tool validates:

- `stateRoot`
- `logs`
- optional return `out`

For expected-failure fixtures, the current rule is intentionally simple:

- if the fixture expects an exception and execution returns an error, the test
  passes
- if the fixture expects no exception and execution errors, the test fails
- if the fixture expects an exception and execution succeeds, the test fails

This means the first version checks exception presence, not exact error-string
equivalence. That is good enough to establish a direct harness around
`monad-revm`, but it is still a gap to close later.

## Why This Is Better Than `execute remote` For Core Compatibility

`execute remote` is still useful, but it validates the stack through:

- RPC transport
- account funding transactions
- deployment side effects
- live gas pricing behavior

That is useful for end-to-end behavior, but it can blur whether a failure comes
from the engine or from the surrounding harness.

The direct consumer is narrower and therefore stronger for engine regression
detection.

## Known Gaps And Next Steps

The most important next steps are:

1. Add a `monad-blockchaintest` binary for `BlockchainFixture`.
2. Add exact exception mapping instead of exception-presence validation only.
3. Move the binary into the `monad-revm` repo once the interface settles.
4. Make the dependency story less local-machine-specific.

## Where To Read The Code

- `tools/monad-statetest/src/fixture.rs`
- `tools/monad-statetest/src/runner.rs`
- `tools/monad-statetest/src/merkle_trie.rs`
- `packages/testing/src/execution_testing/client_clis/clis/monad.py`

## Companion README

There is also a tool-local README with a tighter focus on the binary itself:

- `tools/monad-statetest/README.md`
