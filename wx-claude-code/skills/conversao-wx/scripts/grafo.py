#!/usr/bin/env python3
"""Grafo de rastreabilidade: liga requisito, decisao, codigo, teste e evidencia.

A matriz `traceability.csv` ja tinha as 22 colunas certas, mas so respondia
lendo tudo. O grafo faz as perguntas que ninguem responde a mao num projeto com
duzentas regras -- e que sao sempre as mesmas quatro:

  1. codigo sem requisito       arquivo alterado que nenhuma linha reivindica
  2. requisito sem teste        regra convertida que ninguem provou
  3. teste sem evidencia        teste que existe mas nao virou prova registrada
  4. prova vencida              evidencia sobre arquivo que mudou depois dela

E mais tres que o proprio projeto ja pediu pelo caminho: decisao citada que nao
existe, restricao cujo escopo nao alcanca arquivo nenhum, e regra cuja ORIGEM
no legado mudou depois de convertida (o `source_sha256` da linha nao bate mais).

O que este script NAO faz, de proposito: nao inventa aresta. Se a coluna esta
vazia, a ligacao nao existe -- e a resposta e a pergunta 1, 2 ou 3, nao um
palpite. Grafo que completa lacuna sozinho e pior que planilha, porque parece
completo.

Uso:
  grafo.py conferir [--json]        as sete perguntas, de uma vez
  grafo.py de BR-001 [--json]       tudo que um no alcanca
  grafo.py mermaid [BR-001]         o desenho, para colar num documento
"""
from __future__ import annotations

import argparse
import csv
import fnmatch
import hashlib
import json
import re
import sys
from pathlib import Path

# Extensoes que contam como "codigo do destino" na pergunta 1. Documento,
# configuracao e dado nao entram: eles nao precisam de requisito.
CODIGO = {".rs", ".py", ".ts", ".tsx", ".js", ".jsx", ".java", ".cs", ".go",
          ".rb", ".php", ".kt", ".swift", ".c", ".h", ".cpp", ".hpp", ".sql"}
IGNORAR = {".git", ".wx-migration", "node_modules", "target", "dist", "build",
           "__pycache__", ".venv", "venv", "inputs", "artefatos", "vendor"}


def linhas_da_matriz(raiz: Path) -> tuple[list[dict], str]:
    arq = raiz / ".wx-migration" / "traceability.csv"
    if not arq.is_file():
        return [], "sem .wx-migration/traceability.csv"
    with arq.open(encoding="utf-8", newline="") as f:
        return [dict(r) for r in csv.DictReader(f)], ""


def evidencias(raiz: Path) -> list[dict]:
    pasta = raiz / ".wx-migration" / "evidencias"
    if not pasta.is_dir():
        return []
    fichas = []
    for a in sorted(pasta.glob("EVID-*.json")):
        try:
            fichas.append(json.loads(a.read_text(encoding="utf-8")))
        except (OSError, json.JSONDecodeError):
            continue
    return fichas


def constraints(raiz: Path) -> list[dict]:
    arq = raiz / ".wx-migration" / "constraints.json"
    if not arq.is_file():
        return []
    try:
        return json.loads(arq.read_text(encoding="utf-8")).get("constraints", [])
    except json.JSONDecodeError:
        return []


def decisoes(raiz: Path) -> set[str]:
    achadas = set()
    for pasta in (raiz / ".wx-migration" / "decisoes", raiz / ".wx-migration", raiz / "docs" / "decisoes"):
        if pasta.is_dir():
            achadas.update(a.stem for a in pasta.glob("DEC-*.md"))
    return achadas


def gerados_pelo_plugin(raiz: Path) -> set[str]:
    """O esqueleto que o proprio questionario escreveu nao e codigo convertido.

    Sem isto, o grafo acusava `database/migrations/0001_base.sql` de nao ter
    requisito -- verdade literal e ruido puro, porque ninguem pediu aquele
    arquivo: o plugin o gerou. Ruido demais mata o sinal das outras seis
    perguntas, que sao as que importam.
    """
    indice = raiz / "INDEX_FILES.md"
    if not indice.is_file():
        return set()
    # o indice lista arquivo (`Dockerfile`) e tambem PASTA (`database/`); a
    # segunda forma cobre o que esta dentro dela, senao o esqueleto inteiro
    # voltaria como lacuna, um arquivo por vez
    return set(re.findall(r"^\| `([^`]+)`", indice.read_text(encoding="utf-8"), re.M))


