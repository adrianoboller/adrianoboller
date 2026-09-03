#!/usr/bin/env python3
"""Offline structural validator used when the Claude CLI is unavailable."""

from __future__ import annotations

import argparse
import hashlib
import json
import re
import sys
from pathlib import Path


NAME_RE = re.compile(r"^[a-z0-9]+(?:-[a-z0-9]+)*$")
MODELS = {"opus", "sonnet", "haiku", "fable", "inherit"}
EFFORTS = {"low", "medium", "high", "xhigh", "max"}
PROHIBITED_PLUGIN_AGENT_FIELDS = {"hooks", "mcpServers", "permissionMode"}
CORPUS_SHA256 = "a95ed5536549ecc39fb1163415042d6597c8913e5edbfdb531cba756546a82a2"
CORPUS_SIZE = 26_750_976


def frontmatter(path: Path) -> tuple[dict[str, str], str]:
    if path.stat().st_size > 1_000_000:
        raise ValueError("arquivo de frontmatter excede 1 MB")
    text = path.read_text(encoding="utf-8")
    if not text.startswith("---\n"):
        raise ValueError("frontmatter precisa começar na primeira linha")
    end = text.find("\n---\n", 4)
    if end < 0:
        raise ValueError("frontmatter sem fechamento")
    data: dict[str, str] = {}
    for raw in text[4:end].splitlines():
        if not raw.strip() or raw.lstrip().startswith("#"):
            continue
        if ":" not in raw:
            raise ValueError(f"linha YAML simples inválida: {raw}")
        key, value = raw.split(":", 1)
        data[key.strip()] = value.strip().strip('"\'')
    return data, text[end + 5 :]


