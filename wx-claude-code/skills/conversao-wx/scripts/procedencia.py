#!/usr/bin/env python3
"""Procedencia da entrega: SLSA provenance e CycloneDX BOM, medidos do projeto.

Por que existe: em banco, governo e saude, "confie em mim" nao compra nada. A
pergunta na mesa de compras e sempre a mesma -- *de onde veio este artefato, quem
o construiu, com que fontes, e o que tem dentro dele?* Sem resposta em formato
que a area de seguranca do cliente ja le, o contrato trava.

O plugin ja tinha metade: o `exportar_projeto.py` grava SHA-256 de todo arquivo
entregue e o `FONTES.md` e inventario medido. Faltava dizer isso em SLSA e
CycloneDX, que sao os dois formatos que aquelas areas pedem pelo nome.

O que ele afirma, e o que NAO afirma -- esta distincao e o coracao do script:

  AFIRMA    o que foi medido agora: hash de cada arquivo, commit, quem rodou,
            quando, com que versao do plugin, com que modelos e skills, e as
            evidencias e restricoes ligadas.
  NAO AFIRMA que o build e reprodutivel bit a bit, que a cadeia foi assinada por
            terceiro, nem nivel de SLSA que dependa de infraestrutura que este
            plugin nao controla. Campo que nao se mede sai "INDISPONIVEL", e o
            documento diz por que.

Um BOM que enche campo para parecer completo e pior que BOM nenhum: ele passa na
conferencia automatica do cliente e mente na auditoria seguinte.

Uso:
  procedencia.py bom [--saida ARQ]          CycloneDX 1.5, JSON
  procedencia.py slsa [--saida ARQ]         SLSA provenance v1, in-toto
  procedencia.py tudo [--assinar CHAVE]     os dois, e assina se pedirem
  procedencia.py conferir ARQ               reconfere os hashes do documento
"""
from __future__ import annotations

import argparse
import hashlib
import json
import os
import platform
import subprocess
import sys
import uuid
from datetime import datetime, timezone
from pathlib import Path

INDISP = "INDISPONÍVEL"
# O que entra no BOM como "componente do produto". Documento e evidencia entram
# como outra categoria, porque nao sao software entregue.
FONTE = {".rs", ".py", ".ts", ".tsx", ".js", ".java", ".cs", ".go", ".rb", ".php",
         ".kt", ".swift", ".c", ".h", ".cpp", ".hpp", ".sql", ".sh", ".ps1"}
IGNORAR = {".git", "node_modules", "target", "dist", "build", "__pycache__",
           ".venv", "venv", "inputs", "artefatos", "vendor", ".wx-migration"}


def agora() -> str:
    return datetime.now(timezone.utc).isoformat(timespec="seconds")


def sha256(p: Path) -> str:
    h = hashlib.sha256()
    with p.open("rb") as f:
        for b in iter(lambda: f.read(1 << 20), b""):
            h.update(b)
    return h.hexdigest()


def git(raiz: Path, *args: str) -> str:
    try:
        r = subprocess.run(["git", *args], cwd=raiz, capture_output=True, text=True, timeout=20)
        return r.stdout.strip() if r.returncode == 0 else ""
    except (OSError, subprocess.TimeoutExpired):
        return ""


def arquivos(raiz: Path) -> list[Path]:
    achados = []
    for p in sorted(raiz.rglob("*")):
        if not p.is_file() or p.suffix.lower() not in FONTE:
            continue
        if any(parte in IGNORAR for parte in p.relative_to(raiz).parts):
            continue
        achados.append(p)
    return achados


