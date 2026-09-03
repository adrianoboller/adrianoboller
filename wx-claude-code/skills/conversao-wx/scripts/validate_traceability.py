#!/usr/bin/env python3
"""Validate the evidence-to-test traceability contract.

The base validation is self-contained. Optional inputs make the validation
stronger without changing the traceability CSV format:

* ``--inventory`` proves that source artifacts and hashes were inventoried;
* ``--project-root`` proves that target and test files exist below the project;
* ``--expected-id(s)`` proves exact trace-ID coverage for a gate.
"""

from __future__ import annotations

import argparse
import csv
import io
import json
import re
from collections.abc import Iterable
from datetime import datetime
from pathlib import Path, PurePosixPath
from urllib.parse import urlsplit


REQUIRED_COLUMNS = (
    "trace_id", "kind", "source_artifact", "source_locator", "source_sha256",
    "legacy_symbol", "rule_summary", "decision_id", "target_component",
    "target_file", "target_symbol", "test_id", "test_file", "expected",
    "actual", "target_commit", "test_result_ref", "approved_by", "approved_at",
    "status", "confidence", "notes",
)
REQUIRED_COLUMN_SET = set(REQUIRED_COLUMNS)
INVENTORY_REQUIRED_COLUMNS = {"path", "sha256"}

KINDS = {"business_rule", "ui", "query", "database", "integration", "report", "non_functional"}
STATUSES = {"inventoried", "specified", "implemented", "verified", "accepted", "blocked"}
CONFIDENCE = {"high", "medium", "low"}
TRACE_RE = re.compile(r"^(BR|UI|QRY|DB|INT|RPT|NFR)-[A-Z0-9][A-Z0-9._-]*$", re.I)
DECISION_RE = re.compile(r"^DEC-[A-Z0-9][A-Z0-9._-]*$", re.I)
TEST_RE = re.compile(r"^TST-[A-Z0-9][A-Z0-9._-]*$", re.I)
SHA256_RE = re.compile(r"^[0-9a-f]{64}$", re.I)
COMMIT_RE = re.compile(r"^[0-9a-f]{7,64}$", re.I)
WINDOWS_DRIVE_RE = re.compile(r"^[A-Za-z]:[/\\]")
KIND_BY_PREFIX = {
    "BR": "business_rule",
    "UI": "ui",
    "QRY": "query",
    "DB": "database",
    "INT": "integration",
    "RPT": "report",
    "NFR": "non_functional",
}

# Generous defensive bounds for real migrations and hostile/malformed input.
MAX_CSV_BYTES = 32 * 1024 * 1024
MAX_ROWS = 100_000
MAX_COLUMNS = 128
MAX_FIELD_CHARS = 1_000_000
MAX_ISSUES = 2_000


def _result(
    errors: list[str],
    warnings: list[str],
    rows: int,
    trace_ids: Iterable[str] = (),
    *,
    inventory_checked: bool = False,
    project_root_checked: bool = False,
    expected_ids_checked: bool = False,
) -> dict:
    return {
        "valid": not errors,
        "rows": rows,
        "trace_ids": sorted(set(trace_ids)),
        "errors": errors,
        "warnings": warnings,
        "checks": {
            "inventory": inventory_checked,
            "project_root": project_root_checked,
            "expected_ids": expected_ids_checked,
        },
    }


def _add_issue(collection: list[str], message: str) -> None:
    if len(collection) < MAX_ISSUES:
        collection.append(message)
    elif len(collection) == MAX_ISSUES:
        collection.append(f"limite de {MAX_ISSUES} ocorrências atingido; demais ocorrências omitidas")