def foi_gerado(arq: str, gerados: set[str]) -> bool:
    return arq in gerados or any(g.endswith("/") and arq.startswith(g) for g in gerados)


# Declaracao de modulo nao carrega regra: `pub mod x;` nao e requisito nenhum.
# O piloto vertical mostrou isso na pratica -- lib.rs com UMA linha aparecia como
# "codigo sem requisito", ao lado de arquivos que carregam logica de verdade e
# merecem mesmo a cobranca. Misturar os dois gasta a atencao de quem le no item
# errado. O criterio e MEDIDO, nao e lista de nomes: o arquivo so escapa se
# TODA linha util dele for declaracao, importacao ou comentario.
DECLARACAO = ("mod ", "pub mod ", "use ", "pub use ", "pub(crate) mod ",
              "extern crate ", "pub(crate) use ")


def so_declara_modulo(p: Path) -> bool:
    try:
        linhas = p.read_text(encoding="utf-8", errors="replace").splitlines()
    except OSError:
        return False
    uteis = [l.strip() for l in linhas
             if l.strip() and not l.strip().startswith(("//", "#", "/*", "*", "*/"))]
    if not uteis:
        return True
    return all(l.startswith(DECLARACAO) for l in uteis)


def arquivos_de_codigo(raiz: Path) -> list[str]:
    achados = []
    for p in raiz.rglob("*"):
        if not p.is_file() or p.suffix.lower() not in CODIGO:
            continue
        if any(parte in IGNORAR for parte in p.relative_to(raiz).parts):
            continue
        achados.append(p.relative_to(raiz).as_posix())
    return sorted(achados)


def sha256(p: Path) -> str:
    h = hashlib.sha256()
    with p.open("rb") as f:
        for b in iter(lambda: f.read(1 << 20), b""):
            h.update(b)
    return h.hexdigest()


def montar(raiz: Path) -> dict:
    matriz, aviso = linhas_da_matriz(raiz)
    evids = evidencias(raiz)
    consts = constraints(raiz)
    decs = decisoes(raiz)
    codigo = arquivos_de_codigo(raiz)

    alvos = {l.get("target_file", "").strip() for l in matriz if l.get("target_file", "").strip()}
    # arquivo de teste JA esta ligado pela coluna test_file: cobrar requisito
    # dele seria cobrar requisito da prova do requisito
    alvos |= {l.get("test_file", "").strip() for l in matriz if l.get("test_file", "").strip()}
    do_plugin = gerados_pelo_plugin(raiz)
    # a evidencia cita o arquivo provado e, quando ha, o id do teste
    provados = {(e.get("assunto") or {}).get("arquivo") for e in evids}
    testes_provados = {e.get("requisito", "") for e in evids if e.get("requisito")}
    for e in evids:
        m = re.findall(r"TST-[A-Za-z0-9._-]+", json.dumps(e, ensure_ascii=False))
        testes_provados.update(m)

    achados: dict[str, list] = {
        "codigo_sem_requisito": [],
        "requisito_sem_teste": [],
        "teste_sem_evidencia": [],
        "prova_vencida": [],
        "decisao_citada_que_nao_existe": [],
        "restricao_sem_alcance": [],
        "origem_mudou_depois_de_convertida": [],
    }

    for arq in codigo:
        ehteste = any(parte in ("tests", "test", "testes", "spec") for parte in Path(arq).parts)
        if arq not in alvos and not foi_gerado(arq, do_plugin) and not ehteste \
                and not so_declara_modulo(raiz / arq):
            achados["codigo_sem_requisito"].append(arq)

    for l in matriz:
        tid = l.get("trace_id", "").strip()
        if not tid:
            continue
        if not l.get("test_id", "").strip():
            achados["requisito_sem_teste"].append(
                {"trace_id": tid, "regra": l.get("rule_summary", "")[:80],
                 "arquivo": l.get("target_file", ""), "status": l.get("status", "")})
        else:
            tst = l["test_id"].strip()
            # a evidencia liga ao teste de dois jeitos: citando o id (o forte,
            # com `evidencia registrar --requisito`) ou sendo sobre o ARQUIVO
            # que a linha implementa. O segundo e o caso comum e vale: a prova
            # e sobre o alvo. O limite dela continua escrito dentro da propria
            # evidencia, que e onde esse limite deve morar.
            provado = (tst in testes_provados
                       or l.get("target_file", "").strip() in provados
                       or bool(l.get("test_result_ref", "").strip()))
            if not provado:
                achados["teste_sem_evidencia"].append(
                    {"trace_id": tid, "test_id": tst, "arquivo": l.get("target_file", "")})
        dec = l.get("decision_id", "").strip()
        if dec and dec not in decs:
            achados["decisao_citada_que_nao_existe"].append({"trace_id": tid, "decision_id": dec})
        # a origem no legado mudou depois de a regra ter sido convertida?
        origem, sha = l.get("source_artifact", "").strip(), l.get("source_sha256", "").strip()
        if origem and sha:
            p = raiz / origem
            if p.is_file() and sha256(p) != sha:
                achados["origem_mudou_depois_de_convertida"].append(
                    {"trace_id": tid, "origem": origem,
                     "detalhe": "o SHA-256 da linha não bate com o arquivo de origem de hoje"})

    for e in evids:
        assunto = (e.get("assunto") or {}).get("arquivo")
        sha = (e.get("assunto") or {}).get("sha256")
        if assunto and sha:
            p = raiz / assunto
            if not p.is_file() or sha256(p) != sha:
                achados["prova_vencida"].append(
                    {"evidencia": e.get("id"), "arquivo": assunto,
                     "afirmacao": e.get("afirmacao", "")[:80]})

    for c in consts:
        if c.get("estado") != "ativa":
            continue
        escopo = c.get("escopo") or "**"
        if escopo in ("**", "*"):
            continue
        if not any(fnmatch.fnmatch(a, escopo) for a in codigo):
            achados["restricao_sem_alcance"].append(
                {"id": c["id"], "escopo": escopo, "titulo": c.get("titulo", "")[:70]})

    return {
        "aviso": aviso,
        "medido": {"linhas_da_matriz": len(matriz), "arquivos_de_codigo": len(codigo),
                   "evidencias": len(evids), "restricoes": len(consts), "decisoes": len(decs)},
        "achados": achados,
        "provados": sorted(x for x in provados if x),
    }


