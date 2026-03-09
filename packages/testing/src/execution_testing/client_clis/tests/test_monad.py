"""Tests for the Monad direct fixture consumer."""

import shutil
import subprocess
from pathlib import Path

import pytest

from execution_testing.client_clis import MonadStateFixtureConsumer


def test_monad_consumer_from_binary(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    """Detect the Monad direct consumer from its binary version output."""

    class MockCompletedProcess:
        def __init__(self, stdout: bytes) -> None:
            self.stdout = stdout
            self.stderr = None
            self.returncode = 0

    def mock_which(self: str) -> str:
        del self
        return "monad-statetest"

    def mock_run(args: list, **kwargs: dict) -> MockCompletedProcess:
        del args, kwargs
        return MockCompletedProcess(b"monad-statetest 0.1.0")

    monkeypatch.setattr(shutil, "which", mock_which)
    monkeypatch.setattr(subprocess, "run", mock_run)

    assert isinstance(
        MonadStateFixtureConsumer.from_binary_path(
            binary_path=Path("monad-statetest")
        ),
        MonadStateFixtureConsumer,
    )


def test_monad_consumer_skips_non_monad_fixture(
    tmp_path: Path,
) -> None:
    """Skip state fixtures that are not pure MONAD_EIGHT fixtures."""
    fixture_path = tmp_path / "fixture.json"
    fixture_path.write_text(
        """
{
  "fixture": {
    "post": {
      "Prague": []
    }
  }
}
""".strip()
    )

    consumer = MonadStateFixtureConsumer(binary=Path("monad-statetest"))

    with pytest.raises(pytest.skip.Exception):
        consumer.consume_state_test(
            fixture_path=fixture_path,
            fixture_name="fixture",
        )
