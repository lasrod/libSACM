"""Tests for agent_write_guard.py (LIB-TOOL-003).

Run: python -m unittest discover -s .claude/hooks -p "test_*.py"
"""

from __future__ import annotations

import io
import json
import os
import re
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


if __name__ == "__main__":
    unittest.main()