def contexto(raiz: Path, plugin: Path) -> dict:
    """Tudo que se sabe MEDINDO. O que nao se mede fica marcado, nao chutado."""
    versao_plugin = INDISP
    manifesto = plugin / ".claude-plugin" / "plugin.json"
    if manifesto.is_file():
        try:
            versao_plugin = json.loads(manifesto.read_text(encoding="utf-8"))["version"]
        except (json.JSONDecodeError, KeyError):
            pass
    cfg = {}
    p_cfg = raiz / ".wx-migration" / "conversion.config.json"
    if p_cfg.is_file():
        try:
            cfg = json.loads(p_cfg.read_text(encoding="utf-8"))
        except json.JSONDecodeError:
            cfg = {}
    nome = INDISP
    man = raiz / ".wx-migration" / "wx-inputs.manifest.json"
    if man.is_file():
        try:
            nome = json.loads(man.read_text(encoding="utf-8")).get("project", {}).get("name") or INDISP
        except json.JSONDecodeError:
            pass
    return {
        "projeto": nome,
        "commit": git(raiz, "rev-parse", "HEAD") or INDISP,
        "branch": git(raiz, "rev-parse", "--abbrev-ref", "HEAD") or INDISP,
        "arvore_limpa": (git(raiz, "status", "--porcelain") == ""
                         if git(raiz, "rev-parse", "HEAD") else INDISP),
        "remoto": git(raiz, "config", "--get", "remote.origin.url") or INDISP,
        "plugin_versao": versao_plugin,
        "destino": cfg.get("target", {}),
        "fidelidade": cfg.get("fidelity", {}),
        "construido_em": agora(),
        # quem rodou: o nome do usuario do sistema, sem e-mail nem credencial
        "construido_por": os.environ.get("USER") or os.environ.get("USERNAME") or INDISP,
        "maquina": f"{platform.system()} {platform.machine()}",
        "python": platform.python_version(),
    }


def bom(raiz: Path, plugin: Path) -> dict:
    ctx = contexto(raiz, plugin)
    componentes = []
    for p in arquivos(raiz):
        rel = p.relative_to(raiz).as_posix()
        componentes.append({
            "type": "file",
            "bom-ref": rel,
            "name": rel,
            "hashes": [{"alg": "SHA-256", "content": sha256(p)}],
            "properties": [{"name": "wx:bytes", "value": str(p.stat().st_size)}],
        })
    # o que a IA usou tambem entra: e o ponto do AI-BOM, e o que diferencia
    # este documento de um SBOM comum
    servicos = []
    modelos = raiz / ".wx-migration" / "pmo" / "roteamento.json"
    if modelos.is_file():
        try:
            d = json.loads(modelos.read_text(encoding="utf-8"))
            for m in sorted({x.get("modelo", "") for x in d.get("decisoes", []) if x.get("modelo")}):
                servicos.append({"bom-ref": f"modelo:{m}", "name": m,
                                 "description": "modelo de linguagem usado na conversão"})
        except json.JSONDecodeError:
            pass
    skills = []
    for s in sorted((plugin / "skills").glob("*/SKILL.md")) if (plugin / "skills").is_dir() else []:
        skills.append({"type": "library", "bom-ref": f"skill:{s.parent.name}",
                       "name": s.parent.name, "version": ctx["plugin_versao"],
                       "hashes": [{"alg": "SHA-256", "content": sha256(s)}],
                       "properties": [{"name": "wx:tipo", "value": "agent-skill"}]})
    return {
        "bomFormat": "CycloneDX",
        "specVersion": "1.5",
        "serialNumber": f"urn:uuid:{uuid.uuid4()}",
        "version": 1,
        "metadata": {
            "timestamp": ctx["construido_em"],
            "tools": [{"vendor": "Boller Sistemas", "name": "wx-claude-code",
                       "version": ctx["plugin_versao"]}],
            "component": {"type": "application", "bom-ref": "produto",
                          "name": ctx["projeto"], "version": ctx["commit"][:12] or INDISP},
            "properties": [
                {"name": "wx:commit", "value": ctx["commit"]},
                {"name": "wx:arvore_limpa", "value": str(ctx["arvore_limpa"])},
                {"name": "wx:destino", "value": ctx["destino"].get("language", INDISP)},
                {"name": "wx:limite",
                 "value": "inventário medido dos arquivos-fonte do projeto; NÃO cobre "
                          "dependências de terceiros resolvidas pelo gerenciador de pacotes, "
                          "que devem ser listadas pelo BOM do ecossistema (cargo/npm/pip)"},
            ],
        },
        "components": componentes + skills,
        "services": servicos,
    }


