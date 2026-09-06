#!/usr/bin/env python3
"""Decisao reproduzivel: guarda COM QUE informacao ela foi tomada, e reconfere.

O `contrato.py` responde *o que vale hoje*. Este responde a pergunta que aparece
seis meses depois, quando troca o CTO do cliente: *com que informacao na mesa
isso foi decidido, e essa informacao ainda vale?*

Uma decisao capturada guarda, tudo medido na hora:
  o hash de cada fonte que estava na mesa (documento, PDF extraido, arquivo),
  o hash do contrato ativo, o conjunto de restricoes, o commit do codigo,
  as alternativas consideradas e a escolhida, quem decidiu e com que autoridade.

`reconferir` compara aquilo com o estado de HOJE e devolve um de tres:

  ESTAVEL       nenhuma fonte mudou; a decisao continua apoiada no que a apoiava
  BASE_MUDOU    alguma fonte, o contrato ou a restricao mudou -- a decisao pode
                continuar certa, mas ninguem sabe sem reexaminar, e agora esta escrito
  INCONCLUSIVO  nao deu para ler alguma fonte; nao se conclui nada

O que ele NAO faz, e a distincao importa: nao "reexecuta o raciocinio" nem diz
se a decisao continua correta. Ele diz se a BASE mudou. Reexecutar o julgamento
e trabalho de gente com o material na mao -- e o material e exatamente o que
este script preserva.

Uso:
  replay.py capturar --id DEC-0007 --titulo "..." --escolhida "..." \\
      --alternativa "..." --alternativa "..." --fonte arq --fonte arq --por "Nome"
  replay.py listar
  replay.py reconferir [DEC-0007]
"""
from __future__ import annotations

import argparse
import hashlib
import json
import subprocess
import sys
from datetime import datetime, timezone
from pathlib import Path

ESTAVEL, MUDOU, INCONCLUSIVO = "estavel", "base_mudou", "inconclusivo"
ROTULOS = {ESTAVEL: "ESTÁVEL", MUDOU: "BASE MUDOU", INCONCLUSIVO: "INCONCLUSIVO"}


def pasta(raiz: Path) -> Path:
    return raiz / ".wx-migration" / "decisoes-capturadas"


def sha256(p: Path) -> str:
    h = hashlib.sha256()
    with p.open("rb") as f:
        for b in iter(lambda: f.read(1 << 20), b""):
            h.update(b)
    return h.hexdigest()


def hash_do_contrato(raiz: Path) -> str:
    arq = raiz / ".wx-migration" / "contrato-ativo.json"
    if not arq.is_file():
        return ""
    try:
        return json.loads(arq.read_text(encoding="utf-8")).get("hash", "")
    except json.JSONDecodeError:
        return ""


def hash_das_restricoes(raiz: Path) -> str:
    arq = raiz / ".wx-migration" / "constraints.json"
    if not arq.is_file():
        return ""
    try:
        ativas = [c["id"] for c in json.loads(arq.read_text(encoding="utf-8")).get("constraints", [])
                  if c.get("estado") == "ativa"]
    except (json.JSONDecodeError, KeyError):
        return ""
    return hashlib.sha256(",".join(sorted(ativas)).encode()).hexdigest()


def commit(raiz: Path) -> str:
    try:
        r = subprocess.run(["git", "rev-parse", "HEAD"], cwd=raiz,
                           capture_output=True, text=True, timeout=15)
        return r.stdout.strip() if r.returncode == 0 else ""
    except (OSError, subprocess.TimeoutExpired):
        return ""


