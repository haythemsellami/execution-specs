use crate::{fixture::load_monad_suite, merkle_trie::compute_roots};
use anyhow::Result;
use monad_revm::{MonadBuilder, MonadCfgEnv, MonadSpecId};
use revm::{
    context::{CfgEnv, Context},
    context_interface::result::{ExecutionResult, HaltReason},
    database,
    inspector::{inspectors::TracerEip3155, InspectCommitEvm},
    primitives::{hardfork::SpecId, Bytes, U256},
    statetest_types::{Test, TestError, TestSuite, TestUnit},
    ExecuteCommitEvm, MainContext,
};
use serde::Serialize;
use std::{io::stderr, path::Path};
use thiserror::Error;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct TestOutcome {
    pub name: String,
    pub pass: bool,
    pub error: String,
}

#[derive(Debug, Error)]
enum RunnerError {
    #[error("unexpected exception: got {got:?}, expected {expected:?}")]
    UnexpectedException {
        expected: Option<String>,
        got: Option<String>,
    },
    #[error("unexpected output: got {got:?}, expected {expected:?}")]
    UnexpectedOutput {
        expected: Option<Bytes>,
        got: Option<Bytes>,
    },
    #[error("logs root mismatch: got {got}, expected {expected}")]
    LogsRootMismatch {
        got: revm::primitives::B256,
        expected: revm::primitives::B256,
    },
    #[error("state root mismatch: got {got}, expected {expected}")]
    StateRootMismatch {
        got: revm::primitives::B256,
        expected: revm::primitives::B256,
    },
    #[error(transparent)]
    StateTest(#[from] TestError),
}

pub fn run_file(path: &Path, trace: bool) -> Result<Vec<TestOutcome>> {
    let suite = load_monad_suite(path)?;
    run_suite(suite, trace)
}

pub fn run_suite(suite: TestSuite, trace: bool) -> Result<Vec<TestOutcome>> {
    let mut outcomes = Vec::new();

    for (fixture_name, unit) in suite.0 {
        for tests in unit.post.values() {
            for test in tests {
                let outcome = run_single(&fixture_name, &unit, test, trace);
                outcomes.push(match outcome {
                    Ok(()) => TestOutcome {
                        name: fixture_name.clone(),
                        pass: true,
                        error: String::new(),
                    },
                    Err(error) => TestOutcome {
                        name: fixture_name.clone(),
                        pass: false,
                        error: error.to_string(),
                    },
                });
            }
        }
    }

    Ok(outcomes)
}

fn run_single(
    fixture_name: &str,
    unit: &TestUnit,
    test: &Test,
    trace: bool,
) -> Result<(), RunnerError> {
    let tx = match test.tx_env(unit) {
        Ok(tx) => tx,
        Err(error) if test.expect_exception.is_some() => {
            let got = Some(error.to_string());
            return validate_exception(test, got);
        }
        Err(error) => return Err(error.into()),
    };

    let mut base_cfg = CfgEnv::new_with_spec(SpecId::PRAGUE);
    base_cfg.chain_id = unit
        .env
        .current_chain_id
        .unwrap_or(U256::ONE)
        .try_into()
        .unwrap_or(1);
    base_cfg.set_max_blobs_per_tx(9);
    let block = unit.block_env(&mut base_cfg);

    let mut monad_cfg =
        MonadCfgEnv::new_with_spec(MonadSpecId::MonadEight).with_chain_id(base_cfg.chain_id);
    monad_cfg.max_blobs_per_tx = base_cfg.max_blobs_per_tx;

    let mut cache_state = unit.state();
    cache_state.set_state_clear_flag(true);
    let mut state = database::State::builder()
        .with_cached_prestate(cache_state)
        .with_bundle_update()
        .build();

    let exec_result: Result<ExecutionResult<HaltReason>, _> = {
        let evm_context = Context::mainnet()
            .with_block(block)
            .with_tx(tx.clone())
            .with_cfg(monad_cfg)
            .with_db(&mut state);

        if trace {
            let mut evm = evm_context
                .build_monad_with_inspector(TracerEip3155::buffered(stderr()).without_summary());
            evm.inspect_tx_commit(tx)
        } else {
            let mut evm = evm_context.build_monad();
            evm.transact_commit(tx)
        }
    };

    if let Err(error) = &exec_result {
        return validate_exception(test, Some(error.to_string()));
    }

    let result = exec_result.expect("checked successful execution result");
    validate_exception(test, None)?;
    validate_output(unit.out.as_ref(), &result)?;
    let roots = compute_roots(&result, &state);

    if roots.logs_root != test.logs {
        return Err(RunnerError::LogsRootMismatch {
            got: roots.logs_root,
            expected: test.logs,
        });
    }

    if roots.state_root != test.hash {
        return Err(RunnerError::StateRootMismatch {
            got: roots.state_root,
            expected: test.hash,
        });
    }

    let _ = fixture_name;
    Ok(())
}

fn validate_exception(test: &Test, got_exception: Option<String>) -> Result<(), RunnerError> {
    match (&test.expect_exception, got_exception) {
        (None, None) => Ok(()),
        (Some(_), Some(_)) => Ok(()),
        (expected, got) => Err(RunnerError::UnexpectedException {
            expected: expected.clone(),
            got,
        }),
    }
}

fn validate_output(
    expected_output: Option<&Bytes>,
    result: &ExecutionResult<HaltReason>,
) -> Result<(), RunnerError> {
    if let Some(expected) = expected_output {
        let got = result.output().cloned();
        if got.as_ref() != Some(expected) {
            return Err(RunnerError::UnexpectedOutput {
                expected: Some(expected.clone()),
                got,
            });
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn write_fixture(body: &str) -> tempfile::NamedTempFile {
        let file = tempfile::NamedTempFile::new().unwrap();
        fs::write(file.path(), body).unwrap();
        file
    }

    #[test]
    fn accepts_monad_success_fixture() {
        let file = write_fixture(
            r#"{
  "monad_success": {
    "env": {
      "currentCoinbase": "0x0000000000000000000000000000000000000000",
      "currentDifficulty": "0x0",
      "currentGasLimit": "0x1c9c380",
      "currentNumber": "0x1",
      "currentTimestamp": "0x1",
      "currentBaseFee": "0x0"
    },
    "pre": {
      "0xa94f5374fce5edbc8e2a8697c15331677e6ebf0b": {
        "balance": "0xde0b6b3a7640000",
        "nonce": "0x0",
        "code": "0x",
        "storage": {}
      },
      "0x1000000000000000000000000000000000000001": {
        "balance": "0x0",
        "nonce": "0x0",
        "code": "0x",
        "storage": {}
      }
    },
    "transaction": {
      "data": ["0x"],
      "gasLimit": ["0x5208"],
      "gasPrice": "0x0",
      "nonce": "0x0",
      "secretKey": "0x45a915e4d060149eb4365960e6a7a45f334393093061116b197e3240065ff2d8",
      "to": "0x1000000000000000000000000000000000000001",
      "value": ["0x1"]
    },
    "post": {
      "MONAD_EIGHT": [{
        "hash": "0xb46a40f7dd5b82fc6739fca95323b231374e856ab41f8782305360153c2dc8ee",
        "logs": "0x1dcc4de8dec75d7aab85b567b6ccd41ad312451b948a7413f0a142fd40d49347",
        "indexes": {"data": 0, "gas": 0, "value": 0}
      }]
    }
  }
}"#,
        );

        let outcomes = run_file(file.path(), false).unwrap();
        assert_eq!(
            outcomes,
            vec![TestOutcome {
                name: "monad_success".to_owned(),
                pass: true,
                error: String::new(),
            }]
        );
    }

    #[test]
    fn accepts_expected_exception_fixture() {
        let file = write_fixture(
            r#"{
  "monad_blob_rejection": {
    "env": {
      "currentCoinbase": "0x0000000000000000000000000000000000000000",
      "currentDifficulty": "0x0",
      "currentGasLimit": "0x1c9c380",
      "currentNumber": "0x1",
      "currentTimestamp": "0x1",
      "currentBaseFee": "0x0"
    },
    "pre": {
      "0xa94f5374fce5edbc8e2a8697c15331677e6ebf0b": {
        "balance": "0xde0b6b3a7640000",
        "nonce": "0x0",
        "code": "0x",
        "storage": {}
      }
    },
    "transaction": {
      "type": 3,
      "data": ["0x"],
      "gasLimit": ["0x5208"],
      "maxFeePerGas": "0x1",
      "maxPriorityFeePerGas": "0x0",
      "nonce": "0x0",
      "secretKey": "0x45a915e4d060149eb4365960e6a7a45f334393093061116b197e3240065ff2d8",
      "to": "0x1000000000000000000000000000000000000001",
      "value": ["0x0"],
      "blobVersionedHashes": [
        "0x0100000000000000000000000000000000000000000000000000000000000000"
      ],
      "maxFeePerBlobGas": "0x1"
    },
    "post": {
      "MONAD_EIGHT": [{
        "expectException": "TransactionException.TYPE_3_TX_PRE_FORK",
        "hash": "0x0000000000000000000000000000000000000000000000000000000000000000",
        "logs": "0x0000000000000000000000000000000000000000000000000000000000000000",
        "indexes": {"data": 0, "gas": 0, "value": 0}
      }]
    }
  }
}"#,
        );

        let outcomes = run_file(file.path(), false).unwrap();
        assert_eq!(
            outcomes,
            vec![TestOutcome {
                name: "monad_blob_rejection".to_owned(),
                pass: true,
                error: String::new(),
            }]
        );
    }
}
