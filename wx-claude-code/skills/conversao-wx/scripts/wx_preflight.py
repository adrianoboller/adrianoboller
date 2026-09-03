#!/usr/bin/env python3
"""Bounded, non-clobbering audit of WX migration evidence.

The CLI requires an explicitly approved evidence root and workspace root. It
never executes supplied artifacts and writes a new versioned run directory
under --output. Exit codes: 0 READY, 2 CONDITIONAL, 3 BLOCKED, 4 invalid input.
"""

from __future__ import annotations

import argparse
import codecs
import ctypes
import csv
import errno
import hashlib
import io
import ipaddress
import json
import math
import os
import re
import shutil
import stat
import struct
import sys
import unicodedata
import zipfile
import zlib
from collections import deque
from datetime import datetime, timezone
from pathlib import Path, PurePosixPath, PureWindowsPath
from urllib.parse import parse_qsl, unquote, unquote_plus, urlsplit, urlunsplit


VALID_STATUSES = {"provided", "partial", "missing", "not_applicable"}
VALID_HELP_STATUSES = VALID_STATUSES | {"bundled"}
VALID_MODES = {"inventory", "plan", "pilot", "complete"}
VALID_EVIDENCE_CLASSES = {"auto", "native", "documentary", "forensic"}
VALID_ACCESS = {"public", "authenticated", "vpn", "unverified"}
VALID_PRODUCTS = {"WINDEV", "WEBDEV", "WINDEV Mobile"}
VALID_EXCEPTION_CODES = {
    "NO_SAMPLE_DATA",
    "NO_SOURCE_RUNTIME",
    "SOURCE_RUNTIME_NOT_AUTHORIZED",
    "RUNTIME_BASELINE_MISSING",
    "RUNTIME_METADATA_MISSING",
    "ACCEPTANCE_CRITERIA_MISSING",
    "ACCEPTANCE_DIMENSIONS_MISSING",
    "ACCEPTANCE_THRESHOLDS_MISSING",
    "ACCEPTANCE_PLATFORM_MATRIX_MISSING",
    "ACCEPTANCE_REHEARSALS_MISSING",
}
CORE_GROUPS = {
    "wlanguage_help_json",
    "code_documents",
    "ui_documents",
    "query_documents",
    "business_rule_documents",
    "sql_scripts",
    "screenshots",
}
KNOWN_ARTIFACT_GROUPS = {
    "native_project_sources",
    "wlanguage_help_json",
    "code_documents",
    "ui_documents",
    "query_documents",
    "business_rule_documents",
    "sql_scripts",
    "screenshots",
    "videos_and_runtime_baselines",
    "api_and_integration_docs",
    "auxiliary_sources",
    "sample_data_and_expected_results",
    "external_links",
}
EXPECTED_EXTENSIONS = {
    "code_documents": {".pdf"},
    "ui_documents": {".pdf"},
    "query_documents": {".pdf"},
    "business_rule_documents": {".pdf", ".md", ".txt"},
    "sql_scripts": {".sql"},
    "screenshots": {".png", ".jpg", ".jpeg", ".webp", ".gif"},
    "api_and_integration_docs": {".pdf", ".json", ".yaml", ".yml", ".md", ".txt"},
}
TEXT_EXTENSIONS = {
    ".json", ".sql", ".txt", ".md", ".ini", ".cfg", ".conf", ".xml",
    ".yaml", ".yml", ".env", ".properties", ".toml", ".csv", ".jsonl",
}
SECRET_PATTERNS = [
    re.compile(
        r"(?i)[\"']?\b(password|passwd|pwd|secret|client[_-]?secret|api[_-]?key|"
        r"access[_-]?token|refresh[_-]?token|private[_-]?key)[\"']?\s*[:=]\s*"
        r"[\"']?[^\s,;\"']+"
    ),
    re.compile(r"-----BEGIN (?:RSA |EC |OPENSSH )?PRIVATE KEY-----"),
    re.compile(r"(?i)\bbearer\s+[a-z0-9._~+/-]{12,}=*"),
    re.compile(r"\bAKIA[0-9A-Z]{16}\b"),
    re.compile(r"\b(?:gh[pousr]_[A-Za-z0-9_]{20,}|xox[baprs]-[A-Za-z0-9-]{20,})\b"),
    re.compile(r"\beyJ[A-Za-z0-9_-]{8,}\.[A-Za-z0-9_-]{8,}\.[A-Za-z0-9_-]{8,}\b"),
    re.compile(
        r"(?i)\b(?:server|data\s+source)\s*=\s*[^;\r\n]+;[^\r\n]*"
        r"\b(?:password|pwd)\s*=\s*[^;\r\n]+"
    ),
]
SECRET_PLACEHOLDERS = (
    "secret_ref", "<redacted>", "[redacted]", "redacted_value",
    "$" + "{secret", "$" + "{env", "example_only",
)
SENSITIVE_QUERY_KEY = re.compile(
    r"(?i)(?:^|[_-])(token|key|secret|password|passwd|pwd|signature|credential|auth|code)(?:$|[_-])"
)
IDENTITY_KEYS = (
    "document_id", "documentid", "help_id", "helpid", "slug", "name",
    "title", "symbol", "function", "heading", "nom", "titre", "fonction",
)
VERSION_KEYS = ("wlanguage_help_version", "help_version", "wx_version", "version")
LANGUAGE_KEYS = ("wlanguage_help_language", "help_language", "language", "lang", "locale")
PRODUCT_KEYS = ("product", "products", "wx_product")

MAX_MANIFEST_SIZE = 2 * 1024 * 1024
MAX_CONFIG_SIZE = 1 * 1024 * 1024
MAX_ARTIFACT_SIZE = 512 * 1024 * 1024
MAX_HELP_MEMBER = 32 * 1024 * 1024
MAX_HELP_TOTAL = 256 * 1024 * 1024
MAX_ZIP_ARCHIVE = 256 * 1024 * 1024
MAX_ZIP_MEMBERS = 256
MAX_COMPRESSION_RATIO = 200
MAX_ITEMS_PER_GROUP = 1000
MAX_JSON_DEPTH = 128
MAX_JSON_NODES = 500_000
MAX_JSON_STRING = 4 * 1024 * 1024
MAX_METADATA_SCAN_NODES = 20_000
MAX_URL_LENGTH = 4096
MAX_DISPLAY_LENGTH = 1024
MAX_JSON_INTEGER_DIGITS = 1024
MAX_TOTAL_ARTIFACT_BYTES = 2 * 1024 * 1024 * 1024
MAX_INVENTORY_ITEMS = 5000
MAX_PATH_DEPTH = 32
MAX_COMPONENT_CHARS = 255
MAX_IMAGE_DIMENSION = 100_000
MAX_IMAGE_PIXELS = 250_000_000
MAX_JPEG_SEGMENTS = 512
MAX_PDF_DECLARED_PAGES = 1_000_000
READ_BLOCK = 1024 * 1024
SECRET_SCAN_OVERLAP = 4096
ALLOWED_ZIP_COMPRESSION = {zipfile.ZIP_STORED, zipfile.ZIP_DEFLATED}
WINDOWS_RESERVED_NAMES = {
    "CON", "PRN", "AUX", "NUL",
    *(f"COM{index}" for index in range(1, 10)),
    *(f"LPT{index}" for index in range(1, 10)),
}

BUNDLED_HELP_ARCHIVE_NAME = "Help_WL_12k_Json.zip"
BUNDLED_HELP_ROOT = "Help_WL_12k_Json"
BUNDLED_HELP_CORPUS_ID = "wlanguage-help-12k-2026-08"
BUNDLED_HELP_SHA256 = "a95ed5536549ecc39fb1163415042d6597c8913e5edbfdb531cba756546a82a2"
BUNDLED_HELP_SIZE = 26_750_976
BUNDLED_HELP_JSON_COUNT = 12_037
BUNDLED_HELP_PAGE_COUNT = 12_036
BUNDLED_HELP_VALID_PAGE_COUNT = 12_035
BUNDLED_HELP_FILE_COUNT = 12_038
BUNDLED_HELP_MEMBER_COUNT = 12_039
BUNDLED_HELP_UNCOMPRESSED_BYTES = 115_844_631
BUNDLED_HELP_INDEX_MEMBER = f"{BUNDLED_HELP_ROOT}/00_indice_de_grupos.json"
BUNDLED_HELP_PROGRESS_MEMBER = f"{BUNDLED_HELP_ROOT}/progresso.ini"
BUNDLED_HELP_INVALID_MEMBER = (
    f"{BUNDLED_HELP_ROOT}/"
    "01-04-01_00655__emailgetall_function__1000018727.json"
)
BUNDLED_HELP_INVALID_SIZE = 23_627
BUNDLED_HELP_INVALID_SHA256 = (
    "d95886e1dc971804e4fe98c784504c54665c5aa4a4adcc4de90e4f58e54e5148"
)
BUNDLED_HELP_LANGUAGE = "en-US"
BUNDLED_HELP_VERSION_COVERAGE = ["24", "25", "26", "27", "28", "2024", "2025", "2026"]
BUNDLED_HELP_PRODUCT_SCOPE = ["WINDEV", "WEBDEV", "WINDEV Mobile"]
BUNDLED_HELP_SANITIZATION = {
    "source_sha256": "a6b42f59796ccf51298712aff00c043a9be2c404ce761a99720ea31b91ca6b93",
    "private_key_members_redacted": 2,
    "private_key_blocks_redacted": 15,
    "private_key_pem_blocks_remaining": 0,
}
BUNDLED_HELP_KNOWN_GAPS = [
    {
        "kind": "index_count_mismatch",
        "index_page_count": 12_037,
        "physical_page_count": 12_036,
    },
    {
        "kind": "missing_sequence",
        "group": "02-03-01",
        "sequence": "00223",
    },
    {
        "kind": "progress_inconsistent",
        "total_do_mapa": 12_037,
        "processadas": 7_077,
        "falhas": 1,
        "restantes": 0,
        "ultima_posicao": 12_037,
    },
]
BUNDLED_HELP_MAX_MEMBER = 1024 * 1024
BUNDLED_HELP_MAX_COMPRESSION_RATIO = 700
BUNDLED_HELP_MAX_CENTRAL_DIRECTORY = 16 * 1024 * 1024
BUNDLED_HELP_PAGE_NAME = re.compile(
    rf"^{re.escape(BUNDLED_HELP_ROOT)}/"
    r"(?P<group>\d{2}-\d{2}-\d{2})_(?P<sequence>\d{5})__.+\.json$"
)


class RunResult(str):
    """String-compatible status with the real immutable output directory."""

    output_dir: Path
    run_id: str
    ready_for: str | None

    def __new__(
        cls,
        status: str,
        output_dir: Path,
        run_id: str,
        ready_for: str | None,
    ) -> "RunResult":
        value = str.__new__(cls, status)
        value.output_dir = output_dir
        value.run_id = run_id
        value.ready_for = ready_for
        return value


def clean_text(value: object, limit: int = MAX_DISPLAY_LENGTH) -> str:
    if not isinstance(value, str):
        return ""
    rendered: list[str] = []
    length = 0
    for character in value:
        code = ord(character)
        if character == "\n":
            part = "\\n"
        elif character == "\r":
            part = "\\r"
        elif character == "\t":
            part = "\\t"
        elif code < 32 or code == 127 or 0xD800 <= code <= 0xDFFF:
            part = f"\\u{code:04x}"
        else:
            part = character
        rendered.append(part)
        length += len(part)
        if length >= limit:
            rendered.append("…")
            break
    return "".join(rendered)[: limit + 1]


def markdown_text(value: object) -> str:
    text = clean_text(value)
    for character in ("\\", chr(96), "*", "_", "[", "]", "<", ">", "#"):
        text = text.replace(character, "\\" + character)
    return text


def csv_safe(value: object) -> object:
    if not isinstance(value, str):
        return value
    if value and value[0] in {"=", "+", "-", "@", "\t", "\r"}:
        return "'" + value
    return value


class Audit:
    def __init__(self) -> None:
        self.errors: list[dict] = []
        self.warnings: list[dict] = []
        self.inventory: list[dict] = []
        self.secret_alerts: list[dict] = []
        self.help_identities: list[dict] = []
        self.help_count = 0
        self.help_identity_verified = False
        self.runtime_assessment: dict = {}
        self.acceptance_assessment: dict = {}
        self.total_artifact_bytes = 0

    def issue(
        self,
        level: str,
        code: str,
        message: str,
        group: str = "",
        item: str = "",
    ) -> None:
        record = {
            "code": clean_text(code, 80),
            "message": clean_text(message),
            "group": clean_text(group, 160),
            "item": clean_text(item),
        }
        (self.errors if level == "error" else self.warnings).append(record)

    def secret(self, group: str, display: str, finding_types: int) -> None:
        safe_group = clean_text(group, 160)
        safe_display = clean_text(display)
        for record in self.secret_alerts:
            if record["group"] == safe_group and record["path"] == safe_display:
                record["possible_finding_types"] = max(
                    int(record["possible_finding_types"]), finding_types
                )
                return
        self.secret_alerts.append({
            "group": safe_group,
            "path": safe_display,
            "possible_finding_types": finding_types,
        })
        self.issue(
            "error",
            "POSSIBLE_SECRET",
            "Possível segredo/credencial detectado; remova-o ou use apenas uma referência de segredo.",
            group,
            display,
        )

    def add_inventory(
        self,
        group: str,
        display_path: str,
        size: int | None,
        digest: str,
        kind: str,
        status: str = "readable",
        notes: str = "",
    ) -> None:
        if len(self.inventory) >= MAX_INVENTORY_ITEMS:
            self.issue(
                "error",
                "INVENTORY_ITEM_LIMIT",
                f"Inventário excede {MAX_INVENTORY_ITEMS} itens.",
                group,
                display_path,
            )
            return
        safe_path = clean_text(display_path)
        seed = digest or hashlib.sha256(safe_path.encode("utf-8")).hexdigest()
        self.inventory.append({
            "evidence_id": f"ART-{seed[:12].upper()}",
            "group": clean_text(group, 160),
            "path": safe_path,
            "kind": clean_text(kind, 80),
            "size_bytes": size if size is not None else "",
            "sha256": digest,
            "status": clean_text(status, 80),
            "notes": clean_text(notes),
        })

    def reserve_artifact_bytes(self, size: int, group: str, display: str) -> bool:
        if size < 0 or self.total_artifact_bytes + size > MAX_TOTAL_ARTIFACT_BYTES:
            self.issue(
                "error",
                "TOTAL_ARTIFACT_SIZE_LIMIT",
                f"Evidências excedem orçamento agregado de {MAX_TOTAL_ARTIFACT_BYTES} bytes.",
                group,
                display,
            )
            return False
        self.total_artifact_bytes += size
        return True


def _is_within(path: Path, root: Path) -> bool:
    try:
        path.relative_to(root)
        return True
    except ValueError:
        return False


def _contains_control(value: str) -> bool:
    return any(ord(character) < 32 or ord(character) == 127 for character in value)


def require_well_formed_unicode(value: str, label: str) -> None:
    """Reject isolated UTF-16 surrogate code points without echoing them."""
    if any(0xD800 <= ord(character) <= 0xDFFF for character in value):
        raise ValueError(f"{label}: texto Unicode contém surrogate isolado")


def normalized_relative_path(raw: object, label: str, allow_parent: bool = False) -> Path:
    if not isinstance(raw, str) or not raw.strip() or len(raw) > MAX_DISPLAY_LENGTH:
        raise ValueError(f"{label}: caminho relativo ausente")
    require_well_formed_unicode(raw, label)
    if "\\" in raw or "\x00" in raw or _contains_control(raw):
        raise ValueError(f"{label}: caminho contém caractere de controle")
    if raw != unicodedata.normalize("NFC", raw):
        raise ValueError(f"{label}: caminho não está normalizado em Unicode NFC")
    if allow_parent and raw == ".":
        return Path(".")
    raw_parts = raw.split("/")
    if allow_parent and raw_parts and raw_parts[0] == ".":
        raw_parts = raw_parts[1:]
    if (
        len(raw_parts) > MAX_PATH_DEPTH
        or any(part == "" or part == "." for part in raw_parts)
        or (not allow_parent and ".." in raw_parts)
    ):
        raise ValueError(f"{label}: caminho contém travessia ou componente vazio")
    for part in raw_parts:
        if part == ".." and allow_parent:
            continue
        if len(part) > MAX_COMPONENT_CHARS or part.endswith((" ", ".")) or ":" in part:
            raise ValueError(f"{label}: caminho contém componente não portátil")
        if part.split(".", 1)[0].upper() in WINDOWS_RESERVED_NAMES:
            raise ValueError(f"{label}: caminho usa nome reservado")
    posix = PurePosixPath(*raw_parts)
    windows = PureWindowsPath(raw)
    if posix.is_absolute() or windows.is_absolute() or windows.drive:
        raise ValueError(f"{label}: caminho absoluto não é permitido")
    return Path(*posix.parts)


def ensure_no_symlink_components(root: Path, lexical_path: Path, label: str) -> None:
    try:
        relative = lexical_path.relative_to(root)
    except ValueError as exc:
        raise ValueError(f"{label}: caminho fora da raiz permitida") from exc
    current = root
    for part in relative.parts:
        current = current / part
        try:
            metadata = os.lstat(current)
        except FileNotFoundError:
            continue
        if stat.S_ISLNK(metadata.st_mode):
            raise ValueError(f"{label}: symlink não permitido: {clean_text(str(current))}")


def ensure_absolute_no_symlink_components(lexical_path: Path, label: str) -> None:
    if not lexical_path.is_absolute():
        raise ValueError(f"{label}: caminho absoluto interno esperado")
    anchor = Path(lexical_path.anchor)
    ensure_no_symlink_components(anchor, lexical_path, label)


