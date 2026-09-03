#!/usr/bin/env python3
"""Verify and query the bundled WLanguage Help corpus without extracting it.

The archive is treated as hostile input even when its SHA-256 is known.  This
utility never extracts a member, imports corpus content, executes code, or uses
the network.  Output is deliberately bounded and contains metadata plus short
excerpts only.
"""

from __future__ import annotations

import argparse
import configparser
import hashlib
import json
import os
import re
import stat
import sys
import time
import unicodedata
import zipfile
from collections import defaultdict
from collections.abc import Iterator
from contextlib import contextmanager
from dataclasses import dataclass
from pathlib import Path, PurePosixPath
from typing import BinaryIO
from urllib.parse import urlsplit


ARCHIVE_NAME = "Help_WL_12k_Json.zip"
ARCHIVE_ROOT = "Help_WL_12k_Json"
DEFAULT_CORPUS = Path(__file__).resolve().parent.parent / "resources" / ARCHIVE_NAME
EXPECTED_SHA256 = "a95ed5536549ecc39fb1163415042d6597c8913e5edbfdb531cba756546a82a2"
SOURCE_SHA256 = "a6b42f59796ccf51298712aff00c043a9be2c404ce761a99720ea31b91ca6b93"
EXPECTED_JSON_COUNT = 12_037
EXPECTED_PAGE_COUNT = 12_036
EXPECTED_FILE_COUNT = 12_038
EXPECTED_MEMBER_COUNT = 12_039
EXPECTED_UNCOMPRESSED_BYTES = 115_844_631
REDACTED_PRIVATE_KEY_MEMBERS = 2
REDACTED_PRIVATE_KEY_BLOCKS = 15
INDEX_MEMBER = f"{ARCHIVE_ROOT}/00_indice_de_grupos.json"
PROGRESS_MEMBER = f"{ARCHIVE_ROOT}/progresso.ini"

# This defect is tolerated only for the exact, pinned upstream archive.  It is
# always quarantined and surfaced in both verification and query output.
KNOWN_QUARANTINED_MEMBER = (
    f"{ARCHIVE_ROOT}/"
    "01-04-01_00655__emailgetall_function__1000018727.json"
)
KNOWN_QUARANTINED_SIZE = 23_627
KNOWN_QUARANTINED_SHA256 = (
    "d95886e1dc971804e4fe98c784504c54665c5aa4a4adcc4de90e4f58e54e5148"
)

MAX_ARCHIVE_BYTES = 64 * 1024 * 1024
MAX_MEMBER_BYTES = 1024 * 1024
MAX_TOTAL_BYTES = 128 * 1024 * 1024
MAX_MEMBERS = 12_064
MAX_COMPRESSION_RATIO = 700.0
MAX_PATH_CHARS = 240
MAX_COMPONENT_CHARS = 180
MAX_JSON_DEPTH = 64
MAX_JSON_NODES = 200_000
MAX_QUERY_COUNT = 8
MAX_QUERY_CHARS = 200
MAX_INDEXED_CHARS = 750_000
MAX_EXCERPT_CHARS = 400
MAX_OUTPUT_BYTES = 256 * 1024
MAX_REPORTED_GAPS = 100
MAX_DUPLICATE_SAMPLES = 20
READ_CHUNK = 64 * 1024
ALLOWED_COMPRESSION = {zipfile.ZIP_STORED, zipfile.ZIP_DEFLATED}
WINDOWS_RESERVED = {
    "CON", "PRN", "AUX", "NUL",
    *(f"COM{number}" for number in range(1, 10)),
    *(f"LPT{number}" for number in range(1, 10)),
}
PAGE_NAME_RE = re.compile(
    rf"^{re.escape(ARCHIVE_ROOT)}/"
    r"(?P<group>\d{2}-\d{2}-\d{2})_(?P<sequence>\d{5})__.+\.json$"
)
SHA256_RE = re.compile(r"^[0-9a-fA-F]{64}$")
WORD_RE = re.compile(r"\w+", re.UNICODE)
PRIVATE_KEY_PEM_RE = re.compile(
    rb"-----BEGIN (?:(?:RSA|EC|DSA|OPENSSH) )?PRIVATE KEY-----"
)

