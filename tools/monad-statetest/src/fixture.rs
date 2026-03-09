use anyhow::{bail, Context, Result};
use revm::statetest_types::TestSuite;
use serde_json::{Map, Value};
use std::{fs, path::Path};

const MONAD_FIXTURE_FORK: &str = "MONAD_EIGHT";
const PARSED_BASE_FORK: &str = "Prague";

pub fn load_monad_suite(path: &Path) -> Result<TestSuite> {
    let raw = fs::read_to_string(path)
        .with_context(|| format!("failed to read fixture file {}", path.display()))?;
    let mut root: Map<String, Value> =
        serde_json::from_str(&raw).with_context(|| "failed to decode fixture JSON")?;

    for (fixture_name, fixture_value) in &mut root {
        let fixture = fixture_value
            .as_object_mut()
            .with_context(|| format!("fixture {fixture_name} is not a JSON object"))?;
        let post = fixture
            .get_mut("post")
            .and_then(Value::as_object_mut)
            .with_context(|| format!("fixture {fixture_name} is missing a post section"))?;

        match post.len() {
            1 if post.contains_key(MONAD_FIXTURE_FORK) => {
                let monad_post = post
                    .remove(MONAD_FIXTURE_FORK)
                    .expect("checked MONAD_EIGHT entry must exist");
                post.insert(PARSED_BASE_FORK.to_owned(), monad_post);
            }
            _ => {
                let forks = post.keys().cloned().collect::<Vec<_>>().join(", ");
                bail!(
                    "fixture {fixture_name} is not a pure {MONAD_FIXTURE_FORK} state fixture; found [{forks}]"
                );
            }
        }
    }

    serde_json::from_value(Value::Object(root))
        .with_context(|| format!("failed to decode transformed suite from {}", path.display()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rewrites_monad_fork_name() {
        let fixture = serde_json::json!({
            "monad-fixture": {
                "env": {
                    "currentCoinbase": "0x0000000000000000000000000000000000000000",
                    "currentDifficulty": "0x0",
                    "currentGasLimit": "0x1c9c380",
                    "currentNumber": "0x1",
                    "currentTimestamp": "0x1",
                },
                "pre": {},
                "transaction": {
                    "data": ["0x"],
                    "gasLimit": ["0x5208"],
                    "gasPrice": "0x0",
                    "nonce": "0x0",
                    "secretKey": "0x0000000000000000000000000000000000000000000000000000000000000001",
                    "to": "0x1000000000000000000000000000000000000001",
                    "value": ["0x0"]
                },
                "post": {
                    "MONAD_EIGHT": [{
                        "hash": "0x0000000000000000000000000000000000000000000000000000000000000000",
                        "logs": "0x0000000000000000000000000000000000000000000000000000000000000000",
                        "indexes": {"data": 0, "gas": 0, "value": 0}
                    }]
                }
            }
        });
        let path = tempfile::NamedTempFile::new().unwrap();
        fs::write(path.path(), serde_json::to_vec(&fixture).unwrap()).unwrap();

        let suite = load_monad_suite(path.path()).unwrap();
        let (_, unit) = suite.0.into_iter().next().unwrap();
        assert!(unit
            .post
            .contains_key(&revm::statetest_types::SpecName::Prague));
    }
}
