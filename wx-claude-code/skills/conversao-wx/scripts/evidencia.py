#!/usr/bin/env python3
"""Livro de evidencias da conversao: o que foi provado, contra o que, e ate onde.

Por que existe: o plugin ja tinha prova espalhada -- golden master, pre-flight,
bateria, laudo -- e nenhum lugar onde ela virasse UM registro com escopo. Sem
isso, "os testes passaram" vira "o sistema esta correto", que e outra coisa.

Duas regras mandam aqui, e as duas nasceram de erro conhecido:

1. ESTADO NAO E BINARIO. `passou / falhou` esconde o caso mais comum de
   migracao: 7 de 10 casos do golden batem. Isso nao e PASSOU nem FALHOU, e
   PARCIAL, e quem le precisa saber disso sem abrir o relatorio.

     VERIFICADO   o que se afirmou foi conferido inteiro
     PARCIAL      parte foi conferida; o resto esta escrito
     NAO_VERIFICADO  ninguem conferiu ainda (o padrao honesto)
     FALHOU       foi conferido e nao bate

2. TODA EVIDENCIA DIZ O QUE **NAO** PROVA. "Nenhuma falha encontrada" nao e
   "esta correto"; "42 testes passaram" nao e "o sistema e seguro". O campo
   `nao_prova` e obrigatorio, e o script recusa gravar sem ele -- porque a frase
   que falta e exatamente a que o leitor completa sozinho, para o lado errado.

E evidencia ENVELHECE: ela aponta para um arquivo e guarda o SHA-256 dele. Se o
arquivo mudou depois, `conferir` marca VENCIDA. Prova de ontem sobre codigo de
hoje nao e prova; e a mesma regra do CRC de pagina do lado do PhxSql.

Uso:
  evidencia.py registrar --afirmacao "..." --metodo golden --assunto ARQ \\
      --estado parcial --prova "..." --nao-prova "..." [--medida 7/10]
  evidencia.py listar [--json]
  evidencia.py conferir [--json]     # reconfere os hashes; VENCIDA se mudou
  evidencia.py do-golden RELATORIO   # le o relatorio do golden.py e registra
"""
from __future__ import annotations

import argparse
import hashlib
import json
import re
import sys
from datetime import date, datetime, timezone
from pathlib import Path

ESTADOS = ("verificado", "parcial", "nao_verificado", "falhou")
ROTULOS = {
    "verificado": "VERIFICADO",
    "parcial": "PARCIAL",
    "nao_verificado": "NÃO VERIFICADO",
    "falhou": "FALHOU",
    "vencida": "VENCIDA",
}


def pasta(raiz: Path) -> Path:
    return raiz / ".wx-migration" / "evidencias"


def sha256(p: Path) -> str:
    h = hashlib.sha256()
    with p.open("rb") as f:
        for bloco in iter(lambda: f.read(1 << 20), b""):
            h.update(bloco)
    return h.hexdigest()


def proximo_id(destino: Path) -> str:
    usados = [int(m.group(1)) for a in destino.glob("EVID-*.json")
              if (m := re.match(r"EVID-(\d+)\.json$", a.name))]
    return f"EVID-{max(usados, default=0) + 1:04d}"


def carregar(raiz: Path) -> list[dict]:
    destino = pasta(raiz)
    if not destino.is_dir():
        return []
    fichas = []
    for arq in sorted(destino.glob("EVID-*.json")):
        try:
            fichas.append(json.loads(arq.read_text(encoding="utf-8")))
        except (OSError, json.JSONDecodeError) as e:
            fichas.append({"id": arq.stem, "erro": f"ficha ilegivel: {e}"})
    return fichas


def situacao(ficha: dict, raiz: Path) -> str:
    """Estado de hoje: uma evidencia verificada sobre arquivo que mudou vence."""
    assunto = ficha.get("assunto") or {}
    caminho = assunto.get("arquivo")
    if not caminho:
        return ficha.get("estado", "nao_verificado")
    p = raiz / caminho
    if not p.is_file():
        return "vencida"
    if assunto.get("sha256") and sha256(p) != assunto["sha256"]:
        return "vencida"
    return ficha.get("estado", "nao_verificado")