FIELD_SPECS: tuple[tuple[str, tuple[str, ...], int], ...] = (
    ("name", ("nome",), 120),
    ("short_name", ("nome_curto",), 120),
    ("title", ("titulo_do_mapa", "titulo_da_pagina", "titulo", "title"), 100),
    ("trail", ("trilha",), 70),
    ("syntax", ("sintaxes",), 60),
    ("description", ("descricao",), 25),
    ("code", ("codigos",), 10),
)


class CorpusError(ValueError):
    """A controlled failure for an unsafe or incompatible corpus."""


class MalformedDocument(ValueError):
    """A syntactically invalid JSON document (distinct from invalid Unicode)."""


@dataclass(frozen=True)
class AuditResult:
    metadata: dict[str, object]
    page_members: tuple[str, ...]


def require_unicode(value: str, label: str) -> None:
    try:
        value.encode("utf-8", errors="strict")
    except UnicodeEncodeError as exc:
        raise CorpusError(f"{label}: Unicode contém surrogate isolado") from exc


def normalize(value: str) -> str:
    decomposed = unicodedata.normalize("NFKD", value).casefold()
    return " ".join(
        "".join(character for character in decomposed if not unicodedata.combining(character)).split()
    )


def bounded_int(raw: str) -> int:
    try:
        value = int(raw)
    except ValueError as exc:
        raise argparse.ArgumentTypeError("deve ser um inteiro entre 1 e 50") from exc
    if not 1 <= value <= 50:
        raise argparse.ArgumentTypeError("deve estar entre 1 e 50")
    return value


def safe_member_name(raw: str) -> str:
    require_unicode(raw, "nome de membro ZIP")
    if raw != unicodedata.normalize("NFC", raw):
        raise CorpusError("nome de membro ZIP não está normalizado em NFC")
    if not raw or len(raw) > MAX_PATH_CHARS:
        raise CorpusError("nome de membro ZIP vazio ou longo demais")
    if "\\" in raw or "\x00" in raw or any(ord(ch) < 32 or ord(ch) == 127 for ch in raw):
        raise CorpusError("nome de membro ZIP contém caractere inseguro")
    if raw.startswith("/") or re.match(r"^[A-Za-z]:", raw):
        raise CorpusError("nome de membro ZIP precisa ser relativo")
    directory = raw.endswith("/")
    candidate = raw[:-1] if directory else raw
    parts = candidate.split("/")
    if not parts or any(part in {"", ".", ".."} for part in parts):
        raise CorpusError("nome de membro ZIP contém travessia ou componente vazio")
    for part in parts:
        if len(part) > MAX_COMPONENT_CHARS or part.endswith((" ", ".")) or ":" in part:
            raise CorpusError("nome de membro ZIP contém componente não portátil")
        if part.split(".", 1)[0].upper() in WINDOWS_RESERVED:
            raise CorpusError("nome de membro ZIP usa nome reservado")
    path = PurePosixPath(*parts)
    if path.is_absolute() or ".." in path.parts:
        raise CorpusError("nome de membro ZIP não é relativo e seguro")
    return path.as_posix() + ("/" if directory else "")


def validate_url(value: object) -> str:
    if not isinstance(value, str) or not value or len(value) > 2_048:
        raise CorpusError("página contém URL ausente ou longa demais")
    require_unicode(value, "URL da página")
    try:
        parsed = urlsplit(value)
        port = parsed.port
    except ValueError as exc:
        raise CorpusError("página contém URL inválida") from exc
    if (
        parsed.scheme != "https"
        or parsed.hostname != "help.windev.com"
        or parsed.username is not None
        or parsed.password is not None
        or port not in {None, 443}
    ):
        raise CorpusError("página contém URL fora de https://help.windev.com")
    return value


def _reject_constant(_value: str) -> object:
    raise CorpusError("JSON contém número não finito")


def _unique_object(pairs: list[tuple[str, object]]) -> dict[str, object]:
    result: dict[str, object] = {}
    for key, value in pairs:
        if key in result:
            raise CorpusError("JSON contém chave duplicada")
        result[key] = value
    return result


def validate_json_tree(root: object) -> None:
    stack: list[tuple[object, int]] = [(root, 0)]
    nodes = 0
    while stack:
        value, depth = stack.pop()
        nodes += 1
        if nodes > MAX_JSON_NODES:
            raise CorpusError("JSON excede o limite de nós")
        if depth > MAX_JSON_DEPTH:
            raise CorpusError("JSON excede o limite de profundidade")
        if isinstance(value, str):
            require_unicode(value, "texto JSON")
        elif isinstance(value, dict):
            for key, child in value.items():
                require_unicode(key, "chave JSON")
                stack.append((child, depth + 1))
        elif isinstance(value, list):
            stack.extend((child, depth + 1) for child in value)


