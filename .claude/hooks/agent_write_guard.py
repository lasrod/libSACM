#!/usr/bin/env python3
"""Claude Code PreToolUse hook: confine project subagents to their write scopes.

Why this exists: a subagent's `tools:` frontmatter is not a reliable sandbox --
background subagents can be handed Write/Edit regardless of it. The roles in
this repository only mean something if they hold: a verifier that can edit what
it judges is not independent, and a test engineer who can edit production code
(or an implementer who can edit the tests) breaks test-first. So write tools
called by a scoped subagent are checked against `write-scopes.json`.

Behaviour:
- Main session (no `agent_type` in the hook input): not touched.
- Subagent not listed in write-scopes.json: not touched (normal permission flow).
- Listed subagent writing outside its scope, or outside the repository: denied.
- write-scopes.json unreadable: every subagent write is denied (fail closed);
  the agents that must not write are the ones whose value is that they cannot.

Limits, stated so nobody reads more into it: Bash is not covered (a shell
command that writes cannot be recognised reliably from its text), and nothing is
enforced when project hooks are disabled or Python is unavailable. The agent
definitions prohibit both routes in prose.

Stdlib only. Input: hook JSON on stdin. Output: a deny decision on stdout, or
nothing to defer to the normal flow.
"""

from __future__ import annotations

import json
import os
import re
import sys
from pathlib import Path, PurePosixPath

WRITE_TOOLS = frozenset({"Write", "Edit", "MultiEdit", "NotebookEdit"})
SCOPES_FILE = Path(__file__).resolve().parent / "write-scopes.json"


def glob_to_regex(pattern: str) -> re.Pattern[str]:
    """Translate a repository glob: '**' spans directories, '*' and '?' do not."""
    out = []
    i = 0
    while i < len(pattern):
        if pattern.startswith("**/", i):
            out.append("(?:.*/)?")
            i += 3
        elif pattern.startswith("**", i):
            out.append(".*")
            i += 2
        elif pattern[i] == "*":
            out.append("[^/]*")
            i += 1
        elif pattern[i] == "?":
            out.append("[^/]")
            i += 1
        else:
            out.append(re.escape(pattern[i]))
            i += 1
    flags = re.IGNORECASE if os.name == "nt" else 0
    return re.compile("^" + "".join(out) + "$", flags)


def matches_any(rel: str, patterns: list[str]) -> bool:
    return any(glob_to_regex(p).match(rel) for p in patterns)


def relative_to_project(file_path: str, project_dir: str) -> str | None:
    """Repository-relative POSIX path, or None when the path is outside it."""
    if not file_path:
        return None
    root = Path(project_dir).resolve()
    target = Path(file_path)
    if not target.is_absolute():
        target = root / target
    try:
        rel = target.resolve().relative_to(root)
    except ValueError:
        return None
    return PurePosixPath(*rel.parts).as_posix()


def deny(reason: str) -> dict:
    return {
        "hookSpecificOutput": {
            "hookEventName": "PreToolUse",
            "permissionDecision": "deny",
            "permissionDecisionReason": reason,
        }
    }


def decide(payload: dict, scopes: dict | None, project_dir: str) -> dict | None:
    """The decision for one tool call, or None to leave it to the normal flow."""
    tool = payload.get("tool_name")
    agent = payload.get("agent_type")
    if tool not in WRITE_TOOLS or not isinstance(agent, str) or not agent:
        return None
    if scopes is None:
        return deny(
            f"{agent}: write refused because .claude/hooks/write-scopes.json could not be "
            "read; subagent write scopes cannot be checked."
        )
    scope = scopes.get("agents", {}).get(agent)
    if scope is None:
        return None
    allow = scope.get("allow", [])
    if not allow:
        return deny(
            f"{agent} is a read-only role in this repository. Report the change you "
            "want made in your result instead of making it."
        )
    tool_input = payload.get("tool_input") or {}
    file_path = tool_input.get("file_path") or tool_input.get("notebook_path") or ""
    rel = relative_to_project(file_path, project_dir)
    if rel is None:
        return deny(f"{agent} may only write inside the repository (got {file_path!r}).")
    if matches_any(rel, scope.get("deny", [])) or not matches_any(rel, allow):
        return deny(
            f"{agent} may not write {rel}. Its write scope is {allow}"
            + (f" excluding {scope['deny']}" if scope.get("deny") else "")
            + ". Hand this change back to the lead with the reason it is needed."
        )
    return None


def load_scopes() -> dict | None:
    try:
        return json.loads(SCOPES_FILE.read_text(encoding="utf-8"))
    except (OSError, ValueError):
        return None


def main() -> int:
    try:
        payload = json.load(sys.stdin)
    except ValueError:
        return 0
    project_dir = os.environ.get("CLAUDE_PROJECT_DIR") or payload.get("cwd") or os.getcwd()
    decision = decide(payload, load_scopes(), project_dir)
    if decision is not None:
        json.dump(decision, sys.stdout)
    return 0


if __name__ == "__main__":
    sys.exit(main())
