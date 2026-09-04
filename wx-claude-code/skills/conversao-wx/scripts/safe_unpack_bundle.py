#!/usr/bin/env python3
"""Safely unpack an untrusted ZIP into one new, atomically published folder.

The extractor accepts no archive-controlled absolute path, link, special file,
permission bit, or overwrite. It does not import, execute, render, fetch, or run
any attachment. All files are created mode 0600 in a private staging directory,
validated, inventoried, and only then published with a no-replace rename.
"""

from __future__ import annotations

import argparse
import ctypes
import errno
import hashlib
import ipaddress
import json
import math
import os
import re
import shutil
import stat
import sys
import tempfile
import unicodedata
import zipfile
from pathlib import Path, PurePosixPath
from urllib.parse import parse_qsl, unquote, unquote_plus, urlsplit


INVENTORY_NAME = "inventory.json"
MAX_ARCHIVE_BYTES = 512 * 1024 * 1024
MAX_MEMBER_BYTES = 256 * 1024 * 1024
MAX_TOTAL_BYTES = 1024 * 1024 * 1024
MAX_MEMBERS = 2_000
MAX_COMPRESSION_RATIO = 100
MAX_PATH_DEPTH = 32
MAX_PATH_CHARS = 1_024
MAX_COMPONENT_CHARS = 255
MAX_TEXT_SCAN_BYTES = 32 * 1024 * 1024
MAX_JSON_BYTES = 128 * 1024 * 1024
MAX_JSON_DEPTH = 128
MAX_JSON_NODES = 2_000_000
MAX_CONTAINER_ITEMS = 500_000
MAX_STRING_CHARS = 16 * 1024 * 1024
MAX_TOTAL_STRING_CHARS = 256 * 1024 * 1024
MAX_NUMBER_CHARS = 1_000

ALLOWED_COMPRESSION = {zipfile.ZIP_STORED, zipfile.ZIP_DEFLATED}
WINDOWS_RESERVED_NAMES = {
    "CON", "PRN", "AUX", "NUL",
    *(f"COM{i}" for i in range(1, 10)),
    *(f"LPT{i}" for i in range(1, 10)),
}
TEXT_SUFFIXES = {
    ".cfg", ".conf", ".css", ".csv", ".html", ".ini", ".js", ".json",
    ".md", ".pem", ".properties", ".ps1", ".py", ".sh", ".sql", ".toml", ".tsv",
    ".txt", ".xml", ".yaml", ".yml",
}
SENSITIVE_BASENAMES = {
    ".env", ".npmrc", ".pypirc", "credentials.json", "id_dsa", "id_ecdsa",
    "id_ed25519", "id_rsa", "secrets.json",
}
SENSITIVE_SUFFIXES = {".jks", ".key", ".keystore", ".p12", ".pfx", ".pkcs12"}
URL_KEYS = {"href", "link", "uri", "url"}
URL_SECRET_KEYS = {
    "access_token", "api_key", "apikey", "client_secret", "password", "passwd",
    "pwd", "secret", "signature", "sig", "token",
}
URL_PATTERN = re.compile(r"https?://[^\s<>\"']+", re.IGNORECASE)
URL_SECRET_PATH_PATTERN = re.compile(
    r"(?:^|[;/])(?:" + "|".join(re.escape(key) for key in sorted(URL_SECRET_KEYS, key=len, reverse=True)) + r")=",
    re.IGNORECASE,
)
URL_TRAILING_PUNCTUATION = ".,;:!?)}"

SECRET_PATTERNS: tuple[tuple[str, re.Pattern[str]], ...] = (
    ("PRIVATE_KEY", re.compile(r"-----BEGIN (?:RSA |EC |DSA |OPENSSH )?PRIVATE KEY-----")),
    ("AWS_ACCESS_KEY", re.compile(r"\b(?:AKIA|ASIA)[A-Z0-9]{16}\b")),
    ("GITHUB_TOKEN", re.compile(r"\bgh(?:p|o|u|s|r)_[A-Za-z0-9]{30,}\b")),
    ("BEARER_TOKEN", re.compile(r"(?i)\bbearer\s+[A-Za-z0-9._~+/=-]{20,}")),
    ("JWT", re.compile(r"\beyJ[A-Za-z0-9_-]{8,}\.[A-Za-z0-9_-]{8,}\.[A-Za-z0-9_-]{8,}\b")),
    (
        "ASSIGNED_SECRET",
        re.compile(
            r"(?i)\b(?:password|passwd|pwd|secret|api[_-]?key|access[_-]?token|client[_-]?secret)"
            r"\s*[:=]\s*[\"']?([A-Za-z0-9._~+/=-]{12,})"
        ),
    ),
)
PLACEHOLDER_TOKENS = {
    "changeme", "example", "examplekey", "fake", "placeholder", "redacted",
    "secret_ref", "sample", "test", "token_here", "your_token", "xxxxxxxxxxxx",
}