def load_json_object(raw: bytes) -> dict[str, object]:
    try:
        text = raw.decode("utf-8", errors="strict")
    except UnicodeDecodeError as exc:
        raise CorpusError("membro JSON contém UTF-8 inválido") from exc
    try:
        value = json.loads(
            text,
            object_pairs_hook=_unique_object,
            parse_constant=_reject_constant,
        )
    except json.JSONDecodeError as exc:
        raise MalformedDocument("JSON sintaticamente inválido") from exc
    except RecursionError as exc:
        raise CorpusError("JSON excede o limite de aninhamento") from exc
    validate_json_tree(value)
    if not isinstance(value, dict):
        raise CorpusError("documento JSON precisa ser um objeto")
    return value


def file_fingerprint(file: BinaryIO) -> tuple[int, int, int, int]:
    state = os.fstat(file.fileno())
    return state.st_dev, state.st_ino, state.st_size, state.st_mtime_ns


@contextmanager
def open_pinned_archive(path: Path, expected_sha256: str) -> Iterator[tuple[zipfile.ZipFile, str]]:
    flags = os.O_RDONLY | getattr(os, "O_CLOEXEC", 0) | getattr(os, "O_NOFOLLOW", 0)
    try:
        descriptor = os.open(path, flags)
    except OSError as exc:
        raise CorpusError("corpus não pôde ser aberto com segurança") from exc
    try:
        file = os.fdopen(descriptor, "rb", closefd=True)
    except Exception:
        os.close(descriptor)
        raise
    with file:
        before = os.fstat(file.fileno())
        if not stat.S_ISREG(before.st_mode):
            raise CorpusError("corpus não é um arquivo regular")
        if before.st_size > MAX_ARCHIVE_BYTES:
            raise CorpusError("arquivo ZIP excede o limite de tamanho")
        digest = hashlib.sha256()
        while True:
            block = file.read(1024 * 1024)
            if not block:
                break
            digest.update(block)
        actual_sha256 = digest.hexdigest()
        if actual_sha256 != expected_sha256.lower():
            raise CorpusError("SHA-256 do corpus não corresponde ao valor exigido")
        file.seek(0)
        try:
            archive = zipfile.ZipFile(file, mode="r")
        except (OSError, UnicodeError, ValueError, zipfile.BadZipFile) as exc:
            raise CorpusError("corpus não é um ZIP válido") from exc
        try:
            yield archive, actual_sha256
        finally:
            archive.close()
            if file_fingerprint(file) != (
                before.st_dev,
                before.st_ino,
                before.st_size,
                before.st_mtime_ns,
            ):
                raise CorpusError("corpus foi alterado durante a leitura")


