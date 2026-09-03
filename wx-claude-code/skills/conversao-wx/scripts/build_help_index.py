#!/usr/bin/env python3
"""Build a bounded, provenance-preserving index from 12 Help JSON files.

The manifest never grants filesystem access. Evidence and output roots must be
provided explicitly. Inputs are treated as hostile data and are never executed,
imported as Python, fetched from the network, or allowed to overwrite output.
"""

from __future__ import annotations

import argparse
import errno
import hashlib
import ipaddress
import json
import math
import os
import re
import stat
import sys
import tempfile
import unicodedata
import zipfile
from collections.abc import Iterator
from pathlib import Path, PurePosixPath
from urllib.parse import parse_qsl, urlsplit, urlunsplit


HELP_DOCUMENT_COUNT = 12
MAX_MANIFEST_BYTES = 2 * 1024 * 1024
MAX_MEMBER_BYTES = 128 * 1024 * 1024
MAX_TOTAL_BYTES = 512 * 1024 * 1024
MAX_ZIP_BYTES = 512 * 1024 * 1024
MAX_ZIP_MEMBERS = 1_000
MAX_COMPRESSION_RATIO = 100
MAX_JSON_DEPTH = 128
MAX_JSON_NODES = 2_000_000
MAX_CONTAINER_ITEMS = 500_000
MAX_STRING_CHARS = 16 * 1024 * 1024
MAX_TOTAL_STRING_CHARS = 256 * 1024 * 1024
MAX_NUMBER_CHARS = 1_000
MAX_RECORDS = 2_000_000
MAX_OUTPUT_BYTES = 1024 * 1024 * 1024
MAX_PATH_DEPTH = 32
MAX_PATH_CHARS = 1_024
MAX_COMPONENT_CHARS = 255
MAX_TITLE_CHARS = 500
CHUNK_CHARS = 3_000

TITLE_KEYS = ("name", "title", "symbol", "function", "heading", "nom", "titre", "fonction")
URL_KEYS = ("url", "uri", "link", "href")
URL_SECRET_KEYS = {
    "access_token", "api_key", "apikey", "client_secret", "password", "passwd",
    "pwd", "secret", "signature", "sig", "token",
}
URL_IN_TEXT_PATTERN = re.compile(r"https?://[^\s<>\"']+", re.IGNORECASE)
URL_SECRET_PATH_PATTERN = re.compile(
    r"(?:^|[;/])(?:" + "|".join(re.escape(key) for key in sorted(URL_SECRET_KEYS, key=len, reverse=True)) + r")=",
    re.IGNORECASE,
)
VALID_PRODUCTS = {"WINDEV", "WEBDEV", "WINDEV Mobile"}
ALLOWED_ZIP_COMPRESSION = {zipfile.ZIP_STORED, zipfile.ZIP_DEFLATED}
WINDOWS_RESERVED_NAMES = {
    "CON", "PRN", "AUX", "NUL",
    *(f"COM{i}" for i in range(1, 10)),
    *(f"LPT{i}" for i in range(1, 10)),
}

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


class InputError(ValueError):
    """Raised when hostile or malformed input violates the contract."""


def require_well_formed_unicode(value: str, label: str) -> None:
    """Reject JSON strings/keys that cannot be represented as UTF-8.

    ``json.loads`` correctly combines valid escaped surrogate pairs, but leaves
    isolated surrogate code points in the resulting Python string.  Those
    values otherwise fail later in hashing, JSON serialization, or stderr
    output with ``UnicodeEncodeError``.  Reject them before any such use and
    never echo the hostile value in the diagnostic.
    """
    if any(0xD800 <= ord(character) <= 0xDFFF for character in value):
        raise InputError(f"{label}: texto Unicode contém surrogate isolado")


def _under(path: Path, root: Path, label: str) -> Path:
    try:
        path.relative_to(root)
    except ValueError as exc:
        raise InputError(f"{label} fora da raiz autorizada") from exc
    return path


def authorized_root(raw: Path, label: str) -> Path:
    try:
        root = raw.resolve(strict=True)
    except OSError as exc:
        raise InputError(f"{label} inválida: {exc}") from exc
    if not root.is_dir():
        raise InputError(f"{label} não é diretório: {root}")
    return root


