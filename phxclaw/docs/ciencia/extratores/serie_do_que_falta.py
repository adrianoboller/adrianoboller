#!/usr/bin/env python3
"""Extrator do estudo L-001: a serie da «% que falta das sprints» do PhxClaw, commit a commit.

Grao das tabelas fato (cada linha diz o commit de onde veio e a data do commit):
  fato_sprints       um commit que mudou docs/absorcao/SPRINTS.md
  fato_sprint_item   um commit x uma sprint da tabela «Visao geral» (estado naquele commit)
  fato_absorcao      um commit que mudou docs/absorcao/absorcao.json x uma fonte
  dim_rodada         um arquivo de docs/sprints/ (registro de rodada), com a data do nome
  dim_dia            um dia UTC com commits em phxclaw/

Motor unico: o estado de cada sprint sai de tools/dossie/numeros.py::sprints(), o MESMO leitor
que da ao dossie o «falta X% das sprints». O texto historico vem de `git show <commit>:<arq>`
e e entregue ao leitor por uma raiz temporaria; a data passa a ser a do commit (nunca o relogio).

Rodar duas vezes sem mudanca de fonte nao muda nenhum numero: nada aqui le o relogio, a ordem
e a do `git log --reverse` e o .sqlite e recriado do zero.

Uso:  python3 docs/ciencia/extratores/serie_do_que_falta.py [--db CAMINHO]
      (padrao: phxclaw/target/ciencia/serie_do_que_falta.sqlite, fora do git)
"""
from __future__ import annotations

import argparse
import json
import re
import sqlite3
import subprocess
import sys
import tempfile
from pathlib import Path

PHX = Path(__file__).resolve().parents[3]          # .../phxclaw
REPO = PHX.parent
sys.path.insert(0, str(PHX / "tools/dossie"))
import numeros as N  # noqa: E402  (o leitor do dossie; um motor so)

ARQ_SPRINTS = "phxclaw/docs/absorcao/SPRINTS.md"
ARQ_ABSORCAO = "phxclaw/docs/absorcao/absorcao.json"


def git(*args: str) -> str:
    r = subprocess.run(["git", *args], cwd=REPO, capture_output=True, text=True)
    if r.returncode != 0:
        raise SystemExit(f"PARADA: git {' '.join(args)}: {r.stderr.strip()}")
    return r.stdout


def commits_de(arq: str) -> list[tuple[str, str, str]]:
    """(hash, data ISO do commit, assunto), do mais velho ao mais novo."""
    saida = git("log", "--reverse", "--format=%H%x1f%cI%x1f%s", "--", arq)
    return [tuple(l.split("\x1f", 2)) for l in saida.splitlines() if l.strip()]


def ler_sprints_em(commit: str) -> dict:
    """Aplica numeros.sprints() ao texto do SPRINTS.md naquele commit."""
    texto = git("show", f"{commit}:{ARQ_SPRINTS}")
    with tempfile.TemporaryDirectory() as tmp:
        alvo = Path(tmp) / "docs/absorcao/SPRINTS.md"
        alvo.parent.mkdir(parents=True)
        alvo.write_text(texto, encoding="utf-8")
        raiz, data_de = N.RAIZ, N.data_de
        N.RAIZ = Path(tmp)
        N.data_de = lambda p: {"quando": None, "texto": "", "como": f"commit {commit[:8]}"}
        try:
            return N.sprints()
        finally:
            N.RAIZ, N.data_de = raiz, data_de


ESQUEMA = """
CREATE TABLE dim_dia (dia TEXT PRIMARY KEY, commits_phxclaw INTEGER);
CREATE TABLE dim_rodada (arquivo TEXT PRIMARY KEY, sprint TEXT, data_iso TEXT, dia TEXT);
CREATE TABLE fato_sprints (
  seq INTEGER PRIMARY KEY, commit_h TEXT, data_iso TEXT, dia TEXT, rodada_dia INTEGER,
  assunto TEXT, total INTEGER, concluidas INTEGER, em_execucao INTEGER, planejadas INTEGER,
  bloqueadas INTEGER, falta INTEGER, falta_pct REAL,
  entrou INTEGER, saiu INTEGER, reabriu INTEGER, removido INTEGER, ids_entrou TEXT,
  ids_saiu TEXT, fonte TEXT, erro TEXT);
CREATE TABLE fato_sprint_item (commit_h TEXT, data_iso TEXT, sprint TEXT, estado TEXT,
  PRIMARY KEY (commit_h, sprint));
CREATE TABLE fato_absorcao (commit_h TEXT, data_iso TEXT, dia TEXT, fonte TEXT, total INTEGER,
  no_agente INTEGER, parcial INTEGER, nao INTEGER, lido_em TEXT, PRIMARY KEY (commit_h, fonte));
"""