class BundleError(ValueError):
    """Raised for any unsafe or malformed bundle condition."""


def require_well_formed_unicode(value: str, label: str) -> None:
    """Reject isolated UTF-16 surrogates before filesystem or UTF-8 use."""
    if any(0xD800 <= ord(character) <= 0xDFFF for character in value):
        raise BundleError(f"{label}: texto Unicode contém surrogate isolado")


def authorized_root(raw: Path, label: str) -> Path:
    try:
        root = raw.resolve(strict=True)
    except OSError as exc:
        raise BundleError(f"{label} inválida: {exc}") from exc
    if not root.is_dir():
        raise BundleError(f"{label} não é diretório: {root}")
    return root


def portable_relative_path(raw: object, label: str) -> PurePosixPath:
    if not isinstance(raw, str) or not raw or len(raw) > MAX_PATH_CHARS:
        raise BundleError(f"{label} precisa ser caminho relativo não vazio")
    require_well_formed_unicode(raw, label)
    if raw != unicodedata.normalize("NFC", raw):
        raise BundleError(f"{label} não está normalizado em Unicode NFC")
    if "\\" in raw or "\x00" in raw or any(ord(ch) < 32 or ord(ch) == 127 for ch in raw):
        raise BundleError(f"{label} contém caractere inseguro")
    if raw.startswith("/") or re.match(r"^[A-Za-z]:", raw):
        raise BundleError(f"{label} precisa ser relativo")
    parts = raw.split("/")
    if len(parts) > MAX_PATH_DEPTH or any(part in {"", ".", ".."} for part in parts):
        raise BundleError(f"{label} contém travessia ou componente vazio")
    for part in parts:
        if len(part) > MAX_COMPONENT_CHARS or part.endswith((" ", ".")) or ":" in part:
            raise BundleError(f"{label} contém componente não portátil")
        if part.split(".", 1)[0].upper() in WINDOWS_RESERVED_NAMES:
            raise BundleError(f"{label} usa nome reservado: {part}")
    return PurePosixPath(*parts)


def _cli_relative(raw: Path, root: Path, label: str) -> PurePosixPath:
    if raw.is_absolute():
        try:
            relative_raw = raw.relative_to(root).as_posix()
        except ValueError as exc:
            raise BundleError(f"{label} fora da raiz autorizada") from exc
    else:
        relative_raw = raw.as_posix()
    return portable_relative_path(relative_raw, label)


def _reject_symlink_chain(root: Path, relative: PurePosixPath, label: str) -> Path:
    current = root
    for part in relative.parts:
        current = current / part
        try:
            mode = current.lstat().st_mode
        except OSError as exc:
            raise BundleError(f"{label} inacessível: {exc}") from exc
        if stat.S_ISLNK(mode):
            raise BundleError(f"{label} contém link simbólico")
    resolved = current.resolve(strict=True)
    try:
        resolved.relative_to(root)
    except ValueError as exc:
        raise BundleError(f"{label} escapou da raiz autorizada") from exc
    return resolved


def archive_path(root: Path, raw: Path) -> tuple[Path, str]:
    relative = _cli_relative(raw, root, "archive")
    path = _reject_symlink_chain(root, relative, "archive")
    mode = path.stat(follow_symlinks=False).st_mode
    if not stat.S_ISREG(mode):
        raise BundleError("archive não é arquivo regular")
    if path.suffix.casefold() != ".zip":
        raise BundleError("archive precisa ter extensão .zip")
    return path, relative.as_posix()


def new_output_path(root: Path, raw: Path) -> Path:
    relative = _cli_relative(raw, root, "output")
    output = root.joinpath(*relative.parts)
    current = root
    for part in relative.parts[:-1]:
        current = current / part
        try:
            os.mkdir(current, 0o700)
        except FileExistsError:
            mode = current.lstat().st_mode
            if stat.S_ISLNK(mode) or not stat.S_ISDIR(mode):
                raise BundleError("output atravessa componente que não é diretório regular")
    if os.path.lexists(output):
        raise BundleError(f"output já existe; sobrescrita é proibida: {output}")
    return output


