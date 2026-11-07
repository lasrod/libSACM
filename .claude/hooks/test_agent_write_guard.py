"""Tests for agent_write_guard.py (LIB-TOOL-003).

Run: python -m unittest discover -s .claude/hooks -p "test_*.py"
"""

from __future__ import annotations

import io
import json
import os
import re
import shutil
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path
from unittest import mock

sys.path.insert(0, str(Path(__file__).resolve().parent))

import agent_write_guard as guard  # noqa: E402

REPO_ROOT = Path(__file__).resolve().parents[2]
AGENTS_DIR = REPO_ROOT / ".claude" / "agents"
SETTINGS_FILE = REPO_ROOT / ".claude" / "settings.json"

READ_ONLY_ROLES = (
    "sacm-researcher",
    "sacm-architect",
    "sacm-verifier",
    "matrix-steward",
    "sacm-docs-reviewer",
)
WRITING_ROLES = ("sacm-test-engineer", "sacm-implementer")
GUARD_FILES = (
    ".claude/hooks/agent_write_guard.py",
    ".claude/hooks/write-scopes.json",
    ".claude/settings.json",
)

SCOPES = {
    "agents": {
        "sacm-verifier": {"allow": []},
        "sacm-test-engineer": {"allow": ["crates/*/tests/**"]},
        "sacm-implementer": {
            "allow": ["crates/*/src/**", "docs/**"],
            "deny": ["docs/matrix/**"],
        },
    }
}


def call(tool: str, agent: str | None, path: str, root: str) -> dict:
    payload = {"tool_name": tool, "tool_input": {"file_path": str(Path(root) / path)}}
    if agent is not None:
        payload["agent_type"] = agent
    return payload


def denied(decision: dict | None) -> bool:
    return (
        decision is not None
        and decision["hookSpecificOutput"]["permissionDecision"] == "deny"
    )


class WriteGuardTests(unittest.TestCase):
    def setUp(self) -> None:
        self.root = tempfile.mkdtemp()

    def decide(self, tool: str, agent: str | None, path: str, scopes=SCOPES):
        return guard.decide(call(tool, agent, path, self.root), scopes, self.root)

    def test_lib_tool_003_main_session_is_never_restricted(self) -> None:
        self.assertIsNone(self.decide("Write", None, "crates/sacm/src/lib.rs"))

    def test_lib_tool_003_read_only_roles_cannot_write_anywhere(self) -> None:
        self.assertTrue(denied(self.decide("Edit", "sacm-verifier", "docs/x.md")))
        self.assertTrue(denied(self.decide("NotebookEdit", "sacm-verifier", "a.ipynb")))

    def test_lib_tool_003_non_write_tools_pass_through(self) -> None:
        self.assertIsNone(self.decide("Read", "sacm-verifier", "docs/x.md"))
        self.assertIsNone(self.decide("Bash", "sacm-verifier", "docs/x.md"))

    def test_lib_tool_003_test_engineer_confined_to_tests(self) -> None:
        self.assertIsNone(self.decide("Write", "sacm-test-engineer", "crates/sacm/tests/xmi.rs"))
        self.assertIsNone(
            self.decide("Write", "sacm-test-engineer", "crates/sacm/tests/data/a/b.sacm.xmi")
        )
        self.assertTrue(denied(self.decide("Edit", "sacm-test-engineer", "crates/sacm/src/lib.rs")))

    def test_lib_tool_003_implementer_cannot_touch_tests_or_matrix(self) -> None:
        self.assertIsNone(self.decide("Edit", "sacm-implementer", "crates/sacm/src/model/mod.rs"))
        self.assertIsNone(self.decide("Edit", "sacm-implementer", "docs/guide.md"))
        self.assertTrue(denied(self.decide("Edit", "sacm-implementer", "crates/sacm/tests/a.rs")))
        self.assertTrue(
            denied(self.decide("Edit", "sacm-implementer", "docs/matrix/functionality-matrix.toml"))
        )

    def test_lib_tool_003_paths_outside_repository_are_denied(self) -> None:
        outside = str(Path(self.root).parent / "elsewhere.rs")
        payload = {
            "tool_name": "Write",
            "agent_type": "sacm-implementer",
            "tool_input": {"file_path": outside},
        }
        self.assertTrue(denied(guard.decide(payload, SCOPES, self.root)))

    def test_lib_tool_003_dot_dot_cannot_escape_scope(self) -> None:
        sneaky = "crates/sacm/tests/../src/lib.rs"
        self.assertTrue(denied(self.decide("Write", "sacm-test-engineer", sneaky)))

    def test_lib_tool_003_unlisted_agents_use_normal_flow(self) -> None:
        self.assertIsNone(self.decide("Write", "general-purpose", "crates/sacm/src/lib.rs"))

    def test_lib_tool_003_unreadable_scopes_fail_closed_for_subagents(self) -> None:
        self.assertTrue(denied(self.decide("Write", "general-purpose", "x.rs", scopes=None)))
        self.assertIsNone(self.decide("Write", None, "x.rs", scopes=None))

    def test_lib_tool_003_glob_semantics(self) -> None:
        self.assertTrue(guard.matches_any("crates/sacm/src/a/b.rs", ["crates/*/src/**"]))
        self.assertFalse(guard.matches_any("crates/a/b/src/x.rs", ["crates/*/src/**"]))
        self.assertTrue(guard.matches_any("Cargo.toml", ["Cargo.toml"]))

    def test_lib_tool_003_repository_scopes_file_is_valid(self) -> None:
        scopes = json.loads(guard.SCOPES_FILE.read_text(encoding="utf-8"))
        agents_dir = guard.SCOPES_FILE.parent.parent / "agents"
        for name, scope in scopes["agents"].items():
            self.assertTrue(
                (agents_dir / f"{name}.md").is_file(),
                f"write-scopes.json names {name}, which has no .claude/agents/{name}.md",
            )
            self.assertIsInstance(scope.get("allow"), list, name)


