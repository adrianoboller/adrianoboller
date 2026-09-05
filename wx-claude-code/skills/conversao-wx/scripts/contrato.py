#!/usr/bin/env python3
"""Contrato ativo do projeto: o que vale HOJE, separado do historico.

O problema, que aparece em todo projeto longo: na sprint 10 decidiu-se MySQL, na
20 avaliou-se PostgreSQL, na 30 fechou-se PhxSql -- e na 40 alguem cita MySQL
numa conversa. Um agente que le o historico inteiro sem saber o que foi superado
usa a decisao velha, com toda a confianca do mundo.

A separacao e simples e nao apaga nada:

  HISTORICO   tudo que ja se decidiu, com data e motivo -- fica
  CONTRATO    o subconjunto que esta em vigor agora

O contrato sai MEDIDO das fontes que ja existem no projeto -- decisoes
(`decisoes/DEC-*.md`), restricoes (`constraints.json`), destino e fidelidade
(`conversion.config.json`) e as respostas do questionario -- e traz um hash, que
e como uma sessao nova percebe que o contrato mudou desde a ultima leitura.

Decisao com `Status: superseded` sai do contrato e vai para a lista de superadas,
com o motivo. Decisao `rejected` idem. O que nao tem status legivel entra como
`indefinida` e aparece no contrato como pendencia -- nunca como vigente, porque
o silencio de um campo nao e aprovacao.

Uso:
  contrato.py gerar [--json]     escreve .wx-migration/contrato-ativo.md
  contrato.py conferir           sai 1 se o contrato mudou desde o hash gravado
"""
from __future__ import annotations

import argparse
import hashlib
import json
import re
import sys
from datetime import date
from pathlib import Path

VIGENTES = {"approved", "accepted", "aprovada", "aprovado", "ativa", "ativo"}
SUPERADAS = {"superseded", "superada", "superado", "rejected", "rejeitada", "rejeitado"}


def ler_decisoes(raiz: Path) -> list[dict]:
    """Le as fichas DEC-*.md onde quer que o projeto as guarde."""
    achadas: list[dict] = []
    for pasta in (raiz / ".wx-migration" / "decisoes", raiz / ".wx-migration", raiz / "docs" / "decisoes"):
        if not pasta.is_dir():
            continue
        for arq in sorted(pasta.glob("DEC-*.md")):
            texto = arq.read_text(encoding="utf-8", errors="replace")
            titulo = texto.splitlines()[0].lstrip("# ").strip() if texto.strip() else arq.stem
            # o cabecalho da ficha ja comeca pelo id; repeti-lo daria
            # "DEC-0003 — DEC-0003 — Banco", que e ruido no contrato
            titulo = re.sub(rf"^{re.escape(arq.stem)}\s*[—-]\s*", "", titulo)
            def campo(nome: str) -> str:
                m = re.search(rf"^[-*]?\s*{nome}\s*:\s*(.+)$", texto, re.M | re.I)
                return m.group(1).strip() if m else ""
            estado = campo("Status").lower()
            # a ficha modelo lista as opcoes na propria linha; isso nao e estado
            if "|" in estado:
                estado = ""
            situacao = ("vigente" if estado in VIGENTES else
                        "superada" if estado in SUPERADAS else "indefinida")
            achadas.append({
                "id": arq.stem, "titulo": titulo, "arquivo": str(arq.relative_to(raiz)),
                "status": estado or "(em branco)", "situacao": situacao,
                "decisao": campo("Decisão") or campo("Decisao"),
                "superada_por": campo("Superada por") or campo("Superseded by"),
                "data": campo("Data"),
            })
    return achadas