def _is_within(path: Path, root: Path) -> bool:
    try:
        path.relative_to(root)
    except ValueError:
        return False
    return True


def _no_duplicate_object(pairs: list[tuple[str, object]]) -> dict[str, object]:
    result: dict[str, object] = {}
    for key, value in pairs:
        require_well_formed_unicode(key, "chave JSON")
        if key in result:
            raise BundleError(f"chave JSON duplicada: {key!r}")
        result[key] = value
    return result


def _strict_integer(token: str) -> int:
    if len(token) > MAX_NUMBER_CHARS:
        raise BundleError("inteiro JSON excessivamente longo")
    return int(token)


def _strict_float(token: str) -> float:
    if len(token) > MAX_NUMBER_CHARS:
        raise BundleError("número JSON excessivamente longo")
    value = float(token)
    if not math.isfinite(value):
        raise BundleError("número JSON não finito")
    return value


def _reject_constant(token: str) -> object:
    raise BundleError(f"constante JSON inválida: {token}")


def strict_json_loads(raw: bytes, label: str) -> object:
    try:
        value = json.loads(
            raw.decode("utf-8-sig", errors="strict"),
            object_pairs_hook=_no_duplicate_object,
            parse_constant=_reject_constant,
            parse_int=_strict_integer,
            parse_float=_strict_float,
        )
    except (UnicodeError, json.JSONDecodeError, RecursionError, BundleError, ValueError) as exc:
        raise BundleError(f"{label}: JSON estrito inválido: {exc}") from exc
    if not isinstance(value, (dict, list)):
        raise BundleError(f"{label}: raiz JSON precisa ser objeto ou array")
    return value


def _secret_kind(text: str) -> str:
    for kind, pattern in SECRET_PATTERNS:
        match = pattern.search(text)
        if not match:
            continue
        if kind == "ASSIGNED_SECRET":
            candidate = match.group(1).casefold()
            if candidate in PLACEHOLDER_TOKENS or any(
                marker in candidate
                for marker in ("dummy", "example", "not_a_real", "sample", "redacted", "secret_ref", "your_", "xxxx")
            ):
                continue
        return kind
    return ""


def _trim_embedded_url(raw: str) -> str:
    """Drop prose punctuation after a URL without corrupting an IPv6 host.

    ``URL_PATTERN`` deliberately accepts a broad, bounded non-whitespace tail so
    an URL in natural-language JSON cannot evade validation by being followed by
    punctuation.  A closing square bracket is special: it can be part of an IPv6
    authority, so only remove unmatched trailing brackets.
    """
    candidate = raw.rstrip(URL_TRAILING_PUNCTUATION)
    while candidate.endswith("]") and candidate.count("]") > candidate.count("["):
        candidate = candidate[:-1]
    return candidate


def _url_component_keys(component: str) -> set[str]:
    """Return decoded query/fragment keys, treating ';' as a legacy separator.

    ``parse_qsl`` is retained for standards-compliant decoding.  The extra split
    catches legacy semicolon-separated URLs too, which is important for secret
    names such as ``access_token`` even when they are percent-encoded.
    """
    keys = {key.casefold() for key, _ in parse_qsl(component, keep_blank_values=True)}
    # Split after decoding as well: ``%26access_token%3D...`` must not hide a
    # credential-shaped parameter inside another query value.
    for item in re.split(r"[&;]", unquote_plus(component)):
        key = item.partition("=")[0].casefold()
        if key:
            keys.add(key)
    return keys