def resolve_workspace_member(
    workspace_root: Path,
    requested: Path,
    label: str,
    must_exist: bool = True,
) -> Path:
    lexical = Path(os.path.abspath(requested))
    if not _is_within(lexical, workspace_root):
        raise ValueError(f"{label}: caminho fora de workspace-root")
    ensure_no_symlink_components(workspace_root, lexical, label)
    resolved = lexical.resolve(strict=must_exist)
    if not _is_within(resolved, workspace_root):
        raise ValueError(f"{label}: caminho resolvido fora de workspace-root")
    return resolved


def resolve_evidence_item(
    root: Path,
    raw: object,
    audit: Audit,
    group: str,
) -> Path | None:
    try:
        relative = normalized_relative_path(raw, f"{group}.path")
        lexical = root.joinpath(relative)
        ensure_no_symlink_components(root, lexical, f"{group}.path")
        resolved = lexical.resolve(strict=False)
        if not _is_within(resolved, root):
            raise ValueError("caminho resolvido fora de evidence-root")
    except (OSError, ValueError) as exc:
        audit.issue("error", "PATH_ESCAPE", str(exc), group, clean_text(raw))
        return None
    return resolved


def open_binary_nofollow(path: Path):
    flags = os.O_RDONLY | getattr(os, "O_CLOEXEC", 0) | getattr(os, "O_NOFOLLOW", 0)
    descriptor = os.open(path, flags)
    return os.fdopen(descriptor, "rb")


def read_limited(path: Path, limit: int, label: str) -> bytes:
    with open_binary_nofollow(path) as handle:
        before = os.fstat(handle.fileno())
        if not stat.S_ISREG(before.st_mode):
            raise ValueError(f"{label}: não é arquivo regular")
        if before.st_size > limit:
            raise ValueError(f"{label}: excede limite de {limit} bytes")
        data = handle.read(limit + 1)
        after = os.fstat(handle.fileno())
    if len(data) > limit:
        raise ValueError(f"{label}: excede limite de {limit} bytes")
    if (before.st_dev, before.st_ino, before.st_size, before.st_mtime_ns) != (
        after.st_dev, after.st_ino, after.st_size, after.st_mtime_ns
    ) or len(data) != after.st_size:
        raise ValueError(f"{label}: arquivo mudou durante a leitura")
    return data


def _duplicate_object(pairs: list[tuple[str, object]]) -> dict:
    result: dict = {}
    for key, value in pairs:
        require_well_formed_unicode(key, "chave JSON")
        if key in result:
            raise ValueError(f"chave JSON duplicada: {clean_text(key, 160)}")
        result[key] = value
    return result


def _reject_json_constant(value: str) -> None:
    raise ValueError(f"constante JSON não padrão: {value}")


def _bounded_json_integer(value: str) -> int:
    digits = value.lstrip("-")
    if len(digits) > MAX_JSON_INTEGER_DIGITS:
        raise ValueError(
            f"inteiro JSON excede {MAX_JSON_INTEGER_DIGITS} dígitos"
        )
    return int(value)


def _bounded_json_float(value: str) -> float:
    if len(value) > MAX_JSON_INTEGER_DIGITS:
        raise ValueError(
            f"número JSON excede {MAX_JSON_INTEGER_DIGITS} caracteres"
        )
    parsed = float(value)
    if not math.isfinite(parsed):
        raise ValueError("número JSON não finito")
    return parsed


def validate_json_complexity(value: object, label: str) -> None:
    stack: list[tuple[object, int]] = [(value, 0)]
    nodes = 0
    while stack:
        node, depth = stack.pop()
        nodes += 1
        if nodes > MAX_JSON_NODES:
            raise ValueError(f"{label}: JSON excede {MAX_JSON_NODES} nós")
        if depth > MAX_JSON_DEPTH:
            raise ValueError(f"{label}: JSON excede profundidade {MAX_JSON_DEPTH}")
        if isinstance(node, str):
            require_well_formed_unicode(node, label)
            if len(node) > MAX_JSON_STRING:
                raise ValueError(f"{label}: string JSON excede {MAX_JSON_STRING} caracteres")
        elif isinstance(node, dict):
            for key, child in node.items():
                require_well_formed_unicode(key, f"{label}: chave JSON")
                if len(key) > 4096:
                    raise ValueError(f"{label}: chave JSON excessivamente longa")
                stack.append((child, depth + 1))
        elif isinstance(node, list):
            for child in node:
                stack.append((child, depth + 1))


def strict_json_loads(raw: bytes, label: str) -> object:
    try:
        text = raw.decode("utf-8-sig")
        value = json.loads(
            text,
            object_pairs_hook=_duplicate_object,
            parse_constant=_reject_json_constant,
            parse_int=_bounded_json_integer,
            parse_float=_bounded_json_float,
        )
    except (UnicodeError, json.JSONDecodeError, ValueError, RecursionError) as exc:
        raise ValueError(f"{label}: JSON estrito inválido: {exc}") from exc
    validate_json_complexity(value, label)
    return value


def load_json(path: Path, label: str, limit: int) -> dict:
    value = strict_json_loads(read_limited(path, limit, label), label)
    if not isinstance(value, dict):
        raise ValueError(f"{label}: raiz precisa ser objeto JSON")
    return value


def load_json_with_hash(path: Path, label: str, limit: int) -> tuple[dict, str]:
    raw = read_limited(path, limit, label)
    value = strict_json_loads(raw, label)
    if not isinstance(value, dict):
        raise ValueError(f"{label}: raiz precisa ser objeto JSON")
    return value, hashlib.sha256(raw).hexdigest()


def sensitive_json_value_count(value: object) -> int:
    """Count sensitive-looking populated keys without retaining values."""
    count = 0
    stack: list[object] = [value]
    visited = 0
    while stack and visited < MAX_JSON_NODES:
        node = stack.pop()
        visited += 1
        if isinstance(node, dict):
            for key, child in node.items():
                if SENSITIVE_QUERY_KEY.search(str(key)):
                    if isinstance(child, str):
                        rendered = child.strip().lower()
                        if rendered and not any(marker in rendered for marker in SECRET_PLACEHOLDERS):
                            count += 1
                    elif child not in (None, False, [], {}):
                        count += 1
                stack.append(child)
        elif isinstance(node, list):
            stack.extend(node)
    return count


def is_text_candidate(path: Path) -> bool:
    name = path.name.lower()
    return path.suffix.lower() in TEXT_EXTENSIONS or name == ".env" or name.startswith(".env.")


def secret_matches(text: str) -> set[int]:
    matches: set[int] = set()
    for index, pattern in enumerate(SECRET_PATTERNS):
        for match in pattern.finditer(text):
            rendered = match.group(0).lower()
            if any(placeholder in rendered for placeholder in SECRET_PLACEHOLDERS):
                continue
            matches.add(index)
            break
    return matches


class StreamingSecretScanner:
    def __init__(self) -> None:
        self.decoder = codecs.getincrementaldecoder("utf-8-sig")(errors="ignore")
        self.tail = ""
        self.matches: set[int] = set()

    def feed(self, block: bytes) -> None:
        text = self.tail + self.decoder.decode(block)
        self.matches.update(secret_matches(text))
        self.tail = text[-SECRET_SCAN_OVERLAP:]

    def finish(self) -> set[int]:
        text = self.tail + self.decoder.decode(b"", final=True)
        self.matches.update(secret_matches(text))
        return self.matches


def scan_secret_bytes(data: bytes) -> set[int]:
    scanner = StreamingSecretScanner()
    for start in range(0, len(data), READ_BLOCK):
        scanner.feed(data[start : start + READ_BLOCK])
    return scanner.finish()


def stream_digest_and_scan(
    path: Path,
    limit: int,
    scan_text: bool,
    audit: Audit,
    group: str,
    display: str,
) -> tuple[str, int, bytes] | None:
    try:
        digest = hashlib.sha256()
        head = b""
        total = 0
        scanner = StreamingSecretScanner() if scan_text else None
        with open_binary_nofollow(path) as handle:
            before = os.fstat(handle.fileno())
            if not stat.S_ISREG(before.st_mode):
                raise ValueError("não é arquivo regular")
            if before.st_size > limit:
                raise ValueError(f"arquivo excede limite de {limit} bytes")
            while True:
                block = handle.read(READ_BLOCK)
                if not block:
                    break
                total += len(block)
                if total > limit:
                    raise ValueError(f"arquivo excede limite de {limit} bytes")
                if len(head) < 16:
                    head = (head + block)[:16]
                digest.update(block)
                if scanner:
                    scanner.feed(block)
            after = os.fstat(handle.fileno())
        identity_before = (before.st_dev, before.st_ino, before.st_size, before.st_mtime_ns)
        identity_after = (after.st_dev, after.st_ino, after.st_size, after.st_mtime_ns)
        if identity_before != identity_after or total != after.st_size:
            audit.issue("error", "FILE_CHANGED", "Arquivo mudou durante a auditoria.", group, display)
        if scanner:
            findings = scanner.finish()
            if findings:
                audit.secret(group, display, len(findings))
        return digest.hexdigest(), total, head
    except (OSError, ValueError) as exc:
        audit.issue("error", "FILE_READ_ERROR", str(exc), group, display)
        return None


def valid_signature(head: bytes, size: int, path: Path, group: str) -> tuple[bool, str]:
    suffix = path.suffix.lower()
    if suffix == ".pdf" and not head.startswith(b"%PDF-"):
        return False, "assinatura PDF ausente"
    if suffix == ".png" and not head.startswith(b"\x89PNG\r\n\x1a\n"):
        return False, "assinatura PNG ausente"
    if suffix in {".jpg", ".jpeg"} and not head.startswith(b"\xff\xd8\xff"):
        return False, "assinatura JPEG ausente"
    if suffix == ".gif" and head[:6] not in {b"GIF87a", b"GIF89a"}:
        return False, "assinatura GIF ausente"
    if suffix == ".webp" and not (head.startswith(b"RIFF") and head[8:12] == b"WEBP"):
        return False, "assinatura WEBP ausente"
    if group == "sql_scripts" and size == 0:
        return False, "script SQL vazio"
    return True, ""


def _tail_bytes(path: Path, count: int) -> bytes:
    with open_binary_nofollow(path) as handle:
        handle.seek(0, os.SEEK_END)
        size = handle.tell()
        handle.seek(max(0, size - count), os.SEEK_SET)
        return handle.read(count)


def image_dimensions(path: Path, size: int) -> tuple[int, int]:
    """Return dimensions after bounded header parsing and trailer checks."""
    with open_binary_nofollow(path) as handle:
        data = handle.read(min(size, 4 * 1024 * 1024))
    suffix = path.suffix.lower()
    width = height = 0
    if suffix == ".png":
        if (
            len(data) < 33
            or data[8:12] != b"\x00\x00\x00\x0d"
            or data[12:16] != b"IHDR"
        ):
            raise ValueError("PNG sem IHDR completo")
        width, height = struct.unpack(">II", data[16:24])
        expected_crc = struct.unpack(">I", data[29:33])[0]
        if zlib.crc32(data[12:29]) & 0xFFFFFFFF != expected_crc:
            raise ValueError("CRC do IHDR PNG inválido")
        bit_depth, color_type, compression, filtering, interlace = data[24:29]
        allowed_depths = {
            0: {1, 2, 4, 8, 16},
            2: {8, 16},
            3: {1, 2, 4, 8},
            4: {8, 16},
            6: {8, 16},
        }
        if (
            color_type not in allowed_depths
            or bit_depth not in allowed_depths[color_type]
            or compression != 0
            or filtering != 0
            or interlace not in {0, 1}
        ):
            raise ValueError("parâmetros IHDR PNG inválidos")
        if b"IEND" not in _tail_bytes(path, 64):
            raise ValueError("PNG sem terminador IEND")
    elif suffix == ".gif":
        if len(data) < 10:
            raise ValueError("GIF sem descritor lógico completo")
        width, height = struct.unpack("<HH", data[6:10])
        if len(data) < 13:
            raise ValueError("GIF sem logical screen descriptor")
        if data[10] & 0x80:
            table_size = 3 * (2 ** ((data[10] & 0x07) + 1))
            if size < 13 + table_size + 1:
                raise ValueError("tabela global de cores GIF truncada")
        if not _tail_bytes(path, 16).rstrip(b"\x00\r\n\t ").endswith(b";"):
            raise ValueError("GIF sem trailer")
    elif suffix in {".jpg", ".jpeg"}:
        if not data.startswith(b"\xff\xd8"):
            raise ValueError("JPEG sem SOI")
        offset = 2
        sof_markers = {
            0xC0, 0xC1, 0xC2, 0xC3, 0xC5, 0xC6, 0xC7,
            0xC9, 0xCA, 0xCB, 0xCD, 0xCE, 0xCF,
        }
        segments = 0
        while offset + 3 < len(data):
            if data[offset] != 0xFF:
                offset += 1
                continue
            while offset < len(data) and data[offset] == 0xFF:
                offset += 1
            if offset >= len(data):
                break
            marker = data[offset]
            offset += 1
            segments += 1
            if segments > MAX_JPEG_SEGMENTS:
                raise ValueError("JPEG excede limite de segmentos no probe")
            if marker in {0x01, *range(0xD0, 0xDA)}:
                continue
            if marker == 0xDA:
                break
            if offset + 2 > len(data):
                break
            segment_length = struct.unpack(">H", data[offset : offset + 2])[0]
            if segment_length < 2 or offset + segment_length > len(data):
                break
            if marker in sof_markers and segment_length >= 7:
                height, width = struct.unpack(">HH", data[offset + 3 : offset + 7])
                break
            offset += segment_length
        if not _tail_bytes(path, 64).rstrip(b"\x00\r\n\t ").endswith(b"\xff\xd9"):
            raise ValueError("JPEG sem EOI")
    elif suffix == ".webp":
        if len(data) < 30 or data[:4] != b"RIFF" or data[8:12] != b"WEBP":
            raise ValueError("WebP sem cabeçalho RIFF/WEBP completo")
        declared_size = struct.unpack("<I", data[4:8])[0] + 8
        if declared_size != size:
            raise ValueError("tamanho RIFF do WebP diverge do arquivo")
        chunk = data[12:16]
        chunk_size = struct.unpack("<I", data[16:20])[0]
        if 20 + chunk_size > size or chunk_size < 1:
            raise ValueError("primeiro chunk WebP truncado ou inválido")
        if chunk == b"VP8X":
            width = 1 + int.from_bytes(data[24:27], "little")
            height = 1 + int.from_bytes(data[27:30], "little")
        elif chunk == b"VP8L" and len(data) >= 25 and data[20] == 0x2F:
            b0, b1, b2, b3 = data[21:25]
            width = 1 + b0 + ((b1 & 0x3F) << 8)
            height = 1 + ((b1 >> 6) | (b2 << 2) | ((b3 & 0x0F) << 10))
        elif chunk == b"VP8 " and len(data) >= 30 and data[23:26] == b"\x9d\x01\x2a":
            width = struct.unpack("<H", data[26:28])[0] & 0x3FFF
            height = struct.unpack("<H", data[28:30])[0] & 0x3FFF
        else:
            raise ValueError("subtipo WebP não reconhecido")
    if width <= 0 or height <= 0:
        raise ValueError("dimensões ausentes ou inválidas")
    if (
        width > MAX_IMAGE_DIMENSION
        or height > MAX_IMAGE_DIMENSION
        or width * height > MAX_IMAGE_PIXELS
    ):
        raise ValueError("dimensões excedem limite seguro")
    return width, height


def validate_sql_declaration(item: dict, audit: Audit, display: str) -> str | None:
    valid = True
    for field in (
        "dialect", "database_version", "encoding", "collation", "charset", "timezone"
    ):
        value = item.get(field)
        if not isinstance(value, str) or not value.strip():
            audit.issue(
                "error",
                "SQL_METADATA_MISSING",
                f"Item SQL exige {field} como string não vazia.",
                "sql_scripts",
                display,
            )
            valid = False
        elif len(value) > 256 or _contains_control(value):
            audit.issue(
                "error", "SQL_METADATA_TYPE",
                f"{field} contém valor longo demais ou controles.",
                "sql_scripts", display,
            )
            valid = False
    if not valid:
        return None
    encoding = item["encoding"].strip().lower().replace("_", "-")
    aliases = {
        "utf8": "utf-8",
        "utf-8": "utf-8",
        "utf-8-sig": "utf-8-sig",
        "utf16": "utf-16",
        "utf-16": "utf-16",
        "utf-16le": "utf-16-le",
        "utf-16-le": "utf-16-le",
        "utf-16be": "utf-16-be",
        "utf-16-be": "utf-16-be",
        "iso-8859-1": "iso-8859-1",
        "latin-1": "iso-8859-1",
        "cp1252": "cp1252",
        "windows-1252": "cp1252",
    }
    codec = aliases.get(encoding)
    if codec is None:
        audit.issue(
            "error", "SQL_ENCODING_UNSUPPORTED",
            "Encoding SQL não pertence à allowlist portátil.",
            "sql_scripts", display,
        )
    return codec


def inspect_sql_text(path: Path, codec: str) -> tuple[bool, bool, set[int]]:
    decoder = codecs.getincrementaldecoder(codec)(errors="strict")
    state = "normal"
    candidate = ""
    block_star = False
    block_first = False
    meaningful = False
    findings: set[int] = set()
    secret_tail = ""

    def consume(text: str) -> None:
        nonlocal state, candidate, block_star, block_first, meaningful, secret_tail
        scanned = secret_tail + text
        findings.update(secret_matches(scanned))
        secret_tail = scanned[-SECRET_SCAN_OVERLAP:]
        for character in text:
            if meaningful:
                continue
            if state == "line":
                if character in "\r\n":
                    state = "normal"
                continue
            if state == "block":
                if block_first:
                    block_first = False
                    if character == "!":
                        meaningful = True
                        continue
                if block_star and character == "/":
                    state = "normal"
                    block_star = False
                else:
                    block_star = character == "*"
                continue
            if candidate:
                previous = candidate
                candidate = ""
                if previous == "-" and character == "-":
                    state = "line"
                    continue
                if previous == "/" and character == "*":
                    state = "block"
                    block_first = True
                    block_star = False
                    continue
                meaningful = True
                continue
            if character.isspace() or character == ";":
                continue
            if character in {"-", "/"}:
                candidate = character
            else:
                meaningful = True

    with open_binary_nofollow(path) as handle:
        while True:
            block = handle.read(READ_BLOCK)
            if not block:
                break
            consume(decoder.decode(block))
    consume(decoder.decode(b"", final=True))
    if candidate:
        meaningful = True
    return meaningful, state == "block", findings


