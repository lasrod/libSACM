"""Tests for agent_write_guard.py (LIB-TOOL-003).

Run: python -m unittest discover -s .claude/hooks -p "test_*.py"
"""

from __future__ import annotations

import json
import sys
import tempfile
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))

import agent_write_guard as guard  # noqa: E402

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


if __name__ == "__main__":
    unittest.main()
