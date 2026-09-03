#!/usr/bin/env python3
"""Create a non-destructive WX migration workspace from plugin templates."""

from __future__ import annotations

import argparse
import json
import os
import sys
from pathlib import Path


def ensure_safe_destination(base: Path, destination: Path) -> None:
    base = base.resolve(strict=True)
    parent = destination.parent
    parent.mkdir(parents=True, exist_ok=True, mode=0o700)
    resolved_parent = parent.resolve(strict=True)
    if base != resolved_parent and base not in resolved_parent.parents:
        raise ValueError(f"destino fora do projeto: {destination}")
    cursor = parent
    while cursor != base:
        if cursor.is_symlink():
            raise ValueError(f"diretório de destino é symlink: {cursor}")
        cursor = cursor.parent
    if destination.is_symlink():
        raise ValueError(f"destino é symlink: {destination}")


def write_new(destination: Path, payload: bytes) -> str:
    flags = os.O_WRONLY | os.O_CREAT | os.O_EXCL
    if hasattr(os, "O_NOFOLLOW"):
        flags |= os.O_NOFOLLOW
    try:
        descriptor = os.open(destination, flags, 0o600)
    except FileExistsError:
        return f"SKIPPED {destination} (já existe)"
    try:
        with os.fdopen(descriptor, "wb") as stream:
            stream.write(payload)
            stream.flush()
            os.fsync(stream.fileno())
    except Exception:
        try:
            destination.unlink()
        except OSError:
            pass
        raise
    return f"CREATED {destination}"


def copy_new(project_root: Path, source: Path, destination: Path) -> str:
    ensure_safe_destination(project_root, destination)
    return write_new(destination, source.read_bytes())


def bootstrap(project_root: Path, evidence_root: Path, install_claude_md: bool) -> list[str]:
    project_root = project_root.resolve(strict=True)
    evidence_root = evidence_root.resolve(strict=True)
    if not project_root.is_dir() or not evidence_root.is_dir():
        raise ValueError("project-root e evidence-root precisam ser diretórios")
    skill_root = Path(__file__).resolve().parent.parent
    templates = skill_root / "templates"
    schemas = skill_root / "schemas"
    migration = project_root / ".wx-migration"
    if migration.is_symlink():
        raise ValueError(".wx-migration não pode ser symlink")
    migration.mkdir(parents=True, exist_ok=True, mode=0o700)
    messages: list[str] = []

    manifest_destination = migration / "wx-inputs.manifest.json"
    ensure_safe_destination(project_root, manifest_destination)
    if not manifest_destination.exists():
        manifest = json.loads((templates / "wx-inputs.manifest.json").read_text(encoding="utf-8"))
        try:
            manifest["evidence_root"] = os.path.relpath(evidence_root, migration)
        except ValueError:
            manifest["evidence_root"] = str(evidence_root)
        payload = (json.dumps(manifest, ensure_ascii=False, indent=2) + "\n").encode("utf-8")
        messages.append(write_new(manifest_destination, payload))
    else:
        messages.append(f"SKIPPED {manifest_destination} (já existe)")

    messages.append(copy_new(project_root, templates / "conversion.config.json", migration / "conversion.config.json"))
    messages.append(copy_new(project_root, templates / "traceability.csv", migration / "traceability.csv"))
    messages.append(copy_new(project_root, schemas / "wx-inputs.schema.json", migration / "schemas" / "wx-inputs.schema.json"))
    messages.append(copy_new(project_root, schemas / "conversion-config.schema.json", migration / "schemas" / "conversion-config.schema.json"))
    if install_claude_md:
        messages.append(copy_new(project_root, templates / "CLAUDE.md", project_root / "CLAUDE.md"))
    return messages


def main() -> int:
    parser = argparse.ArgumentParser(description="Cria .wx-migration sem sobrescrever arquivos.")
    parser.add_argument("--project-root", required=True, type=Path)
    parser.add_argument("--evidence-root", required=True, type=Path)
    parser.add_argument("--install-claude-md", action="store_true")
    args = parser.parse_args()
    try:
        messages = bootstrap(args.project_root, args.evidence_root, args.install_claude_md)
    except (OSError, ValueError, json.JSONDecodeError) as exc:
        print(f"bootstrap falhou: {exc}", file=sys.stderr)
        return 2
    print("\n".join(messages))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