def main() -> None:
    ap = argparse.ArgumentParser()
    ap.add_argument("--db", default=str(PHX / "target/ciencia/serie_do_que_falta.sqlite"))
    db = Path(ap.parse_args().db)
    db.parent.mkdir(parents=True, exist_ok=True)
    if db.exists():
        db.unlink()
    con = sqlite3.connect(db)
    con.executescript(ESQUEMA)

    # dim_dia: commits em phxclaw/ por dia UTC
    dias: dict[str, int] = {}
    for iso in git("log", "--format=%cI", "--", "phxclaw/").split():
        d = N.datetime.fromisoformat(iso).astimezone(N.timezone.utc).strftime("%Y-%m-%d")
        dias[d] = dias.get(d, 0) + 1
    con.executemany("INSERT INTO dim_dia VALUES (?,?)", sorted(dias.items()))

    # dim_rodada: registros de rodada em docs/sprints/ (a data vem do nome do arquivo)
    for p in sorted((PHX / "docs/sprints").glob("Sessao_*_Sprint_*_*.md")):
        m = re.search(r"Sprint_(SP\d+)_(\d{14})", p.name)
        if m:
            t = m.group(2)
            iso = f"{t[:4]}-{t[4:6]}-{t[6:8]}T{t[8:10]}:{t[10:12]}:{t[12:14]}+00:00"
            con.execute("INSERT INTO dim_rodada VALUES (?,?,?,?)", (p.name, m.group(1), iso, iso[:10]))

    # fato_sprints e fato_sprint_item
    antes: dict[str, str] = {}
    ordem_dias: list[str] = []
    for seq, (h, iso, assunto) in enumerate(commits_de(ARQ_SPRINTS), 1):
        dia = N.datetime.fromisoformat(iso).astimezone(N.timezone.utc).strftime("%Y-%m-%d")
        if dia not in ordem_dias:
            ordem_dias.append(dia)
        try:
            sp = ler_sprints_em(h)
        except SystemExit as e:  # formato que o leitor de hoje nao le: registra, nao inventa
            con.execute("INSERT INTO fato_sprints (seq, commit_h, data_iso, dia, rodada_dia, assunto, "
                        "fonte, erro) VALUES (?,?,?,?,?,?,?,?)",
                        (seq, h[:8], iso, dia, len(ordem_dias), assunto, ARQ_SPRINTS, str(e)))
            continue
        agora = {i["id"]: i["estado"] for i in sp["itens"]}
        c = sp["contagem"]
        novos = sorted(set(agora) - set(antes))
        fechados = sorted(k for k, v in agora.items() if v == "CONCLUÍDA" and antes.get(k) not in (None, "CONCLUÍDA"))
        # sprint que ja nasce CONCLUIDA entra e sai na mesma rodada: conta nos dois fluxos
        nascidas_feitas = [k for k in novos if agora[k] == "CONCLUÍDA"]
        reabertos = [k for k, v in agora.items() if antes.get(k) == "CONCLUÍDA" and v != "CONCLUÍDA"]
        removidos = [k for k in antes if k not in agora]
        falta = sp["total"] - c["CONCLUÍDA"]
        con.execute("INSERT INTO fato_sprints VALUES (?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?)",
                    (seq, h[:8], iso, dia, len(ordem_dias), assunto, sp["total"], c["CONCLUÍDA"],
                     c["EM EXECUÇÃO"], c["PLANEJADA"], c["BLOQUEADA"], falta,
                     round(100 * falta / sp["total"], 4), len(novos),
                     len(fechados) + len(nascidas_feitas), len(reabertos), len(removidos),
                     ",".join(novos), ",".join(fechados + nascidas_feitas), ARQ_SPRINTS, None))
        con.executemany("INSERT INTO fato_sprint_item VALUES (?,?,?,?)",
                        [(h[:8], iso, k, v) for k, v in sorted(agora.items())])
        antes = agora

    # fato_absorcao
    for h, iso, _ in commits_de(ARQ_ABSORCAO):
        dia = N.datetime.fromisoformat(iso).astimezone(N.timezone.utc).strftime("%Y-%m-%d")
        d = json.loads(git("show", f"{h}:{ARQ_ABSORCAO}"))
        for fonte, v in sorted(d.items()):
            con.execute("INSERT INTO fato_absorcao VALUES (?,?,?,?,?,?,?,?,?)",
                        (h[:8], iso, dia, fonte, v.get("total"), v.get("no_agente"),
                         v.get("parcial"), v.get("nao"), v.get("lido_em")))
    con.commit()
    n = con.execute("SELECT COUNT(*), SUM(erro IS NOT NULL) FROM fato_sprints").fetchone()
    print(f"{db}: fato_sprints {n[0]} commits ({n[1]} sem leitura), "
          f"{con.execute('SELECT COUNT(DISTINCT commit_h) FROM fato_absorcao').fetchone()[0]} commits de absorcao")
    con.close()


if __name__ == "__main__":
    main()
