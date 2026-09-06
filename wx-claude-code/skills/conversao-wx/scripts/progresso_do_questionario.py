#!/usr/bin/env python3
"""Onde o questionario parou, e como voltar para la.

Sao 60 itens -- medidos com `listar_perguntas.py`, nao estimados; o levantamento
antigo dizia "mais de setenta". Ninguem responde isso numa sessao so, e ate aqui
nao havia como perguntar "onde eu parei".

O que este script NAO faz, de proposito: nao guarda uma segunda copia do que ja
foi respondido. Respondido se DERIVA do proprio `questionario.json`, comparado
com o modelo -- item que continua igual ao modelo esta pendente. Duas fontes
para o mesmo fato e como as duas discordam depois.

O unico estado proprio e a REABERTURA: `revisar F6` diz "este eu quero rever
mesmo estando preenchido", e isso mora em `.wx-migration/progresso.json`, fora
do questionario do cliente.

Uso:
  progresso_do_questionario.py [--project-root .] progresso
  progresso_do_questionario.py retomar          # o proximo item a responder
  progresso_do_questionario.py revisar F6       # reabre um item ja respondido
  progresso_do_questionario.py fechar F6        # desfaz a reabertura
"""
from __future__ import annotations

import argparse
import json
import sys
from pathlib import Path

RESPONDIDA, PENDENTE, REABERTA = "respondida", "pendente", "reaberta"
# O quarto estado nasceu de rodar isto no exemplo: F5 e F12 estao PREENCHIDOS,
# com os valores que o modelo ja traz. Chamar de "pendente" mente (o valor esta
# la) e chamar de "respondida" mente tambem (ninguem confirmou). Sao coisas
# diferentes e ganham nomes diferentes.
COMO_O_MODELO = "como_o_modelo"


def carregar(raiz: Path) -> tuple[dict, Path]:
    """O questionario do projeto, onde quer que ele esteja neste projeto."""
    for c in (raiz / ".wx-migration/questionario.json", raiz / "questionario.json"):
        if c.is_file():
            return json.loads(c.read_text(encoding="utf-8")), c
    raise SystemExit("nao achei questionario.json (nem em .wx-migration/); rode o questionario antes")


def descer(d: dict, caminho: str):
    no = d
    for parte in caminho.split("."):
        if not isinstance(no, dict) or parte not in no:
            return None
        no = no[parte]
    return no


def vazio(v) -> bool:
    if v is None or v == "" or v == [] or v == {}:
        return True
    if isinstance(v, dict):
        return all(vazio(x) for k, x in v.items() if k != "observacao")
    return False


def estado_dos_itens(raiz: Path) -> list[dict]:
    sys.path.insert(0, str(Path(__file__).resolve().parent))
    import listar_perguntas as lp

    q, _ = carregar(raiz)
    modelo = json.loads(lp.MODELO.read_text(encoding="utf-8"))
    reabertos = set(ler_progresso(raiz).get("reabertos", []))
    saida = []
    for p in lp.perguntas():
        atual, padrao = descer(q, p["caminho"]), descer(modelo, p["caminho"])
        if vazio(atual):
            e = PENDENTE
        elif atual == padrao:
            e = COMO_O_MODELO
        else:
            e = REABERTA if p["id"] in reabertos else RESPONDIDA
        saida.append({**p, "estado": e})
    return saida


def ler_progresso(raiz: Path) -> dict:
    arq = raiz / ".wx-migration/progresso.json"
    if not arq.is_file():
        return {}
    try:
        return json.loads(arq.read_text(encoding="utf-8"))
    except (OSError, ValueError):
        return {}


def gravar_progresso(raiz: Path, dados: dict) -> Path:
    pasta = raiz / ".wx-migration"
    pasta.mkdir(parents=True, exist_ok=True)
    arq = pasta / "progresso.json"
    arq.write_text(json.dumps(dados, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
    return arq


def progresso(args, raiz: Path) -> int:
    itens = estado_dos_itens(raiz)
    conta = {e: sum(1 for i in itens if i["estado"] == e)
             for e in (RESPONDIDA, PENDENTE, COMO_O_MODELO, REABERTA)}
    if args.json:
        print(json.dumps({"itens": itens, "contagem": conta}, ensure_ascii=False, indent=2))
        return 0
    print(f"Questionário: {conta[RESPONDIDA]} respondidas, {conta[PENDENTE]} pendentes, "
          f"{conta[COMO_O_MODELO]} como o modelo (preenchidas, mas ninguém confirmou), "
          f"{conta[REABERTA]} reabertas — de {len(itens)} itens\n")
    for i in itens:
        if i["estado"] != RESPONDIDA or args.tudo:
            marca = {RESPONDIDA: "ok  ", PENDENTE: "    ",
                     COMO_O_MODELO: "mod ", REABERTA: "rev "}[i["estado"]]
            print(f"  {marca} {i['id']:<6} {i['titulo'][:64]}")
    return 0


def retomar(args, raiz: Path) -> int:
    itens = estado_dos_itens(raiz)
    # A ordem da fila e a do questionario: o vazio antes do "como o modelo" so
    # trocaria a ordem de leitura por um criterio que o usuario nao pediu.
    fila = [i for i in itens if i["estado"] in (PENDENTE, COMO_O_MODELO, REABERTA)]
    if not fila:
        print("nenhum item pendente: o questionário está completo.")
        return 0
    p = fila[0]
    print(f"próximo item: {p['id']} — {p['titulo']}")
    porque = {REABERTA: "reaberto para revisão", PENDENTE: "ainda não respondido",
              COMO_O_MODELO: "preenchido com o valor do modelo; ninguém confirmou"}[p["estado"]]
    print(f"  {porque};"
          f" caminho no JSON: {p['caminho']}")
    print(f"  responda com: /wx-claude-code:pergunta {p['id']}")
    print(f"  faltam {len(fila)} de {len(itens)}.")
    return 0


def revisar(args, raiz: Path) -> int:
    ids = {i["id"] for i in estado_dos_itens(raiz)}
    if args.id not in ids:
        print(f"id desconhecido: {args.id}. Veja `listar_perguntas.py`.", file=sys.stderr)
        return 2
    d = ler_progresso(raiz)
    reabertos = [x for x in d.get("reabertos", []) if x != args.id]
    if args.cmd == "revisar":
        reabertos.append(args.id)
    d["reabertos"] = reabertos
    arq = gravar_progresso(raiz, d)
    print(f"{args.id} {'reaberto' if args.cmd == 'revisar' else 'fechado'}; {arq}")
    return 0


def main() -> int:
    p = argparse.ArgumentParser(description="onde o questionário parou")
    p.add_argument("--project-root", default=".")
    sub = p.add_subparsers(dest="cmd", required=True)
    g = sub.add_parser("progresso", help="a lista, com o que falta")
    g.add_argument("--json", action="store_true")
    g.add_argument("--tudo", action="store_true", help="mostrar também as respondidas")
    sub.add_parser("retomar", help="o próximo item a responder")
    for nome, ajuda in (("revisar", "reabrir um item respondido"), ("fechar", "desfazer a reabertura")):
        c = sub.add_parser(nome, help=ajuda)
        c.add_argument("id")
    args = p.parse_args()
    raiz = Path(args.project_root).resolve()
    return {"progresso": progresso, "retomar": retomar,
            "revisar": revisar, "fechar": revisar}[args.cmd](args, raiz)


try:
    import registro
except ImportError:
    sys.path.insert(0, str(Path(__file__).resolve().parent))
    import registro


if __name__ == "__main__":
    sys.exit(registro.envolver(__file__, main))