def portable_relative_path(raw: object, label: str) -> PurePosixPath:
    if not isinstance(raw, str) or not raw or len(raw) > MAX_PATH_CHARS:
        raise InputError(f"{label} precisa ser caminho relativo não vazio")
    require_well_formed_unicode(raw, label)
    if raw != unicodedata.normalize("NFC", raw):
        raise InputError(f"{label} não está normalizado em Unicode NFC")
    if "\\" in raw or "\x00" in raw or any(ord(ch) < 32 or ord(ch) == 127 for ch in raw):
        raise InputError(f"{label} contém caractere inseguro")
    if raw.startswith("/") or re.match(r"^[A-Za-z]:", raw):
        raise InputError(f"{label} precisa ser relativo")
    raw_parts = raw.split("/")
    if len(raw_parts) > MAX_PATH_DEPTH or any(part in {"", ".", ".."} for part in raw_parts):
        raise InputError(f"{label} contém travessia ou componente vazio")
    for part in raw_parts:
        if len(part) > MAX_COMPONENT_CHARS or part.endswith((" ", ".")) or ":" in part:
            raise InputError(f"{label} contém componente não portátil")
        if part.split(".", 1)[0].upper() in WINDOWS_RESERVED_NAMES:
            raise InputError(f"{label} usa nome reservado: {part}")
    path = PurePosixPath(*raw_parts)
    if path.is_absolute() or ".." in path.parts:
        raise InputError(f"{label} precisa ser relativo")
    return path


def _reject_symlink_chain(root: Path, relative: PurePosixPath, label: str) -> Path:
    current = root
    for part in relative.parts:
        current = current / part
        try:
            mode = current.lstat().st_mode
        except OSError as exc:
            raise InputError(f"{label} inacessível: {exc}") from exc
        if stat.S_ISLNK(mode):
            raise InputError(f"{label} contém link simbólico")
    resolved = current.resolve(strict=True)
    return _under(resolved, root, label)


def existing_relative_file(root: Path, raw: object, label: str) -> tuple[Path, str]:
    relative = portable_relative_path(raw, label)
    path = _reject_symlink_chain(root, relative, label)
    try:
        mode = path.stat(follow_symlinks=False).st_mode
    except OSError as exc:
        raise InputError(f"{label} inacessível: {exc}") from exc
    if not stat.S_ISREG(mode):
        raise InputError(f"{label} não é arquivo regular")
    return path, relative.as_posix()


def workspace_file(workspace_root: Path, raw: Path, label: str) -> Path:
    if raw.is_absolute():
        try:
            relative_raw = raw.relative_to(workspace_root).as_posix()
        except ValueError as exc:
            raise InputError(f"{label} fora de workspace_root") from exc
    else:
        relative_raw = raw.as_posix()
    relative = portable_relative_path(relative_raw, label)
    return _reject_symlink_chain(workspace_root, relative, label)


def _is_within(path: Path, root: Path) -> bool:
    try:
        path.relative_to(root)
    except ValueError:
        return False
    return True


def _ensure_output_parent(
    workspace_root: Path,
    raw: Path,
    label: str,
    *,
    forbidden_root: Path,
) -> Path:
    if raw.is_absolute():
        try:
            relative_raw = raw.relative_to(workspace_root).as_posix()
        except ValueError as exc:
            raise InputError(f"{label} fora de workspace_root") from exc
    else:
        relative_raw = raw.as_posix()
    relative = portable_relative_path(relative_raw, label)
    target = workspace_root.joinpath(*relative.parts)
    if _is_within(target, forbidden_root):
        raise InputError(f"{label} não pode ficar dentro de allowed-evidence-root")
    current = workspace_root
    for part in relative.parts[:-1]:
        current = current / part
        try:
            os.mkdir(current, mode=0o700)
        except FileExistsError:
            mode = current.lstat().st_mode
            if stat.S_ISLNK(mode) or not stat.S_ISDIR(mode):
                raise InputError(f"{label} atravessa componente que não é diretório regular")
    if os.path.lexists(target):
        raise InputError(f"{label} já existe; sobrescrita é proibida: {target}")
    return target


def read_regular_file(path: Path, limit: int, label: str) -> bytes:
    flags = os.O_RDONLY | getattr(os, "O_CLOEXEC", 0) | getattr(os, "O_NOFOLLOW", 0)
    try:
        descriptor = os.open(path, flags)
    except OSError as exc:
        raise InputError(f"{label} não pôde ser aberto com segurança: {exc}") from exc
    try:
        before = os.fstat(descriptor)
        if not stat.S_ISREG(before.st_mode):
            raise InputError(f"{label} não é arquivo regular")
        if before.st_size > limit:
            raise InputError(f"{label} excede {limit} bytes")
        blocks: list[bytes] = []
        consumed = 0
        while True:
            block = os.read(descriptor, min(1024 * 1024, limit + 1 - consumed))
            if not block:
                break
            consumed += len(block)
            if consumed > limit:
                raise InputError(f"{label} excede {limit} bytes")
            blocks.append(block)
        after = os.fstat(descriptor)
        if (before.st_dev, before.st_ino, before.st_size, before.st_mtime_ns) != (
            after.st_dev, after.st_ino, after.st_size, after.st_mtime_ns
        ):
            raise InputError(f"{label} foi alterado durante a leitura")
        return b"".join(blocks)
    finally:
        os.close(descriptor)


def _no_duplicate_object(pairs: list[tuple[str, object]]) -> dict[str, object]:
    result: dict[str, object] = {}
    for key, value in pairs:
        require_well_formed_unicode(key, "chave JSON")
        if key in result:
            raise InputError("chave JSON duplicada")
        result[key] = value
    return result