def inspect_pdf(path: Path, item: dict, audit: Audit, group: str, display: str) -> str:
    declared_pages = item.get("page_count")
    if (
        not isinstance(declared_pages, int)
        or isinstance(declared_pages, bool)
        or not 1 <= declared_pages <= MAX_PDF_DECLARED_PAGES
    ):
        audit.issue(
            "error", "PDF_METADATA_MISSING",
            f"PDF exige page_count inteiro entre 1 e {MAX_PDF_DECLARED_PAGES}.", group, display,
        )
        declared_pages = None
    searchable = item.get("searchable")
    if not isinstance(searchable, bool):
        audit.issue(
            "error", "PDF_METADATA_MISSING",
            "PDF exige searchable booleano.", group, display,
        )
        searchable = None
    content_scope = item.get("content_scope")
    allowed_scope = {
        "code", "events", "ui", "queries", "business_rules", "reports", "integrations"
    }
    if (
        not isinstance(content_scope, list)
        or not content_scope
        or any(not isinstance(value, str) or value not in allowed_scope for value in content_scope)
        or len(set(content_scope)) != len(content_scope)
    ):
        audit.issue(
            "error", "PDF_METADATA_MISSING",
            "PDF exige content_scope como lista não vazia de escopos reconhecidos e únicos.",
            group, display,
        )

    marker = re.compile(rb"/Type\s*/Page(?!s)\b")
    text_hint = re.compile(rb"(?:/ToUnicode\b|/Font\b|\bBT\s)")
    page_markers = 0
    searchable_hint = False
    carry = b""
    tail = b""
    before = os.stat(path, follow_symlinks=False)
    prefix = b""
    with open_binary_nofollow(path) as handle:
        while True:
            block = handle.read(READ_BLOCK)
            if not block:
                break
            if len(prefix) < 2 * 1024 * 1024:
                prefix = (prefix + block)[: 2 * 1024 * 1024]
            combined = carry + block
            cutoff = len(carry)
            page_markers += sum(1 for match in marker.finditer(combined) if match.end() > cutoff)
            if not searchable_hint and text_hint.search(combined):
                searchable_hint = True
            carry = combined[-64:]
            tail = (tail + block)[-4096:]
    after = os.stat(path, follow_symlinks=False)
    if (before.st_dev, before.st_ino, before.st_size, before.st_mtime_ns) != (
        after.st_dev, after.st_ino, after.st_size, after.st_mtime_ns
    ):
        audit.issue("error", "FILE_CHANGED", "PDF mudou durante a inspeção.", group, display)
    if not re.match(rb"%PDF-(?:1\.[0-7]|2\.0)(?:\r|\n)", prefix):
        audit.issue("error", "INVALID_SIGNATURE", "Versão/cabeçalho PDF inválido.", group, display)
    if not re.search(rb"\b\d+\s+\d+\s+obj\b", prefix):
        audit.issue(
            "error", "CONTENT_UNVERIFIED",
            "PDF não apresenta objeto estrutural no probe inicial.", group, display,
        )
    trimmed_tail = tail.rstrip(b"\x00\t\n\r\f ")
    if not trimmed_tail.endswith(b"%%EOF"):
        audit.issue("error", "INVALID_SIGNATURE", "PDF sem %%EOF terminal.", group, display)
    startxref_matches = list(re.finditer(rb"startxref\s+(\d+)", tail))
    if not startxref_matches:
        audit.issue("error", "INVALID_SIGNATURE", "PDF sem startxref no trailer.", group, display)
    else:
        startxref = int(startxref_matches[-1].group(1))
        if startxref >= before.st_size:
            audit.issue("error", "INVALID_SIGNATURE", "startxref aponta para fora do PDF.", group, display)
    if declared_pages is not None:
        if page_markers and page_markers != declared_pages:
            audit.issue(
                "warning", "CONTENT_UNVERIFIED",
                f"page_count declarado ({declared_pages}) diverge dos marcadores heurísticos ({page_markers}); requer parser PDF.",
                group, display,
            )
        elif not page_markers:
            audit.issue(
                "warning", "CONTENT_UNVERIFIED",
                "Contagem de páginas não pôde ser comprovada sem um parser PDF completo.",
                group, display,
            )
    if searchable is False:
        audit.issue(
            "warning", "OCR_REQUIRED",
            "PDF declarado como não pesquisável; OCR e revisão são necessários.",
            group, display,
        )
    elif searchable is True and not searchable_hint:
        audit.issue(
            "warning", "CONTENT_UNVERIFIED",
            "Searchability declarada não pôde ser comprovada estruturalmente.",
            group, display,
        )
    required_scope = {
        "code_documents": "code",
        "ui_documents": "ui",
        "query_documents": "queries",
        "business_rule_documents": "business_rules",
    }.get(group)
    if required_scope and isinstance(content_scope, list) and required_scope not in content_scope:
        audit.issue(
            "error", "PDF_SCOPE_MISMATCH",
            f"content_scope precisa incluir {required_scope!r} para {group}.",
            group, display,
        )
    return (
        f"declared_pages={declared_pages or ''}; heuristic_page_markers={page_markers}; "
        f"declared_searchable={searchable}"
    )


def audit_regular_file(
    root: Path,
    group: str,
    item: dict,
    audit: Audit,
    *,
    max_size: int = MAX_ARTIFACT_SIZE,
    scan_secrets: bool = True,
) -> tuple[Path | None, str]:
    raw = item.get("path") if isinstance(item, dict) else None
    path = resolve_evidence_item(root, raw, audit, group)
    if path is None:
        return None, ""
    if path.exists() and path.is_dir():
        audit.issue(
            "error",
            "DIR_INVENTORY_REQUIRED",
            "Diretório declarado precisa de inventário recursivo seguro; não foi lido.",
            group,
            clean_text(raw),
        )
        return None, ""
    if not path.exists() or not path.is_file():
        audit.issue(
            "error",
            "FILE_NOT_FOUND",
            "Arquivo não encontrado ou não é arquivo regular.",
            group,
            clean_text(raw),
        )
        return None, ""

    try:
        candidate_size = path.stat().st_size
    except OSError as exc:
        audit.issue("error", "FILE_READ_ERROR", str(exc), group, clean_text(raw))
        return None, ""
    if not audit.reserve_artifact_bytes(candidate_size, group, clean_text(raw)):
        return None, ""

    allowed = EXPECTED_EXTENSIONS.get(group)
    if allowed and path.suffix.lower() not in allowed:
        audit.issue(
            "error",
            "UNEXPECTED_EXTENSION",
            f"Extensão {path.suffix or '(sem extensão)'} não aceita; esperado: {', '.join(sorted(allowed))}.",
            group,
            clean_text(raw),
        )

    display = clean_text(path.relative_to(root).as_posix())
    result = stream_digest_and_scan(
        path,
        max_size,
        scan_secrets and is_text_candidate(path),
        audit,
        group,
        display,
    )
    if result is None:
        return None, ""
    digest, size, head = result
    ok, reason = valid_signature(head, size, path, group)
    if not ok:
        audit.issue("error", "INVALID_SIGNATURE", reason, group, display)
    notes = ""
    if group == "sql_scripts":
        codec = validate_sql_declaration(item, audit, display)
        if codec is not None:
            try:
                meaningful, unterminated_comment, sql_findings = inspect_sql_text(path, codec)
                if not meaningful:
                    audit.issue(
                        "error", "SQL_EMPTY",
                        "Script SQL não contém token executável além de comentários/separadores.",
                        group, display,
                    )
                if unterminated_comment:
                    audit.issue(
                        "error", "SQL_UNTERMINATED_COMMENT",
                        "Script SQL termina dentro de comentário de bloco.",
                        group, display,
                    )
                if sql_findings:
                    audit.secret(group, display, len(sql_findings))
                notes = f"declared_encoding={codec}"
            except (LookupError, UnicodeError, OSError) as exc:
                audit.issue(
                    "error", "SQL_ENCODING_INVALID",
                    f"Conteúdo SQL não decodifica estritamente no encoding declarado: {exc}",
                    group, display,
                )
    if path.suffix.lower() == ".json" and group != "wlanguage_help_json":
        try:
            json_value = strict_json_loads(
                read_limited(path, min(max_size, MAX_HELP_MEMBER), display),
                display,
            )
            structured_findings = sensitive_json_value_count(json_value)
            if structured_findings:
                audit.secret(group, display, structured_findings)
        except (OSError, ValueError) as exc:
            audit.issue("error", "INVALID_JSON", str(exc), group, display)
    if path.suffix.lower() in {".png", ".gif", ".jpg", ".jpeg", ".webp"}:
        try:
            width, height = image_dimensions(path, size)
            notes = f"dimensions={width}x{height}"
            for field, observed in (("width_px", width), ("height_px", height)):
                declared = item.get(field)
                if declared is not None and (
                    not isinstance(declared, int)
                    or isinstance(declared, bool)
                    or declared < 1
                ):
                    audit.issue(
                        "error", "IMAGE_METADATA_TYPE",
                        f"{field} precisa ser inteiro positivo.", group, display,
                    )
                elif isinstance(declared, int) and declared != observed:
                    audit.issue(
                        "error", "IMAGE_DIMENSIONS_MISMATCH",
                        f"{field} declarado ({declared}) diverge do valor físico ({observed}).",
                        group, display,
                    )
            if group == "screenshots":
                for field in ("screen_or_report", "state", "platform"):
                    value = item.get(field)
                    if not isinstance(value, str) or not value.strip():
                        audit.issue(
                            "error", "SCREENSHOT_METADATA_MISSING",
                            f"Screenshot exige {field} como string não vazia.",
                            group, display,
                        )
        except (OSError, ValueError, struct.error) as exc:
            audit.issue(
                "error",
                "IMAGE_DIMENSIONS_UNVERIFIED",
                f"Dimensões/estrutura da imagem não puderam ser verificadas: {exc}",
                group,
                display,
            )
    elif path.suffix.lower() == ".pdf":
        try:
            notes = inspect_pdf(path, item, audit, group, display)
        except OSError as exc:
            audit.issue("error", "PDF_READ_ERROR", str(exc), group, display)
    audit.add_inventory(
        group,
        display,
        size,
        digest,
        path.suffix.lower().lstrip(".") or "file",
        notes=notes,
    )
    return path, digest


def scalar_metadata(value: object, keys: tuple[str, ...]) -> str:
    wanted = {key.casefold() for key in keys}
    queue: deque[object] = deque([value])
    visited = 0
    while queue and visited < MAX_METADATA_SCAN_NODES:
        node = queue.popleft()
        visited += 1
        if isinstance(node, dict):
            lowered = {str(key).casefold(): child for key, child in node.items()}
            for key in wanted:
                child = lowered.get(key)
                if isinstance(child, str) and child.strip():
                    return child.strip()
                if isinstance(child, (int, float)) and not isinstance(child, bool):
                    return str(child)
            queue.extend(node.values())
        elif isinstance(node, list):
            queue.extend(node)
    return ""


def declared_string(
    item: dict,
    keys: tuple[str, ...],
    label: str,
    audit: Audit,
    source: str,
) -> str:
    for key in keys:
        if key not in item:
            continue
        value = item.get(key)
        if isinstance(value, str) and value.strip():
            return value.strip()
        audit.issue("error", "HELP_METADATA_TYPE", f"{label} precisa ser string não vazia.", "wlanguage_help_json", source)
        return ""
    return ""


def assess_help_document(
    data: bytes,
    source: str,
    declaration: dict,
    expected_version: str,
    expected_language: str,
    expected_products: set[str],
    audit: Audit,
) -> tuple[str, str, bool] | None:
    findings = scan_secret_bytes(data)
    if findings:
        audit.secret("wlanguage_help_json", source, len(findings))
    try:
        value = strict_json_loads(data, source)
    except ValueError as exc:
        audit.issue("error", "INVALID_HELP_JSON", str(exc), "wlanguage_help_json", source)
        return None
    if not isinstance(value, (dict, list)) or not value:
        audit.issue(
            "error",
            "EMPTY_HELP_JSON",
            "Documento do Help precisa ser objeto/lista JSON não vazio.",
            "wlanguage_help_json",
            source,
        )
        return None
    structured_findings = sensitive_json_value_count(value)
    if structured_findings:
        audit.secret("wlanguage_help_json", source, structured_findings)

    canonical = json.dumps(
        value,
        ensure_ascii=False,
        sort_keys=True,
        separators=(",", ":"),
        allow_nan=False,
    ).encode("utf-8")
    canonical_digest = hashlib.sha256(canonical).hexdigest()

    declared_identity = declared_string(
        declaration,
        ("identity", "document_id", "help_id"),
        "identity",
        audit,
        source,
    )
    identity = declared_identity or scalar_metadata(value, IDENTITY_KEYS)
    identity_verified = bool(identity)
    if not identity:
        identity = f"canonical:{canonical_digest[:16]}"
        audit.issue(
            "warning",
            "HELP_IDENTITY_UNVERIFIED",
            "Não foi possível confirmar uma identidade semântica do documento.",
            "wlanguage_help_json",
            source,
        )

    declared_version = declared_string(
        declaration,
        ("version", "help_version"),
        "version",
        audit,
        source,
    )
    observed_version = scalar_metadata(value, VERSION_KEYS)
    version = observed_version or declared_version
    version_verified = bool(version and expected_version)
    if not version:
        audit.issue(
            "warning",
            "HELP_VERSION_UNVERIFIED",
            "Versão do documento não aparece no JSON nem nos metadados do item.",
            "wlanguage_help_json",
            source,
        )
    elif expected_version and version.casefold() != expected_version.casefold():
        version_verified = False
        audit.issue(
            "error",
            "HELP_VERSION_MISMATCH",
            f"Versão declarada/observada {clean_text(version, 120)!r} difere da versão esperada.",
            "wlanguage_help_json",
            source,
        )

    declared_language = declared_string(
        declaration,
        ("language", "lang", "locale"),
        "language",
        audit,
        source,
    )
    observed_language = scalar_metadata(value, LANGUAGE_KEYS)
    language = observed_language or declared_language
    language_verified = bool(language and expected_language)
    if not language:
        audit.issue(
            "warning",
            "HELP_LANGUAGE_UNVERIFIED",
            "Idioma do documento não aparece no JSON nem nos metadados do item.",
            "wlanguage_help_json",
            source,
        )
    elif expected_language and language.casefold() != expected_language.casefold():
        language_verified = False
        audit.issue(
            "error",
            "HELP_LANGUAGE_MISMATCH",
            f"Idioma declarado/observado {clean_text(language, 120)!r} difere do idioma esperado.",
            "wlanguage_help_json",
            source,
        )

    declared_product = declared_string(
        declaration,
        ("product",),
        "product",
        audit,
        source,
    )
    observed_product = scalar_metadata(value, PRODUCT_KEYS)
    product = observed_product or declared_product
    product_verified = False
    if not product:
        audit.issue(
            "warning",
            "HELP_PRODUCT_UNVERIFIED",
            "Produto WX do documento não aparece no JSON nem nos metadados do item.",
            "wlanguage_help_json",
            source,
        )
    elif expected_products and product not in expected_products:
        audit.issue(
            "error",
            "HELP_PRODUCT_MISMATCH",
            f"Produto declarado/observado {clean_text(product, 120)!r} não pertence ao projeto.",
            "wlanguage_help_json",
            source,
        )
    else:
        product_verified = True

    verified = identity_verified and version_verified and language_verified and product_verified
    audit.help_identities.append({
        "source": clean_text(source),
        "identity": clean_text(identity, 240),
        "canonical_sha256": canonical_digest,
        "version": clean_text(version, 120),
        "language": clean_text(language, 120),
        "product": clean_text(product, 120),
        "verified": verified,
    })
    return canonical_digest, clean_text(identity, 240), verified


def safe_zip_member(name: str) -> bool:
    if (
        not name
        or len(name) > MAX_DISPLAY_LENGTH
        or "\x00" in name
        or _contains_control(name)
        or "\\" in name
    ):
        return False
    try:
        require_well_formed_unicode(name, "nome de membro ZIP")
    except ValueError:
        return False
    if name != unicodedata.normalize("NFC", name):
        return False
    raw_parts = name.split("/")
    if len(raw_parts) > MAX_PATH_DEPTH or any(part in {"", ".", ".."} for part in raw_parts):
        return False
    for part in raw_parts:
        if len(part) > MAX_COMPONENT_CHARS or part.endswith((" ", ".")) or ":" in part:
            return False
        if part.split(".", 1)[0].upper() in WINDOWS_RESERVED_NAMES:
            return False
    posix = PurePosixPath(name)
    windows = PureWindowsPath(name)
    return (
        not posix.is_absolute()
        and not windows.is_absolute()
        and not windows.drive
        and ".." not in posix.parts
    )


def zip_member_is_symlink(info: zipfile.ZipInfo) -> bool:
    mode = (info.external_attr >> 16) & 0o170000
    return mode == stat.S_IFLNK


def preflight_zip_directory(path: Path) -> None:
    """Bound central-directory allocation and reject multi-disk/ZIP64 archives."""
    size = path.stat().st_size
    with path.open("rb") as handle:
        handle.seek(max(0, size - 65_557), os.SEEK_SET)
        tail = handle.read(65_557)
    offset = tail.rfind(b"PK\x05\x06")
    if offset < 0 or offset + 22 > len(tail):
        raise ValueError("EOCD ZIP ausente ou truncado")
    (
        signature,
        disk_number,
        directory_disk,
        entries_on_disk,
        total_entries,
        directory_size,
        directory_offset,
        comment_length,
    ) = struct.unpack("<4s4H2LH", tail[offset : offset + 22])
    if signature != b"PK\x05\x06":
        raise ValueError("assinatura EOCD inválida")
    if disk_number or directory_disk or entries_on_disk != total_entries:
        raise ValueError("ZIP multidisco não é aceito")
    if total_entries in {0xFFFF} or directory_size == 0xFFFFFFFF or directory_offset == 0xFFFFFFFF:
        raise ValueError("ZIP64 não é aceito para o bundle limitado do Help")
    if total_entries > MAX_ZIP_MEMBERS:
        raise ValueError(f"ZIP excede {MAX_ZIP_MEMBERS} entradas")
    if directory_size > 16 * 1024 * 1024:
        raise ValueError("diretório central ZIP excede 16 MiB")
    if directory_offset + directory_size > size:
        raise ValueError("diretório central ZIP aponta para fora do arquivo")
    if offset + 22 + comment_length != len(tail):
        raise ValueError("comentário/trailer ZIP inconsistente")