def validate_http_url(raw: str, label: str) -> None:
    raw = _trim_embedded_url(raw)
    if not raw or len(raw) > 2_048 or any(ch.isspace() or ord(ch) < 32 for ch in raw):
        raise BundleError(f"{label}: URL vazia, longa ou com controles")
    try:
        parsed = urlsplit(raw)
        port = parsed.port
    except ValueError as exc:
        raise BundleError(f"{label}: URL inválida") from exc
    if parsed.scheme.casefold() not in {"http", "https"} or not parsed.hostname:
        raise BundleError(f"{label}: apenas URL http/https absoluta é aceita")
    if parsed.username is not None or parsed.password is not None:
        raise BundleError(f"{label}: credencial embutida em URL")
    if port is not None and not (1 <= port <= 65_535):
        raise BundleError(f"{label}: porta inválida")
    query_keys = _url_component_keys(parsed.query)
    if query_keys & URL_SECRET_KEYS:
        raise BundleError(f"{label}: segredo em query string")
    fragment_keys = _url_component_keys(parsed.fragment)
    if fragment_keys & URL_SECRET_KEYS:
        raise BundleError(f"{label}: segredo em fragmento URL")
    if URL_SECRET_PATH_PATTERN.search(unquote(parsed.path)):
        raise BundleError(f"{label}: segredo em parâmetro de caminho URL")
    if _secret_kind(raw):
        raise BundleError(f"{label}: possível segredo na URL")
    raw_hostname = parsed.hostname
    try:
        ipaddress.ip_address(raw_hostname)
    except ValueError:
        try:
            hostname = raw_hostname.encode("idna").decode("ascii").casefold().rstrip(".")
        except UnicodeError as exc:
            raise BundleError(f"{label}: hostname inválido") from exc
        labels = hostname.split(".")
        if (
            not hostname
            or len(hostname) > 253
            or any(
                not part
                or len(part) > 63
                or part.startswith("-")
                or part.endswith("-")
                or re.fullmatch(r"[a-z0-9-]+", part) is None
                for part in labels
            )
        ):
            raise BundleError(f"{label}: hostname inválido")


def inspect_json_graph(root: object, label: str) -> int:
    # ``is_named_url_value`` prevents a URL in ``{"url": "..."}`` from
    # being counted twice: once for its strict field contract and once as text.
    stack: list[tuple[object, int, str, bool]] = [(root, 0, "", False)]
    nodes = 0
    total_chars = 0
    url_count = 0
    while stack:
        node, depth, pointer, is_named_url_value = stack.pop()
        nodes += 1
        if nodes > MAX_JSON_NODES:
            raise BundleError(f"{label}: JSON excede {MAX_JSON_NODES} nós")
        if depth > MAX_JSON_DEPTH:
            raise BundleError(f"{label}: JSON excede profundidade {MAX_JSON_DEPTH}")
        if isinstance(node, str):
            require_well_formed_unicode(node, f"{label}{pointer}")
            if len(node) > MAX_STRING_CHARS:
                raise BundleError(f"{label}{pointer}: string excessivamente longa")
            total_chars += len(node)
            if total_chars > MAX_TOTAL_STRING_CHARS:
                raise BundleError(f"{label}: texto JSON excede o limite")
            if "\x00" in node or any(ord(ch) < 32 and ch not in "\t\r\n" for ch in node):
                raise BundleError(f"{label}{pointer}: controle inseguro em string")
            secret = _secret_kind(node)
            if secret:
                raise BundleError(f"{label}{pointer}: possível segredo detectado ({secret})")
            # URLs can appear in documentation fields, error text, and nested
            # examples—not only in objects named url/link/etc.  Validate every
            # occurrence before any later report merely records a count.
            if not is_named_url_value:
                url_count += validate_urls_in_text(node, f"{label}{pointer}")
        elif isinstance(node, dict):
            if len(node) > MAX_CONTAINER_ITEMS:
                raise BundleError(f"{label}{pointer}: objeto excessivamente grande")
            children = list(node.items())
            for key, child in children:
                child_pointer = f"{pointer}/{key.replace('~', '~0').replace('/', '~1')}"
                secret = _secret_kind(key)
                if secret:
                    raise BundleError(f"{label}{child_pointer}: possível segredo na chave ({secret})")
                if URL_PATTERN.search(key):
                    raise BundleError(f"{label}{child_pointer}: URL não é permitida em chave JSON")
                if key.casefold() in URL_KEYS and child not in (None, ""):
                    if not isinstance(child, str):
                        raise BundleError(f"{label}{child_pointer}: URL precisa ser string")
                    validate_http_url(child, f"{label}{child_pointer}")
                    url_count += 1
            for key, child in reversed(children):
                escaped = key.replace("~", "~0").replace("/", "~1")
                stack.append((
                    child,
                    depth + 1,
                    f"{pointer}/{escaped}",
                    key.casefold() in URL_KEYS,
                ))
        elif isinstance(node, list):
            if len(node) > MAX_CONTAINER_ITEMS:
                raise BundleError(f"{label}{pointer}: array excessivamente grande")
            for index in range(len(node) - 1, -1, -1):
                stack.append((node[index], depth + 1, f"{pointer}/{index}", False))
        elif isinstance(node, float) and not math.isfinite(node):
            raise BundleError(f"{label}{pointer}: número não finito")
    return url_count