def _strict_integer(token: str) -> int:
    if len(token) > MAX_NUMBER_CHARS:
        raise InputError("inteiro JSON excessivamente longo")
    return int(token)


def _strict_float(token: str) -> float:
    if len(token) > MAX_NUMBER_CHARS:
        raise InputError("número JSON excessivamente longo")
    value = float(token)
    if not math.isfinite(value):
        raise InputError("número JSON não finito")
    return value


def _reject_constant(token: str) -> object:
    raise InputError(f"constante JSON inválida: {token}")


def strict_json_loads(raw: bytes, label: str, *, require_object: bool = False) -> object:
    try:
        text = raw.decode("utf-8-sig", errors="strict")
        value = json.loads(
            text,
            object_pairs_hook=_no_duplicate_object,
            parse_constant=_reject_constant,
            parse_int=_strict_integer,
            parse_float=_strict_float,
        )
    except (UnicodeError, json.JSONDecodeError, RecursionError, InputError, ValueError) as exc:
        raise InputError(f"{label}: JSON estrito inválido: {exc}") from exc
    if require_object and not isinstance(value, dict):
        raise InputError(f"{label}: raiz precisa ser objeto JSON")
    if not require_object and not isinstance(value, (dict, list)):
        raise InputError(f"{label}: raiz precisa ser objeto ou array JSON")
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


def validate_http_url(raw: str, label: str) -> str:
    """Validate an HTTP URL and return the index-safe form without params."""
    require_well_formed_unicode(raw, label)
    if not raw or len(raw) > 2_048 or any(ch.isspace() or ord(ch) < 32 for ch in raw):
        raise InputError(f"{label}: URL vazia, longa ou com controles")
    try:
        parsed = urlsplit(raw)
        port = parsed.port
    except ValueError as exc:
        raise InputError(f"{label}: URL inválida") from exc
    if parsed.scheme.casefold() not in {"http", "https"} or not parsed.hostname:
        raise InputError(f"{label}: apenas URL http/https absoluta é aceita")
    if parsed.username is not None or parsed.password is not None:
        raise InputError(f"{label}: credencial embutida em URL")
    if port is not None and not (1 <= port <= 65_535):
        raise InputError(f"{label}: porta inválida")
    secret_query = {key.casefold() for key, _ in parse_qsl(parsed.query, keep_blank_values=True)} & URL_SECRET_KEYS
    if secret_query:
        raise InputError(f"{label}: segredo em query string ({', '.join(sorted(secret_query))})")
    secret_fragment = {
        key.casefold() for key, _ in parse_qsl(parsed.fragment, keep_blank_values=True)
    } & URL_SECRET_KEYS
    if secret_fragment:
        raise InputError(f"{label}: segredo em fragmento URL ({', '.join(sorted(secret_fragment))})")
    if URL_SECRET_PATH_PATTERN.search(parsed.path):
        raise InputError(f"{label}: segredo em parâmetro de caminho URL")
    if _secret_kind(raw):
        raise InputError(f"{label}: possível segredo na URL")
    raw_hostname = parsed.hostname
    try:
        address = ipaddress.ip_address(raw_hostname)
    except ValueError:
        try:
            hostname = raw_hostname.encode("idna").decode("ascii").casefold().rstrip(".")
        except UnicodeError as exc:
            raise InputError(f"{label}: hostname inválido") from exc
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
            raise InputError(f"{label}: hostname inválido")
    else:
        hostname = address.compressed
        if address.version == 6:
            hostname = f"[{hostname}]"
    netloc = hostname if port is None else f"{hostname}:{port}"
    return urlunsplit((parsed.scheme.casefold(), netloc, parsed.path or "", "", ""))


def sanitize_urls_in_text(text: str, label: str) -> str:
    """Remove query strings/fragments from every URL before indexing text."""
    require_well_formed_unicode(text, label)

    def replace(match: re.Match[str]) -> str:
        candidate = match.group(0)
        stripped = candidate.rstrip(".,;:!?)]}")
        trailer = candidate[len(stripped):]
        return validate_http_url(stripped, label) + trailer

    return URL_IN_TEXT_PATTERN.sub(replace, text)


def pointer_escape(value: str) -> str:
    require_well_formed_unicode(value, "ponteiro JSON")
    return value.replace("~", "~0").replace("/", "~1")


