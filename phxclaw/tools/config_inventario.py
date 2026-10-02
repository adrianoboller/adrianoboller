#!/usr/bin/env python3
"""Inventario dos JSON de `config/` (SP000020): o que e lido por quem.

Para cada `config/**/*.json`, lista os FONTES DE CODIGO (.rs, .py, .js, .mjs, .sh, .toml,
Cargo) que citam o arquivo pelo nome -- `reports/`, `docs/` e `sprints/` ficam de fora,
porque relatorio citar um arquivo nao e le-lo. Sem leitor pelo nome, tenta o radical
(`anthropic-provider` para `anthropic-provider.v031.json`) e diz que foi pelo radical.

Classifica cada um:
- DADO: lido por codigo Rust (manifesto de papel, constituicao, skill, trust store...);
- FERRAMENTA: lido so por script de verificacao/instalacao (tools/, scripts/, installer/);
- SEM LEITOR: ninguem no codigo o abre -- candidato a arquivo morto ou a chave do catalogo,
  se o conteudo for configuracao de usuario (a coluna `cfg?` marca chaves que coincidem com
  o catalogo do config.json: `model`, `base_url`, `host_policy`...).

O que a suite PROVA sobre todos eles (segredo e variavel fora do catalogo) esta em
`crates/phxclaw-config-runtime/tests/config_json.rs`; este script so conta e lista.

Uso: python3 tools/config_inventario.py [--raiz DIR] [--listar]
"""
import argparse
import json
import re
from pathlib import Path

CODIGO = (".rs", ".py", ".js", ".mjs", ".sh", ".toml")
FORA = ("config", "reports", "docs", "sprints", "target", "node_modules", ".git", "third_party")
PISTAS_DE_CONFIG = {"model", "model_sha256", "base_url", "host_policy", "program", "provider",
                    "runtime", "api", "timeout_seconds", "allow_remote_http"}


def fontes(raiz):
    for p in raiz.rglob("*"):
        if p.is_dir() or any(x in p.parts for x in FORA):
            continue
        if p.suffix in CODIGO or p.name == "Cargo.toml":
            try:
                yield p.relative_to(raiz).as_posix(), p.read_text(encoding="utf-8", errors="replace")
            except OSError:
                pass


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--raiz", default=str(Path(__file__).resolve().parent.parent))
    ap.add_argument("--listar", action="store_true")
    a = ap.parse_args()
    raiz = Path(a.raiz)
    arqs = sorted(raiz.glob("config/**/*.json"))
    fon = dict(fontes(raiz))
    classes = {"DADO": [], "FERRAMENTA": [], "SEM LEITOR": []}
    for arq in arqs:
        rel = arq.relative_to(raiz).as_posix()
        nome = arq.name
        radical = re.sub(r"\.v\d+\.json$|\.json$", "", nome)
        quem = [f for f, t in fon.items() if nome in t and f != "tools/config_inventario.py"]
        por = "nome"
        pasta = arq.parent.relative_to(raiz).as_posix()
        if not quem and pasta != "config":
            # A SUBpasta inteira lida por `load_dir`/`read_dir` (config/agents, config/skills,
            # config/knowledge-sources): o leitor cita a pasta, nao cada arquivo. A raiz
            # `config/` nao conta: todo mundo cita `config/alguma-coisa`.
            quem = [f for f, t in fon.items() if f.endswith(".rs") and pasta in t]
            por = "pasta"
        if not quem and radical not in ("capabilities",):
            # So em script: num .rs ou Cargo.toml o radical casa com o nome de um crate.
            quem = [f for f, t in fon.items() if radical in t and f.endswith((".py", ".sh"))]
            por = "radical"
        try:
            doc = json.loads(arq.read_text(encoding="utf-8"))
            chaves = set(doc.keys()) if isinstance(doc, dict) else set()
        except (OSError, ValueError):
            chaves = set()
        cfg = bool(chaves & PISTAS_DE_CONFIG)
        if any(q.endswith(".rs") for q in quem) and por in ("nome", "pasta"):
            classe = "DADO"
        elif quem:
            classe = "FERRAMENTA"
        else:
            classe = "SEM LEITOR"
        classes[classe].append((rel, por, quem[:3], cfg))
    for classe, lista in classes.items():
        print(f"{classe}: {len(lista)}")
        if a.listar:
            for rel, por, quem, cfg in lista:
                print(f"  {rel}  [{por}] {quem}{'  cfg?' if cfg else ''}")
    total = len(arqs)
    sem = classes["SEM LEITOR"]
    print(f"total: {total} JSON em config/ (agents: {sum(1 for p in arqs if 'agents' in p.parts)}); "
          f"sem leitor no codigo: {len(sem)}, dos quais com cara de configuracao de usuario: "
          f"{sum(1 for x in sem if x[3])}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