def read_zip_member(bundle: zipfile.ZipFile, info: zipfile.ZipInfo) -> bytes:
    with bundle.open(info, "r") as source:
        data = bytearray()
        while True:
            block = source.read(min(READ_BLOCK, MAX_HELP_MEMBER + 1 - len(data)))
            if not block:
                break
            data.extend(block)
            if len(data) > MAX_HELP_MEMBER:
                raise ValueError("membro excede limite durante descompactação")
    return bytes(data)


def normalized_help_member(value: str) -> str:
    require_well_formed_unicode(value, "nome de membro do Help")
    rendered = unicodedata.normalize("NFC", value.replace("\\", "/"))
    return rendered.casefold()


def bundled_help_declaration_is_valid(
    group_data: dict,
    audit: Audit,
    *,
    expected_version: str,
    expected_language: str,
    expected_products: set[str],
) -> bool:
    """Validate the pinned metadata; no manifest field selects the archive path."""
    errors_before = len(audit.errors)
    allowed_fields = {
        "status",
        "notes",
        "corpus_id",
        "sha256",
        "expected_json_count",
        "expected_page_count",
        "valid_page_count",
        "language",
        "version_coverage",
        "product_scope",
        "known_invalid_members",
        "known_gaps",
        "sanitization",
        "archive",
        "items",
    }
    missing_fields = sorted(allowed_fields - set(group_data))
    if missing_fields:
        audit.issue(
            "error",
            "HELP_CORPUS_DECLARATION",
            "Declaração bundled não contém todos os campos obrigatórios: "
            + ", ".join(missing_fields),
            "wlanguage_help_json",
        )
    unknown_fields = sorted(set(group_data) - allowed_fields)
    if unknown_fields:
        audit.issue(
            "error",
            "HELP_CORPUS_DECLARATION",
            "Declaração bundled contém campos não permitidos: "
            + ", ".join(clean_text(field, 80) for field in unknown_fields),
            "wlanguage_help_json",
        )
    notes = group_data.get("notes")
    if not isinstance(notes, str):
        audit.issue(
            "error",
            "HELP_CORPUS_DECLARATION",
            "notes precisa ser string no contrato bundled.",
            "wlanguage_help_json",
        )

    expected_fields: tuple[tuple[str, object], ...] = (
        ("status", "bundled"),
        ("corpus_id", BUNDLED_HELP_CORPUS_ID),
        ("sha256", BUNDLED_HELP_SHA256),
        ("expected_json_count", BUNDLED_HELP_JSON_COUNT),
        ("expected_page_count", BUNDLED_HELP_PAGE_COUNT),
        ("valid_page_count", BUNDLED_HELP_VALID_PAGE_COUNT),
        ("language", BUNDLED_HELP_LANGUAGE),
        ("version_coverage", BUNDLED_HELP_VERSION_COVERAGE),
        ("product_scope", BUNDLED_HELP_PRODUCT_SCOPE),
        ("known_invalid_members", [BUNDLED_HELP_INVALID_MEMBER]),
        ("known_gaps", BUNDLED_HELP_KNOWN_GAPS),
        ("sanitization", BUNDLED_HELP_SANITIZATION),
        ("archive", None),
        ("items", []),
    )
    for field, expected in expected_fields:
        value = group_data.get(field)
        same_type = type(value) is type(expected)
        if not same_type or value != expected:
            audit.issue(
                "error",
                "HELP_CORPUS_DECLARATION",
                f"{field} diverge do contrato fixo do corpus bundled.",
                "wlanguage_help_json",
            )

    if expected_language and expected_language.casefold() != BUNDLED_HELP_LANGUAGE.casefold():
        audit.issue(
            "error",
            "HELP_GROUP_LANGUAGE_MISMATCH",
            "project.wlanguage_help_language precisa ser en-US para o corpus bundled.",
            "wlanguage_help_json",
        )
    if expected_version and expected_version not in BUNDLED_HELP_VERSION_COVERAGE:
        audit.issue(
            "error",
            "HELP_CORPUS_VERSION_COVERAGE",
            "project.wlanguage_help_version não pertence à cobertura de releases do corpus bundled.",
            "wlanguage_help_json",
        )
    if expected_products and not expected_products.issubset(set(BUNDLED_HELP_PRODUCT_SCOPE)):
        audit.issue(
            "error",
            "HELP_PRODUCT_SCOPE_MISMATCH",
            "O corpus bundled não cobre todos os produtos declarados no projeto.",
            "wlanguage_help_json",
        )
    return len(audit.errors) == errors_before


def safe_bundled_help_member(name: str) -> bool:
    """Accept a portable regular-member name or the one canonical root directory."""
    if (
        not name
        or len(name) > MAX_DISPLAY_LENGTH
        or "\x00" in name
        or _contains_control(name)
        or "\\" in name
    ):
        return False
    try:
        require_well_formed_unicode(name, "nome de membro ZIP bundled")
    except ValueError:
        return False
    if name != unicodedata.normalize("NFC", name):
        return False
    directory = name.endswith("/")
    candidate = name[:-1] if directory else name
    raw_parts = candidate.split("/")
    if len(raw_parts) > MAX_PATH_DEPTH or any(part in {"", ".", ".."} for part in raw_parts):
        return False
    for part in raw_parts:
        if len(part) > MAX_COMPONENT_CHARS or part.endswith((" ", ".")) or ":" in part:
            return False
        if part.split(".", 1)[0].upper() in WINDOWS_RESERVED_NAMES:
            return False
    posix = PurePosixPath(*raw_parts)
    windows = PureWindowsPath(candidate)
    return (
        not posix.is_absolute()
        and not windows.is_absolute()
        and not windows.drive
        and ".." not in posix.parts
        and (not directory or name == f"{BUNDLED_HELP_ROOT}/")
    )


def preflight_bundled_zip_directory(handle, size: int) -> None:
    """Bound central-directory parsing before ZipFile allocates member records."""
    handle.seek(max(0, size - 65_557), os.SEEK_SET)
    tail = handle.read(65_557)
    offset = tail.rfind(b"PK\x05\x06")
    if offset < 0 or offset + 22 > len(tail):
        raise ValueError("EOCD ZIP bundled ausente ou truncado")
    (
        signature,
        disk_number,
        directory_disk,
        entries_on_disk,
        total_entries,
        directory_size,
        directory_offset,
        comment_length,
    ) = struct.unpack("<4s4H2LH", tail[offset : offset + 22])
    if signature != b"PK\x05\x06":
        raise ValueError("assinatura EOCD bundled inválida")
    if disk_number or directory_disk or entries_on_disk != total_entries:
        raise ValueError("ZIP bundled multidisco não é aceito")
    if total_entries == 0xFFFF or directory_size == 0xFFFFFFFF or directory_offset == 0xFFFFFFFF:
        raise ValueError("ZIP64 não é aceito para o corpus bundled")
    if total_entries != BUNDLED_HELP_MEMBER_COUNT:
        raise ValueError("quantidade inesperada de entradas no diretório central bundled")
    if directory_size > BUNDLED_HELP_MAX_CENTRAL_DIRECTORY:
        raise ValueError("diretório central bundled excede o limite")
    if directory_offset + directory_size > size:
        raise ValueError("diretório central bundled aponta para fora do arquivo")
    if offset + 22 + comment_length != len(tail):
        raise ValueError("comentário/trailer do ZIP bundled é inconsistente")


def validate_bundled_zip_structure(bundle: zipfile.ZipFile) -> dict[str, zipfile.ZipInfo]:
    infos = bundle.infolist()
    if len(infos) != BUNDLED_HELP_MEMBER_COUNT:
        raise ValueError("quantidade inesperada de membros no ZIP bundled")
    by_name: dict[str, zipfile.ZipInfo] = {}
    portable_names: set[str] = set()
    header_offsets: set[int] = set()
    total_uncompressed = 0
    file_count = 0
    json_count = 0
    directory_count = 0
    page_sequences: dict[str, set[int]] = {}

    for info in infos:
        name = info.filename
        if not safe_bundled_help_member(name):
            raise ValueError("ZIP bundled contém nome inseguro ou fora do layout")
        portable_name = unicodedata.normalize("NFC", name.rstrip("/")).casefold()
        if name in by_name or portable_name in portable_names:
            raise ValueError("ZIP bundled contém nomes duplicados ou colisão portátil")
        if info.header_offset < 0 or info.header_offset in header_offsets:
            raise ValueError("ZIP bundled contém offset de cabeçalho inválido ou duplicado")
        by_name[name] = info
        portable_names.add(portable_name)
        header_offsets.add(info.header_offset)

        if info.flag_bits & 0x1:
            raise ValueError("ZIP bundled contém membro criptografado")
        if info.compress_type not in ALLOWED_ZIP_COMPRESSION:
            raise ValueError("ZIP bundled usa compressão não permitida")
        if (
            info.file_size < 0
            or info.compress_size < 0
            or info.file_size > BUNDLED_HELP_MAX_MEMBER
        ):
            raise ValueError("membro do ZIP bundled excede o limite")
        if info.file_size and not info.compress_size:
            raise ValueError("membro bundled possui razão de compressão inválida")
        if (
            info.compress_size
            and info.file_size / info.compress_size > BUNDLED_HELP_MAX_COMPRESSION_RATIO
        ):
            raise ValueError("membro bundled excede a razão de compressão permitida")

        mode = (info.external_attr >> 16) & 0xFFFF
        if info.create_system != 3 or not mode:
            raise ValueError("membro bundled não possui tipo Unix verificável")
        if stat.S_ISLNK(mode):
            raise ValueError("ZIP bundled contém symlink")
        if info.is_dir():
            if not stat.S_ISDIR(mode):
                raise ValueError("diretório bundled possui tipo central divergente")
            directory_count += 1
        else:
            if not stat.S_ISREG(mode):
                raise ValueError("ZIP bundled contém arquivo especial")
            file_count += 1
            if name.endswith(".json"):
                json_count += 1

        total_uncompressed += info.file_size
        if total_uncompressed > BUNDLED_HELP_UNCOMPRESSED_BYTES:
            raise ValueError("ZIP bundled excede o total descomprimido fixado")

        if info.is_dir() or name in {BUNDLED_HELP_INDEX_MEMBER, BUNDLED_HELP_PROGRESS_MEMBER}:
            continue
        match = BUNDLED_HELP_PAGE_NAME.fullmatch(name)
        if match is None:
            raise ValueError("ZIP bundled contém arquivo fora do layout de páginas")
        page_sequences.setdefault(match.group("group"), set()).add(int(match.group("sequence")))

    if total_uncompressed != BUNDLED_HELP_UNCOMPRESSED_BYTES:
        raise ValueError("total descomprimido do ZIP bundled diverge do valor fixado")
    if directory_count != 1 or f"{BUNDLED_HELP_ROOT}/" not in by_name:
        raise ValueError("diretório raiz bundled ausente ou duplicado")
    if file_count != BUNDLED_HELP_FILE_COUNT or json_count != BUNDLED_HELP_JSON_COUNT:
        raise ValueError("contagem de arquivos/JSONs bundled divergente")
    if BUNDLED_HELP_INDEX_MEMBER not in by_name or BUNDLED_HELP_PROGRESS_MEMBER not in by_name:
        raise ValueError("índice ou progresso do corpus bundled está ausente")
    page_count = sum(len(sequences) for sequences in page_sequences.values())
    if page_count != BUNDLED_HELP_PAGE_COUNT:
        raise ValueError("contagem de páginas bundled divergente")

    gaps: dict[str, list[int]] = {}
    for group, sequences in page_sequences.items():
        missing = sorted(set(range(1, max(sequences) + 1)) - sequences)
        if missing:
            gaps[group] = missing
    if gaps != {"02-03-01": [223]}:
        raise ValueError("lacunas físicas do corpus bundled divergiram do contrato")
    return by_name


def validate_bundled_index(data: bytes) -> None:
    document = strict_json_loads(data, BUNDLED_HELP_INDEX_MEMBER)
    if not isinstance(document, dict):
        raise ValueError("índice bundled precisa ser objeto JSON")
    themes = document.get("temas")
    declared_themes = document.get("total_de_temas")
    if (
        not isinstance(themes, list)
        or not isinstance(declared_themes, int)
        or isinstance(declared_themes, bool)
        or declared_themes != len(themes)
    ):
        raise ValueError("índice bundled possui estrutura de temas inválida")
    total_pages = 0
    for theme in themes:
        pages = theme.get("paginas") if isinstance(theme, dict) else None
        if not isinstance(pages, int) or isinstance(pages, bool) or pages < 0:
            raise ValueError("índice bundled contém contagem de páginas inválida")
        total_pages += pages
    if total_pages != BUNDLED_HELP_JSON_COUNT:
        raise ValueError("soma de páginas do índice bundled diverge de 12037")


def validate_bundled_progress(data: bytes) -> None:
    try:
        text = data.decode("utf-8", errors="strict")
        require_well_formed_unicode(text, BUNDLED_HELP_PROGRESS_MEMBER)
    except (UnicodeError, ValueError) as exc:
        raise ValueError("progresso bundled possui texto inválido") from exc
    values: dict[str, int] = {}
    in_harvest_section = False
    for raw_line in text.splitlines():
        line = raw_line.strip()
        if not line or line.startswith(("#", ";")):
            continue
        if line.startswith("[") and line.endswith("]"):
            in_harvest_section = line == "[colheita]"
            continue
        if not in_harvest_section or "=" not in line:
            continue
        key, raw_value = (part.strip() for part in line.split("=", 1))
        if key in values or not re.fullmatch(r"\d+", raw_value):
            raise ValueError("progresso bundled contém contador duplicado ou inválido")
        values[key] = int(raw_value)
    expected = {
        "total_do_mapa": 12_037,
        "processadas": 7_077,
        "falhas": 1,
        "restantes": 0,
        "ultima_posicao": 12_037,
    }
    if any(values.get(key) != value for key, value in expected.items()):
        raise ValueError("contadores conhecidos de progresso bundled divergiram")