def validate_json_graph(root: object, label: str) -> None:
    stack: list[tuple[object, int, str]] = [(root, 0, "")]
    nodes = 0
    total_string_chars = 0
    while stack:
        node, depth, pointer = stack.pop()
        nodes += 1
        if nodes > MAX_JSON_NODES:
            raise InputError(f"{label}: JSON excede {MAX_JSON_NODES} nós")
        if depth > MAX_JSON_DEPTH:
            raise InputError(f"{label}: JSON excede profundidade {MAX_JSON_DEPTH}")
        if isinstance(node, str):
            require_well_formed_unicode(node, f"{label}{pointer}")
            if len(node) > MAX_STRING_CHARS:
                raise InputError(f"{label}{pointer}: string JSON excessivamente longa")
            total_string_chars += len(node)
            if total_string_chars > MAX_TOTAL_STRING_CHARS:
                raise InputError(f"{label}: texto JSON excede o limite")
            if "\x00" in node or any(ord(ch) < 32 and ch not in "\t\r\n" for ch in node):
                raise InputError(f"{label}{pointer}: string contém controle inseguro")
            secret = _secret_kind(node)
            if secret:
                raise InputError(f"{label}{pointer}: possível segredo detectado ({secret})")
        elif isinstance(node, float) and not math.isfinite(node):
            raise InputError(f"{label}{pointer}: número não finito")
        elif isinstance(node, dict):
            if len(node) > MAX_CONTAINER_ITEMS:
                raise InputError(f"{label}{pointer}: objeto JSON excessivamente grande")
            children = list(node.items())
            for key, child in children:
                require_well_formed_unicode(key, f"{label}{pointer}/<chave>")
                escaped_key = pointer_escape(key)
                key_pointer = f"{pointer}/{escaped_key}"
                if len(key) > MAX_COMPONENT_CHARS:
                    raise InputError(f"{label}{key_pointer}: chave excessivamente longa")
                secret = _secret_kind(key)
                if secret:
                    raise InputError(f"{label}{key_pointer}: possível segredo na chave ({secret})")
                if URL_IN_TEXT_PATTERN.search(key):
                    raise InputError(f"{label}{key_pointer}: URL não é permitida em chave JSON")
                if key.casefold() in URL_KEYS and child not in (None, ""):
                    if not isinstance(child, str):
                        raise InputError(f"{label}{key_pointer}: URL precisa ser string")
                    validate_http_url(child, f"{label}{key_pointer}")
            for key, child in reversed(children):
                stack.append((child, depth + 1, f"{pointer}/{pointer_escape(key)}"))
        elif isinstance(node, list):
            if len(node) > MAX_CONTAINER_ITEMS:
                raise InputError(f"{label}{pointer}: array JSON excessivamente grande")
            for index in range(len(node) - 1, -1, -1):
                stack.append((node[index], depth + 1, f"{pointer}/{index}"))


def load_manifest(path: Path) -> dict:
    raw = read_regular_file(path, MAX_MANIFEST_BYTES, "manifesto")
    data = strict_json_loads(raw, "manifesto", require_object=True)
    assert isinstance(data, dict)
    validate_json_graph(data, "manifesto")
    if data.get("schema_version") != "1.0":
        raise InputError("schema_version do manifesto deve ser 1.0")
    return data


def _manifest_root_matches(manifest: dict, manifest_path: Path, allowed_root: Path) -> None:
    raw = manifest.get("evidence_root")
    if not isinstance(raw, str) or not raw:
        raise InputError("evidence_root ausente no manifesto")
    candidate = Path(raw)
    try:
        declared = (candidate if candidate.is_absolute() else manifest_path.parent / candidate).resolve(strict=True)
    except OSError as exc:
        raise InputError(f"evidence_root declarado é inválido: {exc}") from exc
    if declared != allowed_root:
        raise InputError("evidence_root do manifesto não coincide com --allowed-evidence-root")


def _zip_member_path(info: zipfile.ZipInfo) -> tuple[PurePosixPath, bool]:
    original = getattr(info, "orig_filename", info.filename)
    is_directory = info.is_dir() or original.endswith("/")
    name = original[:-1] if is_directory and original.endswith("/") else original
    path = portable_relative_path(name, f"membro ZIP {original!r}")
    unix_mode = (info.external_attr >> 16) & 0xFFFF
    if unix_mode:
        file_type = stat.S_IFMT(unix_mode)
        if file_type not in {0, stat.S_IFREG, stat.S_IFDIR}:
            raise InputError(f"membro ZIP especial/link rejeitado: {original}")
        if stat.S_ISLNK(unix_mode):
            raise InputError(f"link simbólico no ZIP: {original}")
    if info.flag_bits & 0x1:
        raise InputError(f"membro ZIP criptografado não é aceito: {original}")
    if not is_directory and info.compress_type not in ALLOWED_ZIP_COMPRESSION:
        raise InputError(f"compressão ZIP não aceita em {original}")
    return path, is_directory


def _canonical_member(path: PurePosixPath) -> str:
    return "/".join(part.casefold() for part in path.parts)