ROTULOS = {
    "codigo_sem_requisito": "código sem requisito (nenhuma linha da matriz reivindica o arquivo)",
    "requisito_sem_teste": "requisito sem teste (regra convertida que ninguém provou)",
    "teste_sem_evidencia": "teste sem evidência (existe o teste, falta a prova registrada)",
    "prova_vencida": "prova vencida (o arquivo mudou depois da evidência)",
    "decisao_citada_que_nao_existe": "decisão citada que não existe",
    "restricao_sem_alcance": "restrição cujo escopo não alcança arquivo nenhum",
    "origem_mudou_depois_de_convertida": "a origem no legado mudou depois da conversão",
}


def conferir(args, raiz: Path) -> int:
    g = montar(raiz)
    if args.json:
        print(json.dumps(g, ensure_ascii=False))
        return 1 if any(g["achados"].values()) else 0
    m = g["medido"]
    print(f"{m['linhas_da_matriz']} linhas na matriz · {m['arquivos_de_codigo']} arquivos de código · "
          f"{m['evidencias']} evidências · {m['restricoes']} restrições · {m['decisoes']} decisões")
    if g["aviso"]:
        print(f"aviso: {g['aviso']}")
    print()
    total = 0
    for chave, rotulo in ROTULOS.items():
        itens = g["achados"][chave]
        total += len(itens)
        marca = "ok  " if not itens else "  ⚑ "
        print(f"{marca}{len(itens):>4}  {rotulo}")
        for i in itens[:args.n]:
            print(f"          {i if isinstance(i, str) else json.dumps(i, ensure_ascii=False)}")
        if len(itens) > args.n:
            print(f"          … mais {len(itens) - args.n}")
    print(f"\n{total} lacuna(s). Lacuna não é defeito: é o que ainda não foi ligado — e agora está escrito.")
    return 1 if total else 0