def validate_zip_structure(archive: zipfile.ZipFile) -> dict[str, zipfile.ZipInfo]:
    infos = archive.infolist()
    if len(infos) > MAX_MEMBERS or len(infos) != EXPECTED_MEMBER_COUNT:
        raise CorpusError("quantidade inesperada de membros ZIP")
    names: dict[str, zipfile.ZipInfo] = {}
    portable_names: set[str] = set()
    header_offsets: set[int] = set()
    total_bytes = 0
    file_count = 0
    json_count = 0
    directory_count = 0
    for info in infos:
        name = safe_member_name(info.filename)
        if name != info.filename:
            raise CorpusError("nome de membro ZIP não possui forma canônica")
        portable_key = unicodedata.normalize("NFC", name.rstrip("/")).casefold()
        if name in names or portable_key in portable_names:
            raise CorpusError("ZIP contém nomes duplicados ou colisão portátil")
        if info.header_offset in header_offsets:
            raise CorpusError("ZIP contém membros com cabeçalho sobreposto")
        names[name] = info
        portable_names.add(portable_key)
        header_offsets.add(info.header_offset)
        if info.flag_bits & 0x1:
            raise CorpusError("ZIP contém membro criptografado")
        if info.compress_type not in ALLOWED_COMPRESSION:
            raise CorpusError("ZIP usa método de compressão não permitido")
        if info.file_size < 0 or info.compress_size < 0 or info.file_size > MAX_MEMBER_BYTES:
            raise CorpusError("membro ZIP excede o limite de tamanho")
        if info.file_size and not info.compress_size:
            raise CorpusError("membro ZIP possui razão de compressão inválida")
        if info.compress_size and info.file_size / info.compress_size > MAX_COMPRESSION_RATIO:
            raise CorpusError("membro ZIP excede a razão de compressão permitida")
        mode = (info.external_attr >> 16) & 0xFFFF
        if info.create_system != 3 or not mode:
            raise CorpusError("membro ZIP não possui tipo Unix verificável")
        if stat.S_ISLNK(mode):
            raise CorpusError("ZIP contém link simbólico")
        if not (stat.S_ISREG(mode) or stat.S_ISDIR(mode)):
            raise CorpusError("ZIP contém tipo especial de arquivo")
        total_bytes += info.file_size
        if total_bytes > MAX_TOTAL_BYTES:
            raise CorpusError("ZIP excede o limite total descomprimido")
        if info.is_dir():
            directory_count += 1
        else:
            file_count += 1
            if name.endswith(".json"):
                json_count += 1

    if total_bytes != EXPECTED_UNCOMPRESSED_BYTES:
        raise CorpusError("total descomprimido do ZIP é inesperado")
    if file_count != EXPECTED_FILE_COUNT or json_count != EXPECTED_JSON_COUNT:
        raise CorpusError("contagem de arquivos do corpus é inesperada")
    if directory_count != 1 or f"{ARCHIVE_ROOT}/" not in names:
        raise CorpusError("diretório raiz esperado está ausente ou duplicado")
    if INDEX_MEMBER not in names or PROGRESS_MEMBER not in names:
        raise CorpusError("índice ou progresso.ini está ausente")

    page_names = []
    for name, info in names.items():
        if info.is_dir() or name in {INDEX_MEMBER, PROGRESS_MEMBER}:
            continue
        if not PAGE_NAME_RE.fullmatch(name):
            raise CorpusError("ZIP contém caminho fora do layout permitido")
        page_names.append(name)
    if len(page_names) != EXPECTED_PAGE_COUNT:
        raise CorpusError("quantidade de páginas JSON é inesperada")
    return names


def read_member(archive: zipfile.ZipFile, info: zipfile.ZipInfo) -> tuple[bytes, str]:
    if info.file_size > MAX_MEMBER_BYTES:
        raise CorpusError("membro excede o limite antes da leitura")
    digest = hashlib.sha256()
    blocks: list[bytes] = []
    consumed = 0
    try:
        with archive.open(info, mode="r") as source:
            while True:
                block = source.read(min(READ_CHUNK, MAX_MEMBER_BYTES + 1 - consumed))
                if not block:
                    break
                consumed += len(block)
                if consumed > MAX_MEMBER_BYTES or consumed > info.file_size:
                    raise CorpusError("membro expandiu além do tamanho declarado")
                digest.update(block)
                blocks.append(block)
    except (OSError, EOFError, RuntimeError, ValueError, zipfile.BadZipFile) as exc:
        raise CorpusError("membro ZIP falhou na leitura/CRC") from exc
    if consumed != info.file_size:
        raise CorpusError("tamanho real de membro diverge do diretório ZIP")
    return b"".join(blocks), digest.hexdigest()


def page_identity(document: dict[str, object]) -> tuple[str, str]:
    identifier = document.get("identificador")
    if not isinstance(identifier, str) or not identifier or len(identifier) > 128:
        raise CorpusError("página contém identificador inválido")
    require_unicode(identifier, "identificador")
    return identifier, validate_url(document.get("url"))


def parse_progress(raw: bytes) -> tuple[dict[str, int], list[str]]:
    try:
        text = raw.decode("utf-8", errors="strict")
    except UnicodeDecodeError as exc:
        raise CorpusError("progresso.ini contém UTF-8 inválido") from exc
    require_unicode(text, "progresso.ini")
    parser = configparser.ConfigParser(interpolation=None, strict=True)
    try:
        parser.read_string(text)
    except configparser.Error as exc:
        raise CorpusError("progresso.ini é inválido") from exc
    required = ("total_do_mapa", "ultima_posicao", "processadas", "falhas", "restantes")
    try:
        values = {key: parser.getint("colheita", key) for key in required}
    except (configparser.Error, ValueError) as exc:
        raise CorpusError("progresso.ini não contém contadores válidos") from exc
    issues: list[str] = []
    if values["processadas"] + values["falhas"] + values["restantes"] != values["total_do_mapa"]:
        issues.append("processed_failed_remaining_do_not_sum_to_total")
    if values["restantes"] == 0 and values["processadas"] < values["total_do_mapa"]:
        issues.append("zero_remaining_conflicts_with_processed")
    if values["ultima_posicao"] >= values["total_do_mapa"] and values["processadas"] < values["total_do_mapa"]:
        issues.append("final_position_conflicts_with_processed")
    return values, issues