def registrar(args, raiz: Path) -> int:
    if not (raiz / ".wx-migration").is_dir():
        print("erro: rode dentro de um projeto com .wx-migration/", file=sys.stderr)
        return 2
    if args.estado not in ESTADOS:
        print(f"erro: estado precisa ser um de {', '.join(ESTADOS)}", file=sys.stderr)
        return 2
    if not args.nao_prova.strip():
        # a regra que originou este script: a frase que falta e a que o leitor
        # completa sozinho, sempre para o lado otimista
        print("erro: --nao-prova é obrigatório. Evidência que não declara o "
              "limite vira 'está correto' na cabeça de quem lê.", file=sys.stderr)
        return 2
    assunto = {}
    if args.assunto:
        p = Path(args.assunto)
        alvo = p if p.is_absolute() else raiz / p
        if not alvo.is_file():
            print(f"erro: assunto não encontrado: {alvo}", file=sys.stderr)
            return 2
        try:
            rel = alvo.resolve().relative_to(raiz.resolve()).as_posix()
        except ValueError:
            rel = alvo.as_posix()
        assunto = {"arquivo": rel, "sha256": sha256(alvo), "bytes": alvo.stat().st_size}
    destino = pasta(raiz)
    destino.mkdir(parents=True, exist_ok=True)
    ident = proximo_id(destino)
    ficha = {
        "id": ident,
        "afirmacao": args.afirmacao,
        "metodo": args.metodo,
        "estado": args.estado,
        "medida": args.medida or "",
        "prova": args.prova,
        "nao_prova": args.nao_prova,
        "assunto": assunto,
        "requisito": args.requisito or "",
        "constraint": args.constraint or "",
        "registrado_em": datetime.now(timezone.utc).isoformat(timespec="seconds"),
    }
    (destino / f"{ident}.json").write_text(
        json.dumps(ficha, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
    escrever_indice(raiz)
    if args.json:
        print(json.dumps(ficha, ensure_ascii=False))
    else:
        print(f"{ident} {ROTULOS[args.estado]}: {args.afirmacao}")
        print(f"  não prova: {args.nao_prova}")
    return 0


def escrever_indice(raiz: Path) -> None:
    """O indice legivel, que e o que um agente le sem abrir doze JSON."""
    fichas = carregar(raiz)
    linhas = [
        "# Evidências da conversão", "",
        f"Gerado por `evidencia.py` em {date.today().isoformat()}. Não se edita à mão.", "",
        "Cada linha diz **o que foi provado, por qual método e até onde vale**. "
        "`VENCIDA` significa que o arquivo mudou depois da prova: prova de ontem "
        "sobre código de hoje não é prova.", "",
        "| id | estado | afirmação | método | medida | não prova |",
        "| --- | --- | --- | --- | --- | --- |",
    ]
    for f in fichas:
        est = situacao(f, raiz)
        linhas.append(
            f"| {f.get('id', '')} | {ROTULOS.get(est, est)} | {f.get('afirmacao', '')} | "
            f"{f.get('metodo', '')} | {f.get('medida', '') or '—'} | {f.get('nao_prova', '')} |")
    if not fichas:
        linhas.append("| — | — | nenhuma evidência registrada ainda | — | — | — |")
    (raiz / ".wx-migration" / "evidencias.md").write_text("\n".join(linhas) + "\n", encoding="utf-8")


def listar(args, raiz: Path) -> int:
    fichas = carregar(raiz)
    if args.json:
        print(json.dumps([{**f, "situacao": situacao(f, raiz)} for f in fichas], ensure_ascii=False))
        return 0
    if not fichas:
        print("nenhuma evidência registrada")
        return 0
    for f in fichas:
        est = situacao(f, raiz)
        print(f"{f['id']}  {ROTULOS.get(est, est):<14} {f.get('afirmacao', '')}")
        print(f"          não prova: {f.get('nao_prova', '')}")
    return 0


def conferir(args, raiz: Path) -> int:
    """Reconfere os assuntos. Codigo 1 se alguma evidencia venceu ou falhou."""
    fichas = carregar(raiz)
    vencidas = [f for f in fichas if situacao(f, raiz) == "vencida"]
    falhas = [f for f in fichas if situacao(f, raiz) == "falhou"]
    escrever_indice(raiz)
    resumo = {
        "total": len(fichas),
        "vencidas": [f["id"] for f in vencidas],
        "falharam": [f["id"] for f in falhas],
        "por_estado": {e: sum(1 for f in fichas if situacao(f, raiz) == e)
                       for e in (*ESTADOS, "vencida")},
    }
    if args.json:
        print(json.dumps(resumo, ensure_ascii=False))
    else:
        print(f"{len(fichas)} evidências")
        for e in (*ESTADOS, "vencida"):
            n = resumo["por_estado"][e]
            if n:
                print(f"  {ROTULOS[e]:<14} {n}")
        for f in vencidas:
            print(f"  VENCIDA {f['id']}: {f['assunto'].get('arquivo')} mudou depois da prova")
    return 1 if (vencidas or falhas) else 0


def do_golden(args, raiz: Path) -> int:
    """Le o relatorio do golden.py e registra a evidencia com a medida real.

    O golden ja devolve n/total; o que faltava era transformar isso num estado
    honesto: 10/10 e VERIFICADO, 7/10 e PARCIAL, 0/10 e FALHOU.
    """
    arq = Path(args.relatorio)
    alvo = arq if arq.is_absolute() else raiz / arq
    try:
        rel = json.loads(alvo.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError) as e:
        print(f"erro: relatório do golden ilegível: {e}", file=sys.stderr)
        return 2
    # os nomes sao os que o golden.py grava de verdade (total/passaram/passou),
    # conferidos no arquivo dele -- adivinhar nome de campo aqui daria um
    # "0 de 0 casos" silencioso, que e o pior resultado possivel num livro de provas
    casos = rel.get("casos") or []
    total = rel.get("total", len(casos))
    iguais = rel.get("passaram", sum(1 for c in casos if c.get("passou")))
    if not total:
        print("erro: relatório sem casos; não há o que registrar", file=sys.stderr)
        return 2
    estado = "verificado" if iguais == total else ("falhou" if iguais == 0 else "parcial")
    divergentes = [c.get("id", "?") for c in casos if not c.get("passou")]
    ns = argparse.Namespace(
        afirmacao=args.afirmacao or "o sistema novo reproduz o legado nos casos do golden master",
        metodo="golden-master",
        estado=estado,
        medida=f"{iguais}/{total}",
        assunto=str(alvo),
        prova=f"{iguais} de {total} casos capturados do legado batem dentro da tolerância",
        nao_prova=(f"nada sobre os {total - iguais} casos divergentes ({', '.join(divergentes[:6])})"
                   if divergentes else
                   f"nada sobre entradas fora dos {total} casos capturados, nem sobre desempenho, segurança ou tela"),
        requisito=args.requisito or "",
        constraint="",
        json=args.json,
    )
    return registrar(ns, raiz)


def main() -> int:
    p = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    p.add_argument("--project-root", default=".")
    p.add_argument("--json", action="store_true")
    sub = p.add_subparsers(dest="cmd", required=True)

    r = sub.add_parser("registrar", help="registra uma evidência")
    r.add_argument("--afirmacao", required=True, help="o que se afirma ter provado")
    r.add_argument("--metodo", required=True, help="teste, golden-master, pre-flight, revisão humana…")
    r.add_argument("--estado", required=True, choices=ESTADOS)
    r.add_argument("--prova", default="", help="o que exatamente foi conferido")
    r.add_argument("--nao-prova", required=True, dest="nao_prova", help="o limite: o que isto NÃO prova")
    r.add_argument("--assunto", help="arquivo provado; o SHA-256 dele fica guardado")
    r.add_argument("--medida", help="número medido, ex.: 7/10")
    r.add_argument("--requisito")
    r.add_argument("--constraint")

    sub.add_parser("listar", help="lista as evidências")
    sub.add_parser("conferir", help="reconfere hashes; sai 1 se alguma venceu ou falhou")

    g = sub.add_parser("do-golden", help="registra evidência a partir de um relatório do golden.py")
    g.add_argument("relatorio")
    g.add_argument("--afirmacao")
    g.add_argument("--requisito")

    args = p.parse_args()
    raiz = Path(args.project_root).resolve()
    return {"registrar": registrar, "listar": listar, "conferir": conferir, "do-golden": do_golden}[args.cmd](args, raiz)


try:
    import registro
except ImportError:
    sys.path.insert(0, str(Path(__file__).resolve().parent))
    import registro


if __name__ == "__main__":
    sys.exit(registro.envolver(__file__, main))