def slsa(raiz: Path, plugin: Path) -> dict:
    ctx = contexto(raiz, plugin)
    saidas = [{"name": p.relative_to(raiz).as_posix(),
               "digest": {"sha256": sha256(p)}} for p in arquivos(raiz)]
    entradas = []
    inputs = raiz / "inputs"
    if inputs.is_dir():
        for p in sorted(inputs.rglob("*")):
            if p.is_file():
                entradas.append({"uri": p.relative_to(raiz).as_posix(),
                                 "digest": {"sha256": sha256(p)}})
    return {
        "_type": "https://in-toto.io/Statement/v1",
        "subject": saidas,
        "predicateType": "https://slsa.dev/provenance/v1",
        "predicate": {
            "buildDefinition": {
                "buildType": "https://boller.dev/wx-claude-code/conversao/v1",
                "externalParameters": {
                    "projeto": ctx["projeto"],
                    "destino": ctx["destino"],
                    "fidelidade": ctx["fidelidade"],
                },
                "internalParameters": {
                    "plugin": {"name": "wx-claude-code", "version": ctx["plugin_versao"]},
                    "python": ctx["python"],
                },
                "resolvedDependencies": entradas,
            },
            "runDetails": {
                "builder": {
                    "id": "https://boller.dev/wx-claude-code",
                    "version": {"plugin": ctx["plugin_versao"]},
                },
                "metadata": {
                    "invocationId": str(uuid.uuid4()),
                    "startedOn": ctx["construido_em"],
                    "finishedOn": ctx["construido_em"],
                },
                "byproducts": [
                    {"name": "wx:construido_por", "content": ctx["construido_por"]},
                    {"name": "wx:maquina", "content": ctx["maquina"]},
                    {"name": "wx:commit", "content": ctx["commit"]},
                    {"name": "wx:branch", "content": ctx["branch"]},
                    {"name": "wx:arvore_limpa", "content": str(ctx["arvore_limpa"])},
                ],
            },
            # a parte que a maioria dos geradores omite, e que e a mais honesta
            "_limites": {
                "nivel_slsa": INDISP,
                "por_que": ("nível de SLSA depende de propriedades da INFRAESTRUTURA de build "
                            "(isolamento do executor, ausência de acesso do autor ao builder, "
                            "retenção da proveniência) que este plugin não controla e não "
                            "consegue medir. Este documento afirma apenas o que mediu."),
                "nao_afirma": [
                    "que o build é reprodutível bit a bit",
                    "que a proveniência foi assinada por um terceiro de confiança",
                    "que o executor era isolado do autor",
                ],
            },
        },
    }


def assinar_documento(doc: dict, chave: Path) -> dict:
    """Assina com a MESMA RSA do serial: nada de dependencia nova para isto."""
    sys.path.insert(0, str(Path(__file__).resolve().parent))
    import licenca  # noqa: PLC0415
    priv = json.loads(chave.read_text(encoding="utf-8"))
    corpo = json.dumps(doc, ensure_ascii=False, sort_keys=True, separators=(",", ":")).encode()
    return {
        "payload": doc,
        "assinatura": {
            "algoritmo": "RSA-2048 SHA-256 (implementação do plugin, licenca.py)",
            "valor": licenca._b64(licenca.assinar(corpo, priv)),
            "assinado_em": agora(),
            "confere_com": "licenca/chave-publica.json ou a pública do par usado",
        },
    }