def de(args, raiz: Path) -> int:
    """Tudo que um no alcanca, atravessando a matriz e as evidencias."""
    alvo = args.no.strip()
    matriz, _ = linhas_da_matriz(raiz)
    evids = evidencias(raiz)
    linhas = [l for l in matriz if alvo in (l.get("trace_id", ""), l.get("test_id", ""),
                                            l.get("decision_id", ""), l.get("target_file", ""),
                                            l.get("legacy_symbol", ""))]
    provas = [e for e in evids
              if alvo in json.dumps(e, ensure_ascii=False)
              or (e.get("assunto") or {}).get("arquivo") in {l.get("target_file") for l in linhas}]
    saida = {"no": alvo, "linhas": linhas, "evidencias": [e["id"] for e in provas]}
    if args.json:
        print(json.dumps(saida, ensure_ascii=False))
        return 0 if linhas or provas else 1
    if not linhas and not provas:
        print(f"{alvo}: nada na matriz nem nas evidências")
        return 1
    for l in linhas:
        print(f"{l.get('trace_id', '')}  {l.get('kind', '')}  {l.get('rule_summary', '')[:70]}")
        print(f"   origem   {l.get('source_artifact', '')} {l.get('source_locator', '')} · {l.get('legacy_symbol', '')}")
        print(f"   decisão  {l.get('decision_id') or '—'}")
        print(f"   destino  {l.get('target_file', '')} · {l.get('target_symbol', '')}")
        print(f"   teste    {l.get('test_id') or '—'} {l.get('test_file', '')}")
        print(f"   estado   {l.get('status', '')} · confiança {l.get('confidence', '')} · aprovou {l.get('approved_by') or '—'}")
    for e in provas:
        print(f"evidência {e['id']}  {e.get('estado', '')}  {e.get('afirmacao', '')[:70]}")
        print(f"   não prova: {e.get('nao_prova', '')[:100]}")
    return 0


def mermaid(args, raiz: Path) -> int:
    matriz, _ = linhas_da_matriz(raiz)
    if args.no:
        matriz = [l for l in matriz if l.get("trace_id") == args.no]
    evids = {(e.get("assunto") or {}).get("arquivo"): e for e in evidencias(raiz)}
    L = ["graph LR"]
    vistos = set()

    def no(ident: str, rotulo: str, forma: str = "[]") -> str:
        chave = re.sub(r"[^A-Za-z0-9_]", "_", ident)
        if chave not in vistos:
            vistos.add(chave)
            a, b = forma[0], forma[1]
            L.append(f'  {chave}{a}"{rotulo}"{b}')
        return chave

    for l in matriz[:args.n]:
        req = no(l["trace_id"], l["trace_id"])
        if l.get("legacy_symbol"):
            L.append(f'  {no("LEG" + l["legacy_symbol"], l["legacy_symbol"], "()")} -->|origem| {req}')
        if l.get("decision_id"):
            L.append(f'  {req} -->|decidido em| {no(l["decision_id"], l["decision_id"], "{}")}')
        if l.get("target_file"):
            alvo = no("F" + l["target_file"], l["target_file"])
            L.append(f"  {req} -->|implementado em| {alvo}")
            if l.get("test_id"):
                t = no(l["test_id"], l["test_id"], "()")
                L.append(f"  {alvo} -->|verificado por| {t}")
                e = evids.get(l["target_file"])
                if e:
                    L.append(f'  {t} -->|comprovado por| {no(e["id"], e["id"] + " " + e.get("estado", ""), "[]")}')
    print("\n".join(L))
    return 0


def main() -> int:
    # `grafo.py conferir | head` fecha a saida no meio; sem isto o Python
    # imprime um traceback de BrokenPipe que parece defeito do script
    try:
        import signal
        signal.signal(signal.SIGPIPE, signal.SIG_DFL)
    except (ImportError, AttributeError, ValueError):
        pass
    p = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    p.add_argument("--project-root", default=".")
    p.add_argument("--json", action="store_true")
    sub = p.add_subparsers(dest="cmd", required=True)
    c = sub.add_parser("conferir", help="as sete perguntas do grafo")
    c.add_argument("-n", type=int, default=5, help="quantos exemplos mostrar por lacuna")
    d = sub.add_parser("de", help="tudo que um nó alcança")
    d.add_argument("no")
    m = sub.add_parser("mermaid", help="o desenho, para colar num documento")
    m.add_argument("no", nargs="?")
    m.add_argument("-n", type=int, default=25)
    args = p.parse_args()
    raiz = Path(args.project_root).resolve()
    return {"conferir": conferir, "de": de, "mermaid": mermaid}[args.cmd](args, raiz)


try:
    import registro
except ImportError:
    sys.path.insert(0, str(Path(__file__).resolve().parent))
    import registro


if __name__ == "__main__":
    sys.exit(registro.envolver(__file__, main))