def validate(root: Path, strict: bool = False) -> dict:
    errors: list[str] = []
    warnings: list[str] = []
    testes = "não executados (use --strict)"
    if strict and (root / "tests" / "testes.py").is_file():
        import subprocess
        proc = subprocess.run([sys.executable, str(root / "tests" / "testes.py")], capture_output=True, text=True)
        resumo = (proc.stderr.strip().splitlines() or ["?"])[-1]
        if proc.returncode != 0:
            errors.append(f"tests/testes.py falhou: {resumo}")
        testes = resumo
    manifest_path = root / ".claude-plugin" / "plugin.json"
    try:
        if manifest_path.stat().st_size > 1_000_000:
            raise ValueError("arquivo excede 1 MB")
        manifest = json.loads(manifest_path.read_text(encoding="utf-8"))
    except (OSError, UnicodeError, json.JSONDecodeError, ValueError) as exc:
        return {"valid": False, "errors": [f"plugin.json: {exc}"], "warnings": []}
    if not isinstance(manifest, dict):
        return {
            "valid": False,
            "errors": ["plugin.json: o documento JSON deve ser um objeto"],
            "warnings": [],
        }
    name = manifest.get("name")
    if not isinstance(name, str) or not NAME_RE.fullmatch(name):
        errors.append("plugin.json: name precisa estar em kebab-case")
    if name != root.name:
        errors.append(f"plugin.json: name {name!r} difere da pasta {root.name!r}")
    version = manifest.get("version")
    if version and not re.fullmatch(r"\d+\.\d+\.\d+(?:[-+][0-9A-Za-z.-]+)?", str(version)):
        errors.append("plugin.json: version não é SemVer")

    skill_files = sorted((root / "skills").glob("*/SKILL.md"))
    if not skill_files:
        errors.append("nenhuma skill em skills/*/SKILL.md")
    for path in skill_files:
        try:
            data, body = frontmatter(path)
        except (OSError, UnicodeError, ValueError) as exc:
            errors.append(f"{path.relative_to(root)}: {exc}")
            continue
        if not data.get("description"):
            errors.append(f"{path.relative_to(root)}: description ausente")
        if data.get("name") and not NAME_RE.fullmatch(data["name"]):
            errors.append(f"{path.relative_to(root)}: name inválido")
        if data.get("name") != path.parent.name:
            errors.append(f"{path.relative_to(root)}: name precisa coincidir com a pasta da skill")
        # O limite formal do Claude Code e 1024, mas medido: com 895 caracteres a
        # skill do Impeccable sumia da listagem quando o plugin inteiro carregava
        # (26 agentes e 4 comandos). Acima de 300 e aviso; acima de 1024, erro.
        tamanho = len(data.get("description", ""))
        if tamanho > 1024:
            errors.append(f"{path.relative_to(root)}: description excede 1024 caracteres")
        elif tamanho > 300:
            warnings.append(f"{path.relative_to(root)}: description com {tamanho} caracteres; acima de 300 pode sumir da listagem")
        if data.get("model") and data["model"] not in MODELS and not data["model"].startswith("claude-"):
            errors.append(f"{path.relative_to(root)}: model inválido")
        if data.get("effort") and data["effort"] not in EFFORTS:
            errors.append(f"{path.relative_to(root)}: effort inválido")
        if not body.strip():
            errors.append(f"{path.relative_to(root)}: corpo vazio")

    agent_files = sorted((root / "agents").glob("*.md"))
    if not agent_files:
        errors.append("nenhum agente em agents/*.md")
    names: set[str] = set()
    for path in agent_files:
        try:
            data, body = frontmatter(path)
        except (OSError, UnicodeError, ValueError) as exc:
            errors.append(f"{path.relative_to(root)}: {exc}")
            continue
        agent_name = data.get("name", "")
        if not NAME_RE.fullmatch(agent_name):
            errors.append(f"{path.relative_to(root)}: name inválido")
        if agent_name in names:
            errors.append(f"{path.relative_to(root)}: name duplicado")
        names.add(agent_name)
        if not data.get("description"):
            errors.append(f"{path.relative_to(root)}: description ausente")
        model = data.get("model", "")
        if model not in MODELS and not model.startswith("claude-"):
            errors.append(f"{path.relative_to(root)}: model inválido: {model}")
        if data.get("effort") not in EFFORTS:
            errors.append(f"{path.relative_to(root)}: effort inválido")
        if data.get("isolation") and data["isolation"] != "worktree":
            errors.append(f"{path.relative_to(root)}: isolation inválido")
        for prohibited in PROHIBITED_PLUGIN_AGENT_FIELDS & data.keys():
            errors.append(f"{path.relative_to(root)}: campo proibido em agente de plugin: {prohibited}")
        if not body.strip():
            errors.append(f"{path.relative_to(root)}: corpo vazio")

    required = [
        "skills/conversao-wx/templates/CLAUDE.md",
        "skills/conversao-wx/templates/wx-inputs.manifest.json",
        "skills/conversao-wx/templates/conversion.config.json",
        "skills/conversao-wx/templates/traceability.csv",
        "skills/conversao-wx/scripts/wx_preflight.py",
        "skills/conversao-wx/scripts/build_help_index.py",
        "skills/conversao-wx/scripts/query_wlanguage_help.py",
        "skills/conversao-wx/scripts/safe_unpack_bundle.py",
        "skills/conversao-wx/scripts/validate_traceability.py",
        "skills/conversao-wx/scripts/aplicar_questionario.py",
        "skills/conversao-wx/templates/questionario.json",
        "skills/conversao-wx/scripts/licenca.py",
        "skills/conversao-wx/scripts/verificar_ambiente.py",
        "licenca/chave-publica.json",
        "licenca/LEIA-ME.md",
        "commands/questionario.md",
        "commands/pmo.md",
        "skills/conversao-wx/scripts/extrair_pdf.py",
        "skills/conversao-wx/scripts/golden.py",
        "skills/conversao-wx/scripts/uso_de_tokens.py",
        "skills/conversao-wx/references/corpus-saneamento.md",
        "hooks/portao_g0.py",
        "tests/testes.py",
        "exemplos/estoque-wx/questionario.json",
        "exemplos/estoque-wx/inputs/banco.sql",
        "exemplos/estoque-wx/inputs/estoque-completo.pdf",
        "MANUAL.md",
        "skills/conversao-wx/references/qualidade-erp.md",
        "skills/conversao-wx/references/papeis-e-pdca.md",
        "agents/papel-a-orquestrador.md",
        "agents/papel-j-pesquisador-act.md",
        "skills/conversao-wx/references/perfis-de-destino.md",
        "skills/conversao-wx/references/perfil-csharp-wl.md",
        "skills/conversao-wx/resources/wl-csharp/funcoes.json",
        "skills/conversao-wx/scripts/pmo.py",
        "skills/conversao-wx/scripts/rotear_modelo.py",
        "skills/conversao-wx/references/pmo.md",
        "skills/conversao-wx/references/equipe-wlanguage.md",
        "skills/conversao-wx/references/balanceamento-de-modelos.md",
        "agents/pmo-gerente-de-projetos.md",
        "agents/wl-hfsql-specialist.md",
        "commands/converter.md",
        "commands/estilo-telas.md",
        "commands/laudo-tokens.md",
        "skills/impeccable/SKILL.md",
        "skills/impeccable/LICENSE",
        "skills/laudo-uso-tokens/SKILL.md",
        "skills/laudo-uso-tokens/PROMPT-MESTRE-CURTO.md",
    ]
    # Strict mode verifies the complete offline workflow rather than only its
    # entrypoints: create the workspace, inventory/validate the evidence,
    # index WLanguage Help, and enforce the schema contracts used by preflight.
    if strict:
        required.extend(
            [
                "skills/conversao-wx/scripts/bootstrap_workspace.py",
                "skills/conversao-wx/scripts/validate_plugin_bundle.py",
                "skills/conversao-wx/schemas/wx-inputs.schema.json",
                "skills/conversao-wx/schemas/conversion-config.schema.json",
                "skills/conversao-wx/references/intake-and-evidence.md",
                "skills/conversao-wx/references/agent-orchestration.md",
                "skills/conversao-wx/references/conversion-workflow.md",
                "skills/conversao-wx/references/deliverables-and-gates.md",
                "skills/conversao-wx/references/traceability.md",
                "skills/conversao-wx/references/wlanguage-semantics.md",
                "skills/conversao-wx/references/official-sources.md",
                "skills/conversao-wx/references/bundled-help-corpus.md",
                "skills/conversao-wx/resources/README.md",
                "skills/conversao-wx/resources/Help_WL_12k_Json.zip",
            ]
        )
    for relative in required:
        if not (root / relative).is_file():
            errors.append(f"arquivo obrigatório ausente: {relative}")

    if strict:
        corpus = root / "skills/conversao-wx/resources/Help_WL_12k_Json.zip"
        try:
            if corpus.stat().st_size != CORPUS_SIZE:
                errors.append("corpus WLanguage bundled possui tamanho inesperado")
            else:
                digest = hashlib.sha256()
                with corpus.open("rb") as handle:
                    for block in iter(lambda: handle.read(1024 * 1024), b""):
                        digest.update(block)
                if digest.hexdigest() != CORPUS_SHA256:
                    errors.append("corpus WLanguage bundled possui SHA-256 inesperado")
        except OSError as exc:
            errors.append(f"corpus WLanguage bundled não pôde ser validado: {exc}")

    todo_marker = "[" + "TODO:"
    for path in root.rglob("*"):
        if path.is_file() and ".git" not in path.parts and "__pycache__" not in path.parts:
            if path.stat().st_size > 2_000_000:
                continue
            try:
                text = path.read_text(encoding="utf-8")
            except (OSError, UnicodeError):
                continue
            if todo_marker in text or "TODO" + "_PLACEHOLDER" in text:
                errors.append(f"placeholder não resolvido: {path.relative_to(root)}")
    if len(agent_files) < 8:
        warnings.append("menos de oito agentes especializados")
    valid = not errors and (not strict or not warnings)
    return {
        "valid": valid,
        "plugin": name,
        "tests": testes,
        "skills": len(skill_files),
        "agents": len(agent_files),
        "errors": errors,
        "warnings": warnings,
    }


def main() -> int:
    parser = argparse.ArgumentParser(description="Valida estrutura do plugin WX sem Claude CLI.")
    parser.add_argument("root", nargs="?", default="../../../", type=Path)
    parser.add_argument("--strict", action="store_true")
    args = parser.parse_args()
    result = validate(args.root.resolve(), args.strict)
    print(json.dumps(result, ensure_ascii=False, indent=2))
    return 0 if result["valid"] else 2


if __name__ == "__main__":
    raise SystemExit(main())