def audit_bundled_help_group(
    group_data: dict,
    audit: Audit,
    *,
    expected_version: str,
    expected_language: str,
    expected_products: set[str],
) -> None:
    errors_before = len(audit.errors)
    if not bundled_help_declaration_is_valid(
        group_data,
        audit,
        expected_version=expected_version,
        expected_language=expected_language,
        expected_products=expected_products,
    ):
        return

    skill_root = Path(__file__).resolve().parent.parent
    resources = skill_root / "resources"
    corpus_path = resources / BUNDLED_HELP_ARCHIVE_NAME
    try:
        resources_metadata = os.lstat(resources)
        corpus_metadata = os.lstat(corpus_path)
        if stat.S_ISLNK(resources_metadata.st_mode) or not stat.S_ISDIR(resources_metadata.st_mode):
            raise ValueError("diretório resources não é diretório regular sem symlink")
        if stat.S_ISLNK(corpus_metadata.st_mode) or not stat.S_ISREG(corpus_metadata.st_mode):
            raise ValueError("corpus bundled não é arquivo regular sem symlink")
    except (OSError, ValueError) as exc:
        audit.issue(
            "error",
            "HELP_CORPUS_FILE",
            str(exc),
            "wlanguage_help_json",
            BUNDLED_HELP_ARCHIVE_NAME,
        )
        return

    flags = os.O_RDONLY | getattr(os, "O_CLOEXEC", 0) | getattr(os, "O_NOFOLLOW", 0)
    try:
        descriptor = os.open(corpus_path, flags)
        with os.fdopen(descriptor, "rb") as handle:
            before = os.fstat(handle.fileno())
            if not stat.S_ISREG(before.st_mode):
                raise ValueError("corpus bundled aberto não é arquivo regular")
            if before.st_size != BUNDLED_HELP_SIZE:
                audit.issue(
                    "error",
                    "HELP_CORPUS_SIZE",
                    f"Corpus bundled precisa ter exatamente {BUNDLED_HELP_SIZE} bytes.",
                    "wlanguage_help_json",
                    BUNDLED_HELP_ARCHIVE_NAME,
                )
                return

            digest = hashlib.sha256()
            while True:
                block = handle.read(READ_BLOCK)
                if not block:
                    break
                digest.update(block)
            hashed_metadata = os.fstat(handle.fileno())
            if (
                before.st_dev,
                before.st_ino,
                before.st_size,
                before.st_mtime_ns,
            ) != (
                hashed_metadata.st_dev,
                hashed_metadata.st_ino,
                hashed_metadata.st_size,
                hashed_metadata.st_mtime_ns,
            ):
                audit.issue(
                    "error",
                    "HELP_CORPUS_CHANGED",
                    "Corpus bundled mudou durante o cálculo de identidade.",
                    "wlanguage_help_json",
                    BUNDLED_HELP_ARCHIVE_NAME,
                )
                return
            actual_digest = digest.hexdigest()
            if actual_digest != BUNDLED_HELP_SHA256:
                audit.issue(
                    "error",
                    "HELP_CORPUS_SHA256",
                    "SHA-256 do corpus bundled diverge da identidade fixada; ZIP não foi aberto.",
                    "wlanguage_help_json",
                    BUNDLED_HELP_ARCHIVE_NAME,
                )
                return

            preflight_bundled_zip_directory(handle, before.st_size)
            handle.seek(0)
            with zipfile.ZipFile(handle, mode="r") as bundle:
                infos = validate_bundled_zip_structure(bundle)
                index_data = read_zip_member(bundle, infos[BUNDLED_HELP_INDEX_MEMBER])
                validate_bundled_index(index_data)

                invalid_data = read_zip_member(bundle, infos[BUNDLED_HELP_INVALID_MEMBER])
                if (
                    len(invalid_data) != BUNDLED_HELP_INVALID_SIZE
                    or hashlib.sha256(invalid_data).hexdigest() != BUNDLED_HELP_INVALID_SHA256
                    or invalid_data != b"\x00" * BUNDLED_HELP_INVALID_SIZE
                ):
                    raise ValueError("membro NUL conhecido divergiu da quarentena fixada")

                progress_data = read_zip_member(bundle, infos[BUNDLED_HELP_PROGRESS_MEMBER])
                validate_bundled_progress(progress_data)

            after = os.fstat(handle.fileno())
            if (
                before.st_dev,
                before.st_ino,
                before.st_size,
                before.st_mtime_ns,
            ) != (after.st_dev, after.st_ino, after.st_size, after.st_mtime_ns):
                raise ValueError("corpus bundled mudou durante a auditoria ZIP")
    except (OSError, EOFError, RuntimeError, ValueError, zipfile.BadZipFile, zipfile.LargeZipFile) as exc:
        audit.issue(
            "error",
            "HELP_CORPUS_STRUCTURE",
            str(exc),
            "wlanguage_help_json",
            BUNDLED_HELP_ARCHIVE_NAME,
        )
        return

    if not audit.reserve_artifact_bytes(
        BUNDLED_HELP_SIZE,
        "wlanguage_help_json",
        f"resources/{BUNDLED_HELP_ARCHIVE_NAME}",
    ):
        return
    audit.add_inventory(
        "wlanguage_help_json",
        f"resources/{BUNDLED_HELP_ARCHIVE_NAME}",
        BUNDLED_HELP_SIZE,
        BUNDLED_HELP_SHA256,
        "bundled-help-corpus",
        status="degraded",
        notes=(
            f"corpus_id={BUNDLED_HELP_CORPUS_ID}; valid_pages={BUNDLED_HELP_VALID_PAGE_COUNT}; "
            "identity_verified=true; private_key_blocks_redacted=15; complete=false"
        ),
    )
    audit.help_count = BUNDLED_HELP_VALID_PAGE_COUNT
    audit.help_identities.append({
        "source": f"resources/{BUNDLED_HELP_ARCHIVE_NAME}",
        "identity": BUNDLED_HELP_CORPUS_ID,
        "canonical_sha256": BUNDLED_HELP_SHA256,
        "version": clean_text(expected_version, 120),
        "language": BUNDLED_HELP_LANGUAGE,
        "product": ", ".join(BUNDLED_HELP_PRODUCT_SCOPE),
        "verified": True,
        "complete": False,
        "status": "DEGRADED/CONDITIONAL",
    })
    audit.help_identity_verified = len(audit.errors) == errors_before
    audit.issue(
        "warning",
        "HELP_CORPUS_DEGRADED",
        (
            "Identidade do corpus bundled confirmada, mas o conteúdo permanece incompleto: "
            "15 blocos de chave privada demonstrativa foram removidos; 1 página NUL está em "
            "quarentena; há índice 12037/12036, lacuna 02-03-01/00223 e progresso inconsistente."
        ),
        "wlanguage_help_json",
        BUNDLED_HELP_ARCHIVE_NAME,
    )


def audit_help_group(
    root: Path,
    group_data: dict,
    audit: Audit,
    *,
    expected_version: str,
    expected_language: str,
    expected_products: set[str],
    metadata_only: bool = False,
) -> None:
    errors_before = len(audit.errors)
    expected = group_data.get("expected_count", 12)
    if not isinstance(expected, int) or isinstance(expected, bool) or expected != 12:
        audit.issue(
            "error",
            "HELP_EXPECTED_COUNT",
            "expected_count precisa ser o inteiro 12.",
            "wlanguage_help_json",
        )
        expected = 12

    expected_members_value = group_data.get("expected_members")
    if (
        not isinstance(expected_members_value, list)
        or len(expected_members_value) != 12
        or any(not isinstance(name, str) or not name.strip() for name in expected_members_value)
    ):
        audit.issue(
            "error",
            "HELP_EXPECTED_MEMBERS",
            "expected_members precisa listar exatamente 12 nomes JSON não vazios.",
            "wlanguage_help_json",
        )
        expected_members: set[str] = set()
    else:
        normalized_expected = [normalized_help_member(name.strip()) for name in expected_members_value]
        if len(set(normalized_expected)) != 12:
            audit.issue(
                "error",
                "HELP_EXPECTED_MEMBERS",
                "expected_members contém nomes duplicados após normalização Unicode/casefold.",
                "wlanguage_help_json",
            )
        for name in expected_members_value:
            if not safe_zip_member(name) or not name.casefold().endswith(".json"):
                audit.issue(
                    "error",
                    "HELP_EXPECTED_MEMBERS",
                    "expected_members aceita apenas nomes JSON relativos e seguros.",
                    "wlanguage_help_json",
                    name,
                )
        expected_members = set(normalized_expected)

    group_version = group_data.get("version")
    if not isinstance(group_version, str) or not group_version.strip():
        audit.issue(
            "error", "HELP_GROUP_VERSION",
            "wlanguage_help_json.version precisa ser string não vazia.",
            "wlanguage_help_json",
        )
    elif expected_version and group_version.strip().casefold() != expected_version.casefold():
        audit.issue(
            "error", "HELP_GROUP_VERSION_MISMATCH",
            "Versão declarada no grupo Help diverge de project.wlanguage_help_version.",
            "wlanguage_help_json",
        )
    group_language = group_data.get("language")
    if not isinstance(group_language, str) or not group_language.strip():
        audit.issue(
            "error", "HELP_GROUP_LANGUAGE",
            "wlanguage_help_json.language precisa ser string não vazia.",
            "wlanguage_help_json",
        )
    elif expected_language and group_language.strip().casefold() != expected_language.casefold():
        audit.issue(
            "error", "HELP_GROUP_LANGUAGE_MISMATCH",
            "Idioma declarado no grupo Help diverge de project.wlanguage_help_language.",
            "wlanguage_help_json",
        )
    product_scope_value = group_data.get("product_scope")
    if (
        not isinstance(product_scope_value, list)
        or not product_scope_value
        or any(not isinstance(product, str) or product not in VALID_PRODUCTS for product in product_scope_value)
        or len(set(product_scope_value)) != len(product_scope_value)
    ):
        audit.issue(
            "error", "HELP_PRODUCT_SCOPE",
            "product_scope precisa ser lista não vazia de produtos WX únicos.",
            "wlanguage_help_json",
        )
        product_scope: set[str] = set()
    else:
        product_scope = set(product_scope_value)
    if expected_products and not expected_products.issubset(product_scope):
        audit.issue(
            "error", "HELP_PRODUCT_SCOPE_MISMATCH",
            "product_scope do Help não cobre todos os produtos declarados no projeto.",
            "wlanguage_help_json",
        )
    if metadata_only:
        return

    archive = group_data.get("archive")
    items = group_data.get("items", [])
    if not isinstance(items, list):
        audit.issue("error", "INVALID_ITEMS", "items precisa ser lista.", "wlanguage_help_json")
        return
    if archive is not None and items:
        audit.issue(
            "error",
            "HELP_TWO_SOURCES",
            "Use archive ou items para o Help, nunca ambos.",
            "wlanguage_help_json",
        )
        return

    raw_digests: list[str] = []
    canonical_digests: list[str] = []
    identities: list[str] = []
    verification_flags: list[bool] = []
    actual_members: list[str] = []
    count = 0

    if archive is not None:
        if not isinstance(archive, dict):
            audit.issue(
                "error",
                "INVALID_ARCHIVE_ITEM",
                "archive precisa ser objeto com path e metadados.",
                "wlanguage_help_json",
            )
            return
        path, _ = audit_regular_file(
            root,
            "wlanguage_help_json",
            archive,
            audit,
            max_size=MAX_ZIP_ARCHIVE,
            scan_secrets=False,
        )
        if path is None:
            return
        if path.suffix.lower() != ".zip":
            audit.issue(
                "error",
                "INVALID_HELP_ARCHIVE",
                "O bundle do Help precisa ter extensão .zip.",
                "wlanguage_help_json",
                clean_text(archive.get("path")),
            )
            return
        try:
            preflight_zip_directory(path)
            with zipfile.ZipFile(path) as bundle:
                all_infos = bundle.infolist()
                if len(all_infos) > MAX_ZIP_MEMBERS:
                    audit.issue(
                        "error",
                        "ZIP_MEMBER_LIMIT",
                        f"ZIP excede {MAX_ZIP_MEMBERS} entradas no diretório central.",
                        "wlanguage_help_json",
                    )
                    return
                infos: list[zipfile.ZipInfo] = []
                seen_names: set[str] = set()
                for info in all_infos:
                    name = info.filename
                    if not safe_zip_member(name):
                        audit.issue(
                            "error",
                            "UNSAFE_ZIP_MEMBER",
                            "Nome de membro ZIP inseguro.",
                            "wlanguage_help_json",
                            name,
                        )
                        continue
                    if info.is_dir():
                        audit.issue(
                            "error",
                            "ZIP_UNEXPECTED_MEMBER",
                            "O bundle do Help não aceita diretórios nem metadados extras.",
                            "wlanguage_help_json",
                            name,
                        )
                        continue
                    if not name.lower().endswith(".json"):
                        audit.issue(
                            "error",
                            "ZIP_UNEXPECTED_MEMBER",
                            "O bundle do Help pode conter somente os 12 JSONs.",
                            "wlanguage_help_json",
                            name,
                        )
                        continue
                    normalized_name = name.casefold()
                    if normalized_name in seen_names:
                        audit.issue(
                            "error",
                            "ZIP_DUPLICATE_NAME",
                            "Nome de membro ZIP duplicado após normalização.",
                            "wlanguage_help_json",
                            name,
                        )
                        continue
                    seen_names.add(normalized_name)
                    if zip_member_is_symlink(info):
                        audit.issue(
                            "error",
                            "ZIP_SYMLINK",
                            "Symlink não é aceito no bundle do Help.",
                            "wlanguage_help_json",
                            name,
                        )
                        continue
                    file_type = (info.external_attr >> 16) & 0o170000
                    if file_type not in {0, stat.S_IFREG}:
                        audit.issue(
                            "error",
                            "ZIP_SPECIAL_FILE",
                            "Apenas arquivos regulares são aceitos no bundle do Help.",
                            "wlanguage_help_json",
                            name,
                        )
                        continue
                    if info.flag_bits & 0x1:
                        audit.issue(
                            "error",
                            "ZIP_ENCRYPTED_MEMBER",
                            "Membro ZIP criptografado não é aceito.",
                            "wlanguage_help_json",
                            name,
                        )
                        continue
                    if info.compress_type not in ALLOWED_ZIP_COMPRESSION:
                        audit.issue(
                            "error",
                            "ZIP_COMPRESSION_METHOD",
                            "Método de compressão ZIP não permitido.",
                            "wlanguage_help_json",
                            name,
                        )
                        continue
                    ratio = info.file_size / max(info.compress_size, 1)
                    if info.file_size > MAX_HELP_MEMBER or ratio > MAX_COMPRESSION_RATIO:
                        audit.issue(
                            "error",
                            "ZIP_MEMBER_LIMIT",
                            "Membro excede tamanho ou razão de compressão permitidos.",
                            "wlanguage_help_json",
                            name,
                        )
                        continue
                    infos.append(info)
                    actual_members.append(normalized_help_member(name))

                count = len(infos)
                total_uncompressed = sum(info.file_size for info in infos)
                if total_uncompressed > MAX_HELP_TOTAL:
                    audit.issue(
                        "error",
                        "ZIP_SIZE_LIMIT",
                        f"JSONs excedem limite total de {MAX_HELP_TOTAL} bytes.",
                        "wlanguage_help_json",
                    )
                    return
                if not audit.reserve_artifact_bytes(
                    total_uncompressed,
                    "wlanguage_help_json",
                    clean_text(path.relative_to(root).as_posix()),
                ):
                    return
                archive_display = clean_text(path.relative_to(root).as_posix())
                for info in infos:
                    label = clean_text(f"{archive_display}!{info.filename}")
                    try:
                        data = read_zip_member(bundle, info)
                    except (EOFError, NotImplementedError, RuntimeError, ValueError, zipfile.BadZipFile) as exc:
                        audit.issue(
                            "error",
                            "ZIP_MEMBER_READ_ERROR",
                            str(exc),
                            "wlanguage_help_json",
                            label,
                        )
                        continue
                    raw_digest = hashlib.sha256(data).hexdigest()
                    raw_digests.append(raw_digest)
                    audit.add_inventory(
                        "wlanguage_help_json",
                        label,
                        len(data),
                        raw_digest,
                        "json-in-zip",
                    )
                    assessed = assess_help_document(
                        data,
                        label,
                        archive,
                        expected_version,
                        expected_language,
                        expected_products,
                        audit,
                    )
                    if assessed:
                        canonical, identity, verified = assessed
                        canonical_digests.append(canonical)
                        identities.append(identity.casefold())
                        verification_flags.append(verified)
        except (OSError, EOFError, RuntimeError, zipfile.BadZipFile, zipfile.LargeZipFile) as exc:
            audit.issue(
                "error",
                "INVALID_HELP_ARCHIVE",
                f"ZIP inválido ou ilegível: {exc}",
                "wlanguage_help_json",
                clean_text(archive.get("path")),
            )
            return
    else:
        count = len(items)
        if count > MAX_ITEMS_PER_GROUP:
            audit.issue(
                "error",
                "ITEM_LIMIT",
                f"Grupo excede {MAX_ITEMS_PER_GROUP} itens.",
                "wlanguage_help_json",
            )
            return
        for item in items:
            if not isinstance(item, dict):
                audit.issue(
                    "error",
                    "INVALID_ITEM",
                    "Item do Help precisa ser objeto com path e metadados.",
                    "wlanguage_help_json",
                )
                continue
            path, digest = audit_regular_file(
                root,
                "wlanguage_help_json",
                item,
                audit,
                max_size=MAX_HELP_MEMBER,
                scan_secrets=False,
            )
            if path is None:
                continue
            if path.suffix.lower() != ".json":
                audit.issue(
                    "error",
                    "HELP_NOT_JSON",
                    "Arquivo do Help precisa ter extensão .json.",
                    "wlanguage_help_json",
                    clean_text(item.get("path")),
                )
                continue
            label = clean_text(path.relative_to(root).as_posix())
            actual_member = path.relative_to(root).as_posix()
            if not safe_zip_member(actual_member):
                audit.issue(
                    "error", "HELP_MEMBER_PATH_UNSAFE",
                    "Path do JSON do Help não é um nome POSIX relativo portátil/NFC.",
                    "wlanguage_help_json", label,
                )
            actual_members.append(normalized_help_member(actual_member))
            try:
                data = read_limited(path, MAX_HELP_MEMBER, label)
            except (OSError, ValueError) as exc:
                audit.issue("error", "HELP_READ_ERROR", str(exc), "wlanguage_help_json", label)
                continue
            if hashlib.sha256(data).hexdigest() != digest:
                audit.issue(
                    "error",
                    "FILE_CHANGED",
                    "Arquivo mudou entre hash e parse.",
                    "wlanguage_help_json",
                    label,
                )
            raw_digests.append(hashlib.sha256(data).hexdigest())
            assessed = assess_help_document(
                data,
                label,
                item,
                expected_version,
                expected_language,
                expected_products,
                audit,
            )
            if assessed:
                canonical, identity, verified = assessed
                canonical_digests.append(canonical)
                identities.append(identity.casefold())
                verification_flags.append(verified)

    audit.help_count = count
    if count != expected:
        audit.issue(
            "error",
            "HELP_COUNT",
            f"Foram encontrados {count} JSONs válidos; são exigidos exatamente {expected}.",
            "wlanguage_help_json",
        )
    if expected_members and set(actual_members) != expected_members:
        audit.issue(
            "error",
            "HELP_MEMBER_SET_MISMATCH",
            "Conjunto real de nomes do Help difere de expected_members.",
            "wlanguage_help_json",
        )
    if len(set(raw_digests)) != len(raw_digests):
        audit.issue(
            "error",
            "DUPLICATE_HELP_JSON",
            "Há JSONs do Help com bytes duplicados.",
            "wlanguage_help_json",
        )
    if len(set(canonical_digests)) != len(canonical_digests):
        audit.issue(
            "error",
            "SEMANTIC_DUPLICATE_HELP_JSON",
            "Há JSONs semanticamente duplicados após canonicalização.",
            "wlanguage_help_json",
        )
    if len(set(identities)) != len(identities):
        audit.issue(
            "error",
            "DUPLICATE_HELP_IDENTITY",
            "Há identidades de documentos do Help duplicadas.",
            "wlanguage_help_json",
        )
    audit.help_identity_verified = (
        count == expected
        and len(canonical_digests) == expected
        and len(identities) == expected
        and len(verification_flags) == expected
        and all(verification_flags)
        and len(audit.errors) == errors_before
    )