def validate_urls_in_text(text: str, label: str) -> int:
    """Validate every HTTP(S) URL embedded in untrusted text.

    This routine intentionally returns only a count.  The extractor does not
    preserve raw URLs in its inventory, so query strings and fragments are never
    re-emitted; unsafe URLs (including secret-bearing query parameters) abort the
    whole extraction instead of being copied or silently redacted.
    """
    count = 0
    for match in URL_PATTERN.finditer(text):
        validate_http_url(match.group(0), label)
        count += 1
    return count


def _zip_member_path(info: zipfile.ZipInfo) -> tuple[PurePosixPath, bool]:
    original = getattr(info, "orig_filename", info.filename)
    is_directory = info.is_dir() or original.endswith("/")
    name = original[:-1] if is_directory and original.endswith("/") else original
    path = portable_relative_path(name, f"membro ZIP {original!r}")
    unix_mode = (info.external_attr >> 16) & 0xFFFF
    if unix_mode:
        file_type = stat.S_IFMT(unix_mode)
        if file_type not in {0, stat.S_IFREG, stat.S_IFDIR} or stat.S_ISLNK(unix_mode):
            raise BundleError(f"link ou arquivo especial no ZIP: {original}")
    if info.flag_bits & 0x1:
        raise BundleError(f"membro criptografado não aceito: {original}")
    if not is_directory and info.compress_type not in ALLOWED_COMPRESSION:
        raise BundleError(f"compressão não aceita: {original}")
    basename = path.name.casefold()
    if not is_directory and (basename in SENSITIVE_BASENAMES or path.suffix.casefold() in SENSITIVE_SUFFIXES):
        raise BundleError(f"arquivo potencialmente secreto rejeitado: {path.as_posix()}")
    return path, is_directory


def _canonical(path: PurePosixPath) -> str:
    return "/".join(part.casefold() for part in path.parts)


def validate_members(bundle: zipfile.ZipFile) -> list[tuple[zipfile.ZipInfo, PurePosixPath]]:
    infos = bundle.infolist()
    if not infos:
        raise BundleError("ZIP vazio")
    if len(infos) > MAX_MEMBERS:
        raise BundleError(f"ZIP excede {MAX_MEMBERS} membros")
    seen: dict[str, bool] = {}
    files: list[tuple[zipfile.ZipInfo, PurePosixPath]] = []
    total = 0
    compressed_total = 0
    reserved = INVENTORY_NAME.casefold()
    for info in infos:
        path, is_directory = _zip_member_path(info)
        canonical = _canonical(path)
        if canonical == reserved:
            raise BundleError(f"membro colide com o inventário reservado: {path.as_posix()}")
        if canonical in seen:
            raise BundleError(f"colisão de membros ZIP: {path.as_posix()}")
        for index in range(1, len(path.parts)):
            prefix = "/".join(part.casefold() for part in path.parts[:index])
            if seen.get(prefix) is False:
                raise BundleError(f"conflito arquivo/diretório: {path.as_posix()}")
        if not is_directory and any(existing.startswith(canonical + "/") for existing in seen):
            raise BundleError(f"conflito diretório/arquivo: {path.as_posix()}")
        seen[canonical] = is_directory
        if is_directory:
            continue
        if info.file_size < 0 or info.compress_size < 0 or info.file_size > MAX_MEMBER_BYTES:
            raise BundleError(f"membro excede limite: {path.as_posix()}")
        if info.file_size and info.compress_size == 0:
            raise BundleError(f"razão de compressão inválida: {path.as_posix()}")
        if info.compress_size and info.file_size / info.compress_size > MAX_COMPRESSION_RATIO:
            raise BundleError(f"possível ZIP bomb: {path.as_posix()}")
        total += info.file_size
        compressed_total += info.compress_size
        if total > MAX_TOTAL_BYTES:
            raise BundleError("ZIP excede o total descompactado permitido")
        files.append((info, path))
    if compressed_total and total / compressed_total > MAX_COMPRESSION_RATIO:
        raise BundleError("possível ZIP bomb pela razão total de compressão")
    return sorted(files, key=lambda item: item[1].as_posix().casefold())


def _mkdirs_private(root: Path, relative: PurePosixPath) -> None:
    current = root
    for part in relative.parts:
        current = current / part
        try:
            os.mkdir(current, 0o700)
        except FileExistsError:
            mode = current.lstat().st_mode
            if stat.S_ISLNK(mode) or not stat.S_ISDIR(mode):
                raise BundleError(f"colisão ao criar diretório: {relative.as_posix()}")


