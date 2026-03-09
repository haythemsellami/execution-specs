"""Monad direct fixture consumer backed by monad-statetest."""

import json
import re
import tempfile
from pathlib import Path
from typing import Any, Dict, List, Optional

import pytest

from execution_testing.fixtures import FixtureFormat, StateFixture

from ..fixture_consumer_tool import FixtureConsumerTool
from .geth import GethEvm


class MonadStateFixtureConsumer(
    GethEvm,
    FixtureConsumerTool,
    fixture_formats=[StateFixture],
):
    """Consume `MONAD_EIGHT` state fixtures through `monad-statetest`."""

    default_binary = Path("monad-statetest")
    detect_binary_pattern = re.compile(r"^monad-statetest\b")
    version_flag = "--version"

    def _load_single_monad_fixture(
        self,
        fixture_path: Path,
        fixture_name: str,
    ) -> Dict[str, Any]:
        with open(fixture_path, "r") as f:
            fixture_file = json.load(f)

        if fixture_name not in fixture_file:
            raise Exception(
                f"Fixture {fixture_name} missing from {fixture_path}"
            )

        fixture = fixture_file[fixture_name]
        post = fixture.get("post", {})
        if list(post.keys()) != ["MONAD_EIGHT"]:
            pytest.skip(
                "Monad direct consumer currently supports only pure MONAD_EIGHT state fixtures."
            )

        return {fixture_name: fixture}

    def consume_state_test(
        self,
        fixture_path: Path,
        fixture_name: Optional[str] = None,
        debug_output_path: Optional[Path] = None,
    ) -> None:
        assert fixture_name is not None, (
            "Monad direct consumer expects a single fixture name."
        )

        fixture = self._load_single_monad_fixture(fixture_path, fixture_name)
        command = [str(self.binary)]
        if debug_output_path:
            command.append("--trace")
        command.extend(["--json"])

        with tempfile.TemporaryDirectory() as tmpdir:
            temp_fixture_path = Path(tmpdir) / "fixture.json"
            with open(temp_fixture_path, "w") as f:
                json.dump(fixture, f, indent=2)

            command.append(str(temp_fixture_path))
            result = self._run_command(command)

            if debug_output_path:
                self._consume_debug_dump(
                    command, result, temp_fixture_path, debug_output_path
                )

        if result.returncode != 0:
            raise Exception(
                f"Unexpected exit code:\n{' '.join(command)}\n\nError:\n{result.stderr}"
            )

        result_json = json.loads(result.stdout)
        if not isinstance(result_json, list):
            raise Exception(
                f"Unexpected result from monad-statetest: {result_json}"
            )

        test_result = [
            candidate
            for candidate in result_json
            if candidate["name"] == fixture_name
        ]
        assert len(test_result) < 2, (
            f"Multiple test results for {fixture_name}"
        )
        assert len(test_result) == 1, (
            f"Test result for {fixture_name} missing"
        )
        assert test_result[0]["pass"], (
            f"State test failed: {test_result[0]['error']}"
        )

    def consume_fixture(
        self,
        fixture_format: FixtureFormat,
        fixture_path: Path,
        fixture_name: Optional[str] = None,
        debug_output_path: Optional[Path] = None,
    ) -> None:
        if fixture_format != StateFixture:
            pytest.skip(
                "Monad direct consumer currently supports only StateFixture."
            )

        self.consume_state_test(
            fixture_path=fixture_path,
            fixture_name=fixture_name,
            debug_output_path=debug_output_path,
        )