def index_page_count(document: dict[str, object]) -> tuple[int, int]:
    themes = document.get("temas")
    declared_themes = document.get("total_de_temas")
    if not isinstance(themes, list) or not isinstance(declared_themes, int):
        raise CorpusError("índice de grupos possui estrutura inválida")
    total = 0
    for theme in themes:
        if not isinstance(theme, dict) or not isinstance(theme.get("paginas"), int):
            raise CorpusError("índice de grupos contém tema inválido")
        pages = theme["paginas"]
        if pages < 0:
            raise CorpusError("índice de grupos contém contagem negativa")
        total += pages
    return declared_themes, total


def audit_archive(
    archive: zipfile.ZipFile,
    actual_sha256: str,
    infos: dict[str, zipfile.ZipInfo],
) -> AuditResult:
    index_raw, _ = read_member(archive, infos[INDEX_MEMBER])
    index_document = load_json_object(index_raw)
    declared_themes, index_pages = index_page_count(index_document)
    progress_raw, _ = read_member(archive, infos[PROGRESS_MEMBER])
    progress, progress_issues = parse_progress(progress_raw)

    page_members = sorted(name for name in infos if PAGE_NAME_RE.fullmatch(name))
    identifiers: dict[str, list[str]] = defaultdict(list)
    group_sequences: dict[str, set[int]] = defaultdict(set)
    quarantined: list[dict[str, object]] = []
    valid_pages = 0

    for member in page_members:
        match = PAGE_NAME_RE.fullmatch(member)
        assert match is not None
        group_sequences[match.group("group")].add(int(match.group("sequence")))
        raw, member_sha256 = read_member(archive, infos[member])
        if PRIVATE_KEY_PEM_RE.search(raw):
            raise CorpusError("página contém bloco PEM de chave privada não sanitizado")
        try:
            document = load_json_object(raw)
        except MalformedDocument as exc:
            known_defect = (
                actual_sha256 == EXPECTED_SHA256
                and member == KNOWN_QUARANTINED_MEMBER
                and len(raw) == KNOWN_QUARANTINED_SIZE
                and member_sha256 == KNOWN_QUARANTINED_SHA256
                and raw == b"\x00" * KNOWN_QUARANTINED_SIZE
            )
            if not known_defect:
                raise CorpusError("página JSON inválida fora da quarentena aprovada") from exc
            quarantined.append(
                {
                    "member": member,
                    "reason": "known_zero_filled_invalid_json",
                    "bytes": len(raw),
                    "sha256": member_sha256,
                }
            )
            continue
        identifier, _url = page_identity(document)
        identifiers[identifier].append(member)
        valid_pages += 1

    gaps: list[dict[str, object]] = []
    gaps_total = 0
    for group in sorted(group_sequences):
        sequences = group_sequences[group]
        if not sequences:
            continue
        missing = sorted(set(range(1, max(sequences) + 1)) - sequences)
        gaps_total += len(missing)
        if missing and len(gaps) < MAX_REPORTED_GAPS:
            room = MAX_REPORTED_GAPS - sum(len(item["missing_sequences"]) for item in gaps)
            if room > 0:
                gaps.append({"group": group, "missing_sequences": missing[:room]})

    duplicate_ids = sorted(identifier for identifier, members in identifiers.items() if len(members) > 1)
    duplicate_extra_pages = sum(len(identifiers[identifier]) - 1 for identifier in duplicate_ids)
    index_issues: list[str] = []
    if index_pages != len(page_members):
        index_issues.append("declared_pages_do_not_match_physical_pages")
    if declared_themes != len(index_document.get("temas", [])):
        index_issues.append("declared_theme_count_mismatch")

    degraded = bool(
        quarantined
        or gaps_total
        or duplicate_ids
        or index_issues
        or progress_issues
    )
    metadata: dict[str, object] = {
        "status": "DEGRADED/CONDITIONAL" if degraded else "VERIFIED",
        "archive": {
            "filename": ARCHIVE_NAME,
            "sha256": actual_sha256,
            "members": len(infos),
            "files": EXPECTED_FILE_COUNT,
            "json_documents": EXPECTED_JSON_COUNT,
            "page_documents": len(page_members),
            "valid_page_documents": valid_pages,
            "uncompressed_bytes": EXPECTED_UNCOMPRESSED_BYTES,
        },
        "quarantined_members": quarantined,
        "gaps": {
            "count": gaps_total,
            "items": gaps,
            "truncated": gaps_total > sum(len(item["missing_sequences"]) for item in gaps),
        },
        "logical_duplicates": {
            "duplicated_ids": len(duplicate_ids),
            "extra_pages": duplicate_extra_pages,
            "sample_ids": duplicate_ids[:MAX_DUPLICATE_SAMPLES],
            "sample_truncated": len(duplicate_ids) > MAX_DUPLICATE_SAMPLES,
        },
        "group_index": {
            "declared_themes": declared_themes,
            "declared_pages": index_pages,
            "physical_pages": len(page_members),
            "issues": index_issues,
        },
        "progress": {
            **progress,
            "issues": progress_issues,
        },
        "sanitization": {
            "source_sha256": SOURCE_SHA256,
            "private_key_members_redacted": REDACTED_PRIVATE_KEY_MEMBERS,
            "private_key_blocks_redacted": REDACTED_PRIVATE_KEY_BLOCKS,
            "private_key_pem_blocks_remaining": 0,
        },
    }
    return AuditResult(metadata=metadata, page_members=tuple(page_members))