def safe_url_for_inventory(url: str, audit: Audit, access: str) -> tuple[str, str] | None:
    try:
        require_well_formed_unicode(url, "external_links.url")
    except ValueError as exc:
        audit.issue("error", "INVALID_URL", str(exc), "external_links")
        return None
    if len(url) > MAX_URL_LENGTH or _contains_control(url):
        audit.issue("error", "INVALID_URL", "URL ausente, longa demais ou contém controles.", "external_links")
        return None
    try:
        parsed = urlsplit(url)
        port = parsed.port
    except (TypeError, ValueError) as exc:
        audit.issue("error", "INVALID_URL", str(exc), "external_links")
        return None
    if parsed.scheme.casefold() != "https" or not parsed.hostname:
        audit.issue("error", "INVALID_URL", "URL precisa usar HTTPS e possuir host.", "external_links")
        return None

    raw_host = parsed.hostname
    address: ipaddress.IPv4Address | ipaddress.IPv6Address | None = None
    try:
        address = ipaddress.ip_address(raw_host)
    except ValueError:
        try:
            host = raw_host.encode("idna").decode("ascii").casefold().rstrip(".")
        except UnicodeError as exc:
            audit.issue("error", "INVALID_URL", f"hostname inválido: {exc}", "external_links")
            return None
        labels = host.split(".")
        if (
            not host
            or len(host) > 253
            or any(
                not part
                or len(part) > 63
                or part.startswith("-")
                or part.endswith("-")
                or re.fullmatch(r"[a-z0-9-]+", part) is None
                for part in labels
            )
        ):
            audit.issue("error", "INVALID_URL", "hostname inválido.", "external_links")
            return None
    else:
        host = address.compressed
    host_display = f"[{host}]" if ":" in host and not host.startswith("[") else host
    netloc = host_display + (f":{port}" if port is not None else "")
    safe_url = urlunsplit(("https", netloc, parsed.path or "/", "", ""))
    notes: list[str] = []

    if parsed.username is not None or parsed.password is not None:
        audit.secret("external_links", safe_url, 1)
        audit.issue(
            "error",
            "URL_CREDENTIALS",
            "URL contém credencial em userinfo; use referência de segredo fora da URL.",
            "external_links",
            safe_url,
        )
    def component_keys(component: str) -> set[str]:
        keys = {key for key, _ in parse_qsl(component, keep_blank_values=True)}
        for part in re.split(r"[&;]", unquote_plus(component)):
            key = part.partition("=")[0]
            if key:
                keys.add(key)
        return keys

    sensitive_keys = sorted({
        key for component in (parsed.query, parsed.fragment)
        for key in component_keys(component)
        if SENSITIVE_QUERY_KEY.search(key)
    })
    if sensitive_keys:
        audit.secret("external_links", safe_url, 1)
        audit.issue(
            "error",
            "URL_SECRET_QUERY",
            "URL contém parâmetros/fragmentos potencialmente secretos: "
            + ", ".join(clean_text(key, 80) for key in sensitive_keys),
            "external_links",
            safe_url,
        )
    decoded_path = unquote(parsed.path)
    if re.search(
        r"(?i)(?:^|[;/])(?:token|key|secret|password|passwd|pwd|signature|credential|auth|code)=",
        decoded_path,
    ):
        audit.secret("external_links", safe_url, 1)
        audit.issue(
            "error",
            "URL_SECRET_PATH",
            "URL contém parâmetro potencialmente secreto no caminho.",
            "external_links",
            safe_url,
        )
    if parsed.query:
        notes.append("query omitida do inventário")
    if parsed.fragment:
        notes.append("fragmento omitido do inventário")
    if isinstance(address, (ipaddress.IPv4Address, ipaddress.IPv6Address)):
        if address.is_private or address.is_loopback or address.is_link_local:
            audit.issue(
                "warning",
                "PRIVATE_URL_REVIEW",
                "Link privado/loopback não deve ser acessado automaticamente.",
                "external_links",
                safe_url,
            )
    elif host == "localhost" or host.endswith(".localhost"):
        audit.issue(
            "warning",
            "PRIVATE_URL_REVIEW",
            "Link localhost não deve ser acessado automaticamente.",
            "external_links",
            safe_url,
        )
    return clean_text(safe_url), "; ".join(notes)


def audit_links(group_data: dict, audit: Audit) -> None:
    items = group_data.get("items", [])
    if not isinstance(items, list):
        audit.issue("error", "INVALID_ITEMS", "items precisa ser lista.", "external_links")
        return
    if len(items) > MAX_ITEMS_PER_GROUP:
        audit.issue("error", "ITEM_LIMIT", f"Grupo excede {MAX_ITEMS_PER_GROUP} itens.", "external_links")
        return
    for item in items:
        if not isinstance(item, dict):
            audit.issue("error", "INVALID_LINK", "Link precisa ser objeto.", "external_links")
            continue
        if "path" in item:
            audit.issue(
                "error",
                "INVALID_LINK_PATH",
                "external_links aceita somente itens por URL HTTPS, nunca path.",
                "external_links",
            )
        url = item.get("url")
        if not isinstance(url, str) or not url:
            audit.issue("error", "INVALID_URL", "url precisa ser string não vazia.", "external_links")
            continue
        access = item.get("access", "unverified")
        if not isinstance(access, str) or access not in VALID_ACCESS:
            audit.issue("error", "INVALID_LINK_ACCESS", "Campo access inválido.", "external_links")
            access = "unverified"
        purpose = item.get("purpose", "")
        if purpose and not isinstance(purpose, str):
            audit.issue("error", "INVALID_LINK_PURPOSE", "purpose precisa ser string.", "external_links")
            purpose = ""
        if not purpose:
            audit.issue("warning", "LINK_WITHOUT_PURPOSE", "Informe a finalidade do link.", "external_links")
        sanitized = safe_url_for_inventory(url, audit, access)
        if sanitized is None:
            continue
        safe_url, url_notes = sanitized
        digest = hashlib.sha256(url.encode("utf-8")).hexdigest()
        notes = "; ".join(part for part in (clean_text(purpose), url_notes) if part)
        audit.add_inventory("external_links", safe_url, None, digest, "url", access, notes)


def group_has_content(group: object) -> bool:
    if not isinstance(group, dict):
        return False
    if group.get("status") == "bundled":
        return (
            group.get("corpus_id") == BUNDLED_HELP_CORPUS_ID
            and group.get("sha256") == BUNDLED_HELP_SHA256
            and group.get("expected_json_count") == BUNDLED_HELP_JSON_COUNT
            and group.get("expected_page_count") == BUNDLED_HELP_PAGE_COUNT
            and group.get("valid_page_count") == BUNDLED_HELP_VALID_PAGE_COUNT
            and group.get("language") == BUNDLED_HELP_LANGUAGE
            and group.get("version_coverage") == BUNDLED_HELP_VERSION_COVERAGE
            and group.get("product_scope") == BUNDLED_HELP_PRODUCT_SCOPE
            and group.get("known_invalid_members") == [BUNDLED_HELP_INVALID_MEMBER]
            and group.get("known_gaps") == BUNDLED_HELP_KNOWN_GAPS
            and group.get("sanitization") == BUNDLED_HELP_SANITIZATION
            and group.get("archive") is None
            and group.get("items") == []
        )
    if group.get("status") != "provided":
        return False
    if group.get("archive") is not None:
        archive = group.get("archive")
        return isinstance(archive, dict) and isinstance(archive.get("path"), str) and bool(archive.get("path"))
    items = group.get("items")
    return isinstance(items, list) and bool(items)


def classify_evidence(manifest: dict, config: dict) -> str:
    artifacts = manifest.get("artifacts")
    if not isinstance(artifacts, dict):
        return "forensic"
    source_runtime = config.get("source_runtime")
    executable = (
        isinstance(source_runtime, dict)
        and source_runtime.get("available") is True
        and source_runtime.get("authorized") is True
    )
    baseline = group_has_content(artifacts.get("videos_and_runtime_baselines"))
    if (
        group_has_content(artifacts.get("native_project_sources"))
        and executable
        and baseline
        and group_has_content(artifacts.get("sample_data_and_expected_results"))
    ):
        return "native"
    if (
        group_has_content(artifacts.get("code_documents"))
        and group_has_content(artifacts.get("sql_scripts"))
        and executable
        and baseline
    ):
        return "documentary"
    return "forensic"


def nonempty_string(
    mapping: dict,
    field: str,
    audit: Audit,
    code: str,
    prefix: str,
    required: bool = True,
) -> str:
    value = mapping.get(field)
    if isinstance(value, str) and value.strip():
        return value.strip()
    if required:
        audit.issue("error", code, f"Preencha {prefix}.{field}.")
    elif value is not None and not isinstance(value, str):
        audit.issue("error", code, f"{prefix}.{field} precisa ser string.")
    return ""


def string_list(
    value: object,
    audit: Audit,
    code: str,
    label: str,
    *,
    nonempty: bool = False,
) -> list[str]:
    if not isinstance(value, list) or any(not isinstance(item, str) or not item.strip() for item in value):
        audit.issue("error", code, f"{label} precisa ser lista de strings não vazias.")
        return []
    rendered = [item.strip() for item in value]
    if nonempty and not rendered:
        audit.issue("error", code, f"{label} não pode ser vazio.")
    if len(set(rendered)) != len(rendered):
        audit.issue("error", code, f"{label} contém valores duplicados.")
    return rendered


def reject_unknown_keys(mapping: dict, allowed: set[str], label: str, audit: Audit) -> None:
    unknown = sorted(key for key in mapping if not isinstance(key, str) or key not in allowed)
    if unknown:
        audit.issue(
            "error",
            "CONFIG_UNKNOWN_FIELD",
            f"{label} contém campos não permitidos: "
            + ", ".join(clean_text(str(key), 80) for key in unknown),
        )


def validate_exception_objects(value: object, audit: Audit) -> set[str]:
    if not isinstance(value, list):
        audit.issue(
            "error", "INVALID_EXCEPTIONS",
            "governance.approved_exceptions precisa ser lista de objetos.",
        )
        return set()
    codes: set[str] = set()
    allowed = {"code", "reason", "approver", "expires_at", "compensating_controls"}
    now = datetime.now(timezone.utc)
    for index, entry in enumerate(value):
        label = f"governance.approved_exceptions[{index}]"
        if not isinstance(entry, dict):
            audit.issue("error", "INVALID_EXCEPTION", f"{label} precisa ser objeto.")
            continue
        reject_unknown_keys(entry, allowed, label, audit)
        rendered: dict[str, str] = {}
        valid = True
        for field in ("code", "reason", "approver", "expires_at"):
            item = entry.get(field)
            if not isinstance(item, str) or not item.strip():
                audit.issue(
                    "error", "INVALID_EXCEPTION",
                    f"{label}.{field} precisa ser string não vazia.",
                )
                valid = False
            else:
                rendered[field] = item.strip()
        controls = string_list(
            entry.get("compensating_controls"),
            audit,
            "INVALID_EXCEPTION",
            f"{label}.compensating_controls",
            nonempty=True,
        )
        if not controls:
            valid = False
        expires_at: datetime | None = None
        if rendered.get("expires_at"):
            try:
                expires_at = datetime.fromisoformat(
                    rendered["expires_at"].replace("Z", "+00:00")
                )
                if expires_at.tzinfo is None:
                    raise ValueError("timezone obrigatório")
                expires_at = expires_at.astimezone(timezone.utc)
            except ValueError:
                audit.issue(
                    "error", "INVALID_EXCEPTION",
                    f"{label}.expires_at precisa ser date-time ISO 8601 com timezone.",
                )
                valid = False
        if expires_at is not None and expires_at <= now:
            audit.issue("error", "EXPIRED_EXCEPTION", f"{label} está expirada.")
            valid = False
        code = rendered.get("code", "")
        if code and code not in VALID_EXCEPTION_CODES:
            audit.issue(
                "error", "UNKNOWN_EXCEPTION_CODE",
                f"Código de exceção não reconhecido: {clean_text(code)}.",
            )
            valid = False
        if code in codes:
            audit.issue("error", "DUPLICATE_EXCEPTION", f"Código de exceção duplicado: {clean_text(code)}.")
            valid = False
        if valid:
            codes.add(code)
    return codes


def validate_project_and_config(manifest: dict, config: dict, audit: Audit) -> dict:
    project = manifest.get("project")
    if not isinstance(project, dict):
        audit.issue("error", "PROJECT_MISSING", "Objeto project ausente ou inválido.")
        project = {}
    project_name = nonempty_string(project, "name", audit, "PROJECT_FIELD", "project")
    wx_version = nonempty_string(project, "wx_version", audit, "PROJECT_FIELD", "project")
    help_version = nonempty_string(project, "wlanguage_help_version", audit, "PROJECT_FIELD", "project")
    help_language = nonempty_string(project, "wlanguage_help_language", audit, "PROJECT_FIELD", "project")
    human_approver = nonempty_string(project, "human_approver", audit, "PROJECT_FIELD", "project")
    products = string_list(project.get("products"), audit, "PROJECT_PRODUCTS", "project.products", nonempty=True)
    if any(product not in VALID_PRODUCTS for product in products):
        audit.issue("error", "PROJECT_PRODUCTS", "project.products contém produto WX inválido.")

    mode = config.get("mode")
    if not isinstance(mode, str) or mode not in VALID_MODES:
        audit.issue("error", "INVALID_MODE", "mode precisa ser inventory, plan, pilot ou complete.")
        mode = "inventory"
    evidence_class = config.get("evidence_class")
    if not isinstance(evidence_class, str) or evidence_class not in VALID_EVIDENCE_CLASSES:
        audit.issue("error", "INVALID_EVIDENCE_CLASS", "evidence_class inválida.")
        evidence_class = "auto"

    required_objects = (
        "source_runtime", "target", "fidelity", "scope", "scale",
        "acceptance", "governance",
    )
    for field in required_objects:
        if not isinstance(config.get(field), dict):
            audit.issue("error", "CONFIG_OBJECT", f"Objeto config.{field} ausente ou inválido.")

    source_runtime = config.get("source_runtime") if isinstance(config.get("source_runtime"), dict) else {}
    runtime_fields = {
        "available", "authorized", "build_id", "configuration", "operating_system",
        "architecture", "wx_runtime", "locale", "timezone", "test_environment_ref",
        "credential_reference", "reset_procedure", "feature_flags",
    }
    reject_unknown_keys(source_runtime, runtime_fields, "source_runtime", audit)
    for field in ("available", "authorized"):
        if not isinstance(source_runtime.get(field), bool):
            audit.issue("error", "RUNTIME_TYPE", f"source_runtime.{field} precisa ser booleano.")
    for field in runtime_fields - {"available", "authorized", "feature_flags"}:
        if not isinstance(source_runtime.get(field), str):
            audit.issue("error", "RUNTIME_TYPE", f"source_runtime.{field} precisa ser string.")
    feature_flags = string_list(
        source_runtime.get("feature_flags"), audit, "RUNTIME_TYPE", "source_runtime.feature_flags"
    )
    source_available = source_runtime.get("available") is True
    source_authorized = source_runtime.get("authorized") is True
    legacy_alias = config.get("source_execution_available")
    if legacy_alias is not None and not isinstance(legacy_alias, bool):
        audit.issue(
            "error", "INVALID_SOURCE_EXECUTION",
            "source_execution_available, quando presente, precisa ser booleano.",
        )
    elif isinstance(legacy_alias, bool) and legacy_alias != source_available:
        audit.issue(
            "error", "SOURCE_RUNTIME_DIVERGENCE",
            "source_execution_available diverge de source_runtime.available; o último é normativo.",
        )

    target = config.get("target") if isinstance(config.get("target"), dict) else {}
    for field in ("language", "database", "architecture", "deployment"):
        value = target.get(field)
        if not isinstance(value, str):
            audit.issue("error", "TARGET_FIELD", f"target.{field} precisa ser string.")
        elif mode != "inventory" and not value.strip():
            audit.issue("error", "TARGET_FIELD", f"Preencha target.{field} para o modo {mode}.")
    for field in ("frameworks", "platforms"):
        values = string_list(target.get(field), audit, "TARGET_FIELD", f"target.{field}")
        if mode != "inventory" and not values:
            audit.issue("error", "TARGET_FIELD", f"Preencha target.{field} para o modo {mode}.")
    minimum_versions = target.get("minimum_versions", {})
    if not isinstance(minimum_versions, dict) or any(
        not isinstance(key, str) or not key.strip() or not isinstance(value, str) or not value.strip()
        for key, value in minimum_versions.items()
    ):
        audit.issue("error", "TARGET_FIELD", "target.minimum_versions precisa mapear nomes para versões string.")

    fidelity = config.get("fidelity") if isinstance(config.get("fidelity"), dict) else {}
    reject_unknown_keys(
        fidelity,
        {"business_behavior", "data_behavior", "ui", "allowed_modernizations"},
        "fidelity",
        audit,
    )
    for field in ("business_behavior", "data_behavior"):
        if fidelity.get(field) not in {"identical", "approved-change"}:
            audit.issue("error", "FIDELITY_FIELD", f"fidelity.{field} possui valor inválido.")
    if fidelity.get("ui") not in {"pixel", "behavioral", "redesign"}:
        audit.issue("error", "FIDELITY_FIELD", "fidelity.ui possui valor inválido.")
    string_list(
        fidelity.get("allowed_modernizations"), audit, "FIDELITY_FIELD",
        "fidelity.allowed_modernizations",
    )

    scope = config.get("scope") if isinstance(config.get("scope"), dict) else {}
    reject_unknown_keys(scope, {"priority_modules", "excluded_modules", "pilot_candidate"}, "scope", audit)
    priority_modules = string_list(
        scope.get("priority_modules"), audit, "SCOPE_FIELD", "scope.priority_modules"
    )
    excluded_modules = string_list(
        scope.get("excluded_modules"), audit, "SCOPE_FIELD", "scope.excluded_modules"
    )
    if set(priority_modules) & set(excluded_modules):
        audit.issue("error", "SCOPE_OVERLAP", "Módulo não pode ser prioritário e excluído simultaneamente.")
    pilot_candidate = scope.get("pilot_candidate")
    if not isinstance(pilot_candidate, str):
        audit.issue("error", "SCOPE_FIELD", "scope.pilot_candidate precisa ser string.")
    elif mode in {"pilot", "complete"} and not pilot_candidate.strip():
        audit.issue("error", "SCOPE_FIELD", "Preencha scope.pilot_candidate para implementação.")

    scale = config.get("scale") if isinstance(config.get("scale"), dict) else {}
    scale_fields = {
        "applications", "estimated_modules", "estimated_wx_objects", "users_and_roles",
        "databases", "data_volume", "growth", "tenants", "integrations", "jobs", "reports",
        "supported_browsers_devices", "sla", "rto", "rpo", "cutover_window",
    }
    reject_unknown_keys(scale, scale_fields, "scale", audit)
    for field, minimum in (("applications", 1), ("estimated_modules", 0), ("estimated_wx_objects", 0)):
        value = scale.get(field)
        if not isinstance(value, int) or isinstance(value, bool) or value < minimum:
            audit.issue("error", "SCALE_FIELD", f"scale.{field} precisa ser inteiro >= {minimum}.")
    for field in (
        "users_and_roles", "databases", "integrations", "jobs", "reports",
        "supported_browsers_devices",
    ):
        string_list(scale.get(field), audit, "SCALE_FIELD", f"scale.{field}")
    for field in ("data_volume", "growth", "tenants", "sla", "rto", "rpo", "cutover_window"):
        if not isinstance(scale.get(field), str):
            audit.issue("error", "SCALE_FIELD", f"scale.{field} precisa ser string.")

    non_functional_value = config.get("non_functional_requirements")
    if non_functional_value is not None and not isinstance(non_functional_value, dict):
        audit.issue(
            "error", "NFR_FIELD",
            "non_functional_requirements, quando presente, precisa ser objeto.",
        )
    elif isinstance(non_functional_value, dict):
        for field, value in non_functional_value.items():
            if not isinstance(field, str) or not field.strip():
                audit.issue("error", "NFR_FIELD", "Chaves de non_functional_requirements devem ser strings.")
            if isinstance(value, list):
                string_list(value, audit, "NFR_FIELD", f"non_functional_requirements.{field}")

    governance = config.get("governance") if isinstance(config.get("governance"), dict) else {}
    policy = governance.get("missing_artifact_policy")
    if policy not in {"block", "allow-scoped-analysis"}:
        audit.issue(
            "error", "GOVERNANCE_POLICY",
            "governance.missing_artifact_policy precisa ser block ou allow-scoped-analysis.",
        )
        policy = "block"
    if governance.get("allow_unrecorded_assumptions") is not False:
        audit.issue(
            "error", "ASSUMPTIONS_NOT_ALLOWED",
            "governance.allow_unrecorded_assumptions precisa ser false.",
        )
    exceptions = validate_exception_objects(governance.get("approved_exceptions"), audit)
    decision_owner_value = governance.get("decision_owner")
    if not isinstance(decision_owner_value, str):
        audit.issue("error", "GOVERNANCE_OWNER", "governance.decision_owner precisa ser string.")
        decision_owner = ""
    else:
        decision_owner = decision_owner_value.strip()
        if mode != "inventory" and not decision_owner:
            audit.issue("error", "GOVERNANCE_OWNER", "Preencha governance.decision_owner.")

    acceptance = config.get("acceptance") if isinstance(config.get("acceptance"), dict) else {}
    acceptance_fields = {
        "approver", "critical_flows", "dimensions", "data_reconciliation_tolerances",
        "performance_thresholds", "visual_diff_threshold", "security_severity_limit",
        "accessibility_standard", "required_platform_matrix", "required_rehearsals",
    }
    reject_unknown_keys(acceptance, acceptance_fields, "acceptance", audit)
    if not isinstance(acceptance.get("approver"), str):
        audit.issue("error", "ACCEPTANCE_TYPE", "acceptance.approver precisa ser string.")
    critical_flows = string_list(
        acceptance.get("critical_flows"), audit, "ACCEPTANCE_TYPE", "acceptance.critical_flows"
    )
    platform_matrix = string_list(
        acceptance.get("required_platform_matrix"), audit, "ACCEPTANCE_TYPE",
        "acceptance.required_platform_matrix",
    )
    for field in ("data_reconciliation_tolerances", "performance_thresholds"):
        if not isinstance(acceptance.get(field), dict):
            audit.issue("error", "ACCEPTANCE_TYPE", f"acceptance.{field} precisa ser objeto.")
    for field in ("visual_diff_threshold", "security_severity_limit", "accessibility_standard"):
        if not isinstance(acceptance.get(field), str):
            audit.issue("error", "ACCEPTANCE_TYPE", f"acceptance.{field} precisa ser string.")
    rehearsals = acceptance.get("required_rehearsals")
    if not isinstance(rehearsals, int) or isinstance(rehearsals, bool) or rehearsals < 0:
        audit.issue(
            "error", "ACCEPTANCE_TYPE",
            "acceptance.required_rehearsals precisa ser inteiro >= 0.",
        )
        rehearsals = 0
    dimensions_value = acceptance.get("dimensions")
    dimensions: list[dict] = []
    dimension_fields = {"name", "dataset", "environment", "tolerance", "severity", "evidence"}
    if not isinstance(dimensions_value, list):
        audit.issue("error", "ACCEPTANCE_TYPE", "acceptance.dimensions precisa ser lista.")
    else:
        for index, dimension in enumerate(dimensions_value):
            label = f"acceptance.dimensions[{index}]"
            if not isinstance(dimension, dict):
                audit.issue("error", "ACCEPTANCE_DIMENSION", f"{label} precisa ser objeto.")
                continue
            reject_unknown_keys(dimension, dimension_fields, label, audit)
            valid = True
            for field in ("name", "dataset", "environment", "tolerance", "evidence"):
                value = dimension.get(field)
                if not isinstance(value, str) or not value.strip():
                    audit.issue(
                        "error", "ACCEPTANCE_DIMENSION",
                        f"{label}.{field} precisa ser string não vazia.",
                    )
                    valid = False
            if dimension.get("severity") not in {"critical", "high", "medium", "low"}:
                audit.issue(
                    "error", "ACCEPTANCE_DIMENSION",
                    f"{label}.severity possui valor inválido.",
                )
                valid = False
            if valid:
                dimensions.append(dimension)

    return {
        "project_name": project_name,
        "wx_version": wx_version,
        "help_version": help_version,
        "help_language": help_language,
        "human_approver": human_approver,
        "products": set(products),
        "mode": mode,
        "requested_evidence_class": evidence_class,
        "source_execution_available": source_available,
        "source_execution_authorized": source_authorized,
        "source_runtime": source_runtime,
        "feature_flags": feature_flags,
        "missing_artifact_policy": policy,
        "approved_exceptions": exceptions,
        "decision_owner": decision_owner,
        "acceptance": acceptance,
        "acceptance_dimensions": dimensions,
        "critical_flows": critical_flows,
        "platform_matrix": platform_matrix,
        "required_rehearsals": rehearsals,
    }