class RealScopesTests(unittest.TestCase):
    """The repository's own write-scopes.json gives each role the scope ADR 0006 states."""

    def setUp(self) -> None:
        self.scopes = guard.load_scopes()
        self.assertIsInstance(
            self.scopes, dict, "LIB-TOOL-003: the real write-scopes.json must load"
        )
        self.root = str(REPO_ROOT)

    def decide(self, agent: str, path: str, tool: str = "Write"):
        return guard.decide(call(tool, agent, path, self.root), self.scopes, self.root)

    def assert_denied(self, agent: str, path: str, tool: str = "Write") -> None:
        self.assertTrue(
            denied(self.decide(agent, path, tool)),
            f"LIB-TOOL-003: {agent} must be denied {tool} on {path}",
        )

    def assert_allowed(self, agent: str, path: str, tool: str = "Write") -> None:
        self.assertIsNone(
            self.decide(agent, path, tool),
            f"LIB-TOOL-003: {agent} must be allowed {tool} on {path}",
        )

    def test_lib_tool_003_real_scopes_deny_read_only_roles_everywhere(self) -> None:
        paths = (
            "crates/sacm/src/lib.rs",
            "crates/sacm/tests/x.rs",
            "docs/x.md",
            "docs/matrix/functionality-matrix.toml",
            "xtask/src/lib.rs",
            "xtask/tests/x.rs",
            ".claude/hooks/test_x.py",
            "README.md",
        )
        for role in READ_ONLY_ROLES:
            for path in paths:
                for tool in ("Write", "Edit"):
                    with self.subTest(role=role, path=path, tool=tool):
                        self.assert_denied(role, path, tool)

    def test_lib_tool_003_real_scopes_confine_test_engineer_to_tests(self) -> None:
        role = "sacm-test-engineer"
        for path in ("crates/sacm/tests/x.rs", "xtask/tests/x.rs", ".claude/hooks/test_x.py"):
            with self.subTest(path=path):
                self.assert_allowed(role, path)
        for path in (
            "crates/sacm/src/lib.rs",
            "xtask/src/lib.rs",
            "docs/matrix/functionality-matrix.toml",
        ):
            with self.subTest(path=path):
                self.assert_denied(role, path)

    def test_lib_tool_003_real_scopes_keep_implementer_out_of_tests_matrix_records_and_adrs(
        self,
    ) -> None:
        role = "sacm-implementer"
        for path in ("crates/sacm/src/lib.rs", "xtask/src/lib.rs", "docs/guide.md"):
            with self.subTest(path=path):
                self.assert_allowed(role, path)
        for path in (
            "crates/sacm/tests/x.rs",
            "xtask/tests/x.rs",
            "docs/matrix/functionality-matrix.toml",
            "docs/verification/x.md",
            "docs/adr/x.md",
        ):
            with self.subTest(path=path):
                self.assert_denied(role, path)

    def test_lib_tool_003_real_scopes_keep_writing_roles_off_the_guard(self) -> None:
        for role in WRITING_ROLES:
            for path in GUARD_FILES:
                for tool in ("Write", "Edit"):
                    with self.subTest(role=role, path=path, tool=tool):
                        self.assert_denied(role, path, tool)

    def test_lib_tool_003_real_scopes_list_every_agent_definition(self) -> None:
        defined = {p.stem for p in AGENTS_DIR.glob("*.md")}
        listed = set(self.scopes["agents"])
        self.assertTrue(defined, "LIB-TOOL-003: .claude/agents/ has agent definitions")
        self.assertEqual(
            sorted(defined - listed),
            [],
            "LIB-TOOL-003: agent definitions missing from write-scopes.json",
        )
        self.assertEqual(
            sorted(listed - defined),
            [],
            "LIB-TOOL-003: write-scopes.json names agents with no .claude/agents/<name>.md",
        )

    def test_lib_tool_003_multiedit_is_treated_like_write(self) -> None:
        def multiedit(agent: str, path: str) -> dict:
            payload = call("MultiEdit", agent, path, self.root)
            payload["tool_input"]["edits"] = [{"old_string": "a", "new_string": "b"}]
            return guard.decide(payload, self.scopes, self.root)

        self.assertTrue(
            denied(multiedit("sacm-verifier", "docs/x.md")),
            "LIB-TOOL-003: MultiEdit by a read-only role is denied",
        )
        self.assertTrue(
            denied(multiedit("sacm-test-engineer", "crates/sacm/src/lib.rs")),
            "LIB-TOOL-003: MultiEdit outside a scope is denied",
        )
        self.assertIsNone(
            multiedit("sacm-test-engineer", "crates/sacm/tests/x.rs"),
            "LIB-TOOL-003: MultiEdit inside a scope is allowed",
        )