def _read_bounded_text(path: Path, label: str, errors: list[str]) -> str | None:
    try:
        stat = path.stat()
    except OSError as exc:
        _add_issue(errors, f"{label}: {exc}")
        return None
    if not path.is_file():
        _add_issue(errors, f"{label}: não é um arquivo regular: {path}")
        return None
    if stat.st_size == 0:
        _add_issue(errors, f"{label}: arquivo vazio")
        return None
    if stat.st_size > MAX_CSV_BYTES:
        _add_issue(errors, f"{label}: excede o limite de {MAX_CSV_BYTES} bytes")
        return None
    try:
        return path.read_text(encoding="utf-8-sig")
    except (OSError, UnicodeError) as exc:
        _add_issue(errors, f"{label}: {exc}")
        return None


def _check_headers(fieldnames: list[str] | None, required: set[str], label: str, errors: list[str]) -> bool:
    if not fieldnames:
        _add_issue(errors, f"{label}: cabeçalho ausente")
        return False
    if len(fieldnames) > MAX_COLUMNS:
        _add_issue(errors, f"{label}: cabeçalho excede o limite de {MAX_COLUMNS} colunas")
    if any(name is None or not name.strip() for name in fieldnames):
        _add_issue(errors, f"{label}: cabeçalho contém coluna vazia")
    if any(name != name.strip() for name in fieldnames if name is not None):
        _add_issue(errors, f"{label}: cabeçalho contém espaços nas extremidades")

    duplicates: list[str] = []
    seen: set[str] = set()
    for name in fieldnames:
        folded = (name or "").casefold()
        if folded in seen and (name or "(vazia)") not in duplicates:
            duplicates.append(name or "(vazia)")
        seen.add(folded)
    if duplicates:
        _add_issue(errors, f"{label}: colunas duplicadas: {', '.join(duplicates)}")

    missing = sorted(required - set(fieldnames))
    if missing:
        _add_issue(errors, f"{label}: colunas ausentes: {', '.join(missing)}")
    clean = bool(fieldnames) and all(name and name.strip() == name for name in fieldnames)
    return not duplicates and not missing and clean and len(fieldnames) <= MAX_COLUMNS


def _dict_reader(text: str, label: str, errors: list[str]) -> csv.DictReader | None:
    if not text.strip():
        _add_issue(errors, f"{label}: arquivo vazio")
        return None
    try:
        csv.field_size_limit(MAX_FIELD_CHARS)
        return csv.DictReader(io.StringIO(text, newline=""), strict=True)
    except (csv.Error, OverflowError) as exc:
        _add_issue(errors, f"{label}: CSV inválido: {exc}")
        return None


def _canonical_artifact(value: str) -> tuple[str | None, str | None]:
    raw = value.strip()
    if not raw:
        return None, "caminho vazio"
    if "\x00" in raw:
        return None, "caminho contém byte NUL"

    parsed = urlsplit(raw)
    if parsed.scheme:
        if parsed.scheme not in {"http", "https"} or not parsed.netloc:
            return None, "URI de evidência inválida"
        if parsed.username or parsed.password:
            return None, "URI de evidência não pode conter credenciais"
        return raw, None

    parts = raw.split("!", 1)
    normalized_parts: list[str] = []
    for part in parts:
        portable = part.replace("\\", "/")
        if not portable or portable.startswith("/") or WINDOWS_DRIVE_RE.match(portable):
            return None, "caminho de evidência deve ser relativo"
        pure = PurePosixPath(portable)
        if ".." in pure.parts:
            return None, "caminho de evidência contém travessia por '..'"
        cleaned = pure.as_posix()
        if cleaned in {"", "."}:
            return None, "caminho de evidência vazio"
        normalized_parts.append(cleaned)
    if raw.count("!") > 1:
        return None, "referência de arquivo compactado inválida"
    return "!".join(normalized_parts), None