def exception_level(code: str, exceptions: set[str], required: bool = True) -> str:
    return "warning" if code in exceptions or not required else "error"


def validate_runtime_and_acceptance(
    manifest: dict,
    config: dict,
    context: dict,
    audit: Audit,
) -> None:
    artifacts = manifest.get("artifacts") if isinstance(manifest.get("artifacts"), dict) else {}
    mode = context["mode"]
    implementation_mode = mode in {"pilot", "complete"}
    exceptions: set[str] = context["approved_exceptions"]
    runtime_group = artifacts.get("videos_and_runtime_baselines")
    sample_group = artifacts.get("sample_data_and_expected_results")
    runtime_reasons: list[str] = []

    if implementation_mode and not group_has_content(sample_group):
        level = exception_level("NO_SAMPLE_DATA", exceptions)
        audit.issue(
            level,
            "NO_SAMPLE_DATA",
            "Piloto/conversão completa exige dados anonimizados e resultados esperados.",
        )
        runtime_reasons.append("sample_data_missing")

    source_execution = context["source_execution_available"]
    source_authorized = context["source_execution_authorized"]
    if implementation_mode and not source_execution:
        level = exception_level("NO_SOURCE_RUNTIME", exceptions)
        audit.issue(
            level,
            "NO_SOURCE_RUNTIME",
            "Piloto/conversão completa exige autorização e ambiente isolado para executar o legado.",
        )
        runtime_reasons.append("source_runtime_unavailable")
    if source_execution and not source_authorized:
        level = exception_level("SOURCE_RUNTIME_NOT_AUTHORIZED", exceptions, implementation_mode)
        audit.issue(
            level,
            "SOURCE_RUNTIME_NOT_AUTHORIZED",
            "Runtime legado disponível só pode ser usado com autorização explícita.",
        )
        runtime_reasons.append("source_runtime_not_authorized")
    if source_execution and source_authorized and not group_has_content(runtime_group):
        level = exception_level("RUNTIME_BASELINE_MISSING", exceptions, implementation_mode)
        audit.issue(
            level,
            "RUNTIME_BASELINE_MISSING",
            "Execução declarada exige baseline reproduzível de runtime.",
        )
        runtime_reasons.append("runtime_baseline_missing")

    source_runtime: dict = context["source_runtime"]
    if source_execution:
        required_fields = (
            "build_id", "configuration", "operating_system", "architecture", "wx_runtime",
            "locale", "timezone", "test_environment_ref", "credential_reference", "reset_procedure",
        )
        for field in required_fields:
            value = source_runtime.get(field)
            if not isinstance(value, str) or not value.strip():
                level = exception_level("RUNTIME_METADATA_MISSING", exceptions, implementation_mode)
                audit.issue(
                    level,
                    "RUNTIME_METADATA_MISSING",
                    f"Preencha source_runtime.{field} sem inserir credenciais.",
                )
                runtime_reasons.append(field)
        credential_reference = source_runtime.get("credential_reference")
        if isinstance(credential_reference, str) and secret_matches(credential_reference):
            audit.issue(
                "error",
                "RUNTIME_CREDENTIAL_VALUE",
                "credential_reference parece conter valor de credencial; use somente referência externa.",
            )
            runtime_reasons.append("credential_value")

    acceptance: dict = context["acceptance"]
    dimensions: list[dict] = context["acceptance_dimensions"]
    critical_flows: list[str] = context["critical_flows"]
    platform_matrix: list[str] = context["platform_matrix"]
    owner = acceptance.get("approver")
    owner_recorded = isinstance(owner, str) and bool(owner.strip())
    acceptance_reasons: list[str] = []
    if implementation_mode and not critical_flows:
        level = exception_level("ACCEPTANCE_CRITERIA_MISSING", exceptions)
        audit.issue(
            level,
            "ACCEPTANCE_CRITERIA_MISSING",
            "Piloto/conversão completa exige acceptance.critical_flows.",
        )
        acceptance_reasons.append("critical_flows_missing")
    if implementation_mode and not dimensions:
        level = exception_level("ACCEPTANCE_DIMENSIONS_MISSING", exceptions)
        audit.issue(
            level,
            "ACCEPTANCE_DIMENSIONS_MISSING",
            "Piloto/conversão completa exige dimensões objetivas de aceite.",
        )
        acceptance_reasons.append("dimensions_missing")
    elif mode == "plan" and not dimensions:
        audit.issue(
            "warning",
            "ACCEPTANCE_DIMENSIONS_MISSING",
            "Defina dimensões de aceite antes do piloto.",
        )
        acceptance_reasons.append("dimensions_missing")
    for field in (
        "data_reconciliation_tolerances", "performance_thresholds", "visual_diff_threshold",
        "security_severity_limit", "accessibility_standard",
    ):
        value = acceptance.get(field)
        empty = not value if isinstance(value, (dict, str)) else True
        if implementation_mode and empty:
            level = exception_level("ACCEPTANCE_THRESHOLDS_MISSING", exceptions)
            audit.issue(
                level, "ACCEPTANCE_THRESHOLDS_MISSING",
                f"Piloto/conversão completa exige acceptance.{field} preenchido.",
            )
            acceptance_reasons.append(field)
    if implementation_mode and not platform_matrix:
        level = exception_level("ACCEPTANCE_PLATFORM_MATRIX_MISSING", exceptions)
        audit.issue(
            level, "ACCEPTANCE_PLATFORM_MATRIX_MISSING",
            "Piloto/conversão completa exige required_platform_matrix.",
        )
        acceptance_reasons.append("platform_matrix_missing")
    required_rehearsals = context["required_rehearsals"]
    minimum_rehearsals = 2 if mode == "complete" else (1 if mode == "pilot" else 0)
    if required_rehearsals < minimum_rehearsals:
        level = exception_level("ACCEPTANCE_REHEARSALS_MISSING", exceptions, implementation_mode)
        audit.issue(
            level, "ACCEPTANCE_REHEARSALS_MISSING",
            f"Modo {mode} exige required_rehearsals >= {minimum_rehearsals}.",
        )
        acceptance_reasons.append("rehearsals_missing")
    if implementation_mode and not owner_recorded:
        audit.issue("error", "ACCEPTANCE_OWNER_MISSING", "Defina acceptance.approver.")
        acceptance_reasons.append("owner_missing")

    audit.runtime_assessment = {
        "source_execution_available": source_execution,
        "source_execution_authorized": source_authorized,
        "baseline_provided": group_has_content(runtime_group),
        "ready_for_implementation_comparison": implementation_mode and not runtime_reasons,
        "reasons": runtime_reasons,
    }
    audit.acceptance_assessment = {
        "critical_flow_count": len(critical_flows),
        "dimension_count": len(dimensions),
        "owner_recorded": owner_recorded,
        "required_rehearsals": required_rehearsals,
        "ready_for_implementation_acceptance": implementation_mode and not acceptance_reasons,
        "reasons": acceptance_reasons,
    }


def default_inventory_config() -> dict:
    return {
        "schema_version": "1.0",
        "mode": "inventory",
        "evidence_class": "auto",
        "source_execution_available": False,
        "source_runtime": {
            "available": False,
            "authorized": False,
            "build_id": "",
            "configuration": "",
            "operating_system": "",
            "architecture": "",
            "wx_runtime": "",
            "locale": "",
            "timezone": "",
            "test_environment_ref": "",
            "credential_reference": "",
            "reset_procedure": "",
            "feature_flags": [],
        },
        "target": {
            "language": "",
            "frameworks": [],
            "database": "",
            "platforms": [],
            "architecture": "",
            "deployment": "",
            "minimum_versions": {},
        },
        "fidelity": {
            "business_behavior": "identical",
            "data_behavior": "identical",
            "ui": "behavioral",
            "allowed_modernizations": [],
        },
        "non_functional_requirements": {
            "performance": [],
            "security": [],
            "availability": [],
            "accessibility": [],
            "offline": [],
            "observability": [],
        },
        "scope": {
            "priority_modules": [],
            "excluded_modules": [],
            "pilot_candidate": "",
        },
        "scale": {
            "applications": 1,
            "estimated_modules": 0,
            "estimated_wx_objects": 0,
            "users_and_roles": [],
            "databases": [],
            "data_volume": "",
            "growth": "",
            "tenants": "",
            "integrations": [],
            "jobs": [],
            "reports": [],
            "supported_browsers_devices": [],
            "sla": "",
            "rto": "",
            "rpo": "",
            "cutover_window": "",
        },
        "acceptance": {
            "approver": "",
            "critical_flows": [],
            "dimensions": [],
            "data_reconciliation_tolerances": {},
            "performance_thresholds": {},
            "visual_diff_threshold": "",
            "security_severity_limit": "",
            "accessibility_standard": "",
            "required_platform_matrix": [],
            "required_rehearsals": 0,
        },
        "governance": {
            "missing_artifact_policy": "block",
            "allow_unrecorded_assumptions": False,
            "approved_exceptions": [],
            "decision_owner": "",
        },
    }


def derive_evidence_root(
    manifest_path: Path,
    manifest: dict,
    workspace_root: Path,
    allowed_evidence_root: Path | None,
    audit: Audit,
) -> tuple[Path, bool]:
    raw_root = manifest.get("evidence_root")
    if not isinstance(raw_root, str) or not raw_root:
        if allowed_evidence_root is not None:
            raise ValueError("evidence_root ausente ou inválido")
        audit.issue("error", "EVIDENCE_ROOT", "evidence_root ausente ou inválido.")
        return allowed_evidence_root or workspace_root, False
    try:
        relative = normalized_relative_path(raw_root, "evidence_root", allow_parent=True)
        declared_lexical = Path(os.path.abspath(manifest_path.parent / relative))
        ensure_absolute_no_symlink_components(declared_lexical, "evidence_root")
        declared = declared_lexical.resolve(strict=False)
    except (OSError, ValueError) as exc:
        if allowed_evidence_root is not None:
            raise ValueError(f"evidence_root inválido: {exc}") from exc
        audit.issue("error", "EVIDENCE_ROOT", str(exc))
        return allowed_evidence_root or workspace_root, False

    if allowed_evidence_root is None:
        if not _is_within(declared, workspace_root):
            audit.issue(
                "error",
                "EVIDENCE_ROOT_NOT_APPROVED",
                "evidence_root externo exige allowed-evidence-root explícito.",
            )
            return workspace_root, False
        root = declared
    else:
        root = allowed_evidence_root
        try:
            matches_approved = declared.exists() and os.path.samefile(declared, allowed_evidence_root)
        except OSError:
            matches_approved = False
        if not matches_approved:
            raise ValueError(
                "evidence_root do manifesto difere de allowed-evidence-root"
            )
    if not root.exists() or not root.is_dir():
        if allowed_evidence_root is not None:
            raise ValueError("evidence_root não existe ou não é diretório")
        audit.issue("error", "EVIDENCE_ROOT", "evidence_root não existe ou não é diretório.")
        return root, False
    return root, True


def artifact_group_ready_for_g1(group: object) -> bool:
    return group_has_content(group)


def readiness_blockers(manifest: dict, audit: Audit) -> list[str]:
    blockers = {record["code"] for record in audit.errors}
    blockers.update(record["code"] for record in audit.warnings)
    artifacts = manifest.get("artifacts") if isinstance(manifest.get("artifacts"), dict) else {}
    for group_name in CORE_GROUPS:
        if not artifact_group_ready_for_g1(artifacts.get(group_name)):
            blockers.add(f"GROUP:{group_name}")
    if not audit.help_identity_verified:
        blockers.add("HELP_IDENTITY_NOT_VERIFIED")
    return sorted(blockers)