def iter_strings(value: object) -> Iterator[str]:
    stack = [value]
    while stack:
        current = stack.pop()
        if isinstance(current, str):
            yield current
        elif isinstance(current, list):
            stack.extend(reversed(current))
        elif isinstance(current, dict):
            stack.extend(reversed(list(current.values())))


def selected_fields(document: dict[str, object]) -> list[tuple[str, int, str]]:
    fields: list[tuple[str, int, str]] = []
    indexed_chars = 0
    for label, keys, weight in FIELD_SPECS:
        for key in keys:
            if key not in document:
                continue
            for text in iter_strings(document[key]):
                indexed_chars += len(text)
                if indexed_chars > MAX_INDEXED_CHARS:
                    raise CorpusError("página excede o limite de texto pesquisável")
                fields.append((label, weight, text))
    return fields


def score_document(
    fields: list[tuple[str, int, str]],
    phrases: tuple[str, ...],
    terms: tuple[str, ...],
) -> tuple[int, str, str] | None:
    seen_terms: set[str] = set()
    best_by_label: dict[str, tuple[int, str]] = {}
    for label, weight, raw in fields:
        haystack = normalize(raw)
        if not haystack:
            continue
        matched_terms = {term for term in terms if term in haystack}
        seen_terms.update(matched_terms)
        score = weight * len(matched_terms)
        for phrase in phrases:
            if haystack == phrase:
                score += weight * 12
            elif haystack.startswith(phrase):
                score += weight * 8
            elif re.search(rf"(?<!\w){re.escape(phrase)}(?!\w)", haystack):
                score += weight * 5
            elif phrase in haystack:
                score += weight * 3
        previous = best_by_label.get(label)
        if score and (previous is None or score > previous[0] or (score == previous[0] and raw < previous[1])):
            best_by_label[label] = (score, raw)
    if not set(terms).issubset(seen_terms):
        return None
    total = sum(score for score, _raw in best_by_label.values())
    label, (_score, best_raw) = max(
        best_by_label.items(),
        key=lambda item: (item[1][0], dict((spec[0], spec[2]) for spec in FIELD_SPECS)[item[0]], -len(item[1][1])),
    )
    return total, label, best_raw


def clipped(value: object, limit: int) -> str | None:
    if not isinstance(value, str):
        return None
    require_unicode(value, "metadado")
    compact = " ".join(value.split())
    if len(compact) <= limit:
        return compact
    return compact[: max(0, limit - 1)].rstrip() + "…"


def string_list(value: object, *, item_limit: int, count_limit: int) -> list[str]:
    if not isinstance(value, list):
        return []
    result: list[str] = []
    for item in value[:count_limit]:
        rendered = clipped(item, item_limit)
        if rendered is not None:
            result.append(rendered)
    return result