def _resolve_project_file(project_root: Path, value: str) -> tuple[Path | None, str | None]:
    raw = value.strip()
    if not raw:
        return None, "caminho vazio"
    if "\x00" in raw or "!" in raw or urlsplit(raw).scheme:
        return None, "caminho de projeto inválido"
    raw_path = Path(raw)
    candidate = raw_path if raw_path.is_absolute() else project_root / raw_path
    try:
        resolved = candidate.resolve(strict=False)
        resolved.relative_to(project_root)
    except (OSError, ValueError):
        return None, "caminho escapa de project-root"
    if not resolved.exists() or not resolved.is_file():
        return None, "arquivo não encontrado sob project-root"
    return resolved, None


def _project_path_error(value: str) -> str | None:
    raw = value.strip()
    if not raw:
        return "caminho vazio"
    portable = raw.replace("\\", "/")
    if "\x00" in raw or "!" in raw or urlsplit(raw).scheme:
        return "caminho de projeto inválido"
    if portable.startswith("/") or WINDOWS_DRIVE_RE.match(portable):
        return "caminho de projeto deve ser relativo"
    if ".." in PurePosixPath(portable).parts:
        return "caminho de projeto contém travessia por '..'"
    return None


def _valid_approved_at(value: str) -> bool:
    try:
        parsed = datetime.fromisoformat(value.replace("Z", "+00:00"))
    except ValueError:
        return False
    return parsed.tzinfo is not None and parsed.utcoffset() is not None


def _load_inventory(path: Path, errors: list[str]) -> dict[str, str]:
    text = _read_bounded_text(path, "inventory", errors)
    if text is None:
        return {}
    reader = _dict_reader(text, "inventory", errors)
    if reader is None:
        return {}
    try:
        headers_ok = _check_headers(reader.fieldnames, INVENTORY_REQUIRED_COLUMNS, "inventory", errors)
    except csv.Error as exc:
        _add_issue(errors, f"inventory: CSV inválido: {exc}")
        return {}
    if not headers_ok:
        return {}

    records: dict[str, str] = {}
    row_count = 0
    try:
        for row in reader:
            row_count += 1
            line = reader.line_num
            if row_count > MAX_ROWS:
                _add_issue(errors, f"inventory: excede o limite de {MAX_ROWS} linhas")
                break
            if row.get(None) is not None:
                _add_issue(errors, f"inventory linha {line}: quantidade de colunas excede o cabeçalho")
            raw_path = (row.get("path") or "").strip()
            digest = (row.get("sha256") or "").strip().lower()
            canonical, path_error = _canonical_artifact(raw_path)
            if path_error:
                _add_issue(errors, f"inventory linha {line}: path inválido: {path_error}")
                continue
            if not SHA256_RE.fullmatch(digest):
                _add_issue(errors, f"inventory linha {line}: sha256 inválido")
                continue
            assert canonical is not None
            if canonical in records:
                _add_issue(errors, f"inventory linha {line}: path duplicado: {canonical}")
                continue
            records[canonical] = digest
    except csv.Error as exc:
        _add_issue(errors, f"inventory: CSV inválido na linha {reader.line_num}: {exc}")
    if row_count == 0:
        _add_issue(errors, "inventory: CSV sem linhas de dados")
    return records


def _load_expected_ids_file(path: Path, errors: list[str]) -> list[str]:
    text = _read_bounded_text(path, "expected-ids", errors)
    if text is None:
        return []
    if path.suffix.lower() == ".json":
        try:
            payload = json.loads(text)
        except json.JSONDecodeError as exc:
            _add_issue(errors, f"expected-ids: JSON inválido: {exc}")
            return []
        if isinstance(payload, dict):
            payload = payload.get("trace_ids", payload.get("expected_ids"))
        if not isinstance(payload, list) or not all(isinstance(item, str) for item in payload):
            _add_issue(errors, "expected-ids: JSON deve ser uma lista ou conter trace_ids/expected_ids como lista")
            return []
        return payload
    if path.suffix.lower() == ".csv":
        reader = _dict_reader(text, "expected-ids", errors)
        if reader is None:
            return []
        try:
            if not _check_headers(reader.fieldnames, {"trace_id"}, "expected-ids", errors):
                return []
            values = [(row.get("trace_id") or "").strip() for row in reader]
        except csv.Error as exc:
            _add_issue(errors, f"expected-ids: CSV inválido na linha {reader.line_num}: {exc}")
            return []
        if not values:
            _add_issue(errors, "expected-ids: CSV sem linhas de dados")
        return values
    values: list[str] = []
    for line in text.splitlines():
        content = line.split("#", 1)[0]
        values.extend(token for token in re.split(r"[\s,]+", content) if token)
    return values


