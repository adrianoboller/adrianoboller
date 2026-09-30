#!/usr/bin/env python3
from __future__ import annotations
import base64
import hashlib
import json
import shutil
import subprocess
import sys
import tomllib
import uuid
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
PROCESS_PROTOCOL = "phxclaw-process-v1"


def fail(msg: str) -> None:
    print(f"FAIL: {msg}")
    raise SystemExit(1)


def is_v7(value: str) -> bool:
    try:
        return uuid.UUID(value).version == 7
    except ValueError:
        return False


def signing_message(manifest: dict) -> bytes:
    return (
        "PHXCLAW-PLUGIN-V1\n"
        f"uuid={manifest['uuid']}\n"
        f"name={manifest['name']}\n"
        f"version={manifest['version']}\n"
        f"sha256={manifest['integrity']['digest']}\n"
    ).encode("utf-8")


def verify_plugin_integrity(parsed: dict[Path, object]) -> None:
    trust = parsed[ROOT / "config/trust/plugin-signers.json"]
    signers = {item["id"]: item for item in trust["signers"] if item["status"] == "active"}

    try:
        from cryptography.hazmat.primitives.asymmetric.ed25519 import Ed25519PublicKey
    except Exception as exc:
        print(f"WARN: cryptography unavailable; Ed25519 verification skipped: {exc}")
        Ed25519PublicKey = None

    verified = 0
    for path in sorted((ROOT / "plugins").rglob("*.plugin.json")):
        manifest = parsed[path]
        integrity = manifest["integrity"]
        artifact = (ROOT / integrity["artifact"]).resolve()
        root = ROOT.resolve()
        if artifact != root and root not in artifact.parents:
            fail(f"artifact escapes package root: {path}")
        if not artifact.is_file():
            fail(f"artifact missing for {path}: {artifact}")
        digest = hashlib.sha256(artifact.read_bytes()).hexdigest()
        if digest.lower() != integrity["digest"].lower():
            fail(f"sha256 mismatch for {path}: {digest} != {integrity['digest']}")

        signer = signers.get(integrity["signer"])
        if signer is None:
            fail(f"untrusted signer for {path}: {integrity['signer']}")
        if signer["algorithm"] != "ed25519":
            fail(f"unsupported signer algorithm for {path}")
        if not any(manifest["name"].startswith(prefix) for prefix in signer["allowed_name_prefixes"]):
            fail(f"signer not authorized for plugin name: {manifest['name']}")

        if Ed25519PublicKey is not None:
            try:
                public_key = base64.b64decode(signer["public_key_base64"], validate=True)
                signature = base64.b64decode(integrity["signature"], validate=True)
                Ed25519PublicKey.from_public_bytes(public_key).verify(signature, signing_message(manifest))
            except Exception as exc:
                fail(f"Ed25519 verification failed for {path}: {exc}")
        verified += 1
    print(f"OK plugin integrity: {verified}")


def verify_process_contracts(parsed: dict[Path, object]) -> None:
    message_uuid = "0199a5f1-00f0-70f0-8abc-0000000000f0"
    envelope = {
        "protocol": PROCESS_PROTOCOL,
        "message_uuid": message_uuid,
        "correlation_uuid": None,
        "kind": "health",
        "sent_at": "2026-09-27T19:45:00Z",
        "payload": {"source": "verify_bootstrap"},
    }
    checked = 0
    for path in sorted((ROOT / "plugins").rglob("*.plugin.json")):
        manifest = parsed[path]
        if manifest["entrypoint"]["type"] != "process":
            continue
        artifact = (ROOT / manifest["entrypoint"]["value"]).resolve()
        if not artifact.is_file():
            fail(f"process entrypoint missing: {artifact}")
        timeout_s = max(1.0, manifest["sandbox"]["timeout_ms"] / 1000.0)

        health = subprocess.run(
            [str(artifact), "--health"],
            cwd=ROOT,
            capture_output=True,
            text=True,
            timeout=timeout_s,
            check=False,
        )
        if health.returncode != 0:
            fail(f"health command failed for {manifest['name']}: {health.stderr.strip()}")
        try:
            health_json = json.loads(health.stdout.strip())
        except Exception as exc:
            fail(f"health command is not JSON for {manifest['name']}: {exc}")
        if health_json.get("protocol") != PROCESS_PROTOCOL or health_json.get("status") != "healthy":
            fail(f"invalid health contract for {manifest['name']}: {health_json}")

        proc = subprocess.run(
            [str(artifact)],
            cwd=ROOT,
            input=json.dumps(envelope) + "\n",
            capture_output=True,
            text=True,
            timeout=timeout_s,
            check=False,
        )
        if proc.returncode != 0:
            fail(f"process frame failed for {manifest['name']}: {proc.stderr.strip()}")
        lines = [line for line in proc.stdout.splitlines() if line.strip()]
        if len(lines) != 1:
            fail(f"expected one process reply from {manifest['name']}, got {len(lines)}")
        try:
            reply = json.loads(lines[0])
        except Exception as exc:
            fail(f"process reply is not JSON for {manifest['name']}: {exc}")
        if reply.get("protocol") != PROCESS_PROTOCOL:
            fail(f"process protocol mismatch for {manifest['name']}")
        if reply.get("correlation_uuid") != message_uuid:
            fail(f"correlation mismatch for {manifest['name']}")
        if reply.get("status") not in {"ok", "error", "rejected"}:
            fail(f"invalid process reply status for {manifest['name']}")
        if not is_v7(reply.get("message_uuid", "")):
            fail(f"process reply UUID is not v7 for {manifest['name']}")
        checked += 1
    print(f"OK process protocol: {checked}")