def _extract_file(
    bundle: zipfile.ZipFile,
    info: zipfile.ZipInfo,
    relative: PurePosixPath,
    staging: Path,
    running_total: int,
) -> tuple[dict, int]:
    _mkdirs_private(staging, PurePosixPath(*relative.parts[:-1])) if len(relative.parts) > 1 else None
    target = staging.joinpath(*relative.parts)
    flags = (
        os.O_WRONLY | os.O_CREAT | os.O_EXCL | getattr(os, "O_CLOEXEC", 0) |
        getattr(os, "O_NOFOLLOW", 0)
    )
    try:
        descriptor = os.open(target, flags, 0o600)
    except OSError as exc:
        raise BundleError(f"não foi possível criar {relative.as_posix()}: {exc}") from exc
    digest = hashlib.sha256()
    written = 0
    try:
        try:
            source = bundle.open(info, "r")
        except (RuntimeError, zipfile.BadZipFile) as exc:
            raise BundleError(f"falha ao abrir {relative.as_posix()}: {exc}") from exc
        with source, os.fdopen(descriptor, "wb", closefd=False) as sink:
            while True:
                block = source.read(1024 * 1024)
                if not block:
                    break
                written += len(block)
                running_total += len(block)
                if written > info.file_size or written > MAX_MEMBER_BYTES or running_total > MAX_TOTAL_BYTES:
                    raise BundleError(f"conteúdo expandido excede limite em {relative.as_posix()}")
                sink.write(block)
                digest.update(block)
            sink.flush()
            os.fsync(sink.fileno())
    except (OSError, RuntimeError, zipfile.BadZipFile) as exc:
        raise BundleError(f"falha de integridade ao extrair {relative.as_posix()}: {exc}") from exc
    finally:
        os.close(descriptor)
    if written != info.file_size:
        raise BundleError(f"tamanho extraído diverge em {relative.as_posix()}")
    mode = target.stat(follow_symlinks=False).st_mode
    if not stat.S_ISREG(mode):
        raise BundleError(f"saída não regular após extração: {relative.as_posix()}")
    record = {
        "evidence_id": "ART-" + hashlib.sha256(
            relative.as_posix().encode("utf-8") + b"\x00" + digest.digest()
        ).hexdigest()[:12].upper(),
        "path": relative.as_posix(),
        "kind": relative.suffix.casefold().lstrip(".") or "file",
        "size_bytes": written,
        "compressed_bytes": info.compress_size,
        "sha256": digest.hexdigest(),
        "crc32": f"{info.CRC:08x}",
        "json_validated": False,
        "secret_scan": "not_applicable",
        "url_count": 0,
    }
    return record, running_total


def _read_regular(path: Path, limit: int, label: str) -> bytes:
    flags = os.O_RDONLY | getattr(os, "O_CLOEXEC", 0) | getattr(os, "O_NOFOLLOW", 0)
    descriptor = os.open(path, flags)
    try:
        before = os.fstat(descriptor)
        if not stat.S_ISREG(before.st_mode) or before.st_size > limit:
            raise BundleError(f"{label} não é regular ou excede o limite")
        data = bytearray()
        while True:
            block = os.read(descriptor, min(1024 * 1024, limit + 1 - len(data)))
            if not block:
                break
            data.extend(block)
            if len(data) > limit:
                raise BundleError(f"{label} excede o limite")
        after = os.fstat(descriptor)
        if (before.st_dev, before.st_ino, before.st_size, before.st_mtime_ns) != (
            after.st_dev, after.st_ino, after.st_size, after.st_mtime_ns
        ):
            raise BundleError(f"{label} mudou durante a leitura")
        return bytes(data)
    finally:
        os.close(descriptor)


def inspect_extracted_file(path: Path, relative: PurePosixPath, record: dict) -> None:
    suffix = relative.suffix.casefold()
    if suffix == ".json":
        if record["size_bytes"] > MAX_JSON_BYTES:
            raise BundleError(f"JSON grande demais para validação segura: {relative.as_posix()}")
        raw = _read_regular(path, MAX_JSON_BYTES, relative.as_posix())
        root = strict_json_loads(raw, relative.as_posix())
        record["url_count"] = inspect_json_graph(root, relative.as_posix())
        record["json_validated"] = True
        record["secret_scan"] = "passed"
        return
    if suffix not in TEXT_SUFFIXES:
        return
    if record["size_bytes"] > MAX_TEXT_SCAN_BYTES:
        raise BundleError(f"texto grande demais para varredura de segredo: {relative.as_posix()}")
    raw = _read_regular(path, MAX_TEXT_SCAN_BYTES, relative.as_posix())
    try:
        text = raw.decode("utf-8-sig", errors="strict")
    except UnicodeError as exc:
        raise BundleError(f"texto declarado não é UTF-8: {relative.as_posix()}") from exc
    secret = _secret_kind(text)
    if secret:
        raise BundleError(f"possível segredo detectado em {relative.as_posix()} ({secret})")
    url_count = 0
    for match in URL_PATTERN.finditer(text):
        validate_http_url(match.group(0), relative.as_posix())
        url_count += 1
    record["secret_scan"] = "passed"
    record["url_count"] = url_count