def excerpt(value: str, raw_queries: tuple[str, ...]) -> str:
    compact = " ".join(value.split())
    lowered = compact.casefold()
    positions = [lowered.find(query.casefold()) for query in raw_queries]
    positions = [position for position in positions if position >= 0]
    center = min(positions) if positions else 0
    start = max(0, center - 100)
    end = min(len(compact), start + MAX_EXCERPT_CHARS)
    if end - start < MAX_EXCERPT_CHARS:
        start = max(0, end - MAX_EXCERPT_CHARS)
    rendered = compact[start:end]
    if start:
        rendered = "…" + rendered[1:]
    if end < len(compact):
        rendered = rendered[:-1] + "…"
    return rendered[:MAX_EXCERPT_CHARS]


def matches_filter(document: dict[str, object], version: str | None, platform: str | None) -> bool:
    if version is not None:
        versions = {normalize(item) for item in document.get("versoes", []) if isinstance(item, str)}
        if normalize(version) not in versions:
            return False
    if platform is not None:
        platforms = {normalize(item) for item in document.get("plataformas", []) if isinstance(item, str)}
        if normalize(platform) not in platforms:
            return False
    return True


def query_archive(
    archive: zipfile.ZipFile,
    infos: dict[str, zipfile.ZipInfo],
    audit: AuditResult,
    raw_queries: tuple[str, ...],
    version: str | None,
    platform: str | None,
    limit: int,
    groups: tuple[str, ...] = (),
) -> tuple[list[dict[str, object]], int]:
    phrases = tuple(dict.fromkeys(normalize(query) for query in raw_queries))
    terms = tuple(dict.fromkeys(term for phrase in phrases for term in WORD_RE.findall(phrase)))
    if not terms:
        raise CorpusError("consulta não contém termos pesquisáveis")
    ranked: list[tuple[tuple[object, ...], dict[str, object]]] = []
    matched = 0
    quarantined = {
        item["member"]
        for item in audit.metadata["quarantined_members"]  # type: ignore[index]
    }
    for member in audit.page_members:
        if member in quarantined:
            continue
        # O nome do membro comeca pelo codigo do tema (GG-SS-TT): filtrar por
        # prefixo evita ler o JSON das paginas que o especialista nao cobre.
        if groups and not member.rsplit("/", 1)[-1].startswith(groups):
            continue
        raw, member_sha256 = read_member(archive, infos[member])
        try:
            document = load_json_object(raw)
        except MalformedDocument as exc:
            raise CorpusError("página mudou após a etapa de verificação") from exc
        identifier, url = page_identity(document)
        if not matches_filter(document, version, platform):
            continue
        scored = score_document(selected_fields(document), phrases, terms)
        if scored is None:
            continue
        score, matched_field, matched_text = scored
        matched += 1
        name = clipped(document.get("nome"), 200)
        short_name = clipped(document.get("nome_curto"), 200)
        title = clipped(document.get("titulo_do_mapa"), 240) or clipped(
            document.get("titulo_da_pagina"), 240
        )
        result: dict[str, object] = {
            "member": member,
            "id": identifier,
            "name": name,
            "short_name": short_name,
            "title": title,
            "trail": string_list(document.get("trilha"), item_limit=120, count_limit=8),
            "versions": string_list(document.get("versoes"), item_limit=32, count_limit=16),
            "platforms": string_list(document.get("plataformas"), item_limit=64, count_limit=8),
            "url": url,
            "member_sha256": member_sha256,
            "score": score,
            "matched_field": matched_field,
            "excerpt": excerpt(matched_text, raw_queries),
        }
        sort_key = (-score, normalize(title or name or ""), identifier, member)
        ranked.append((sort_key, result))
    ranked.sort(key=lambda item: item[0])
    return [result for _key, result in ranked[:limit]], matched


def validate_cli_text(value: str | None, label: str, max_chars: int) -> None:
    if value is None:
        return
    require_unicode(value, label)
    if not value.strip() or len(value) > max_chars or any(ord(ch) < 32 and ch not in "\t\n\r" for ch in value):
        raise CorpusError(f"{label} vazio, longo demais ou com controles inválidos")