def validated_help_zip(bundle: zipfile.ZipFile) -> list[tuple[zipfile.ZipInfo, str]]:
    infos = bundle.infolist()
    if len(infos) > MAX_ZIP_MEMBERS:
        raise InputError(f"ZIP excede {MAX_ZIP_MEMBERS} membros")
    seen: dict[str, bool] = {}
    json_members: list[tuple[zipfile.ZipInfo, str]] = []
    total = 0
    total_compressed = 0
    for info in infos:
        path, is_directory = _zip_member_path(info)
        canonical = _canonical_member(path)
        if canonical in seen:
            raise InputError(f"colisão de membros ZIP: {path.as_posix()}")
        for index in range(1, len(path.parts)):
            prefix = "/".join(part.casefold() for part in path.parts[:index])
            if seen.get(prefix) is False:
                raise InputError(f"conflito arquivo/diretório no ZIP: {path.as_posix()}")
        if not is_directory and any(key.startswith(canonical + "/") for key in seen):
            raise InputError(f"conflito diretório/arquivo no ZIP: {path.as_posix()}")
        seen[canonical] = is_directory
        if is_directory:
            continue
        if info.file_size < 0 or info.compress_size < 0 or info.file_size > MAX_MEMBER_BYTES:
            raise InputError(f"membro ZIP excede limite: {path.as_posix()}")
        if info.file_size and info.compress_size == 0:
            raise InputError(f"razão de compressão inválida: {path.as_posix()}")
        if info.compress_size and info.file_size / info.compress_size > MAX_COMPRESSION_RATIO:
            raise InputError(f"possível ZIP bomb em {path.as_posix()}")
        total += info.file_size
        total_compressed += info.compress_size
        if total > MAX_TOTAL_BYTES:
            raise InputError("ZIP excede o total descompactado permitido")
        if path.suffix.casefold() != ".json":
            raise InputError(f"bundle do Help contém arquivo não JSON: {path.as_posix()}")
        json_members.append((info, path.as_posix()))
    if total_compressed and total / total_compressed > MAX_COMPRESSION_RATIO:
        raise InputError("possível ZIP bomb pela razão total de compressão")
    if len(json_members) != HELP_DOCUMENT_COUNT:
        raise InputError(f"esperados 12 JSONs no ZIP; encontrados {len(json_members)}")
    return sorted(json_members, key=lambda item: item[1].casefold())


def _read_zip_member(bundle: zipfile.ZipFile, info: zipfile.ZipInfo, label: str) -> bytes:
    data = bytearray()
    try:
        with bundle.open(info, "r") as source:
            while True:
                block = source.read(min(1024 * 1024, MAX_MEMBER_BYTES + 1 - len(data)))
                if not block:
                    break
                data.extend(block)
                if len(data) > MAX_MEMBER_BYTES or len(data) > info.file_size:
                    raise InputError(f"{label}: conteúdo expandido excede o declarado/permitido")
    except (OSError, RuntimeError, zipfile.BadZipFile) as exc:
        raise InputError(f"{label}: falha de integridade ZIP: {exc}") from exc
    if len(data) != info.file_size:
        raise InputError(f"{label}: tamanho expandido diverge do diretório ZIP")
    return bytes(data)