def _sha256_descriptor(descriptor: int, limit: int) -> tuple[str, bytes, os.stat_result]:
    before = os.fstat(descriptor)
    if not stat.S_ISREG(before.st_mode) or before.st_size > limit:
        raise BundleError("archive não é regular ou excede o limite")
    os.lseek(descriptor, 0, os.SEEK_SET)
    digest = hashlib.sha256()
    header = b""
    consumed = 0
    while True:
        block = os.read(descriptor, 1024 * 1024)
        if not block:
            break
        if not header:
            header = block[:4]
        consumed += len(block)
        if consumed > limit:
            raise BundleError("archive excede o limite")
        digest.update(block)
    os.lseek(descriptor, 0, os.SEEK_SET)
    return digest.hexdigest(), header, before


def _write_inventory(staging: Path, inventory: dict) -> None:
    target = staging / INVENTORY_NAME
    flags = os.O_WRONLY | os.O_CREAT | os.O_EXCL | getattr(os, "O_CLOEXEC", 0) | getattr(os, "O_NOFOLLOW", 0)
    descriptor = os.open(target, flags, 0o600)
    try:
        with os.fdopen(descriptor, "w", encoding="utf-8", newline="\n", closefd=False) as handle:
            json.dump(inventory, handle, ensure_ascii=False, indent=2, sort_keys=True, allow_nan=False)
            handle.write("\n")
            handle.flush()
            os.fsync(handle.fileno())
    finally:
        os.close(descriptor)


def _fsync_directory(path: Path) -> None:
    flags = os.O_RDONLY | getattr(os, "O_DIRECTORY", 0) | getattr(os, "O_CLOEXEC", 0)
    try:
        descriptor = os.open(path, flags)
    except OSError:
        return
    try:
        os.fsync(descriptor)
    except OSError:
        pass
    finally:
        os.close(descriptor)


def _rename_no_replace(source: Path, target: Path) -> None:
    if sys.platform.startswith("linux"):
        libc = ctypes.CDLL(None, use_errno=True)
        renameat2 = getattr(libc, "renameat2", None)
        if renameat2 is None:
            raise BundleError("renameat2 indisponível; publicação atômica sem sobrescrita não garantida")
        renameat2.argtypes = [ctypes.c_int, ctypes.c_char_p, ctypes.c_int, ctypes.c_char_p, ctypes.c_uint]
        renameat2.restype = ctypes.c_int
        result = renameat2(-100, os.fsencode(source), -100, os.fsencode(target), 1)
        if result != 0:
            error = ctypes.get_errno()
            if error == errno.EEXIST:
                raise BundleError(f"output já existe; sobrescrita proibida: {target}")
            raise BundleError(f"falha na publicação atômica: {os.strerror(error)}")
        return
    if sys.platform == "darwin":
        libc = ctypes.CDLL(None, use_errno=True)
        renamex_np = getattr(libc, "renamex_np", None)
        if renamex_np is None:
            raise BundleError("renamex_np indisponível; publicação atômica sem sobrescrita não garantida")
        renamex_np.argtypes = [ctypes.c_char_p, ctypes.c_char_p, ctypes.c_uint]
        renamex_np.restype = ctypes.c_int
        result = renamex_np(os.fsencode(source), os.fsencode(target), 0x00000004)  # RENAME_EXCL
        if result != 0:
            error = ctypes.get_errno()
            if error == errno.EEXIST:
                raise BundleError(f"output já existe; sobrescrita proibida: {target}")
            raise BundleError(f"falha na publicação atômica: {os.strerror(error)}")
        return
    if os.name == "nt":
        try:
            os.rename(source, target)
        except FileExistsError as exc:
            raise BundleError(f"output já existe; sobrescrita proibida: {target}") from exc
        return
    raise BundleError("plataforma sem primitiva comprovada de rename atômico no-replace")


