#!/usr/bin/env python3
"""Gemeo da sprint: a sprint inteira num pacote, e o "e se" sobre ela.

Uma sprint fechada some. O relatorio fica, o zip fica, mas o ESTADO -- quais
restricoes estavam ativas, qual contrato valia, quais evidencias existiam,
quais decisoes foram capturadas, o que o grafo apontava como lacuna -- some
junto com a sessao. Seis meses depois, "por que aquela sprint passou?" nao se
responde.

O gemeo e uma fotografia com hash de tudo isso, tirada no fechamento. Duas
coisas se fazem com ela:

  auditar    ler a sprint como ela era, sem depender da memoria de ninguem
  e-se       aplicar uma restricao de HOJE ao estado DAQUELE dia e ver o que
             teria reprovado

O `e-se` e a parte util e a parte perigosa. Util: descobrir que a restricao que
voce acabou de escrever teria pego tres sprints atras, o que diz se ela vale.
Perigosa: parece previsao. Nao e. Ele reexecuta o VALIDADOR contra o estado
gravado; nao adivinha o que a equipe teria feito ao ver a reprovacao, e diz isso
na saida.

Uso:
  gemeo.py fotografar --sprint SP00012 [--nota "..."]
  gemeo.py listar
  gemeo.py auditar SP00012
  gemeo.py e-se SP00012 --constraint CONST-0007
"""
from __future__ import annotations

import argparse
import fnmatch
import hashlib
import json
import shlex
import subprocess
import sys
from datetime import datetime, timezone
from pathlib import Path

WX = ".wx-migration"


def pasta(raiz: Path) -> Path:
    return raiz / WX / "gemeos"


def ler_json(p: Path, padrao):
    if not p.is_file():
        return padrao
    try:
        return json.loads(p.read_text(encoding="utf-8"))
    except json.JSONDecodeError:
        return padrao


def sha256_texto(t: str) -> str:
    return hashlib.sha256(t.encode()).hexdigest()


def sha256_arquivo(p: Path) -> str:
    h = hashlib.sha256()
    with p.open("rb") as f:
        for b in iter(lambda: f.read(1 << 20), b""):
            h.update(b)
    return h.hexdigest()