def help_documents(manifest: dict, evidence_root: Path) -> Iterator[tuple[str, bytes]]:
    artifacts = manifest.get("artifacts")
    if not isinstance(artifacts, dict):
        raise InputError("artifacts ausente no manifesto")
    group = artifacts.get("wlanguage_help_json")
    if not isinstance(group, dict):
        raise InputError("grupo wlanguage_help_json ausente")
    if group.get("expected_count", HELP_DOCUMENT_COUNT) != HELP_DOCUMENT_COUNT:
        raise InputError("expected_count de wlanguage_help_json deve ser 12")
    archive = group.get("archive")
    items = group.get("items", [])
    has_archive = archive is not None
    if has_archive and items:
        raise InputError("use archive ou items para o Help, nunca ambos")

    project = manifest.get("project")
    if not isinstance(project, dict):
        raise InputError("project ausente no manifesto")
    version = group.get("version")
    language = group.get("language")
    product_scope = group.get("product_scope")
    if not isinstance(version, str) or not version.strip():
        raise InputError("wlanguage_help_json.version é obrigatório")
    if not isinstance(language, str) or not language.strip():
        raise InputError("wlanguage_help_json.language é obrigatório")
    if (
        not isinstance(product_scope, list)
        or not product_scope
        or any(product not in VALID_PRODUCTS for product in product_scope)
        or len(set(product_scope)) != len(product_scope)
    ):
        raise InputError("wlanguage_help_json.product_scope precisa conter produtos WX válidos e únicos")
    if version.strip() != str(project.get("wlanguage_help_version", "")).strip():
        raise InputError("versão do grupo Help diverge de project.wlanguage_help_version")
    if language.strip().casefold() != str(project.get("wlanguage_help_language", "")).strip().casefold():
        raise InputError("idioma do grupo Help diverge de project.wlanguage_help_language")
    project_products = project.get("products")
    if (
        not isinstance(project_products, list)
        or not project_products
        or any(product not in VALID_PRODUCTS for product in project_products)
        or not set(project_products).issubset(set(product_scope))
    ):
        raise InputError("product_scope do Help não cobre todos os produtos declarados no projeto")

    expected_members = group.get("expected_members")
    if (
        not isinstance(expected_members, list)
        or len(expected_members) != HELP_DOCUMENT_COUNT
        or any(not isinstance(item, str) for item in expected_members)
    ):
        raise InputError("expected_members é obrigatório e precisa listar exatamente 12 caminhos")
    normalized_expected = [
        portable_relative_path(item, "expected_members").as_posix().casefold()
        for item in expected_members
    ]
    if len(set(normalized_expected)) != HELP_DOCUMENT_COUNT:
        raise InputError("expected_members contém caminhos duplicados")

    if has_archive:
        if not isinstance(archive, dict) or "path" not in archive:
            raise InputError("archive precisa ser objeto com path")
        archive_path, archive_display = existing_relative_file(evidence_root, archive.get("path"), "archive do Help")
        if Path(archive_display).suffix.casefold() != ".zip":
            raise InputError("archive do Help precisa ter extensão .zip")
        flags = os.O_RDONLY | getattr(os, "O_CLOEXEC", 0) | getattr(os, "O_NOFOLLOW", 0)
        try:
            descriptor = os.open(archive_path, flags)
        except OSError as exc:
            raise InputError(f"archive do Help não pôde ser aberto: {exc}") from exc
        try:
            before = os.fstat(descriptor)
            if not stat.S_ISREG(before.st_mode) or before.st_size > MAX_ZIP_BYTES:
                raise InputError("archive do Help não é regular ou excede o limite")
            with os.fdopen(descriptor, "rb", closefd=False) as archive_handle:
                header = archive_handle.read(4)
                if header not in {b"PK\x03\x04", b"PK\x05\x06"}:
                    raise InputError("archive do Help não possui assinatura ZIP direta")
                archive_handle.seek(0)
                try:
                    with zipfile.ZipFile(archive_handle, "r") as bundle:
                        members = validated_help_zip(bundle)
                        expected = sorted(normalized_expected)
                        actual = sorted(name.casefold() for _, name in members)
                        if expected != actual:
                            raise InputError("membros do ZIP divergem de expected_members")
                        for info, member_name in members:
                            label = f"{archive_display}!{member_name}"
                            yield label, _read_zip_member(bundle, info, label)
                except zipfile.BadZipFile as exc:
                    raise InputError(f"archive do Help não é ZIP íntegro: {exc}") from exc
            after = os.fstat(descriptor)
            if (before.st_dev, before.st_ino, before.st_size, before.st_mtime_ns) != (
                after.st_dev, after.st_ino, after.st_size, after.st_mtime_ns
            ):
                raise InputError("archive do Help foi alterado durante a leitura")
        finally:
            os.close(descriptor)
        return

    if not isinstance(items, list):
        raise InputError("items de wlanguage_help_json precisa ser lista")
    if len(items) != HELP_DOCUMENT_COUNT:
        raise InputError(f"esperados 12 JSONs; encontrados {len(items)}")
    resolved_items: list[tuple[Path, str]] = []
    seen_paths: set[str] = set()
    total = 0
    for index, item in enumerate(items):
        if not isinstance(item, dict):
            raise InputError(f"item {index} do Help precisa ser objeto")
        path, display = existing_relative_file(evidence_root, item.get("path"), f"item {index} do Help")
        canonical = display.casefold()
        if canonical in seen_paths:
            raise InputError(f"caminho duplicado nos JSONs do Help: {display}")
        seen_paths.add(canonical)
        if path.suffix.casefold() != ".json":
            raise InputError(f"Help não JSON: {display}")
        size = path.stat(follow_symlinks=False).st_size
        total += size
        if size > MAX_MEMBER_BYTES or total > MAX_TOTAL_BYTES:
            raise InputError("JSONs do Help excedem os limites seguros")
        resolved_items.append((path, display))
    resolved_items.sort(key=lambda item: item[1].casefold())
    expected = sorted(normalized_expected)
    actual = sorted(display.casefold() for _, display in resolved_items)
    if expected != actual:
        raise InputError("items divergem de expected_members")
    for path, display in resolved_items:
        yield display, read_regular_file(path, MAX_MEMBER_BYTES, display)


def walk_iterative(value: object) -> Iterator[tuple[str, object]]:
    stack: list[tuple[str, object, int]] = [("", value, 0)]
    visited = 0
    while stack:
        pointer, node, depth = stack.pop()
        visited += 1
        if visited > MAX_JSON_NODES:
            raise InputError(f"caminhada JSON excede {MAX_JSON_NODES} nós")
        if depth > MAX_JSON_DEPTH:
            raise InputError(f"caminhada JSON excede profundidade {MAX_JSON_DEPTH}")
        yield pointer or "/", node
        if isinstance(node, dict):
            for key, child in reversed(list(node.items())):
                stack.append((f"{pointer}/{pointer_escape(key)}", child, depth + 1))
        elif isinstance(node, list):
            for index in range(len(node) - 1, -1, -1):
                stack.append((f"{pointer}/{index}", node[index], depth + 1))


