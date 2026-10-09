#!/usr/bin/env python3
"""Exercise validation dispatch and failure handling without compiling Rust."""
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile
import unittest

VALIDATOR = Path(__file__).with_name("validate.sh")
BASH = shutil.which("bash")
FAKE_COMMAND = r'''
import json, os, sys
from pathlib import Path
name, args = Path(sys.argv[0]).name, sys.argv[1:]
with Path(os.environ["VALIDATION_TRACE"]).open("a") as trace:
    trace.write(json.dumps([name, *args]) + "\n")
if name == "git" and args == ["rev-parse", "--show-toplevel"]:
    print(os.getcwd())
if name == "cargo" and args == ["nextest", "show-config", "version"]:
    sys.exit(int(os.environ.get("VALIDATION_MISSING_NEXTEST", "0")))
if name == "cargo" and args[:2] == ["nextest", "run"]:
    sys.exit(int(os.environ.get("VALIDATION_NEXTEST_EXIT", "0")))
if name == "cargo" and args[:1] == ["test"]:
    sys.exit(int(os.environ.get("VALIDATION_DOCTEST_EXIT", "0")))
'''


class ValidationContracts(unittest.TestCase):
    def setUp(self):
        self.scratch = tempfile.TemporaryDirectory()
        self.addCleanup(self.scratch.cleanup)
        self.root = Path(self.scratch.name)
        self.bin = self.root / "bin"
        self.bin.mkdir()
        for name in ("cargo", "rustup", "git", "python3"):
            path = self.bin / name
            path.write_text(f"#!{sys.executable}\n" + FAKE_COMMAND)
            path.chmod(0o755)
        (self.bin / "rm").symlink_to(shutil.which("rm"))
        self.trace = self.root / "trace.jsonl"
        self.env = os.environ.copy()
        for key in ("FAST_TEST_ARGS", "VALIDATION_MISSING_NEXTEST",
                    "VALIDATION_NEXTEST_EXIT", "VALIDATION_DOCTEST_EXIT"):
            self.env.pop(key, None)
        self.env.update(PATH=str(self.bin), VALIDATION_TRACE=str(self.trace))

    def validate(self, *args, **environment):
        return subprocess.run(
            [BASH, str(VALIDATOR), *args], cwd=self.root,
            env={**self.env, **environment}, text=True, capture_output=True,
        )

    def commands(self):
        return [json.loads(line) for line in self.trace.read_text().splitlines()]

    def test_standard_preserves_checks_and_separates_doctests(self):
        result = self.validate()
        self.assertEqual(result.returncode, 0, result.stderr)
        commands = self.commands()
        for command in (
            ["python3", "scripts/test-validation.py"],
            ["cargo", "fmt", "--all", "--", "--check"],
            ["cargo", "check", "--locked", "--workspace", "--all-targets"],
            ["rustup", "run", "1.85.0", "cargo", "check", "--locked",
             "--workspace", "--all-targets"],
            ["cargo", "nextest", "run", "--locked", "--workspace",
             "--all-targets", "--profile", "ci"],
            ["cargo", "test", "--locked", "--workspace", "--doc"],
            ["python3", "scripts/test-comparison-adjudicator.py"],
            ["git", "diff", "--check", "HEAD"],
        ):
            self.assertIn(command, commands)
        self.assertEqual(sum(c[:3] == ["cargo", "nextest", "run"] for c in commands), 1)
        self.assertEqual([c for c in commands if c[:2] == ["cargo", "test"]],
                         [["cargo", "test", "--locked", "--workspace", "--doc"]])

    def test_ci_entry_point_runs_same_suite_without_repeating_checks(self):
        result = self.validate("tests")
        self.assertEqual(result.returncode, 0, result.stderr)
        commands = self.commands()
        self.assertIn(["cargo", "nextest", "run", "--locked", "--workspace",
                       "--all-targets", "--profile", "ci"], commands)
        self.assertIn(["cargo", "test", "--locked", "--workspace", "--doc"], commands)
        self.assertFalse(any(c[:2] in (["cargo", "fmt"], ["cargo", "check"])
                             for c in commands))

    def test_missing_nextest_fails_without_cargo_fallback(self):
        report = self.root / ".lantern/nextest/ci/junit.xml"
        report.parent.mkdir(parents=True)
        report.write_text("previous run")
        result = self.validate(VALIDATION_MISSING_NEXTEST="1")
        self.assertEqual(result.returncode, 1)
        self.assertIn("Nextest is required", result.stderr)
        self.assertFalse(report.exists())
        self.assertEqual(self.commands()[1:], [["cargo", "nextest", "show-config", "version"]])

    def test_nextest_failure_propagates_and_clears_stale_report(self):
        report = self.root / ".lantern/nextest/ci/junit.xml"
        report.parent.mkdir(parents=True)
        report.write_text("previous run")
        result = self.validate("tests", VALIDATION_NEXTEST_EXIT="7")
        self.assertEqual(result.returncode, 7)
        self.assertFalse(report.exists())
        self.assertFalse(any(c[:2] == ["cargo", "test"] for c in self.commands()))

    def test_doctest_failure_propagates(self):
        result = self.validate("tests", VALIDATION_DOCTEST_EXIT="11")
        self.assertEqual(result.returncode, 11)

    def test_focused_positional_arguments_preserve_spaces_and_globs(self):
        args = ["-p", "lantern-core", "-E", "test(cdp) and not test(*slow*)"]
        result = self.validate("fast", *args)
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertIn(["cargo", "nextest", "run", "--locked", *args], self.commands())
        self.assertFalse(any(c[:2] == ["cargo", "test"] for c in self.commands()))

    def test_legacy_focused_tokens_do_not_expand_globs(self):
        (self.root / "test-matching-file").touch()
        result = self.validate("fast", FAST_TEST_ARGS="-p lantern-core test*")
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertIn(["cargo", "nextest", "run", "--locked", "-p", "lantern-core",
                       "test*"], self.commands())

    def test_focused_zero_test_failure_is_not_accepted(self):
        result = self.validate("fast", "missing_test", VALIDATION_NEXTEST_EXIT="4")
        self.assertEqual(result.returncode, 4)
        self.assertNotIn(["git", "diff", "--check", "HEAD"], self.commands())

    def test_fast_without_tests_does_not_require_nextest(self):
        result = self.validate("fast", VALIDATION_MISSING_NEXTEST="1")
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertFalse(any(c[:2] == ["cargo", "nextest"] for c in self.commands()))

    def test_conflicting_focused_inputs_are_rejected(self):
        result = self.validate("fast", "one", FAST_TEST_ARGS="two")
        self.assertEqual(result.returncode, 2)
        self.assertIn("not both", result.stderr)

    def test_multiline_legacy_input_is_not_silently_truncated(self):
        result = self.validate("fast", FAST_TEST_ARGS="one\ntwo")
        self.assertEqual(result.returncode, 2)
        self.assertIn("single line", result.stderr)


if __name__ == "__main__":
    unittest.main()