def verify_ui() -> None:
    required = [
        ROOT / "apps/phxclaw-ui/index.html",
        ROOT / "apps/phxclaw-ui/assets/app.css",
        ROOT / "apps/phxclaw-ui/assets/app.js",
        ROOT / "apps/phxclaw-ui/assets/phoenix-mark.svg",
        ROOT / "preview/splash.png",
        ROOT / "preview/dashboard.png",
    ]
    for path in required:
        if not path.is_file() or path.stat().st_size == 0:
            fail(f"UI artifact missing or empty: {path}")
    for path in required[-2:]:
        if path.read_bytes()[:8] != b"\x89PNG\r\n\x1a\n":
            fail(f"invalid PNG preview: {path}")
    node = shutil.which("node")
    if node:
        subprocess.run([node, "--check", str(ROOT / "apps/phxclaw-ui/assets/app.js")], check=True)
        print("OK UI JavaScript syntax")
    print("OK UI shell + previews")


def verify_desktop_host() -> None:
    required = [
        ROOT / "apps/phxclaw-desktop/src-tauri/Cargo.toml",
        ROOT / "apps/phxclaw-desktop/src-tauri/tauri.conf.json",
        ROOT / "apps/phxclaw-desktop/src-tauri/capabilities/default.json",
        ROOT / "apps/phxclaw-desktop/src-tauri/src/main.rs",
        ROOT / "apps/phxclaw-desktop/src-tauri/src/lib.rs",
        ROOT / "crates/phxclaw-live-bus/src/lib.rs",
        ROOT / "crates/phxclaw-api-gateway/src/lib.rs",
        ROOT / "crates/phxclaw-evidence-ledger/src/lib.rs",
        ROOT / "crates/phxclaw-desktop-protocol/src/lib.rs",
        ROOT / "migrations/0009_desktop_host_and_evidence.sql",
        ROOT / "config/desktop-host.json",
    ]
    for path in required:
        if not path.is_file() or path.stat().st_size == 0:
            fail(f"Desktop Host artifact missing or empty: {path}")

    tauri = json.loads((ROOT / "apps/phxclaw-desktop/src-tauri/tauri.conf.json").read_text(encoding="utf-8"))
    if tauri.get("version") != "0.14.0":
        fail("Tauri Desktop version must be 0.14.0")
    if tauri.get("identifier") != "br.com.phxclaw.desktop":
        fail("unexpected Tauri identifier")

    desktop = json.loads((ROOT / "config/desktop-host.json").read_text(encoding="utf-8"))
    if desktop["api"]["bind"] not in {"127.0.0.1", "::1"}:
        fail("Desktop API must bind to loopback")
    if desktop["api"]["allow_host_control_topics"]:
        fail("HTTP host-control must remain deny-by-default")
    if any(desktop["host_policy"].values() if isinstance(desktop["host_policy"], dict) else []):
        # allowlist is a list and falsey; all default booleans must be false.
        for key, value in desktop["host_policy"].items():
            if key != "managed_webview_origin_allowlist" and value is not False:
                fail(f"Desktop host policy must default to deny: {key}")
    print("OK Desktop Host scaffold + deny-by-default config")