def secure_mkdirs(path: Path, workspace_root: Path) -> None:
    if not _is_within(path, workspace_root):
        raise ValueError("diretório de saída fora de workspace-root")
    relative = path.relative_to(workspace_root)
    current = workspace_root
    for part in relative.parts:
        current = current / part
        try:
            os.mkdir(current, 0o700)
        except FileExistsError:
            metadata = os.lstat(current)
            if stat.S_ISLNK(metadata.st_mode) or not stat.S_ISDIR(metadata.st_mode):
                raise ValueError(f"componente de saída inseguro: {clean_text(str(current))}")


def prepare_versioned_output(
    output_base: Path,
    workspace_root: Path,
    evidence_root: Path,
    run_id: str,
) -> tuple[Path, Path]:
    lexical = Path(os.path.abspath(output_base))
    if not _is_within(lexical, workspace_root):
        raise ValueError("output precisa ficar dentro de workspace-root")
    ensure_no_symlink_components(workspace_root, lexical, "output")
    resolved = lexical.resolve(strict=False)
    if not _is_within(resolved, workspace_root):
        raise ValueError("output resolvido fora de workspace-root")
    if _is_within(resolved, evidence_root) or _is_within(evidence_root, resolved):
        raise ValueError("output e evidence-root não podem se sobrepor")
    secure_mkdirs(resolved / "runs", workspace_root)
    run_directory = resolved / "runs" / run_id
    if os.path.lexists(run_directory):
        raise ValueError(f"run já existe: {run_id}")
    staging = resolved / "runs" / f".{run_id}.staging-{os.urandom(8).hex()}"
    os.mkdir(staging, 0o700)
    return staging, run_directory


def fsync_directory(path: Path) -> None:
    flags = os.O_RDONLY | getattr(os, "O_DIRECTORY", 0)
    descriptor = os.open(path, flags)
    try:
        os.fsync(descriptor)
    finally:
        os.close(descriptor)


def atomic_rename_noreplace(source: Path, destination: Path) -> bool:
    """Use renameat2 when available; False means the caller must fail closed."""
    if os.name != "posix":
        return False
    try:
        libc = ctypes.CDLL(None, use_errno=True)
        renameat2 = getattr(libc, "renameat2")
    except (AttributeError, OSError):
        return False
    renameat2.argtypes = [ctypes.c_int, ctypes.c_char_p, ctypes.c_int, ctypes.c_char_p, ctypes.c_uint]
    renameat2.restype = ctypes.c_int
    result = renameat2(
        -100,
        os.fsencode(source),
        -100,
        os.fsencode(destination),
        1,
    )
    if result == 0:
        return True
    error = ctypes.get_errno()
    if error == errno.EEXIST:
        raise ValueError(f"run já existe: {destination.name}")
    if error in {errno.ENOSYS, errno.EINVAL, errno.ENOTSUP}:
        return False
    raise OSError(error, os.strerror(error), str(destination))


def promote_completed_staging(staging: Path, destination: Path) -> None:
    expected = {
        "COMPLETED.json",
        "gaps.md",
        "inventory.csv",
        "report.json",
        "report.md",
    }
    entries = {entry.name: entry for entry in staging.iterdir()}
    if set(entries) != expected or any(
        entry.is_symlink() or not entry.is_file() for entry in entries.values()
    ):
        raise ValueError("staging incompleto ou com entrada inesperada")
    fsync_directory(staging)
    if atomic_rename_noreplace(staging, destination):
        fsync_directory(destination.parent)
        return
    raise ValueError(
        "filesystem não oferece promoção atômica de diretório sem sobrescrita"
    )


def write_text_exclusive(path: Path, content: str) -> None:
    flags = os.O_WRONLY | os.O_CREAT | os.O_EXCL
    if hasattr(os, "O_NOFOLLOW"):
        flags |= os.O_NOFOLLOW
    descriptor = os.open(path, flags, 0o600)
    try:
        with os.fdopen(descriptor, "w", encoding="utf-8", newline="") as handle:
            handle.write(content)
            handle.flush()
            os.fsync(handle.fileno())
    except Exception:
        try:
            path.unlink(missing_ok=True)
        finally:
            raise


def write_outputs(
    output: Path,
    audit: Audit,
    mode: str,
    evidence_class: str,
    run_id: str,
    generated_at: str,
    blockers: list[str],
    input_hashes: dict[str, object],
) -> tuple[str, str | None]:
    status = "BLOCKED" if audit.errors else ("CONDITIONAL" if audit.warnings else "READY")
    ready_for = "G1_INVENTORY" if status == "READY" else None
    report = {
        "schema_version": "1.1",
        "run_id": run_id,
        "generated_at": generated_at,
        "status": status,
        "mode": mode,
        "evidence_class": evidence_class.upper(),
        "input_hashes": input_hashes,
        "readiness_blockers": blockers,
        "counts": {
            "inventory_items": len(audit.inventory),
            "help_documents": audit.help_count,
            "help_identities": len(audit.help_identities),
            "errors": len(audit.errors),
            "warnings": len(audit.warnings),
            "possible_secret_files": len(audit.secret_alerts),
            "artifact_bytes_budgeted": audit.total_artifact_bytes,
        },
        "errors": audit.errors,
        "warnings": audit.warnings,
        "secret_alerts": audit.secret_alerts,
        "help_identities": audit.help_identities,
        "runtime_assessment": audit.runtime_assessment,
        "acceptance_assessment": audit.acceptance_assessment,
        "inventory": audit.inventory,
    }
    if ready_for:
        report["ready_for"] = ready_for
    write_text_exclusive(
        output / "report.json",
        json.dumps(report, ensure_ascii=False, indent=2) + "\n",
    )

    fields = ["evidence_id", "group", "path", "kind", "size_bytes", "sha256", "status", "notes"]
    csv_buffer = io.StringIO(newline="")
    writer = csv.DictWriter(csv_buffer, fieldnames=fields)
    writer.writeheader()
    for row in audit.inventory:
        writer.writerow({key: csv_safe(row.get(key, "")) for key in fields})
    write_text_exclusive(output / "inventory.csv", csv_buffer.getvalue())

    lines = [
        "# Relatório de pré-flight WX",
        "",
        f"- Run: **{markdown_text(run_id)}**",
        f"- Status: **{status}**",
        f"- Classe de evidência: **{evidence_class.upper()}**",
        f"- Modo: **{markdown_text(mode)}**",
        f"- Ready for: **{ready_for or 'não'}**",
        f"- Itens inventariados: {len(audit.inventory)}",
        f"- Erros: {len(audit.errors)}",
        f"- Alertas: {len(audit.warnings)}",
        "",
        "## Erros bloqueantes",
        "",
    ]
    lines.extend([
        f"- {markdown_text(item['code'])} — {markdown_text(item['message'])} "
        f"({markdown_text(item['group'])} {markdown_text(item['item'])})"
        for item in audit.errors
    ] or ["- Nenhum."])
    lines.extend(["", "## Alertas", ""])
    lines.extend([
        f"- {markdown_text(item['code'])} — {markdown_text(item['message'])} "
        f"({markdown_text(item['group'])} {markdown_text(item['item'])})"
        for item in audit.warnings
    ] or ["- Nenhum."])
    lines.extend([
        "",
        "## Limite da conclusão",
        "",
        "- NATIVE: pode buscar equivalência apenas no build, ambiente, dataset e tolerâncias aprovados.",
        "- DOCUMENTARY: reconstrução assistida; lacunas permanecem explícitas.",
        "- FORENSIC: reconstruído conforme evidências; não afirmar equivalência 1:1.",
    ])
    write_text_exclusive(output / "report.md", "\n".join(lines) + "\n")

    gap_lines = ["# Lacunas detectadas", ""]
    all_issues = [("BLOCKER", item) for item in audit.errors] + [
        ("WARNING", item) for item in audit.warnings
    ]
    for index, (severity, item) in enumerate(all_issues, 1):
        gap_lines.extend([
            f"## GAP-{index:04d}",
            "",
            f"- Severidade: {severity}",
            f"- Código: {markdown_text(item['code'])}",
            f"- Grupo: {markdown_text(item['group'] or '-')}",
            f"- Item: {markdown_text(item['item'] or '-')}",
            f"- Problema: {markdown_text(item['message'])}",
            "- Responsável: a definir",
            "- Condição de desbloqueio: a definir",
            "",
        ])
    if not all_issues:
        gap_lines.append("Nenhuma lacuna detectada pelo pré-flight determinístico.")
    write_text_exclusive(output / "gaps.md", "\n".join(gap_lines) + "\n")
    return status, ready_for


def run(
    manifest_path: Path,
    output: Path,
    config_path: Path | None = None,
    *,
    allowed_evidence_root: Path | None = None,
    workspace_root: Path | None = None,
) -> RunResult:
    manifest_requested = Path(os.path.abspath(manifest_path))
    if workspace_root is None:
        workspace = manifest_requested.parent.resolve(strict=True)
    else:
        workspace_lexical = Path(os.path.abspath(workspace_root))
        ensure_absolute_no_symlink_components(workspace_lexical, "workspace-root")
        workspace = workspace_lexical.resolve(strict=True)
    if not workspace.is_dir():
        raise ValueError("workspace-root precisa ser diretório")
    manifest_resolved = resolve_workspace_member(workspace, manifest_requested, "manifest", must_exist=True)
    manifest, manifest_hash = load_json_with_hash(
        manifest_resolved, "manifesto", MAX_MANIFEST_SIZE
    )

    explicit_evidence: Path | None = None
    if allowed_evidence_root is not None:
        explicit_evidence_lexical = Path(os.path.abspath(allowed_evidence_root))
        ensure_absolute_no_symlink_components(explicit_evidence_lexical, "allowed-evidence-root")
        explicit_evidence = explicit_evidence_lexical.resolve(strict=True)
        if not explicit_evidence.is_dir():
            raise ValueError("allowed-evidence-root precisa ser diretório")

    audit = Audit()
    if manifest.get("schema_version") != "1.0":
        audit.issue("error", "MANIFEST_VERSION", "schema_version do manifesto precisa ser 1.0.")
    evidence_root, evidence_root_valid = derive_evidence_root(
        manifest_resolved,
        manifest,
        workspace,
        explicit_evidence,
        audit,
    )

    if config_path is None:
        sibling = manifest_resolved.parent / "conversion.config.json"
        config_requested = sibling if sibling.exists() else None
    else:
        config_requested = Path(os.path.abspath(config_path))
    if config_requested is None:
        config = default_inventory_config()
        config_bytes = json.dumps(
            config, ensure_ascii=False, sort_keys=True, separators=(",", ":"), allow_nan=False
        ).encode("utf-8")
        config_hash = hashlib.sha256(config_bytes).hexdigest()
        config_source = "generated-default"
        audit.issue(
            "warning",
            "CONFIG_MISSING",
            "conversion.config.json não encontrado; assumido somente modo inventory.",
        )
    else:
        config_resolved = resolve_workspace_member(workspace, config_requested, "config", must_exist=True)
        config, config_hash = load_json_with_hash(
            config_resolved, "configuração", MAX_CONFIG_SIZE
        )
        config_source = "file"
        if config.get("schema_version") != "1.0":
            audit.issue("error", "CONFIG_VERSION", "schema_version da configuração precisa ser 1.0.")

    context = validate_project_and_config(manifest, config, audit)
    artifacts = manifest.get("artifacts")
    if not isinstance(artifacts, dict):
        audit.issue("error", "ARTIFACTS_MISSING", "Objeto artifacts ausente ou inválido.")
        artifacts = {}

    for unknown_group in sorted(set(artifacts) - KNOWN_ARTIFACT_GROUPS):
        audit.issue(
            "warning",
            "UNKNOWN_GROUP",
            "Grupo desconhecido não foi aberto nem inventariado.",
            unknown_group,
        )

    for group_name in sorted(KNOWN_ARTIFACT_GROUPS):
        group = artifacts.get(group_name)
        if not isinstance(group, dict):
            audit.issue(
                "error",
                "GROUP_MISSING",
                "Grupo ausente ou inválido no manifesto.",
                group_name,
            )
            continue
        status_value = group.get("status")
        allowed_statuses = VALID_HELP_STATUSES if group_name == "wlanguage_help_json" else VALID_STATUSES
        if not isinstance(status_value, str) or status_value not in allowed_statuses:
            audit.issue("error", "INVALID_STATUS", "Status inválido ou com tipo incorreto.", group_name)
            continue
        notes = group.get("notes", "")
        if not isinstance(notes, str):
            audit.issue("error", "INVALID_NOTES", "notes precisa ser string.", group_name)
            notes = ""
        items = group.get("items")
        if not isinstance(items, list):
            audit.issue("error", "INVALID_ITEMS", "items precisa ser lista.", group_name)
            items = []
        elif len(items) > MAX_ITEMS_PER_GROUP:
            audit.issue("error", "ITEM_LIMIT", f"Grupo excede {MAX_ITEMS_PER_GROUP} itens.", group_name)
            items = []

        if status_value == "not_applicable":
            if not notes.strip():
                audit.issue("error", "NA_WITHOUT_REASON", "not_applicable exige justificativa.", group_name)
            if group_name in CORE_GROUPS:
                audit.issue(
                    "error",
                    "CORE_NOT_APPLICABLE",
                    "Grupo central não pode ser descartado sem redefinir o escopo.",
                    group_name,
                )
            continue
        if status_value == "missing":
            level = "error" if group_name in CORE_GROUPS else "warning"
            if context["missing_artifact_policy"] == "allow-scoped-analysis" and group_name not in CORE_GROUPS:
                level = "warning"
            audit.issue(level, "ARTIFACT_MISSING", "Grupo declarado como ausente.", group_name)
            continue
        if status_value == "partial":
            audit.issue("warning", "ARTIFACT_PARTIAL", notes or "Grupo declarado como parcial.", group_name)

        if group_name == "wlanguage_help_json":
            if status_value == "bundled":
                audit_bundled_help_group(
                    group,
                    audit,
                    expected_version=context["help_version"],
                    expected_language=context["help_language"],
                    expected_products=context["products"],
                )
            elif not evidence_root_valid:
                audit.issue(
                    "error",
                    "EVIDENCE_ROOT_NOT_APPROVED",
                    "Help não foi lido porque evidence-root não foi aprovado.",
                    group_name,
                )
            else:
                audit_help_group(
                    evidence_root,
                    group,
                    audit,
                    expected_version=context["help_version"],
                    expected_language=context["help_language"],
                    expected_products=context["products"],
                )
            continue
        if group_name == "external_links":
            audit_links(group, audit)
            continue

        if status_value == "provided" and not items:
            audit.issue("error", "EMPTY_PROVIDED_GROUP", "Grupo provided sem itens.", group_name)
        for item in items:
            if not isinstance(item, dict):
                audit.issue("error", "INVALID_ITEM", "Item precisa ser objeto com path.", group_name)
                continue
            if not evidence_root_valid:
                audit.issue(
                    "error",
                    "EVIDENCE_ROOT_NOT_APPROVED",
                    "Artefato não foi lido porque evidence-root não foi aprovado.",
                    group_name,
                )
                break
            audit_regular_file(evidence_root, group_name, item, audit)

    validate_runtime_and_acceptance(manifest, config, context, audit)
    evidence_class = classify_evidence(manifest, config)
    requested_class = context["requested_evidence_class"]
    if requested_class not in {"auto", evidence_class}:
        audit.issue(
            "error",
            "EVIDENCE_CLASS_MISMATCH",
            f"Classe solicitada {requested_class} não é sustentada; detectada {evidence_class}.",
        )

    blockers = readiness_blockers(manifest, audit)
    canonical_inputs = json.dumps(
        {"manifest": manifest, "config": config},
        ensure_ascii=False,
        sort_keys=True,
        separators=(",", ":"),
        allow_nan=False,
    ).encode("utf-8")
    input_digest = hashlib.sha256(canonical_inputs).hexdigest()
    now = datetime.now(timezone.utc)
    generated_at = now.isoformat()
    run_id = f"run-{now.strftime('%Y%m%dT%H%M%S%fZ')}-{input_digest[:12]}"
    staging_directory, output_directory = prepare_versioned_output(
        output,
        workspace,
        evidence_root,
        run_id,
    )
    input_hashes: dict[str, object] = {
        "algorithm": "sha256",
        "manifest_sha256": manifest_hash,
        "config_sha256": config_hash,
        "config_source": config_source,
    }
    try:
        status, ready_for = write_outputs(
            staging_directory,
            audit,
            context["mode"],
            evidence_class,
            run_id,
            generated_at,
            blockers,
            input_hashes,
        )
        write_text_exclusive(
            staging_directory / "COMPLETED.json",
            json.dumps(
                {
                    "schema_version": "1.0",
                    "run_id": run_id,
                    "status": status,
                    "completed_at": datetime.now(timezone.utc).isoformat(),
                    "input_hashes": input_hashes,
                },
                ensure_ascii=False,
                indent=2,
            ) + "\n",
        )
        promote_completed_staging(staging_directory, output_directory)
    except Exception:
        if os.path.lexists(staging_directory):
            shutil.rmtree(staging_directory, ignore_errors=True)
        raise
    return RunResult(status, output_directory, run_id, ready_for)


def main() -> int:
    parser = argparse.ArgumentParser(
        description="Audita anexos WX com raízes explícitas, limites e saída imutável."
    )
    parser.add_argument("--manifest", required=True, type=Path)
    parser.add_argument("--allowed-evidence-root", required=True, type=Path)
    parser.add_argument("--workspace-root", required=True, type=Path)
    parser.add_argument("--output", required=True, type=Path)
    parser.add_argument("--config", type=Path)
    args = parser.parse_args()
    try:
        result = run(
            args.manifest,
            args.output,
            args.config,
            allowed_evidence_root=args.allowed_evidence_root,
            workspace_root=args.workspace_root,
        )
    except (OSError, ValueError) as exc:
        print(f"preflight inválido: {clean_text(str(exc), 2000)}", file=sys.stderr)
        return 4
    print(json.dumps({
        "status": str(result),
        "ready_for": result.ready_for,
        "run_id": result.run_id,
        "output": str(result.output_dir),
    }, ensure_ascii=False))
    return {"READY": 0, "CONDITIONAL": 2, "BLOCKED": 3}[str(result)]


if __name__ == "__main__":
    raise SystemExit(main())
