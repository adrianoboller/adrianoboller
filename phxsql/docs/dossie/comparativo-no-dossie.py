#!/usr/bin/env python3
"""Escreve a tabela comparativa da §33 do dossie, do `resultados.json` medido.

    python3 docs/dossie/comparativo-no-dossie.py [dossie-phxsql-0.18.html]

O setimo gerador da pasta, e ele existe pela lei que fez nascer os seis
anteriores: **todo numero visivel sai de um gerador, ou esta errado e ninguem
percebeu ainda.** A §33 e a secao do «o que este motor nao faz», e ate agora
ela era prosa inteira -- uma lista de ausencias que envelhecia como envelhece
toda lista escrita a mao. A do HFSQL.md envelheceu SEIS vezes.

Aqui a lista sai do `bancada/comparativo/resultados.json`, com a data da
medicao ao lado e a procedencia de cada celula. Refaca com:

    python3 bancada/comparativo/medir.py
    python3 docs/dossie/comparativo-no-dossie.py
"""

import datetime
import html
import json
import pathlib
import sys

AQUI = pathlib.Path(__file__).resolve().parent
RAIZ = AQUI.parent.parent
FONTE = RAIZ / "bancada" / "comparativo" / "resultados.json"
PADRAO = AQUI / "dossie-phxsql-0.18.html"

ABRE = "<!-- comparativo:inicio (gerado por docs/dossie/comparativo-no-dossie.py) -->"
FECHA = "<!-- comparativo:fim -->"

COLUNAS = [("phxsql", "PhxSql"), ("hfsql", "HFSQL(R)"),
           ("postgres", "PostgreSQL(R)"), ("cassandra", "Cassandra(R)"),
           ("mysql", "MySQL(R)"), ("sqlite", "SQLite(R)")]

# A marca vira `pino`, que e a classe que o dossie ja usa para estado.
PINO = {"tem": ("ok", "tem"), "meio": ("pend", "meio"),
        "nao": ("mal", "não"), "citado": ("nao", "citado"),
        "sem-motor": ("nao", "—")}


def bloco(d):
    linhas = d["linhas"]
    vivos = d["motores_vivos"]
    quando = datetime.datetime.fromisoformat(d["quando"]).strftime("%d/%m/%Y")
    faltam = sum(1 for l in linhas if l["phxsql"][0] in ("nao", "meio"))
    por_sql = sum(1 for l in linhas if l["como"] == "SQL no motor vivo")

    t = [ABRE]
    t.append(f'  <p><strong>{faltam} de {len(linhas)} capacidades</strong> '
             f'faltam ou estão pela metade aqui, medido em <strong>{quando}</strong>. '
             f'<strong>{por_sql}</strong> destas linhas foram perguntadas '
             f'<strong>por SQL a motores vivos nesta máquina</strong> — a mesma '
             f'instrução na língua de cada um — e o resto por sonda de código '
             f'com arquivo, linha e o trecho citado. HFSQL(R) e Cassandra(R) '
             f'entram <strong>citados</strong>, porque não há motor nem folha '
             f'nesta sessão: célula citada não é medida, e a tabela diz isso '
             f'em cada uma. O documento inteiro, com a recusa que cada motor '
             f'devolveu, é o <code>docs/COMPARATIVO.md</code>.</p>')
    t.append('  <div class="rolo">')
    t.append('    <table>')
    t.append('      <thead><tr><th>capacidade</th><th>como</th>'
             + "".join(f'<th>{html.escape(n)}</th>' for _, n in COLUNAS)
             + '</tr></thead>')
    t.append('      <tbody>')
    for l in linhas:
        como = "SQL" if l["como"] == "SQL no motor vivo" else "sonda"
        cels = "".join(
            f'<td><span class="pino {PINO[l[c][0]][0]}">{PINO[l[c][0]][1]}</span></td>'
            for c, _ in COLUNAS)
        # O titulo vem com crase de Markdown; aqui ela vira <code>.
        titulo = html.escape(l["titulo"]).replace("`", "\x00")
        partes = titulo.split("\x00")
        titulo = "".join(p if i % 2 == 0 else f"<code>{p}</code>"
                         for i, p in enumerate(partes))
        t.append(f'        <tr><td>{titulo}</td><td class="dado">{como}</td>{cels}</tr>')
    t.append('      </tbody>')
    t.append('    </table>')
    t.append('  </div>')
    versoes = ", ".join(f"{n} {html.escape(vivos[c].split()[0])}"
                        for c, n in COLUNAS if c in vivos)
    t.append(f'  <p class="dado" style="font-size:13px">Responderam: {versoes}.</p>')
    t.append(FECHA)
    return "\n".join(t)


def main():
    alvo = pathlib.Path(sys.argv[1]) if len(sys.argv) > 1 else PADRAO
    if not FONTE.exists():
        sys.exit(f"falta {FONTE} -- rode antes: python3 bancada/comparativo/medir.py")
    d = json.loads(FONTE.read_text(encoding="utf-8"))
    texto = alvo.read_text(encoding="utf-8")
    i, j = texto.find(ABRE), texto.find(FECHA)
    if i < 0 or j < 0:
        sys.exit(f"{alvo.name} nao tem as marcas comparativo:inicio/fim")
    novo = texto[:i] + bloco(d) + texto[j + len(FECHA):]
    alvo.write_text(novo, encoding="utf-8")
    print(f"{alvo.name}: bloco comparativo com {len(d['linhas'])} capacidades")


if __name__ == "__main__":
    main()