def verify_research_pipeline(parsed: dict[Path, object]) -> None:
    required = [
        ROOT / "crates/phxclaw-agent-catalog/src/lib.rs",
        ROOT / "crates/phxclaw-research-pipeline/src/lib.rs",
        ROOT / "crates/phxclaw-skill-runtime/src/lib.rs",
        ROOT / "crates/phxclaw-memory-context/src/lib.rs",
        ROOT / "crates/phxclaw-source-registry/src/lib.rs",
        ROOT / "config/skills/registry.index.json",
        ROOT / "config/skills/rust-research.skill.json",
        ROOT / "config/research-pipeline.json",
        ROOT / "migrations/0012_research_context_pipeline.sql",
        ROOT / "scripts/test_research_pipeline_static.py",
    ]
    for path in required:
        if not path.is_file() or path.stat().st_size == 0:
            fail(f"Research pipeline artifact missing or empty: {path}")

    agents = list((ROOT / "config/agents").glob("*.agent.json"))
    if len(agents) != 110:
        fail(f"expected 110 agent manifests, found {len(agents)}")

    research = next((parsed[path] for path in agents if parsed[path].get("name") == "Research Agent"), None)
    if research is None:
        fail("Research Agent manifest missing")
    required_caps = {
        "research.collect", "knowledge.rust.read", "knowledge.source_registry.read",
        "context.compile", "skill.read",
    }
    if not required_caps.issubset(set(research.get("capabilities", []))):
        fail("Research Agent is missing F15/F16 research capabilities")
    if "rust-official" not in research.get("knowledge_sources", []):
        fail("Research Agent must declare rust-official")

    skill = parsed[ROOT / "config/skills/rust-research.skill.json"]
    expected = skill.get("sha256", "")
    body = dict(skill)
    body["sha256"] = ""
    actual = hashlib.sha256(
        json.dumps(body, ensure_ascii=False, separators=(",", ":"), sort_keys=True).encode("utf-8")
    ).hexdigest()
    if expected.lower() != actual.lower():
        fail(f"skill canonical sha256 mismatch: {expected} != {actual}")

    policy = parsed[ROOT / "config/research-pipeline.json"]
    production = policy["profiles"]["production"]
    if production["require_promoted_skills"] is not True or production["allow_validated_skills"] is not False:
        fail("production research pipeline must require promoted skills")

    print("OK Research pipeline + lazy skills + agent catalog")


def verify_mission_runtime(parsed: dict[Path, object]) -> None:
    required = [
        ROOT / "crates/phxclaw-code-workspace/src/lib.rs",
        ROOT / "crates/phxclaw-mission-runtime/src/lib.rs",
        ROOT / "apps/phxclaw-mission-cli/src/main.rs",
        ROOT / "config/mission-runtime.json",
        ROOT / "schemas/mission.schema.json",
        ROOT / "migrations/0014_mission_runtime.sql",
        ROOT / "scripts/test_mission_runtime_static.py",
    ]
    for path in required:
        if not path.is_file() or path.stat().st_size == 0:
            fail(f"Mission Runtime artifact missing or empty: {path}")

    catalog = parsed[ROOT / "config/capability-catalog.json"]
    if catalog.get("count") != len(catalog.get("capabilities", [])):
        fail("capability catalog count mismatch")
    required_caps = {
        "mission.run", "workspace.snapshot", "workspace.diff",
        "workspace.file.write", "workspace.gate.run", "workspace.worktree.create",
    }
    known = {item["name"] for item in catalog.get("capabilities", [])}
    if not required_caps.issubset(known):
        fail("Mission Runtime capabilities missing from catalog")

    agents = [parsed[path] for path in sorted((ROOT / "config/agents").glob("*.agent.json"))]
    by_name = {agent["name"]: agent for agent in agents}
    if "mission.run" not in by_name["Master Orchestrator"].get("capabilities", []):
        fail("Master Orchestrator missing mission.run")
    if "workspace.file.write" not in by_name["Rust Worker Engineer"].get("capabilities", []):
        fail("Rust Worker Engineer missing workspace.file.write")
    print("OK Mission Runtime + Code Workspace static gates")

def is_phoenix_owned(path: Path) -> bool:
    rel = path.relative_to(ROOT).parts
    if len(rel) >= 2 and rel[0] == "private" and rel[1] in {"vendor", "vendor-quarantine"}:
        return False
    return True