def montar(raiz: Path) -> dict:
    wx = raiz / ".wx-migration"
    cfg = {}
    if (wx / "conversion.config.json").is_file():
        cfg = json.loads((wx / "conversion.config.json").read_text(encoding="utf-8"))
    q = {}
    if (wx / "questionario.json").is_file():
        try:
            q = json.loads((wx / "questionario.json").read_text(encoding="utf-8"))
        except json.JSONDecodeError:
            q = {}
    # nem todo projeto guarda o questionario ao lado; o manifesto do pre-flight
    # tem o mesmo nome, produtos e aprovador, e esta sempre la
    man = {}
    if (wx / "wx-inputs.manifest.json").is_file():
        try:
            man = json.loads((wx / "wx-inputs.manifest.json").read_text(encoding="utf-8")).get("project", {})
        except json.JSONDecodeError:
            man = {}
    constraints = []
    if (wx / "constraints.json").is_file():
        constraints = json.loads((wx / "constraints.json").read_text(encoding="utf-8")).get("constraints", [])
    decisoes = ler_decisoes(raiz)
    alvo = cfg.get("target", {})
    projeto = q.get("projeto", {})
    contrato = {
        "gerado_em": date.today().isoformat(),
        "projeto": projeto.get("nome") or man.get("name", ""),
        "legado": projeto.get("produtos") or man.get("products", []),
        "destino": {
            "linguagem": alvo.get("language", ""),
            "frameworks": alvo.get("frameworks", []),
            "banco": alvo.get("database", ""),
            "implantacao": alvo.get("deployment", ""),
        },
        "fidelidade": cfg.get("fidelity", {}),
        "modo": cfg.get("mode", ""),
        "aprovador": projeto.get("aprovador") or man.get("human_approver", ""),
        "decisoes_vigentes": [d for d in decisoes if d["situacao"] == "vigente"],
        "decisoes_superadas": [d for d in decisoes if d["situacao"] == "superada"],
        "decisoes_indefinidas": [d for d in decisoes if d["situacao"] == "indefinida"],
        "constraints_ativas": [c for c in constraints if c.get("estado") == "ativa"],
        "constraints_revogadas": [c for c in constraints if c.get("estado") != "ativa"],
    }
    # o hash nao inclui a data: o que interessa e se o CONTEUDO mudou
    corpo = {k: v for k, v in contrato.items() if k != "gerado_em"}
    contrato["hash"] = hashlib.sha256(
        json.dumps(corpo, ensure_ascii=False, sort_keys=True).encode()).hexdigest()
    return contrato


def texto(c: dict) -> str:
    L = ["# Contrato ativo do projeto", "",
         f"Gerado por `contrato.py` em {c['gerado_em']} · hash `{c['hash'][:16]}…`. Não se edita à mão.", "",
         "**Isto é o que vale hoje.** O histórico continua inteiro nas fichas de decisão; "
         "o que foi superado está listado abaixo, com o motivo, para ninguém reabrir por engano.", "",
         "## Em vigor", "",
         f"- **Projeto**: {c['projeto'] or '—'}",
         f"- **Legado**: {', '.join(c['legado']) or '—'}",
         f"- **Destino**: {c['destino']['linguagem'] or '—'}"
         + (f" · {', '.join(c['destino']['frameworks'])}" if c["destino"]["frameworks"] else "")
         + (f" · banco {c['destino']['banco']}" if c["destino"]["banco"] else ""),
         f"- **Modo**: {c['modo'] or '—'}",
         f"- **Aprovador**: {c['aprovador'] or '—'}",
         f"- **Fidelidade**: comportamento {c['fidelidade'].get('business_behavior', '—')}, "
         f"dados {c['fidelidade'].get('data_behavior', '—')}, tela {c['fidelidade'].get('ui', '—')}"
         + (f", modernizações permitidas: {', '.join(c['fidelidade'].get('allowed_modernizations') or []) or 'nenhuma'}"),
         ""]
    if c["decisoes_vigentes"]:
        L += ["### Decisões vigentes", ""]
        L += [f"- **{d['id']}** — {d['titulo']}" + (f" · {d['decisao']}" if d["decisao"] else "")
              for d in c["decisoes_vigentes"]] + [""]
    if c["constraints_ativas"]:
        L += ["### Restrições ativas", ""]
        L += [f"- **{x['id']}** ({x['severidade']}) — {x['titulo']}" for x in c["constraints_ativas"]] + [""]
    if c["decisoes_superadas"] or c["constraints_revogadas"]:
        L += ["## Superado — não use", "",
              "Continua no histórico porque saber *por que* mudou é metade do valor.", ""]
        L += [f"- ~~{d['id']} — {d['titulo']}~~ ({d['status']}"
              + (f", por {d['superada_por']}" if d["superada_por"] else "") + ")"
              for d in c["decisoes_superadas"]]
        L += [f"- ~~{x['id']} — {x['titulo']}~~ ({x.get('motivo_da_revogacao', 'revogada')})"
              for x in c["constraints_revogadas"]] + [""]
    if c["decisoes_indefinidas"]:
        L += ["## Pendências — decidido não é o mesmo que registrado", "",
              "Estas fichas não trazem `Status` legível. **Elas não entram no contrato**: "
              "campo em branco não é aprovação.", ""]
        L += [f"- {d['id']} — {d['titulo']} (`{d['arquivo']}`)" for d in c["decisoes_indefinidas"]] + [""]
    return "\n".join(L) + "\n"