def _collect_expected_ids(values: object, errors: list[str]) -> set[str]:
    if isinstance(values, Path):
        raw_values: list[object] = _load_expected_ids_file(values, errors)
    elif isinstance(values, str):
        candidate = Path(values)
        is_trace_id = bool(TRACE_RE.fullmatch(values.strip()))
        if is_trace_id:
            raw_values = [values]
        else:
            try:
                candidate_exists = candidate.exists()
            except OSError:
                candidate_exists = False
            if candidate_exists:
                raw_values = _load_expected_ids_file(candidate, errors)
            else:
                raw_values = [token for token in re.split(r"[\s,]+", values) if token]
    elif isinstance(values, Iterable):
        raw_values = list(values)
    else:
        _add_issue(errors, "expected-ids: valor deve ser caminho, texto ou coleção de IDs")
        return set()

    flattened: list[str] = []
    for item in raw_values:
        if isinstance(item, Path):
            flattened.extend(_load_expected_ids_file(item, errors))
        elif isinstance(item, str):
            candidate = Path(item)
            try:
                candidate_is_file = candidate.exists() and candidate.is_file()
            except OSError:
                candidate_is_file = False
            if candidate_is_file and not TRACE_RE.fullmatch(item.strip()):
                flattened.extend(_load_expected_ids_file(candidate, errors))
            else:
                flattened.extend(token for token in re.split(r"[\s,]+", item) if token)
        else:
            _add_issue(errors, f"expected-ids: item inválido: {item!r}")

    result: set[str] = set()
    for raw in flattened:
        trace_id = raw.strip().upper()
        if not TRACE_RE.fullmatch(trace_id):
            _add_issue(errors, f"expected-ids: trace_id inválido: {raw!r}")
            continue
        if trace_id in result:
            _add_issue(errors, f"expected-ids: trace_id duplicado: {trace_id}")
            continue
        result.add(trace_id)
    return result