def main() -> int:
    toml_files = [path for path in ROOT.rglob("Cargo.toml") if is_phoenix_owned(path)]
    for path in toml_files:
        with path.open("rb") as fh:
            tomllib.load(fh)
    print(f"OK TOML: {len(toml_files)}")

    json_files = [path for path in ROOT.rglob("*.json") if is_phoenix_owned(path)]
    parsed: dict[Path, object] = {}
    for path in json_files:
        parsed[path] = json.loads(path.read_text(encoding="utf-8"))
    print(f"OK JSON: {len(json_files)}")

    try:
        import jsonschema
        from jsonschema import Draft202012Validator, FormatChecker
    except Exception:
        print("WARN: jsonschema package unavailable; schema validation skipped")
    else:
        for path in sorted((ROOT / "schemas").glob("*.schema.json")):
            try:
                Draft202012Validator.check_schema(parsed[path])
            except Exception as exc:
                fail(f"invalid JSON Schema {path}: {exc}")

        sprint_schema = parsed[ROOT / "schemas/sprint.schema.json"]
        sprint_validator = Draft202012Validator(sprint_schema, format_checker=FormatChecker())
        for path in sorted((ROOT / "sprints").glob("*.json")):
            errors = sorted(sprint_validator.iter_errors(parsed[path]), key=lambda error: list(error.path))
            if errors:
                fail(f"{path}: {errors[0].message}")

        plugin_schema = parsed[ROOT / "schemas/plugin-manifest.schema.json"]
        plugin_validator = Draft202012Validator(plugin_schema, format_checker=FormatChecker())
        for path in sorted((ROOT / "plugins").rglob("*.plugin.json")):
            errors = sorted(plugin_validator.iter_errors(parsed[path]), key=lambda error: list(error.path))
            if errors:
                fail(f"{path}: {errors[0].message}")

        trust_schema = parsed[ROOT / "schemas/trust-store.schema.json"]
        trust_validator = Draft202012Validator(trust_schema, format_checker=FormatChecker())
        trust_path = ROOT / "config/trust/plugin-signers.json"
        errors = sorted(trust_validator.iter_errors(parsed[trust_path]), key=lambda error: list(error.path))
        if errors:
            fail(f"{trust_path}: {errors[0].message}")

        agent_schema = parsed[ROOT / "schemas/agent-manifest.schema.json"]
        agent_validator = Draft202012Validator(agent_schema, format_checker=FormatChecker())
        for path in sorted((ROOT / "config/agents").glob("*.agent.json")):
            errors = sorted(agent_validator.iter_errors(parsed[path]), key=lambda error: list(error.path))
            if errors:
                fail(f"{path}: {errors[0].message}")

        knowledge_schema = parsed[ROOT / "schemas/knowledge-source.schema.json"]
        knowledge_validator = Draft202012Validator(knowledge_schema, format_checker=FormatChecker())
        for path in sorted((ROOT / "config/knowledge-sources").glob("*.json")):
            errors = sorted(knowledge_validator.iter_errors(parsed[path]), key=lambda error: list(error.path))
            if errors:
                fail(f"{path}: {errors[0].message}")

        skill_schema = parsed[ROOT / "schemas/skill-manifest.schema.json"]
        skill_validator = Draft202012Validator(skill_schema, format_checker=FormatChecker())
        for path in sorted((ROOT / "config/skills").glob("*.skill.json")):
            errors = sorted(skill_validator.iter_errors(parsed[path]), key=lambda error: list(error.path))
            if errors:
                fail(f"{path}: {errors[0].message}")

        community_schema = parsed[ROOT / "schemas/community-registry.schema.json"]
        community_validator = Draft202012Validator(community_schema, format_checker=FormatChecker())
        community_path = ROOT / "config/community/registry.json"
        errors = sorted(community_validator.iter_errors(parsed[community_path]), key=lambda error: list(error.path))
        if errors:
            fail(f"{community_path}: {errors[0].message}")

        print(f"OK JSON Schema: {len(list((ROOT / 'schemas').glob('*.schema.json')))} schemas")

    ids = []
    for path in sorted((ROOT / "sprints").glob("*.json")):
        data = parsed[path]
        ids.append((str(path.relative_to(ROOT)), data["uuid"]))
        for task in data["tasks"]:
            ids.append((str(path.relative_to(ROOT)) + "/task", task["uuid"]))
    for path in sorted((ROOT / "plugins").rglob("*.plugin.json")):
        ids.append((str(path.relative_to(ROOT)), parsed[path]["uuid"]))
    capability_catalog_path = ROOT / "config/capability-catalog.json"
    if capability_catalog_path in parsed:
        catalog = parsed[capability_catalog_path]
        ids.append(("config/capability-catalog.json/catalog", catalog["catalog_uuid"]))
        for capability in catalog.get("capabilities", []):
            ids.append((f"config/capability-catalog.json/{capability['name']}", capability["uuid"]))
    for path in sorted((ROOT / "config/agents").glob("*.agent.json")):
        ids.append((str(path.relative_to(ROOT)), parsed[path]["uuid"]))
    for path in sorted((ROOT / "config/knowledge-sources").glob("*.json")):
        ids.append((str(path.relative_to(ROOT)), parsed[path]["uuid"]))
    for path in sorted((ROOT / "config/skills").glob("*.skill.json")):
        ids.append((str(path.relative_to(ROOT)), parsed[path]["uuid"]))
        for evidence in parsed[path].get("evidence", []):
            ids.append((str(path.relative_to(ROOT)) + "/evidence", evidence["uuid"]))
    for where, value in ids:
        if not is_v7(value):
            fail(f"non-UUIDv7 at {where}: {value}")
    print(f"OK UUIDv7: {len(ids)}")

    constitution = parsed[ROOT / "config/constitution.json"]
    required = {"research", "hypothesis", "installer"}
    if not required.issubset(set(constitution["native_core_capabilities"])):
        fail("native core capabilities missing")
    if constitution["official_state_store"] != "PostgreSQL":
        fail("PostgreSQL must be official state store")
    if constitution["persistent_identity"] != "UUIDv7":
        fail("UUIDv7 must be persistent identity")
    if constitution["core_language"] != "Rust":
        fail("Rust must be core language")
    if constitution["declarative_format"] != "JSON":
        fail("JSON must be declarative format")
    if constitution["default_permission_effect"] != "deny":
        fail("permissions must remain deny-by-default")
    required_services = {
        "plugin_registry", "sandbox", "agent_runtime", "process_protocol",
        "task_graph", "model_gateway", "event_bus", "postgres_adapter",
        "http_client", "egress_broker", "ollama_adapter", "system_automation",
        "webview_control", "document_io", "media_intelligence", "bpm_engine",
        "ui_shell", "agent_catalog", "skill_runtime", "memory_context",
        "source_registry", "research_pipeline", "plugin_sdk",
        "extension_host", "community_registry", "code_workspace", "mission_runtime",
        "checkpoint", "mcp_lsp_runtime", "repo_intelligence", "mission_executors",
    }
    for service in sorted(required_services):
        if service not in constitution.get("platform_services", []):
            fail(f"missing platform service: {service}")
    print("OK constitution")

    verify_plugin_integrity(parsed)
    verify_process_contracts(parsed)
    verify_ui()
    verify_desktop_host()
    verify_research_pipeline(parsed)
    verify_mission_runtime(parsed)

    required_reports = [
        ("PLUGIN_ECOSYSTEM_TEST_REPORT.json", 71, "Community Plugin Ecosystem"),
        ("OLMOCR_PLUGIN_TEST_REPORT.json", 5, "olmOCR adapter"),
        ("V09_SMOKE_TEST_REPORT.json", 9, "real process smoke"),
        ("MISSION_RUNTIME_TEST_REPORT.json", 24, "Mission Runtime host smoke"),
    ]
    for filename, minimum_pass, label in required_reports:
        report_path = ROOT / filename
        if not report_path.is_file():
            fail(f"{label} report missing: {filename}")
        report = json.loads(report_path.read_text(encoding="utf-8"))
        if report.get("fail") != 0 or report.get("pass", 0) < minimum_pass:
            fail(f"{label} report is not green")
        print(f"OK {label} report: {report.get('pass')} PASS / 0 FAIL")

    cargo = shutil.which("cargo")
    if cargo:
        subprocess.run([cargo, "check", "--workspace"], cwd=ROOT, check=True)
        subprocess.run([cargo, "test", "--workspace"], cwd=ROOT, check=True)
        print("OK cargo check/test")
    else:
        print("WARN cargo not installed; Rust gates skipped")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
