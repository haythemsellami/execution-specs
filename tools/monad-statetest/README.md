# `monad-statetest`

`monad-statetest` is a small direct-consumer binary that runs `MONAD_EIGHT`
state fixtures against `monad-revm` without going through JSON-RPC or Anvil.

This lives inside `execution-specs` as a local integration tool so it can be
developed next to the EEST Python consumer adapter while still exercising the
real execution engine from the sibling `monad-revm` checkout.

## What It Covers

- `StateFixture` / `state_test` fixtures only
- `MONAD_EIGHT` fixtures only
- Direct execution through `monad-revm`
- Validation of:
  - expected exception presence or absence
  - expected call output
  - logs hash
  - post-state root

## What It Does Not Cover Yet

- blockchain fixtures / `blockchain_test`
- mixed-fork fixture files
- exact exception-string mapping to EEST exception enums
- a portable dependency story outside a local sibling checkout of
  `../monad-revm`

## Build

The crate depends on the local Monad REVM checkout at:

`../../../monad-revm/crates/monad-revm`

From the `execution-specs` repo root:

```bash
cargo build --release --manifest-path tools/monad-statetest/Cargo.toml
```

The binary will be available at:

```bash
./tools/monad-statetest/target/release/monad-statetest
```

## Run It Manually

```bash
./tools/monad-statetest/target/release/monad-statetest --json path/to/fixture.json
```

Optional trace output:

```bash
./tools/monad-statetest/target/release/monad-statetest --json --trace path/to/fixture.json
```

## How It Handles Monad Fork Names

EEST fixtures encode the Monad fork as `MONAD_EIGHT`. Upstream `revm`
statetest helpers understand Ethereum fork names such as `Prague`, but not the
Monad-specific name.

This tool rewrites:

- `MONAD_EIGHT` -> `Prague` for fixture deserialization only

Execution still happens with:

- `MonadCfgEnv`
- `MonadSpecId::MonadEight`
- `monad-revm` gas schedule and precompiles

So parsing is normalized to upstream REVM statetest helpers, while execution is
still Monad-native.

## Why State Tests First

State tests are the tightest loop for validating the EVM engine itself:

- no RPC transport
- no node orchestration
- no chain-progress assumptions
- direct post-state and logs validation

That makes them the right first target for keeping `monad-revm` aligned with
execution-spec behavior.

Blockchain fixtures matter too, but they need a second layer:

- block assembly/import semantics
- receipts roots and header validation
- invalid block cases
- multi-block sequencing

That is better treated as a follow-on binary, not forced into the first state
consumer.

## File Map

- `src/fixture.rs`
  - validates that the fixture is a pure `MONAD_EIGHT` state fixture
  - rewrites fork labels for deserialization
- `src/runner.rs`
  - adapts upstream `revm` statetest execution flow to `monad-revm`
- `src/merkle_trie.rs`
  - computes post-state root and logs hash for validation
- `src/main.rs`
  - thin CLI wrapper

## Current Integration Contract

The Python side in `execution-specs` is expected to:

- pass one fixture at a time
- ensure that fixture is a `MONAD_EIGHT` state fixture
- invoke the binary with `--json`
- treat each returned object as:

```json
{
  "name": "fixture_name",
  "pass": true,
  "error": ""
}
```

## Why This Is Useful Even Before Blockchain Support

This gets you a direct compatibility signal on the core Monad EVM semantics in
CI without the noise introduced by:

- RPC funding mechanics
- Anvil integration layers
- live gas-price dependent setup behavior

That makes it a much better long-term guardrail for `monad-revm` correctness
than `execute remote` alone.