def validate(
    path: Path,
    inventory: Path | None = None,
    project_root: Path | None = None,
    expected_ids: object | None = None,
    *,
    inventory_path: Path | None = None,
) -> dict:
    """Validate a traceability CSV and optional external proofs."""

    errors: list[str] = []
    warnings: list[str] = []
    if inventory is not None and inventory_path is not None:
        return _result(["use inventory ou inventory_path, não ambos"], warnings, 0)
    inventory = inventory if inventory is not None else inventory_path

    inventory_records: dict[str, str] = {}
    if inventory is not None:
        inventory_records = _load_inventory(Path(inventory), errors)

    resolved_project_root: Path | None = None
    if project_root is not None:
        try:
            resolved_project_root = Path(project_root).resolve(strict=True)
            if not resolved_project_root.is_dir():
                raise ValueError("não é diretório")
        except (OSError, ValueError) as exc:
            _add_issue(errors, f"project-root inválido: {exc}")

    expected: set[str] | None = None
    if expected_ids is not None:
        expected = _collect_expected_ids(expected_ids, errors)

    text = _read_bounded_text(Path(path), "traceability", errors)
    if text is None:
        return _result(
            errors, warnings, 0,
            inventory_checked=inventory is not None,
            project_root_checked=project_root is not None,
            expected_ids_checked=expected_ids is not None,
        )
    reader = _dict_reader(text, "traceability", errors)
    if reader is None:
        return _result(errors, warnings, 0)
    try:
        headers_ok = _check_headers(reader.fieldnames, REQUIRED_COLUMN_SET, "traceability", errors)
    except csv.Error as exc:
        _add_issue(errors, f"traceability: CSV inválido: {exc}")
        headers_ok = False
    if not headers_ok:
        return _result(
            errors, warnings, 0,
            inventory_checked=inventory is not None,
            project_root_checked=project_root is not None,
            expected_ids_checked=expected_ids is not None,
        )

    seen_rows: set[tuple[str, ...]] = set()
    trace_ids: set[str] = set()
    stable_by_trace: dict[str, dict[str, str]] = {}
    artifact_hashes: dict[str, str] = {}
    test_files: dict[str, str] = {}
    row_count = 0
    all_headers = tuple(reader.fieldnames or ())

    try:
        for row in reader:
            row_count += 1
            line_number = reader.line_num
            prefix = f"linha {line_number}"
            if row_count > MAX_ROWS:
                _add_issue(errors, f"traceability: excede o limite de {MAX_ROWS} linhas")
                break
            if row.get(None) is not None:
                _add_issue(errors, f"{prefix}: quantidade de colunas excede o cabeçalho")
            if any(len(value or "") > MAX_FIELD_CHARS for value in row.values() if not isinstance(value, list)):
                _add_issue(errors, f"{prefix}: campo excede o limite de {MAX_FIELD_CHARS} caracteres")

            values = {name: (row.get(name) or "").strip() for name in REQUIRED_COLUMNS}
            if not any(values.values()):
                _add_issue(errors, f"{prefix}: linha vazia")
                continue

            fingerprint = tuple((row.get(name) or "").strip() for name in all_headers)
            if fingerprint in seen_rows:
                _add_issue(errors, f"{prefix}: linha duplicada")
            seen_rows.add(fingerprint)

            trace_id = values["trace_id"].upper()
            kind = values["kind"]
            status = values["status"]
            confidence = values["confidence"]

            if not TRACE_RE.fullmatch(trace_id):
                _add_issue(errors, f"{prefix}: trace_id inválido: {values['trace_id']!r}")
            else:
                trace_ids.add(trace_id)
                expected_kind = KIND_BY_PREFIX[trace_id.split("-", 1)[0]]
                if kind in KINDS and kind != expected_kind:
                    _add_issue(errors, f"{prefix}: prefixo de trace_id incompatível com kind {kind!r}")
            if kind not in KINDS:
                _add_issue(errors, f"{prefix}: kind inválido: {kind!r}")
            if status not in STATUSES:
                _add_issue(errors, f"{prefix}: status inválido: {status!r}")
            if confidence not in CONFIDENCE:
                _add_issue(errors, f"{prefix}: confidence inválida: {confidence!r}")

            source_artifact = values["source_artifact"]
            source_locator = values["source_locator"]
            source_sha256 = values["source_sha256"].lower()
            if not source_artifact or not source_locator:
                _add_issue(errors, f"{prefix}: evidência exige source_artifact e source_locator")
            canonical_source: str | None = None
            if source_artifact:
                canonical_source, path_error = _canonical_artifact(source_artifact)
                if path_error:
                    _add_issue(errors, f"{prefix}: source_artifact inválido: {path_error}")
            if not SHA256_RE.fullmatch(source_sha256):
                _add_issue(errors, f"{prefix}: source_sha256 deve conter 64 dígitos hexadecimais")
            elif canonical_source:
                prior_hash = artifact_hashes.get(canonical_source)
                if prior_hash is not None and prior_hash != source_sha256:
                    _add_issue(errors, f"{prefix}: source_artifact usa hashes inconsistentes: {canonical_source}")
                artifact_hashes[canonical_source] = source_sha256
                if inventory is not None:
                    inventory_hash = inventory_records.get(canonical_source)
                    if inventory_hash is None:
                        _add_issue(errors, f"{prefix}: source_artifact ausente do inventory: {canonical_source}")
                    elif inventory_hash != source_sha256:
                        _add_issue(errors, f"{prefix}: source_sha256 diverge do inventory para {canonical_source}")

            if not values["rule_summary"]:
                _add_issue(errors, f"{prefix}: rule_summary vazio")
            decision = values["decision_id"]
            if decision and not DECISION_RE.fullmatch(decision):
                _add_issue(errors, f"{prefix}: decision_id inválido: {decision!r}")

            target_pair = (values["target_component"], values["target_file"])
            if bool(target_pair[0]) != bool(target_pair[1]):
                _add_issue(errors, f"{prefix}: target_component e target_file devem ser preenchidos juntos")
            if status in {"implemented", "verified", "accepted"} and not all(target_pair):
                _add_issue(errors, f"{prefix}: item {status} exige target_component e target_file")
            if values["target_file"]:
                path_error = _project_path_error(values["target_file"])
                if path_error:
                    _add_issue(errors, f"{prefix}: target_file inválido: {path_error}")

            test_pair = (values["test_id"], values["test_file"])
            if bool(test_pair[0]) != bool(test_pair[1]):
                _add_issue(errors, f"{prefix}: test_id e test_file devem ser preenchidos juntos")
            if values["test_id"] and not TEST_RE.fullmatch(values["test_id"]):
                _add_issue(errors, f"{prefix}: test_id inválido: {values['test_id']!r}")
            if values["test_file"]:
                path_error = _project_path_error(values["test_file"])
                if path_error:
                    _add_issue(errors, f"{prefix}: test_file inválido: {path_error}")
            result_pair = (values["expected"], values["actual"])
            if bool(result_pair[0]) != bool(result_pair[1]):
                _add_issue(errors, f"{prefix}: expected e actual devem ser preenchidos juntos")
            if status in {"verified", "accepted"}:
                if not all(test_pair):
                    _add_issue(errors, f"{prefix}: item {status} exige test_id e test_file")
                if not all(result_pair):
                    _add_issue(errors, f"{prefix}: item {status} exige expected e actual")
                if confidence == "low":
                    _add_issue(errors, f"{prefix}: item {status} não pode ter confidence low")
                if not values["target_commit"]:
                    _add_issue(errors, f"{prefix}: item {status} exige target_commit")
                if not values["test_result_ref"]:
                    _add_issue(errors, f"{prefix}: item {status} exige test_result_ref")

            target_commit = values["target_commit"]
            if target_commit and not COMMIT_RE.fullmatch(target_commit):
                _add_issue(errors, f"{prefix}: target_commit inválido: {target_commit!r}")
            if values["test_result_ref"] and not values["test_id"]:
                _add_issue(errors, f"{prefix}: test_result_ref exige test_id")

            approved_by = values["approved_by"]
            approved_at = values["approved_at"]
            if status == "accepted":
                if not approved_by or not approved_at:
                    _add_issue(errors, f"{prefix}: item accepted exige approved_by e approved_at")
                elif not _valid_approved_at(approved_at):
                    _add_issue(errors, f"{prefix}: approved_at deve ser ISO 8601 com fuso horário")
            elif approved_by or approved_at:
                _add_issue(errors, f"{prefix}: aprovação humana só é permitida em item accepted")
            if bool(approved_by) != bool(approved_at):
                _add_issue(errors, f"{prefix}: approved_by e approved_at devem ser preenchidos juntos")

            if status == "blocked" and not values["notes"]:
                _add_issue(errors, f"{prefix}: item blocked exige notes com condição de desbloqueio")

            if values["test_id"]:
                prior_test_file = test_files.get(values["test_id"])
                if prior_test_file is not None and prior_test_file != values["test_file"]:
                    _add_issue(errors, f"{prefix}: test_id aponta para test_file inconsistente: {values['test_id']}")
                test_files[values["test_id"]] = values["test_file"]

            if TRACE_RE.fullmatch(trace_id):
                stable = {
                    "kind": kind,
                    "rule_summary": values["rule_summary"],
                    "legacy_symbol": values["legacy_symbol"],
                    "decision_id": decision,
                    "status": status,
                    "approved_by": approved_by,
                    "approved_at": approved_at,
                }
                prior = stable_by_trace.get(trace_id)
                if prior is None:
                    stable_by_trace[trace_id] = stable
                else:
                    for name in (
                        "kind", "rule_summary", "legacy_symbol", "decision_id",
                        "status", "approved_by", "approved_at",
                    ):
                        if prior[name] != stable[name]:
                            _add_issue(errors, f"{prefix}: {trace_id} possui {name} inconsistente entre linhas")

            if resolved_project_root is not None:
                for field in ("target_file", "test_file"):
                    if not values[field]:
                        continue
                    _, path_error = _resolve_project_file(resolved_project_root, values[field])
                    if path_error:
                        _add_issue(errors, f"{prefix}: {field} inválido: {path_error}: {values[field]}")
                result_ref = values["test_result_ref"]
                if result_ref and not urlsplit(result_ref).scheme:
                    path_error = _project_path_error(result_ref)
                    if not path_error:
                        _, path_error = _resolve_project_file(resolved_project_root, result_ref)
                    if path_error:
                        _add_issue(errors, f"{prefix}: test_result_ref inválido: {path_error}: {result_ref}")
    except csv.Error as exc:
        _add_issue(errors, f"traceability: CSV inválido na linha {reader.line_num}: {exc}")

    if row_count == 0:
        _add_issue(errors, "traceability: CSV sem linhas de dados")

    if expected is not None:
        missing = sorted(expected - trace_ids)
        unexpected = sorted(trace_ids - expected)
        if missing:
            _add_issue(errors, "trace_ids esperados ausentes: " + ", ".join(missing))
        if unexpected:
            _add_issue(errors, "trace_ids não previstos: " + ", ".join(unexpected))

    return _result(
        errors,
        warnings,
        min(row_count, MAX_ROWS),
        trace_ids,
        inventory_checked=inventory is not None,
        project_root_checked=project_root is not None,
        expected_ids_checked=expected_ids is not None,
    )