# Documents that are valid JSON but not a valid scopes structure.
INVALID_SCOPE_TEXTS = (
    "[]",
    '{"agents": []}',
    '{"agents": {"x": null}}',
    '{"agents": {"x": {"allow": "crates/**"}}}',
    '{"agents": {"x": {"allow": [1]}}}',
    '{"agents": {"x": {"allow": [], "deny": "x"}}}',
    '{"agents": {"x": {"allow": [], "deny": [2]}}}',
    '{"agents": {"x": {"allow": null}}}',
    '{"agents": {"x": {}}}',
    '{"agents": null}',
    "{}",
    "null",
    '"agents"',
    "not json",
)

VALID_SCOPES_TEXT = json.dumps(
    {
        "$comment": "top-level comments are allowed",
        "agents": {
            "reader": {"allow": []},
            "writer": {"allow": ["crates/*/src/**"], "deny": ["crates/x/src/**"]},
        },
    }
)


class ParseScopesTests(unittest.TestCase):
    """Structurally invalid configuration fails closed (parse_scopes contract)."""

    def test_lib_tool_003_parse_scopes_rejects_structurally_invalid_documents(self) -> None:
        for text in INVALID_SCOPE_TEXTS:
            with self.subTest(text=text):
                self.assertIsNone(
                    guard.parse_scopes(text),
                    f"LIB-TOOL-003: parse_scopes must return None for {text!r}",
                )

    def test_lib_tool_003_parse_scopes_accepts_a_valid_document(self) -> None:
        self.assertEqual(
            guard.parse_scopes(VALID_SCOPES_TEXT),
            json.loads(VALID_SCOPES_TEXT),
            "LIB-TOOL-003: a valid document parses to its dict",
        )

    def test_lib_tool_003_parse_scopes_accepts_the_real_file(self) -> None:
        text = guard.SCOPES_FILE.read_text(encoding="utf-8")
        parsed = guard.parse_scopes(text)
        self.assertIsInstance(parsed, dict, "LIB-TOOL-003: the real file is valid")
        self.assertEqual(parsed, json.loads(text), "LIB-TOOL-003: parsed unchanged")

    def test_lib_tool_003_invalid_structure_denies_subagent_writes_only(self) -> None:
        root = tempfile.mkdtemp()
        for text in INVALID_SCOPE_TEXTS:
            scopes = guard.parse_scopes(text)
            with self.subTest(text=text):
                self.assertTrue(
                    denied(guard.decide(call("Write", "x", "a.rs", root), scopes, root)),
                    f"LIB-TOOL-003: a subagent write is denied under {text!r}",
                )
                self.assertIsNone(
                    guard.decide(call("Write", None, "a.rs", root), scopes, root),
                    f"LIB-TOOL-003: the main session is not restricted under {text!r}",
                )

    def test_lib_tool_003_load_scopes_rejects_structurally_invalid_file(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            path = Path(tmp) / "write-scopes.json"
            with mock.patch.object(guard, "SCOPES_FILE", path):
                path.write_text('{"agents": []}', encoding="utf-8")
                self.assertIsNone(
                    guard.load_scopes(),
                    "LIB-TOOL-003: load_scopes must reject a structurally invalid file",
                )
                path.write_text('{"agents": {"x": null}}', encoding="utf-8")
                self.assertIsNone(
                    guard.load_scopes(),
                    "LIB-TOOL-003: load_scopes must reject a null agent entry",
                )
                path.write_text(VALID_SCOPES_TEXT, encoding="utf-8")
                self.assertEqual(
                    guard.load_scopes(),
                    json.loads(VALID_SCOPES_TEXT),
                    "LIB-TOOL-003: load_scopes returns a valid file's dict",
                )


class GuardedDecideTests(unittest.TestCase):
    """Any internal error denies subagent writes (fail closed)."""

    def setUp(self) -> None:
        self.root = tempfile.mkdtemp()

    def test_lib_tool_003_internal_error_denies_subagent_writes(self) -> None:
        with mock.patch.object(guard, "decide", side_effect=RuntimeError("boom")):
            for tool in sorted(guard.WRITE_TOOLS):
                for agent in ("sacm-verifier", "sacm-implementer", "general-purpose"):
                    with self.subTest(tool=tool, agent=agent):
                        payload = call(tool, agent, "crates/sacm/src/lib.rs", self.root)
                        self.assertTrue(
                            denied(guard.guarded_decide(payload, SCOPES, self.root)),
                            f"LIB-TOOL-003: {tool} by {agent} is denied when decide() raises",
                        )

    def test_lib_tool_003_internal_error_leaves_main_session_and_reads_alone(self) -> None:
        with mock.patch.object(guard, "decide", side_effect=RuntimeError("boom")):
            self.assertIsNone(
                guard.guarded_decide(
                    call("Write", None, "crates/sacm/src/lib.rs", self.root), SCOPES, self.root
                ),
                "LIB-TOOL-003: a main-session write is not denied when decide() raises",
            )
            self.assertIsNone(
                guard.guarded_decide(
                    call("Read", "sacm-verifier", "docs/x.md", self.root), SCOPES, self.root
                ),
                "LIB-TOOL-003: a subagent Read is not denied when decide() raises",
            )

    def test_lib_tool_003_guarded_decide_agrees_with_decide_when_nothing_fails(self) -> None:
        cases = (
            ("Write", None, "crates/sacm/src/lib.rs"),
            ("Write", "sacm-verifier", "docs/x.md"),
            ("Write", "sacm-test-engineer", "crates/sacm/tests/x.rs"),
            ("Edit", "sacm-test-engineer", "crates/sacm/src/lib.rs"),
            ("Read", "sacm-verifier", "docs/x.md"),
            ("Write", "general-purpose", "crates/sacm/src/lib.rs"),
        )
        for tool, agent, path in cases:
            payload = call(tool, agent, path, self.root)
            with self.subTest(tool=tool, agent=agent, path=path):
                self.assertEqual(
                    guard.guarded_decide(payload, SCOPES, self.root),
                    guard.decide(payload, SCOPES, self.root),
                    "LIB-TOOL-003: guarded_decide is decide() when nothing raises",
                )

    def run_main(self, payload: dict) -> str:
        stdout = io.StringIO()
        with mock.patch.object(sys, "stdin", io.StringIO(json.dumps(payload))), mock.patch.object(
            sys, "stdout", stdout
        ), mock.patch.dict(os.environ, {"CLAUDE_PROJECT_DIR": self.root}):
            self.assertEqual(guard.main(), 0, "LIB-TOOL-003: the hook exits 0")
        return stdout.getvalue()

    def test_lib_tool_003_main_denies_subagent_writes_when_decide_raises(self) -> None:
        payload = call("Write", "sacm-implementer", "crates/sacm/src/lib.rs", self.root)
        with mock.patch.object(guard, "decide", side_effect=RuntimeError("boom")):
            out = self.run_main(payload)
        self.assertTrue(
            out and denied(json.loads(out)),
            f"LIB-TOOL-003: main() must emit a deny when decide() raises, got {out!r}",
        )

    def test_lib_tool_003_main_denies_subagent_writes_when_loading_scopes_raises(self) -> None:
        payload = call("Write", "sacm-implementer", "crates/sacm/src/lib.rs", self.root)
        with mock.patch.object(guard, "load_scopes", side_effect=RuntimeError("boom")):
            out = self.run_main(payload)
        self.assertTrue(
            out and denied(json.loads(out)),
            f"LIB-TOOL-003: main() must emit a deny when loading scopes raises, got {out!r}",
        )


class SettingsTests(unittest.TestCase):
    """.claude/settings.json registers the guard for every write tool, once."""

    def setUp(self) -> None:
        settings = json.loads(SETTINGS_FILE.read_text(encoding="utf-8"))
        self.entries = [
            (entry.get("matcher", ""), hook.get("command", ""))
            for entry in settings.get("hooks", {}).get("PreToolUse", [])
            for hook in entry.get("hooks", [])
            if "agent_write_guard.py" in hook.get("command", "")
        ]
        self.assertTrue(
            self.entries,
            "LIB-TOOL-003: settings.json registers agent_write_guard.py as a PreToolUse hook",
        )

    def test_lib_tool_003_settings_matcher_covers_every_write_tool(self) -> None:
        for tool in sorted(guard.WRITE_TOOLS):
            with self.subTest(tool=tool):
                self.assertTrue(
                    any(re.fullmatch(matcher, tool) for matcher, _ in self.entries),
                    f"LIB-TOOL-003: no guard hook matcher matches {tool}",
                )

    def test_lib_tool_003_settings_hook_runs_the_guard_exactly_once(self) -> None:
        for _, command in self.entries:
            with self.subTest(command=command):
                self.assertNotIn(
                    "||",
                    command,
                    "LIB-TOOL-003: the hook command must not re-run the guard after a failure",
                )
                self.assertEqual(
                    command.count("agent_write_guard.py"),
                    1,
                    "LIB-TOOL-003: the hook command runs the guard once",
                )


# Exception types an internal error can raise. The guard must fail closed on
# every one of them, not only on the RuntimeError the older tests use.
EXCEPTION_TYPES = (AttributeError, TypeError, KeyError, ValueError, OSError)

# Payload `tool_input` values that make decide() raise for real (no mocks), for
# a scoped role whose allow list is not empty.
MALFORMED_TOOL_INPUTS = (
    ("tool_input is a string", "crates/sacm/tests/x.rs"),
    ("tool_input is a list", ["crates/sacm/tests/x.rs"]),
    ("file_path is an int", {"file_path": 7}),
    ("file_path is a list", {"file_path": ["crates/sacm/tests/x.rs"]}),
)


def malformed(tool_input: object, agent: str | None) -> dict:
    payload = {"tool_name": "Write", "tool_input": tool_input}
    if agent is not None:
        payload["agent_type"] = agent
    return payload


class FailClosedTests(unittest.TestCase):
    """Fail closed on any exception type and on malformed payloads (LIB-TOOL-003)."""

    def setUp(self) -> None:
        self.root = tempfile.mkdtemp()

    def guarded(self, payload: dict, scopes: dict | None = SCOPES) -> dict | None:
        try:
            return guard.guarded_decide(payload, scopes, self.root)
        except Exception as error:  # noqa: BLE001 -- an escape is the failure under test
            self.fail(f"LIB-TOOL-003: guarded_decide() let {error!r} escape instead of denying")

    def run_main(self, stdin_text: str, project_dir: str | None = None) -> str:
        """main() on `stdin_text`; its stdout. `project_dir` None unsets CLAUDE_PROJECT_DIR."""
        stdout = io.StringIO()
        with mock.patch.object(sys, "stdin", io.StringIO(stdin_text)), mock.patch.object(
            sys, "stdout", stdout
        ), mock.patch.dict(os.environ):
            if project_dir is None:
                os.environ.pop("CLAUDE_PROJECT_DIR", None)
            else:
                os.environ["CLAUDE_PROJECT_DIR"] = project_dir
            try:
                code = guard.main()
            except Exception as error:  # noqa: BLE001 -- an escape is the failure under test
                self.fail(
                    f"LIB-TOOL-003: main() let {error!r} escape; the real hook would exit "
                    "non-zero, which does not block the write (fail open)"
                )
        self.assertEqual(code, 0, "LIB-TOOL-003: the hook exits 0")
        return stdout.getvalue()

    def assert_main_denies(self, payload: dict, why: str) -> None:
        out = self.run_main(json.dumps(payload), self.root)
        self.assertTrue(
            out and denied(json.loads(out)),
            f"LIB-TOOL-003: main() must print a deny when {why}, got {out!r}",
        )

    def assert_main_silent(self, payload: dict, why: str) -> None:
        out = self.run_main(json.dumps(payload), self.root)
        self.assertEqual(out, "", f"LIB-TOOL-003: main() must print nothing when {why}")

    def test_lib_tool_003_guarded_decide_fails_closed_on_every_exception_type(self) -> None:
        subagent = call("Write", "sacm-implementer", "crates/sacm/src/lib.rs", self.root)
        main_session = call("Write", None, "crates/sacm/src/lib.rs", self.root)
        read = call("Read", "sacm-verifier", "docs/x.md", self.root)
        for exc in EXCEPTION_TYPES:
            with self.subTest(exception=exc.__name__), mock.patch.object(
                guard, "decide", side_effect=exc("boom")
            ):
                self.assertTrue(
                    denied(self.guarded(subagent)),
                    f"LIB-TOOL-003: a subagent write is denied when decide() raises {exc.__name__}",
                )
                self.assertIsNone(
                    self.guarded(main_session),
                    f"LIB-TOOL-003: a main-session write is not denied on {exc.__name__}",
                )
                self.assertIsNone(
                    self.guarded(read),
                    f"LIB-TOOL-003: a subagent Read is not denied on {exc.__name__}",
                )

    def test_lib_tool_003_main_fails_closed_when_decide_raises_any_exception_type(self) -> None:
        subagent = call("Write", "sacm-implementer", "crates/sacm/src/lib.rs", self.root)
        main_session = call("Write", None, "crates/sacm/src/lib.rs", self.root)
        for exc in EXCEPTION_TYPES:
            with self.subTest(exception=exc.__name__), mock.patch.object(
                guard, "decide", side_effect=exc("boom")
            ):
                self.assert_main_denies(subagent, f"decide() raises {exc.__name__}")
                self.assert_main_silent(
                    main_session, f"decide() raises {exc.__name__} on a main-session write"
                )

    def test_lib_tool_003_main_fails_closed_when_load_scopes_raises_any_exception_type(
        self,
    ) -> None:
        subagent = call("Write", "sacm-implementer", "crates/sacm/src/lib.rs", self.root)
        main_session = call("Write", None, "crates/sacm/src/lib.rs", self.root)
        for exc in EXCEPTION_TYPES:
            with self.subTest(exception=exc.__name__), mock.patch.object(
                guard, "load_scopes", side_effect=exc("boom")
            ):
                self.assert_main_denies(subagent, f"load_scopes() raises {exc.__name__}")
                self.assert_main_silent(
                    main_session, f"load_scopes() raises {exc.__name__} on a main-session write"
                )

    def assert_decide_raises(self, payload: dict, scopes: dict | None, what: str) -> None:
        """Precondition: the payload reaches decide() and makes it raise, so the test
        exercises the real fail-closed path. If decide() is changed to deny such input
        directly, this precondition (not the requirement) is what needs updating."""
        with self.assertRaises(
            Exception, msg=f"LIB-TOOL-003: precondition: decide() raises when {what}"
        ):
            guard.decide(payload, scopes, self.root)

    def test_lib_tool_003_malformed_payload_denies_subagent_writes_through_guarded_decide(
        self,
    ) -> None:
        for what, tool_input in MALFORMED_TOOL_INPUTS:
            with self.subTest(what):
                payload = malformed(tool_input, "sacm-test-engineer")
                self.assert_decide_raises(payload, SCOPES, what)
                self.assertTrue(
                    denied(self.guarded(payload)),
                    f"LIB-TOOL-003: a subagent write is denied when {what}",
                )
                self.assertIsNone(
                    self.guarded(malformed(tool_input, None)),
                    f"LIB-TOOL-003: a main-session write is not denied when {what}",
                )

    def test_lib_tool_003_malformed_payload_denies_subagent_writes_through_main(self) -> None:
        real_scopes = guard.load_scopes()
        self.assertIsInstance(real_scopes, dict, "LIB-TOOL-003: the real write-scopes.json loads")
        for what, tool_input in MALFORMED_TOOL_INPUTS:
            with self.subTest(what):
                payload = malformed(tool_input, "sacm-test-engineer")
                self.assert_decide_raises(payload, real_scopes, what)
                self.assert_main_denies(payload, what)
                self.assert_main_silent(
                    malformed(tool_input, None), f"{what} on a main-session write"
                )

    def test_lib_tool_003_empty_agent_type_is_the_main_session(self) -> None:
        for agent in ("", None):
            payload = {
                "tool_name": "Write",
                "agent_type": agent,
                "tool_input": {"file_path": str(Path(self.root) / "crates/sacm/src/lib.rs")},
            }
            for scopes in (SCOPES, None):
                with self.subTest(agent=agent, scopes="SCOPES" if scopes else None):
                    self.assertIsNone(
                        guard.decide(payload, scopes, self.root),
                        f"LIB-TOOL-003: agent_type {agent!r} is the main session",
                    )
                    self.assertIsNone(
                        self.guarded(payload, scopes),
                        f"LIB-TOOL-003: agent_type {agent!r} is the main session (guarded)",
                    )
            with self.subTest(agent=agent, via="main"):
                self.assert_main_silent(payload, f"agent_type is {agent!r}")

    def test_lib_tool_003_main_ignores_stdin_that_is_not_a_json_object(self) -> None:
        for text in ("[]", '"x"', "null", "7", "", "{"):
            for project_dir in (self.root, None):
                with self.subTest(stdin=text, project_dir_set=project_dir is not None):
                    self.assertEqual(
                        self.run_main(text, project_dir),
                        "",
                        f"LIB-TOOL-003: main() prints nothing for stdin {text!r}",
                    )

    @unittest.skipUnless(
        os.name == "nt", "Windows only: paths are matched case-insensitively only when os.name is 'nt'"
    )
    def test_lib_tool_003_windows_paths_match_scopes_case_insensitively(self) -> None:
        self.assertTrue(
            guard.matches_any("CRATES/SACM/TESTS/x.rs", ["crates/*/tests/**"]),
            "LIB-TOOL-003: on Windows a glob matches regardless of case",
        )
        self.assertIsNone(
            guard.decide(
                call("Write", "sacm-test-engineer", "CRATES/SACM/TESTS/x.rs", self.root),
                SCOPES,
                self.root,
            ),
            "LIB-TOOL-003: on Windows CRATES/SACM/TESTS/x.rs is inside the test engineer's scope",
        )
        self.assertTrue(
            denied(
                guard.decide(
                    call("Write", "sacm-test-engineer", "CRATES/SACM/SRC/lib.rs", self.root),
                    SCOPES,
                    self.root,
                )
            ),
            "LIB-TOOL-003: on Windows CRATES/SACM/SRC/lib.rs is still outside the scope",
        )
        self.assertTrue(
            denied(
                guard.decide(
                    call("Write", "sacm-implementer", "DOCS/MATRIX/m.toml", self.root),
                    SCOPES,
                    self.root,
                )
            ),
            "LIB-TOOL-003: on Windows a deny glob also matches regardless of case",
        )


def hook_commands() -> list[str]:
    settings = json.loads(SETTINGS_FILE.read_text(encoding="utf-8"))
    return [
        hook.get("command", "")
        for entry in settings.get("hooks", {}).get("PreToolUse", [])
        for hook in entry.get("hooks", [])
        if "agent_write_guard.py" in hook.get("command", "")
    ]


def find_bash() -> tuple[str | None, str]:
    """A usable bash, or None and the reason to skip."""
    bash = shutil.which("bash")
    if bash is None:
        return None, "bash is not on PATH, so the hook command from settings.json cannot be run"
    system32 = Path(os.environ.get("SystemRoot", r"C:\Windows")) / "System32"
    if os.name == "nt" and os.path.normcase(str(Path(bash).parent)) == os.path.normcase(
        str(system32)
    ):
        return None, (
            f"bash on PATH is the WSL launcher ({bash}); it does not pass CLAUDE_PROJECT_DIR "
            "through, so it cannot run the hook command as Claude Code does"
        )
    return bash, ""


class RealHookCommandTests(unittest.TestCase):
    """The exact command in .claude/settings.json, run with bash -c (LIB-TOOL-003)."""

    def setUp(self) -> None:
        bash, reason = find_bash()
        if bash is None:
            self.skipTest(reason)
        self.bash = bash
        commands = hook_commands()
        self.assertEqual(
            len(commands), 1, "LIB-TOOL-003: settings.json has one guard hook command"
        )
        self.command = commands[0]

    def run_hook(self, payload: dict) -> subprocess.CompletedProcess[str]:
        env = dict(os.environ, CLAUDE_PROJECT_DIR=str(REPO_ROOT))
        return subprocess.run(
            [self.bash, "-c", self.command],
            input=json.dumps(payload),
            capture_output=True,
            text=True,
            env=env,
            cwd=str(REPO_ROOT),
            timeout=60,
            check=False,
        )

    def payload(self, agent: str | None) -> dict:
        payload = {
            "session_id": "lib-tool-003-test",
            "hook_event_name": "PreToolUse",
            "cwd": str(REPO_ROOT),
            "tool_name": "Write",
            "tool_input": {"file_path": str(REPO_ROOT / "docs" / "x.md"), "content": "x"},
        }
        if agent is not None:
            payload["agent_type"] = agent
        return payload

    def test_lib_tool_003_real_hook_command_denies_a_read_only_role_write(self) -> None:
        proc = self.run_hook(self.payload("sacm-verifier"))
        self.assertEqual(
            proc.returncode, 0, f"LIB-TOOL-003: the hook command exits 0; stderr: {proc.stderr!r}"
        )
        try:
            decision = json.loads(proc.stdout)
        except ValueError:
            self.fail(
                f"LIB-TOOL-003: the hook command must print a JSON decision, got "
                f"{proc.stdout!r}; stderr: {proc.stderr!r}"
            )
        self.assertTrue(
            denied(decision),
            f"LIB-TOOL-003: the hook command denies a read-only role's Write, got {decision!r}",
        )

    def test_lib_tool_003_real_hook_command_is_silent_for_the_main_session(self) -> None:
        proc = self.run_hook(self.payload(None))
        self.assertEqual(
            proc.returncode, 0, f"LIB-TOOL-003: the hook command exits 0; stderr: {proc.stderr!r}"
        )
        self.assertEqual(
            proc.stdout, "", "LIB-TOOL-003: the hook command prints nothing for the main session"
        )


if __name__ == "__main__":
    unittest.main()