def emit_json(payload: dict[str, object], *, stream: object = sys.stdout) -> None:
    rendered = json.dumps(payload, ensure_ascii=False, sort_keys=True, separators=(",", ":"))
    if len(rendered.encode("utf-8")) > MAX_OUTPUT_BYTES:
        raise CorpusError("saída excederia o limite permitido")
    stream.write(rendered + "\n")  # type: ignore[attr-defined]


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(
        description="Verifica ou consulta, sem extrair, o corpus WLanguage Help empacotado.",
    )
    action = parser.add_mutually_exclusive_group(required=True)
    action.add_argument("--verify", action="store_true", help="audita identidade, estrutura e defeitos conhecidos")
    action.add_argument(
        "--query",
        action="append",
        metavar="TEXT",
        help="texto de busca; pode ser repetido (todos os termos precisam ocorrer)",
    )
    parser.add_argument("--version", help="filtra pela versão declarada, por exemplo 2026")
    parser.add_argument("--platform", help="filtra pela plataforma declarada")
    parser.add_argument(
        "--group",
        action="append",
        metavar="GG-SS-TT",
        help="restringe aos temas do indice (prefixo do nome do membro); pode ser repetido",
    )
    parser.add_argument("--limit", type=bounded_int, default=10, metavar="1..50")
    parser.add_argument(
        "--corpus",
        type=Path,
        help="override explícito somente para auditoria/teste; exige --expected-sha256",
    )
    parser.add_argument(
        "--expected-sha256",
        metavar="HEX",
        help="SHA-256 obrigatório quando --corpus é usado",
    )
    return parser


def main(argv: list[str] | None = None) -> int:
    parser = build_parser()
    args = parser.parse_args(argv)
    if args.corpus is not None:
        if args.expected_sha256 is None or not SHA256_RE.fullmatch(args.expected_sha256):
            parser.error("--corpus exige --expected-sha256 com 64 dígitos hexadecimais")
        corpus_path = args.corpus
        expected_sha256 = args.expected_sha256.lower()
    else:
        if args.expected_sha256 is not None:
            parser.error("--expected-sha256 só pode ser usado com --corpus")
        corpus_path = DEFAULT_CORPUS
        expected_sha256 = EXPECTED_SHA256
    if args.verify and (args.version is not None or args.platform is not None or args.group):
        parser.error("--version, --platform e --group são válidos somente com --query")
    groups = tuple(args.group or ())
    for group in groups:
        if not re.fullmatch(r"\d{2}(-\d{2}(-\d{2})?)?", group):
            parser.error(f"--group inválido: {group!r} (formato GG, GG-SS ou GG-SS-TT)")
    queries = tuple(args.query or ())
    if len(queries) > MAX_QUERY_COUNT:
        parser.error(f"--query pode ser repetido no máximo {MAX_QUERY_COUNT} vezes")
    try:
        for query in queries:
            validate_cli_text(query, "consulta", MAX_QUERY_CHARS)
        validate_cli_text(args.version, "versão", 64)
        validate_cli_text(args.platform, "plataforma", 64)

        verification_started = time.perf_counter()
        with open_pinned_archive(corpus_path, expected_sha256) as (archive, actual_sha256):
            infos = validate_zip_structure(archive)
            audit = audit_archive(archive, actual_sha256, infos)
            verification_ms = round((time.perf_counter() - verification_started) * 1000, 3)
            if args.verify:
                output = dict(audit.metadata)
                output["verification_elapsed_ms"] = verification_ms
            else:
                search_started = time.perf_counter()
                results, matched = query_archive(
                    archive,
                    infos,
                    audit,
                    queries,
                    args.version,
                    args.platform,
                    args.limit,
                    groups,
                )
                search_ms = round((time.perf_counter() - search_started) * 1000, 3)
                output = {
                    "status": audit.metadata["status"],
                    "archive": audit.metadata["archive"],
                    "query": list(queries),
                    "filters": {"version": args.version, "platform": args.platform, "groups": list(groups)},
                    "matched": matched,
                    "returned": len(results),
                    "quarantined_members": audit.metadata["quarantined_members"],
                    "gaps": audit.metadata["gaps"],
                    "logical_duplicates": audit.metadata["logical_duplicates"],
                    "verification_elapsed_ms": verification_ms,
                    "search_elapsed_ms": search_ms,
                    "results": results,
                }
        emit_json(output)
        return 0
    except (CorpusError, OSError, UnicodeError, zipfile.BadZipFile) as exc:
        try:
            emit_json(
                {"status": "ERROR", "error": str(exc)[:500]},
                stream=sys.stderr,
            )
        except Exception:
            sys.stderr.write('{"status":"ERROR","error":"falha controlada ao ler o corpus"}\n')
        return 2


if __name__ == "__main__":
    raise SystemExit(main())