def direct_text(value: object) -> str:
    scalar = (str, int, float, bool)
    if isinstance(value, str):
        require_well_formed_unicode(value, "texto do Help")
        return sanitize_urls_in_text(value.strip(), "texto do Help")
    if isinstance(value, list) and value and all(isinstance(item, scalar) or item is None for item in value):
        for item in value:
            if isinstance(item, str):
                require_well_formed_unicode(item, "texto do Help")
        return sanitize_urls_in_text(
            " | ".join(str(item) for item in value if item is not None).strip(),
            "texto do Help",
        )
    if not isinstance(value, dict):
        return ""
    parts: list[str] = []
    for key, item in value.items():
        require_well_formed_unicode(key, "chave do Help")
        if isinstance(item, scalar) or item is None:
            if isinstance(item, str):
                require_well_formed_unicode(item, f"campo {key}")
            rendered = "null" if item is None else str(item)
            if rendered.strip():
                safe_value = sanitize_urls_in_text(rendered.strip(), f"campo {key}")
                parts.append(f"{key}: {safe_value}")
        elif isinstance(item, list) and item and all(isinstance(entry, scalar) or entry is None for entry in item):
            for entry in item:
                if isinstance(entry, str):
                    require_well_formed_unicode(entry, f"campo {key}")
            safe_value = sanitize_urls_in_text(
                " | ".join(str(entry) for entry in item if entry is not None),
                f"campo {key}",
            )
            parts.append(f"{key}: {safe_value}")
    return "\n".join(parts).strip()


def first_scalar(mapping: dict, keys: tuple[str, ...]) -> str:
    lowered: dict[str, object] = {}
    for key, value in mapping.items():
        require_well_formed_unicode(key, "chave do Help")
        lowered[key.casefold()] = value
    for key in keys:
        value = lowered.get(key)
        if isinstance(value, (str, int, float)) and not isinstance(value, bool):
            if isinstance(value, str):
                require_well_formed_unicode(value, f"valor de {key}")
            return str(value).strip()
    return ""


def bounded_title(title: str) -> tuple[str, bool, str]:
    """Bound a repeated title and retain a non-reversible identity if truncated."""
    require_well_formed_unicode(title, "título do Help")
    if len(title) <= MAX_TITLE_CHARS:
        return title, False, ""
    digest = hashlib.sha256(title.encode("utf-8")).hexdigest()
    suffix = f"… [TRUNCATED sha256:{digest[:16]}]"
    return title[: MAX_TITLE_CHARS - len(suffix)] + suffix, True, digest


def text_chunks(text: str, size: int = CHUNK_CHARS) -> Iterator[str]:
    start = 0
    while start < len(text):
        end = min(start + size, len(text))
        if end < len(text):
            split = text.rfind("\n", start, end)
            if split > start + size // 2:
                end = split
        chunk = text[start:end].strip()
        if chunk:
            yield chunk
        start = end


def _stage_file(target: Path) -> tuple[int, Path]:
    descriptor, raw = tempfile.mkstemp(prefix=f".{target.name}.", suffix=".tmp", dir=target.parent)
    os.chmod(raw, 0o600)
    return descriptor, Path(raw)


def _atomic_install_set(staged: list[tuple[Path, Path]]) -> None:
    created: list[tuple[Path, Path]] = []
    try:
        for temporary, target in staged:
            try:
                os.link(temporary, target, follow_symlinks=False)
            except FileExistsError as exc:
                raise InputError(f"saída já existe; sobrescrita proibida: {target}") from exc
            except OSError as exc:
                if exc.errno in {errno.EPERM, errno.EOPNOTSUPP, errno.ENOTSUP}:
                    raise InputError("filesystem não oferece instalação atômica sem sobrescrita") from exc
                raise
            created.append((temporary, target))
        for temporary, _ in staged:
            temporary.unlink()
    except Exception:
        for temporary, target in reversed(created):
            try:
                temp_stat = temporary.stat(follow_symlinks=False)
                target_stat = target.stat(follow_symlinks=False)
                if (temp_stat.st_dev, temp_stat.st_ino) == (target_stat.st_dev, target_stat.st_ino):
                    target.unlink()
            except OSError:
                pass
        raise