def escrever(doc: dict, saida: Path | None, padrao: Path, json_saida: bool) -> Path:
    alvo = saida or padrao
    alvo.parent.mkdir(parents=True, exist_ok=True)
    alvo.write_text(json.dumps(doc, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
    if json_saida:
        print(json.dumps({"escrito": str(alvo)}, ensure_ascii=False))
    else:
        print(f"escrito {alvo}")
    return alvo


def cmd_bom(args, raiz: Path, plugin: Path) -> int:
    d = bom(raiz, plugin)
    escrever(d, Path(args.saida) if args.saida else None,
             raiz / ".wx-migration" / "procedencia" / "bom-cyclonedx.json", args.json)
    if not args.json:
        print(f"  {len(d['components'])} componentes · {len(d['services'])} serviços de IA")
    return 0


def cmd_slsa(args, raiz: Path, plugin: Path) -> int:
    d = slsa(raiz, plugin)
    if args.assinar:
        d = assinar_documento(d, Path(args.assinar))
    escrever(d, Path(args.saida) if args.saida else None,
             raiz / ".wx-migration" / "procedencia" / "slsa-provenance.json", args.json)
    if not args.json:
        alvo = d.get("payload", d)
        print(f"  {len(alvo['subject'])} artefatos · nível SLSA {INDISP} (por quê, dentro do documento)")
    return 0


def cmd_tudo(args, raiz: Path, plugin: Path) -> int:
    cmd_bom(args, raiz, plugin)
    args.saida = None
    return cmd_slsa(args, raiz, plugin)


def cmd_conferir(args, raiz: Path, plugin: Path) -> int:
    """Reconfere os hashes do documento contra os arquivos de hoje."""
    doc = json.loads(Path(args.arquivo).read_text(encoding="utf-8"))
    doc = doc.get("payload", doc)
    itens = []
    if doc.get("bomFormat") == "CycloneDX":
        for c in doc.get("components", []):
            h = next((x["content"] for x in c.get("hashes", []) if x["alg"] == "SHA-256"), "")
            if h and not c["name"].startswith("skill:"):
                itens.append((c["name"], h))
    else:
        for s in doc.get("subject", []):
            itens.append((s["name"], s["digest"]["sha256"]))
    mudados, sumidos = [], []
    for nome, h in itens:
        p = raiz / nome
        if not p.is_file():
            sumidos.append(nome)
        elif sha256(p) != h:
            mudados.append(nome)
    resumo = {"conferidos": len(itens), "mudados": mudados, "sumidos": sumidos}
    if args.json:
        print(json.dumps(resumo, ensure_ascii=False))
    else:
        print(f"{len(itens)} artefatos conferidos · {len(mudados)} mudaram · {len(sumidos)} sumiram")
        for n in (mudados + sumidos)[:10]:
            print(f"  {n}")
    return 1 if (mudados or sumidos) else 0


def main() -> int:
    p = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    p.add_argument("--project-root", default=".")
    p.add_argument("--plugin-root", default=str(Path(__file__).resolve().parents[3]))
    p.add_argument("--json", action="store_true")
    sub = p.add_subparsers(dest="cmd", required=True)
    b = sub.add_parser("bom", help="CycloneDX 1.5 do projeto")
    b.add_argument("--saida")
    s = sub.add_parser("slsa", help="SLSA provenance v1 (in-toto)")
    s.add_argument("--saida")
    s.add_argument("--assinar", help="chave privada JSON (a mesma do serial)")
    t = sub.add_parser("tudo", help="os dois documentos")
    t.add_argument("--saida")
    t.add_argument("--assinar")
    c = sub.add_parser("conferir", help="reconfere os hashes do documento")
    c.add_argument("arquivo")
    args = p.parse_args()
    raiz, plugin = Path(args.project_root).resolve(), Path(args.plugin_root).resolve()
    return {"bom": cmd_bom, "slsa": cmd_slsa, "tudo": cmd_tudo,
            "conferir": cmd_conferir}[args.cmd](args, raiz, plugin)


try:
    import registro
except ImportError:
    sys.path.insert(0, str(Path(__file__).resolve().parent))
    import registro


if __name__ == "__main__":
    sys.exit(registro.envolver(__file__, main))