def main() -> int:
    parser = argparse.ArgumentParser(description="Valida traceability.csv da conversão WX.")
    parser.add_argument("path", type=Path)
    parser.add_argument("--inventory", type=Path, help="inventory.csv do pré-flight para validar fontes e hashes")
    parser.add_argument("--project-root", type=Path, help="raiz do projeto para validar target_file e test_file")
    parser.add_argument("--expected-id", action="append", default=[], help="trace_id esperado; opção repetível")
    parser.add_argument(
        "--expected-ids",
        action="extend",
        nargs="+",
        default=[],
        metavar="ID[,ID...]|ARQUIVO",
        help="um ou mais IDs, listas separadas por vírgula ou arquivos TXT/CSV/JSON",
    )
    parser.add_argument(
        "--expected-ids-file",
        action="append",
        default=[],
        type=Path,
        help="arquivo TXT/CSV/JSON com IDs esperados; opção repetível",
    )
    parser.add_argument("--json", action="store_true")
    args = parser.parse_args()
    expected_inputs: list[object] | None = [*args.expected_id, *args.expected_ids, *args.expected_ids_file]
    if not expected_inputs:
        expected_inputs = None
    result = validate(
        args.path,
        inventory=args.inventory,
        project_root=args.project_root,
        expected_ids=expected_inputs,
    )
    if args.json:
        print(json.dumps(result, ensure_ascii=False, indent=2))
    else:
        print("VALID" if result["valid"] else "INVALID")
        for error in result["errors"]:
            print(f"ERROR: {error}")
        for warning in result["warnings"]:
            print(f"WARNING: {warning}")
    return 0 if result["valid"] else 2


if __name__ == "__main__":
    raise SystemExit(main())
