#!/usr/bin/env python3
"""Static-only validation for the experimental kit; does not exercise IPC."""

from __future__ import annotations

import json
import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
KIT = ROOT / "tools" / "blender-transport-spike"
EXPECTED_IDS = {
    "PRE-01", "PRE-02", "PIPE-01", "PIPE-02", "PIPE-03",
    "ID-01", "ID-02", "ID-03", "END-01", "END-02",
    "PROTO-01", "PROTO-02", "PROTO-03", "LIFE-01", "LIFE-02",
    "BLEND-01", "BLEND-02", "BLEND-03",
}


def require(condition: bool, message: str) -> None:
    if not condition:
        raise AssertionError(message)


def main() -> int:
    readme_path = KIT / "README.md"
    plan_path = KIT / "TEST_PLAN.md"
    preflight_path = KIT / "Invoke-Preflight.ps1"
    template_path = KIT / "result-template.json"
    for path in (readme_path, plan_path, preflight_path, template_path):
        require(path.is_file(), f"required kit file missing: {path.relative_to(ROOT)}")

    readme = readme_path.read_text(encoding="utf-8")
    plan = plan_path.read_text(encoding="utf-8")
    preflight = preflight_path.read_text(encoding="utf-8")
    template = json.loads(template_path.read_text(encoding="utf-8"))

    plan_ids = re.findall(r"^\|\s*`([A-Z]+-\d{2})`", plan, flags=re.MULTILINE)
    require(len(plan_ids) == len(set(plan_ids)), "duplicate test IDs in TEST_PLAN.md")
    require(set(plan_ids) == EXPECTED_IDS, "test plan IDs do not match the reviewed expected matrix")

    result_runs = template.get("runs")
    require(isinstance(result_runs, list), "result template must contain a runs array")
    result_ids = [entry.get("test_id") for entry in result_runs]
    require(len(result_ids) == len(set(result_ids)), "duplicate result-template test IDs")
    require(set(result_ids) == set(plan_ids), "result-template IDs do not match TEST_PLAN.md")
    require(all(entry.get("status") == "NOT_RUN" for entry in result_runs), "template must not contain a passed test")
    require(template.get("decision") == "NO_GO", "default production decision must remain NO_GO")
    require("preflight/static checks are not transport evidence" in template.get("decision_note", "").lower(), "template must caution against preflight-only evidence")

    for phrase in (
        "PREREQUISITES_DETECTED_NOT_TESTED",
        "BLOCKED_NO_VALIDATION_HARNESS",
        "rustc", "cargo", "Get-CimInstance", "--disable-autoexec",
        "BLENDER_USER_CONFIG", "missing required prerequisites",
    ):
        require(phrase.lower() in preflight.lower(), f"preflight missing required contract: {phrase}")

    forbidden_system_changes = (
        r"\bSet-ExecutionPolicy\b", r"\bInstall-Module\b", r"\bwinget\b",
        r"\bchoco\s+install\b", r"\bNew-NetFirewallRule\b", r"\bSet-Acl\b",
        r"\bnetsh\s+advfirewall\b", r"\bStart-Service\b",
    )
    for pattern in forbidden_system_changes:
        require(not re.search(pattern, preflight, flags=re.IGNORECASE), f"preflight contains prohibited system-changing command: {pattern}")

    dangerous_blender_operations = ("bpy.ops", "save_as_mainfile", "exec(", "eval(", "TcpListener", "HttpListener")
    for marker in dangerous_blender_operations:
        require(marker not in preflight, f"fixed Blender version probe contains prohibited operation: {marker}")

    print(f"PASS static kit consistency: {len(plan_ids)} test IDs match the result template")
    print("PASS default result decision is NO_GO and every case starts NOT_RUN")
    print("PASS preflight contract and non-invasive static checks")
    print("LIMIT: this checker does not parse/execute PowerShell or test Windows pipes, Rust, Blender, peer identity, or a handshake")
    return 0


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except (AssertionError, OSError, json.JSONDecodeError) as error:
        print(f"FAIL static kit validation: {error}", file=sys.stderr)
        raise SystemExit(1)