def unpack(archive: Path, allowed_evidence_root: Path, workspace_root: Path, output: Path) -> dict:
    evidence_root = authorized_root(allowed_evidence_root, "allowed-evidence-root")
    workspace = authorized_root(workspace_root, "workspace-root")
    source_path, source_display = archive_path(evidence_root, archive)
    requested_output = workspace.joinpath(*_cli_relative(output, workspace, "output").parts)
    if _is_within(requested_output, evidence_root):
        raise BundleError("output não pode ficar dentro de allowed-evidence-root")
    destination = new_output_path(workspace, output)

    staging = Path(tempfile.mkdtemp(prefix=f".{destination.name}.", suffix=".staging", dir=destination.parent))
    os.chmod(staging, 0o700)
    flags = os.O_RDONLY | getattr(os, "O_CLOEXEC", 0) | getattr(os, "O_NOFOLLOW", 0)
    descriptor: int | None = None
    published = False
    try:
        descriptor = os.open(source_path, flags)
        archive_digest, header, before = _sha256_descriptor(descriptor, MAX_ARCHIVE_BYTES)
        if header not in {b"PK\x03\x04", b"PK\x05\x06"}:
            raise BundleError("archive não possui assinatura ZIP direta")
        records: list[dict] = []
        actual_total = 0
        with os.fdopen(descriptor, "rb", closefd=False) as archive_handle:
            try:
                with zipfile.ZipFile(archive_handle, "r") as bundle:
                    members = validate_members(bundle)
                    for info, relative in members:
                        record, actual_total = _extract_file(bundle, info, relative, staging, actual_total)
                        inspect_extracted_file(staging.joinpath(*relative.parts), relative, record)
                        records.append(record)
            except zipfile.BadZipFile as exc:
                raise BundleError(f"ZIP inválido ou corrompido: {exc}") from exc
        after = os.fstat(descriptor)
        if (before.st_dev, before.st_ino, before.st_size, before.st_mtime_ns) != (
            after.st_dev, after.st_ino, after.st_size, after.st_mtime_ns
        ):
            raise BundleError("archive foi alterado durante a extração")

        inventory = {
            "schema_version": "1.0",
            "archive": {
                "path": source_display,
                "size_bytes": before.st_size,
                "sha256": archive_digest,
            },
            "limits": {
                "max_archive_bytes": MAX_ARCHIVE_BYTES,
                "max_member_bytes": MAX_MEMBER_BYTES,
                "max_total_bytes": MAX_TOTAL_BYTES,
                "max_members": MAX_MEMBERS,
                "max_compression_ratio": MAX_COMPRESSION_RATIO,
            },
            "counts": {
                "files": len(records),
                "total_uncompressed_bytes": actual_total,
            },
            "files": records,
        }
        _write_inventory(staging, inventory)
        _fsync_directory(staging)
        _rename_no_replace(staging, destination)
        published = True
        _fsync_directory(destination.parent)
        return {
            "files": len(records),
            "total_uncompressed_bytes": actual_total,
            "output": destination.relative_to(workspace).as_posix(),
            "inventory": f"{destination.relative_to(workspace).as_posix()}/{INVENTORY_NAME}",
        }
    finally:
        if descriptor is not None:
            try:
                os.close(descriptor)
            except OSError:
                pass
        if not published:
            shutil.rmtree(staging, ignore_errors=True)


def main() -> int:
    parser = argparse.ArgumentParser(description="Descompacta ZIP hostil sem executar anexos nem sobrescrever saídas.")
    parser.add_argument("--archive", required=True, type=Path)
    parser.add_argument("--allowed-evidence-root", required=True, type=Path)
    parser.add_argument("--workspace-root", required=True, type=Path)
    parser.add_argument("--output", required=True, type=Path)
    args = parser.parse_args()
    try:
        result = unpack(args.archive, args.allowed_evidence_root, args.workspace_root, args.output)
    except (OSError, BundleError) as exc:
        print(f"descompactação recusada: {exc}", file=sys.stderr)
        return 2
    print(json.dumps(result, ensure_ascii=False, sort_keys=True, allow_nan=False))
    return 0


# Registro das operacoes do plugin (.wx-migration/logs/): sem projeto por
# perto, nao grava nada; falha de registro nunca derruba a operacao.
try:
    import registro
except ImportError:  # rodando de outro diretorio
    sys.path.insert(0, str(Path(__file__).resolve().parent))
    import registro


if __name__ == "__main__":
    raise SystemExit(registro.envolver(__file__, main))