def capturar(args, raiz: Path) -> int:
    if not (raiz / ".wx-migration").is_dir():
        print("erro: rode dentro de um projeto com .wx-migration/", file=sys.stderr)
        return 2
    if not args.alternativa:
        # decisao sem alternativa registrada nao e decisao, e anotacao. E o
        # campo que mais falta quando alguem contesta a escolha seis meses depois
        print("erro: registre ao menos uma --alternativa considerada. Decisão sem "
              "alternativa não se defende: vira 'foi assim porque sim'.", file=sys.stderr)
        return 2
    fontes = []
    for f in args.fonte or []:
        p = Path(f)
        alvo = p if p.is_absolute() else raiz / p
        if not alvo.is_file():
            print(f"erro: fonte não encontrada: {f}", file=sys.stderr)
            return 2
        try:
            rel = alvo.resolve().relative_to(raiz.resolve()).as_posix()
        except ValueError:
            rel = alvo.as_posix()
        fontes.append({"arquivo": rel, "sha256": sha256(alvo), "bytes": alvo.stat().st_size})
    ficha = {
        "id": args.id,
        "titulo": args.titulo,
        "escolhida": args.escolhida,
        "alternativas": args.alternativa,
        "por_que": args.por_que or "",
        "decidida_por": args.por or "",
        "autoridade": args.autoridade,
        "capturada_em": datetime.now(timezone.utc).isoformat(timespec="seconds"),
        "base": {
            "fontes": fontes,
            "contrato_hash": hash_do_contrato(raiz),
            "restricoes_hash": hash_das_restricoes(raiz),
            "commit": commit(raiz),
        },
    }
    destino = pasta(raiz)
    destino.mkdir(parents=True, exist_ok=True)
    (destino / f"{args.id}.json").write_text(
        json.dumps(ficha, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
    if args.json:
        print(json.dumps(ficha, ensure_ascii=False))
    else:
        print(f"{args.id} capturada: {len(fontes)} fonte(s), {len(args.alternativa)} alternativa(s)")
        print(f"  escolhida: {args.escolhida}")
    return 0


def carregar(raiz: Path, ident: str | None) -> list[dict]:
    destino = pasta(raiz)
    if not destino.is_dir():
        return []
    fichas = []
    for a in sorted(destino.glob("*.json")):
        if ident and a.stem != ident:
            continue
        try:
            fichas.append(json.loads(a.read_text(encoding="utf-8")))
        except json.JSONDecodeError:
            continue
    return fichas


def situacao(f: dict, raiz: Path) -> tuple[str, list[str]]:
    motivos: list[str] = []
    inconclusivo = False
    for fo in f["base"]["fontes"]:
        p = raiz / fo["arquivo"]
        if not p.is_file():
            motivos.append(f"a fonte {fo['arquivo']} não existe mais")
            inconclusivo = True
        elif sha256(p) != fo["sha256"]:
            motivos.append(f"a fonte {fo['arquivo']} mudou depois da decisão")
    ch, rh, cm = hash_do_contrato(raiz), hash_das_restricoes(raiz), commit(raiz)
    if f["base"]["contrato_hash"] and ch and ch != f["base"]["contrato_hash"]:
        motivos.append("o contrato ativo mudou desde a decisão")
    if f["base"]["restricoes_hash"] and rh and rh != f["base"]["restricoes_hash"]:
        motivos.append("o conjunto de restrições ativas mudou")
    if f["base"]["commit"] and cm and cm != f["base"]["commit"]:
        motivos.append(f"o código andou ({f['base']['commit'][:8]} → {cm[:8]})")
    if inconclusivo:
        return INCONCLUSIVO, motivos
    return (MUDOU, motivos) if motivos else (ESTAVEL, [])


def reconferir(args, raiz: Path) -> int:
    fichas = carregar(raiz, args.id)
    if not fichas:
        print("nenhuma decisão capturada" if not args.id else f"{args.id} não foi capturada",
              file=sys.stderr)
        return 2
    saida, pior = [], ESTAVEL
    for f in fichas:
        est, motivos = situacao(f, raiz)
        if est == INCONCLUSIVO or (est == MUDOU and pior == ESTAVEL):
            pior = est
        saida.append({"id": f["id"], "titulo": f["titulo"], "situacao": est,
                      "escolhida": f["escolhida"], "motivos": motivos})
    if args.json:
        print(json.dumps({"decisoes": saida, "pior": pior}, ensure_ascii=False))
    else:
        for d in saida:
            print(f"{d['id']}  {ROTULOS[d['situacao']]:<12} {d['titulo']}")
            print(f"          escolhida: {d['escolhida']}")
            for m in d["motivos"]:
                print(f"          · {m}")
        if pior != ESTAVEL:
            print("\nBase mudou não quer dizer decisão errada: quer dizer que ninguém sabe "
                  "sem reexaminar — e agora está escrito, com o material do dia na mão.")
    return {ESTAVEL: 0, MUDOU: 1, INCONCLUSIVO: 2}[pior]


def listar(args, raiz: Path) -> int:
    fichas = carregar(raiz, None)
    if args.json:
        print(json.dumps(fichas, ensure_ascii=False))
        return 0
    if not fichas:
        print("nenhuma decisão capturada")
        return 0
    for f in fichas:
        print(f"{f['id']}  {f['titulo']}")
        print(f"   escolhida    {f['escolhida']}")
        print(f"   alternativas {', '.join(f['alternativas'])}")
        print(f"   base         {len(f['base']['fontes'])} fonte(s), commit {(f['base']['commit'] or '—')[:8]}")
        print(f"   decidiu      {f['decidida_por'] or '—'} ({f['autoridade']})")
    return 0


def main() -> int:
    p = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    p.add_argument("--project-root", default=".")
    p.add_argument("--json", action="store_true")
    sub = p.add_subparsers(dest="cmd", required=True)
    c = sub.add_parser("capturar", help="guarda a decisão com a base dela")
    c.add_argument("--id", required=True)
    c.add_argument("--titulo", required=True)
    c.add_argument("--escolhida", required=True)
    c.add_argument("--alternativa", action="append", help="pode repetir")
    c.add_argument("--fonte", action="append", help="arquivo que estava na mesa; pode repetir")
    c.add_argument("--por", help="quem decidiu")
    c.add_argument("--autoridade", default="humano", choices=["humano", "delegada", "automatica"])
    c.add_argument("--por-que", dest="por_que")
    sub.add_parser("listar", help="as decisões capturadas")
    r = sub.add_parser("reconferir", help="a base ainda é a mesma?")
    r.add_argument("id", nargs="?")
    args = p.parse_args()
    raiz = Path(args.project_root).resolve()
    return {"capturar": capturar, "listar": listar, "reconferir": reconferir}[args.cmd](args, raiz)


try:
    import registro
except ImportError:
    sys.path.insert(0, str(Path(__file__).resolve().parent))
    import registro


if __name__ == "__main__":
    sys.exit(registro.envolver(__file__, main))