def build(
    manifest_path: Path,
    output: Path,
    summary: Path | None,
    *,
    allowed_evidence_root: Path,
    workspace_root: Path,
) -> dict:
    evidence_root = authorized_root(allowed_evidence_root, "allowed-evidence-root")
    workspace = authorized_root(workspace_root, "workspace-root")
    manifest_file = workspace_file(workspace, manifest_path, "manifest")
    manifest = load_manifest(manifest_file)
    _manifest_root_matches(manifest, manifest_file, evidence_root)

    output_file = _ensure_output_parent(
        workspace,
        output,
        "output",
        forbidden_root=evidence_root,
    )
    summary_file = _ensure_output_parent(
        workspace,
        summary,
        "summary",
        forbidden_root=evidence_root,
    ) if summary else None
    if summary_file is not None and summary_file == output_file:
        raise InputError("output e summary não podem apontar para o mesmo arquivo")

    output_fd, output_temp = _stage_file(output_file)
    summary_temp: Path | None = None
    staged: list[tuple[Path, Path]] = []
    document_count = 0
    record_count = 0
    output_bytes = 0
    source_digests: set[str] = set()
    semantic_digests: set[str] = set()
    try:
        with os.fdopen(output_fd, "w", encoding="utf-8", newline="\n") as sink:
            for source, raw in help_documents(manifest, evidence_root):
                document_count += 1
                digest = hashlib.sha256(raw).hexdigest()
                if digest in source_digests:
                    raise InputError("há JSONs do Help com conteúdo duplicado")
                source_digests.add(digest)
                root = strict_json_loads(raw, source)
                validate_json_graph(root, source)
                semantic_digest = hashlib.sha256(
                    json.dumps(
                        root,
                        ensure_ascii=False,
                        sort_keys=True,
                        separators=(",", ":"),
                        allow_nan=False,
                    ).encode("utf-8")
                ).hexdigest()
                if semantic_digest in semantic_digests:
                    raise InputError("há JSONs do Help semanticamente duplicados")
                semantic_digests.add(semantic_digest)
                for pointer, node in walk_iterative(root):
                    text = direct_text(node)
                    title = first_scalar(node, TITLE_KEYS) if isinstance(node, dict) else ""
                    if title:
                        title = sanitize_urls_in_text(title, f"{source}{pointer}/title")
                    title, title_truncated, title_sha256 = bounded_title(title)
                    if not text or (len(text) < 20 and not title):
                        continue
                    raw_url = first_scalar(node, URL_KEYS) if isinstance(node, dict) else ""
                    url = validate_http_url(raw_url, f"{source}{pointer}") if raw_url else ""
                    url_sha256 = hashlib.sha256(raw_url.encode("utf-8")).hexdigest() if raw_url else ""
                    for chunk_number, chunk in enumerate(text_chunks(text), 1):
                        record_count += 1
                        if record_count > MAX_RECORDS:
                            raise InputError(f"índice excede {MAX_RECORDS} registros")
                        record = {
                            "source": source,
                            "source_sha256": digest,
                            "json_pointer": pointer,
                            "title_or_symbol": title,
                            "title_truncated": title_truncated,
                            "title_sha256": title_sha256,
                            "url": url,
                            "url_sha256": url_sha256,
                            "chunk": chunk_number,
                            "text": chunk,
                        }
                        line = json.dumps(record, ensure_ascii=False, sort_keys=True, allow_nan=False) + "\n"
                        output_bytes += len(line.encode("utf-8"))
                        if output_bytes > MAX_OUTPUT_BYTES:
                            raise InputError("índice excede o limite seguro de saída")
                        sink.write(line)
            if document_count != HELP_DOCUMENT_COUNT:
                raise InputError(f"esperados 12 JSONs; encontrados {document_count}")
            sink.flush()
            os.fsync(sink.fileno())

        result = {
            "documents": document_count,
            "records": record_count,
            "output": output_file.relative_to(workspace).as_posix(),
            "output_bytes": output_bytes,
        }
        staged.append((output_temp, output_file))
        if summary_file is not None:
            summary_fd, summary_temp = _stage_file(summary_file)
            with os.fdopen(summary_fd, "w", encoding="utf-8", newline="\n") as handle:
                json.dump(result, handle, ensure_ascii=False, indent=2, allow_nan=False)
                handle.write("\n")
                handle.flush()
                os.fsync(handle.fileno())
            staged.append((summary_temp, summary_file))
        _atomic_install_set(staged)
        return result
    finally:
        for temporary in (output_temp, summary_temp):
            if temporary is not None:
                try:
                    temporary.unlink()
                except FileNotFoundError:
                    pass


def main() -> int:
    parser = argparse.ArgumentParser(description="Indexa com segurança exatamente 12 JSONs do Help WLanguage.")
    parser.add_argument("--manifest", required=True, type=Path)
    parser.add_argument("--allowed-evidence-root", required=True, type=Path)
    parser.add_argument("--workspace-root", required=True, type=Path)
    parser.add_argument("--output", required=True, type=Path)
    parser.add_argument("--summary", type=Path)
    args = parser.parse_args()
    try:
        result = build(
            args.manifest,
            args.output,
            args.summary,
            allowed_evidence_root=args.allowed_evidence_root,
            workspace_root=args.workspace_root,
        )
    except (OSError, InputError) as exc:
        print(f"indexação falhou: {exc}", file=sys.stderr)
        return 2
    print(json.dumps(result, ensure_ascii=False, sort_keys=True, allow_nan=False))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