def gerar(args, raiz: Path) -> int:
    if not (raiz / ".wx-migration").is_dir():
        print("erro: rode dentro de um projeto com .wx-migration/", file=sys.stderr)
        return 2
    c = montar(raiz)
    (raiz / ".wx-migration" / "contrato-ativo.md").write_text(texto(c), encoding="utf-8")
    (raiz / ".wx-migration" / "contrato-ativo.json").write_text(
        json.dumps(c, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
    if args.json:
        print(json.dumps(c, ensure_ascii=False))
    else:
        print(f"contrato ativo em {c['hash'][:16]}…: "
              f"{len(c['decisoes_vigentes'])} decisões vigentes, "
              f"{len(c['constraints_ativas'])} restrições ativas, "
              f"{len(c['decisoes_superadas'])} superadas, "
              f"{len(c['decisoes_indefinidas'])} sem status")
    return 0


def conferir(args, raiz: Path) -> int:
    """Sai 1 quando o contrato de agora nao bate com o gravado.

    E o que uma sessao nova pergunta antes de confiar no que leu ontem.
    """
    gravado = raiz / ".wx-migration" / "contrato-ativo.json"
    if not gravado.is_file():
        print("contrato ainda não gerado: rode contrato.py gerar", file=sys.stderr)
        return 2
    antigo = json.loads(gravado.read_text(encoding="utf-8")).get("hash", "")
    novo = montar(raiz)["hash"]
    if args.json:
        print(json.dumps({"igual": antigo == novo, "gravado": antigo, "agora": novo}, ensure_ascii=False))
    elif antigo == novo:
        print(f"contrato inalterado ({novo[:16]}…)")
    else:
        print(f"contrato MUDOU: gravado {antigo[:16]}…, agora {novo[:16]}… — rode contrato.py gerar")
    return 0 if antigo == novo else 1


def main() -> int:
    p = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    p.add_argument("--project-root", default=".")
    p.add_argument("--json", action="store_true")
    sub = p.add_subparsers(dest="cmd", required=True)
    sub.add_parser("gerar", help="escreve o contrato ativo do projeto")
    sub.add_parser("conferir", help="sai 1 se o contrato mudou desde o gravado")
    args = p.parse_args()
    raiz = Path(args.project_root).resolve()
    return {"gerar": gerar, "conferir": conferir}[args.cmd](args, raiz)


try:
    import registro
except ImportError:
    sys.path.insert(0, str(Path(__file__).resolve().parent))
    import registro


if __name__ == "__main__":
    sys.exit(registro.envolver(__file__, main))