def fotografar(args, raiz: Path) -> int:
    if not (raiz / WX).is_dir():
        print("erro: rode dentro de um projeto com .wx-migration/", file=sys.stderr)
        return 2
    wx = raiz / WX
    constraints = ler_json(wx / "constraints.json", {}).get("constraints", [])
    contrato = ler_json(wx / "contrato-ativo.json", {})
    evidencias = []
    if (wx / "evidencias").is_dir():
        for a in sorted((wx / "evidencias").glob("EVID-*.json")):
            evidencias.append(ler_json(a, {}))
    capturadas = []
    if (wx / "decisoes-capturadas").is_dir():
        for a in sorted((wx / "decisoes-capturadas").glob("*.json")):
            capturadas.append(ler_json(a, {}))
    # o grafo do dia: as lacunas como estavam, nao como estao
    lacunas = {}
    try:
        sys.path.insert(0, str(Path(__file__).resolve().parent))
        import grafo  # noqa: PLC0415
        lacunas = grafo.montar(raiz)["achados"]
    except Exception as e:  # noqa: BLE001
        lacunas = {"INDISPONÍVEL": f"o grafo não pôde ser lido: {e}"}
    # os arquivos do produto, com hash: e o que torna o "e se" possivel depois
    arquivos = {}
    for p in sorted(raiz.rglob("*")):
        if not p.is_file():
            continue
        rel = p.relative_to(raiz).as_posix()
        if any(x in rel.split("/") for x in (".git", "node_modules", "target", "__pycache__", WX)):
            continue
        arquivos[rel] = sha256_arquivo(p)

    foto = {
        "sprint": args.sprint,
        "tirada_em": datetime.now(timezone.utc).isoformat(timespec="seconds"),
        "nota": args.nota or "",
        "contrato": contrato,
        "constraints": constraints,
        "evidencias": evidencias,
        "decisoes_capturadas": capturadas,
        "lacunas_do_grafo": lacunas,
        "arquivos": arquivos,
        "medido": {"arquivos": len(arquivos), "restricoes_ativas": sum(1 for c in constraints if c.get("estado") == "ativa"),
                   "evidencias": len(evidencias), "decisoes_capturadas": len(capturadas)},
    }
    foto["hash"] = sha256_texto(json.dumps({k: v for k, v in foto.items() if k != "hash"},
                                           ensure_ascii=False, sort_keys=True))
    destino = pasta(raiz)
    destino.mkdir(parents=True, exist_ok=True)
    alvo = destino / f"{args.sprint}.json"
    alvo.write_text(json.dumps(foto, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
    if args.json:
        print(json.dumps({"escrito": str(alvo), "hash": foto["hash"], **foto["medido"]}, ensure_ascii=False))
    else:
        m = foto["medido"]
        print(f"{args.sprint} fotografada em {foto['hash'][:16]}…")
        print(f"  {m['arquivos']} arquivos · {m['restricoes_ativas']} restrições ativas · "
              f"{m['evidencias']} evidências · {m['decisoes_capturadas']} decisões capturadas")
    return 0


def carregar(raiz: Path, sprint: str) -> dict | None:
    p = pasta(raiz) / f"{sprint}.json"
    return ler_json(p, None) if p.is_file() else None


def listar(args, raiz: Path) -> int:
    destino = pasta(raiz)
    fotos = sorted(destino.glob("*.json")) if destino.is_dir() else []
    if args.json:
        print(json.dumps([f.stem for f in fotos], ensure_ascii=False))
        return 0
    if not fotos:
        print("nenhum gêmeo tirado")
        return 0
    for f in fotos:
        d = ler_json(f, {})
        m = d.get("medido", {})
        print(f"{d.get('sprint', f.stem):<12} {d.get('tirada_em', '')[:10]}  "
              f"{m.get('arquivos', '?')} arquivos · {m.get('evidencias', '?')} evidências  "
              f"{d.get('hash', '')[:12]}…")
    return 0


def auditar(args, raiz: Path) -> int:
    foto = carregar(raiz, args.sprint)
    if foto is None:
        print(f"erro: sem gêmeo de {args.sprint}", file=sys.stderr)
        return 2
    if args.json:
        print(json.dumps(foto, ensure_ascii=False))
        return 0
    m = foto["medido"]
    print(f"{foto['sprint']} · fotografada em {foto['tirada_em']} · hash {foto['hash'][:16]}…")
    if foto.get("nota"):
        print(f"nota: {foto['nota']}")
    print(f"\ncontrato daquele dia: {(foto.get('contrato') or {}).get('hash', '—')[:16]}…")
    print(f"restrições ativas: {m['restricoes_ativas']}")
    for c in foto["constraints"]:
        if c.get("estado") == "ativa":
            print(f"  {c['id']}  {c['severidade']:<11} {c['titulo']}")
    print(f"\nevidências: {m['evidencias']}")
    for e in foto["evidencias"]:
        print(f"  {e.get('id')}  {e.get('estado', ''):<14} {e.get('afirmacao', '')[:60]}")
        print(f"        não provava: {e.get('nao_prova', '')[:80]}")
    lac = foto.get("lacunas_do_grafo", {})
    total = sum(len(v) for v in lac.values() if isinstance(v, list))
    print(f"\nlacunas que o grafo apontava naquele dia: {total}")
    for k, v in lac.items():
        if isinstance(v, list) and v:
            print(f"  {len(v):>3}  {k}")
    return 0


def e_se(args, raiz: Path) -> int:
    """Aplica uma restricao de hoje ao estado daquele dia.

    O validador roda com o diretorio do PROJETO de hoje, mas os arquivos
    conferidos sao os da foto: por isso a saida diz, sempre, o que este
    exercicio nao prova.
    """
    foto = carregar(raiz, args.sprint)
    if foto is None:
        print(f"erro: sem gêmeo de {args.sprint}", file=sys.stderr)
        return 2
    hoje = ler_json(raiz / WX / "constraints.json", {}).get("constraints", [])
    alvo = next((c for c in hoje if c["id"] == args.constraint), None)
    if alvo is None:
        print(f"erro: {args.constraint} não existe no registro de hoje", file=sys.stderr)
        return 2
    ja_estava = any(c["id"] == alvo["id"] for c in foto["constraints"])
    alcance = [a for a in foto["arquivos"]
               if fnmatch.fnmatch(a, alvo.get("escopo") or "**")]
    resultado, motivo = "inconclusivo", "restrição sem validador automático (manual)"
    if alvo.get("validador"):
        try:
            sys.path.insert(0, str(Path(__file__).resolve().parent))
            import constraints as C  # noqa: PLC0415
            comando = C.expandir(alvo["validador"])
            faltando = C.programa_ausente(comando, raiz)
            if faltando:
                motivo = f"validador não existe: {faltando}"
            else:
                r = subprocess.run(shlex.split(comando), cwd=raiz, capture_output=True,
                                   text=True, timeout=args.timeout)
                achou = r.returncode == 0
                vale = (not achou) if alvo.get("inverter") else achou
                resultado = "aprovada" if vale else "violada"
                saida = (r.stdout + r.stderr).strip().splitlines()
                motivo = saida[-1][:200] if saida else f"código {r.returncode}"
        except (OSError, ValueError, subprocess.TimeoutExpired) as e:
            motivo = f"não deu para rodar: {e}"

    saida = {
        "sprint": args.sprint, "constraint": alvo["id"], "titulo": alvo.get("titulo", ""),
        "ja_estava_ativa_na_sprint": ja_estava,
        "arquivos_no_escopo_naquele_dia": len(alcance),
        "resultado": resultado, "motivo": motivo,
        "nao_prova": ("o validador roda contra o código de HOJE, não contra o de então; "
                      "e nada aqui diz o que a equipe teria feito ao ver a reprovação. "
                      "Isto responde 'a regra pega este caso?', não 'a sprint teria passado?'"),
    }
    if args.json:
        print(json.dumps(saida, ensure_ascii=False))
    else:
        print(f"e se {alvo['id']} valesse em {args.sprint}?")
        print(f"  {'já estava ativa' if ja_estava else 'NÃO estava ativa naquele dia'}")
        print(f"  {len(alcance)} arquivo(s) daquele dia dentro do escopo")
        print(f"  resultado: {resultado.upper()} — {motivo}")
        print(f"\n  não prova: {saida['nao_prova']}")
    return {"aprovada": 0, "violada": 1, "inconclusivo": 2}[resultado]


def main() -> int:
    p = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    p.add_argument("--project-root", default=".")
    p.add_argument("--json", action="store_true")
    sub = p.add_subparsers(dest="cmd", required=True)
    f = sub.add_parser("fotografar", help="tira o gêmeo da sprint")
    f.add_argument("--sprint", required=True)
    f.add_argument("--nota")
    sub.add_parser("listar", help="os gêmeos já tirados")
    a = sub.add_parser("auditar", help="lê a sprint como ela era")
    a.add_argument("sprint")
    e = sub.add_parser("e-se", help="aplica uma restrição de hoje ao estado daquele dia")
    e.add_argument("sprint")
    e.add_argument("--constraint", required=True)
    e.add_argument("--timeout", type=int, default=300)
    args = p.parse_args()
    raiz = Path(args.project_root).resolve()
    return {"fotografar": fotografar, "listar": listar, "auditar": auditar, "e-se": e_se}[args.cmd](args, raiz)


try:
    import registro
except ImportError:
    sys.path.insert(0, str(Path(__file__).resolve().parent))
    import registro


if __name__ == "__main__":
    sys.exit(registro.envolver(__file__, main))
