#!/usr/bin/env python3
"""Regrava as duas tabelas de cobertura do `docs/TESTES.md` com numeros MEDIDOS.

Existe pela mesma razao dos outros dois scripts desta pasta, e pela mesma lei:
numero digitado a mao envelhece calado. Uma tabela que diz «Replicacao: 11
testes» e digitada mente no dia seguinte ao primeiro teste novo -- e mente
justamente sobre onde a cobertura esta rala, que e a unica coisa que essa
tabela serve para dizer.

O que ele conta, e a definicao de cada numero:

    testes por area   `#[test]` por arquivo `.rs` de `crates/*/src` e
                      `crates/*/tests`, agrupados pelo mapa AREA_DE abaixo.
                      NAO e o que o `cargo test` reporta: aquele numero (que
                      o `numeros-do-projeto.py` mede) inclui os doc-tests e
                      nao sabe dizer de que arquivo cada teste veio.
    sem cobertura     arquivo de `src` com mais de 120 linhas e ZERO `#[test]`.
                      «Sem `#[test]` dentro» nao quer dizer «sem teste»: o
                      `table.rs` e o `ndx.rs` sao cobertos inteiros por
                      `store/tests/`. Por isso a tabela tem uma coluna
                      dizendo quem cobre cada um -- e essa coluna e escrita a
                      mao, no proprio TESTES.md, porque e julgamento.

    python3 docs/dossie/cobertura-por-area.py             mede e grava
    python3 docs/dossie/cobertura-por-area.py --so-medir  mede e mostra

Passando um `.html` junto, ele grava TAMBEM a tabela da secao «Estado e
roteiro» do dossie, entre as marcas `<!-- cobertura:... -->`:

    python3 docs/dossie/cobertura-por-area.py docs/dossie/dossie-phxsql-0.18.html
"""

import collections
import os
import pathlib
import re
import sys

RAIZ = pathlib.Path(__file__).resolve().parents[2]
CRATES = RAIZ / "crates"
ALVO = RAIZ / "docs" / "TESTES.md"

INICIO = "<!-- cobertura:inicio -->"
FIM = "<!-- cobertura:fim -->"

# Arquivos de `crates/phxsql-server/tests/` que pertencem a outra area que nao
# a que o nome do modulo sugere.
TESTES_DO_SERVER = {
    "jobs.rs": "Jobs",
    "telemetria.rs": "Telemetria e profiler",
    "servico.rs": "Interface web (servidor HTTP)",
    "mcp_stdio.rs": "MCP",
    "sonda-da-replicacao.rs": "Replicação",
    "dblink-postgres-no-fio.rs": "DbLink",
    "cifra-pelo-config.rs": "Configuração",
    "corte-do-diario-pelo-config.rs": "Configuração",
    "cifra-do-fio.rs": "Criptografia e codificação",
}

# Modulos do servidor, por area. O que nao estiver aqui cai em "Servidor
# (outros)" -- e aparecer ali e o sinal de que este mapa envelheceu.
DO_SERVIDOR = {
    "servidor.rs": "Protocolo e portões (despachar)",
    "acesso.rs": "Protocolo e portões (despachar)",
    "catalogo.rs": "Protocolo e portões (despachar)",
    "valores.rs": "Protocolo e portões (despachar)",
    "usuarios.rs": "Usuários e permissões",
    "blacklist.rs": "Segurança de rede (blacklist, firewall)",
    "config.rs": "Configuração",
    "http.rs": "Interface web (servidor HTTP)",
    "replica.rs": "Replicação",
    "bidirecional.rs": "Replicação",
    "cluster.rs": "Cluster",
    "jobs.rs": "Jobs",
    "rotinas.rs": "Gatilhos e procedimentos",
    "telemetria.rs": "Telemetria e profiler",
    "profiler.rs": "Telemetria e profiler",
    "mensagens.rs": "Mensagens (i18n do servidor)",
    "idiomas.rs": "Mensagens (i18n do servidor)",
    "email.rs": "Alertas e e-mail",
    "sistema.rs": "Monitor de máquina",
    "exportar.rs": "Exportação",
    "carga.rs": "Carga em lote (BULKINSERT)",
    "juncao.rs": "Junções e união",
    "pivot.rs": "Pivot",
    "ligacoes.rs": "DbLink",
    "mcp.rs": "MCP",
}

# Esta lista e escrita a mao porque "isto e criptografia" e julgamento, e nao
# coisa que se leia do codigo. O preco de ser escrita a mao e que ela envelhece:
# arquivo de cripto novo que ninguem acrescentar aqui cai calado em "Nucleo" e
# a tabela passa a mentir sobre onde a cobertura esta. Quem criar um modulo de
# criptografia acrescenta o nome AQUI, no mesmo commit.
CRIPTO = {"sha1.rs", "sha512.rs", "hash.rs", "cifra.rs", "ed25519.rs",
          "senha.rs", "desafio.rs", "base64.rs", "crc.rs", "keyenc.rs",
          "x25519.rs", "hkdf.rs", "fio.rs"}


def area_de(crate, rel):
    nome = os.path.basename(rel)
    if crate == "phxsql-core":
        return ("Criptografia e codificação" if nome in CRIPTO
                else "Núcleo (JSON, tipos, UUID, zip, paralelo)")
    if crate == "phxsql-store":
        return "Motor de dados (arquivos, índice, diários)"
    if crate == "phxsql-sql":
        return ("Gatilhos e procedimentos" if "rotina" in nome
                else "Camada SQL (léxico, sintaxe, tradução)")
    if crate == "phxsql-odbc":
        return "ODBC"
    if crate == "phxsql-cmd":
        return "Console de terminal (phxsqlcmd)"
    if crate == "phxsql-cli":
        return "CLI"
    if rel.startswith("tests"):
        return TESTES_DO_SERVER.get(nome, DO_SERVIDOR.get(nome, "Servidor (outros)"))
    if rel.startswith(os.path.join("src", "dblink")) or rel.startswith(os.path.join("src", "pg")):
        return "DbLink"
    return DO_SERVIDOR.get(nome, "Servidor (outros)")


def medir():
    por_area = collections.Counter()
    sem_teste = []
    for crate in sorted(os.listdir(CRATES)):
        base = CRATES / crate
        for raiz, _, arquivos in os.walk(base):
            for f in sorted(arquivos):
                if not f.endswith(".rs"):
                    continue
                caminho = pathlib.Path(raiz) / f
                rel = os.path.relpath(caminho, base)
                if not (rel.startswith("src") or rel.startswith("tests")):
                    continue
                txt = caminho.read_text(encoding="utf-8", errors="replace")
                n = len(re.findall(r"#\[test\]", txt))
                if n:
                    por_area[area_de(crate, rel)] += n
                elif rel.startswith("src") and txt.count("\n") > 120:
                    sem_teste.append((txt.count("\n"), f"{crate}/{rel}"))
    return por_area, sorted(sem_teste, reverse=True)


def tabelas(por_area, sem_teste):
    total = sum(por_area.values())
    linhas = ["| área | testes | % |", "|---|---:|---:|"]
    for a, n in por_area.most_common():
        # Negrito no que esta ralo -- abaixo de 1,5% do total. A virgula
        # decimal se monta AQUI, no numero, e nao por `replace` na linha
        # inteira: um nome de area com ponto viraria virgula junto.
        pct = f"{100.0 * n / total:.1f}".replace(".", ",")
        m = "**" if 100.0 * n / total < 1.5 else ""
        linhas.append(f"| {m}{a}{m} | {m}{n}{m} | {m}{pct}{m} |")
    linhas.append(f"| **total** | **{total}** | |")
    fora = ["", f"Arquivos de `src` com mais de 120 linhas e **zero** `#[test]`:", "",
            "| arquivo | linhas |", "|---|---:|"]
    for loc, p in sem_teste:
        fora.append(f"| `{p}` | {loc} |")
    return "\n".join(linhas + fora)


DOSSIE_ABRE = "<!-- cobertura:inicio (gerado por docs/dossie/cobertura-por-area.py) -->"
DOSSIE_FECHA = "<!-- cobertura:fim -->"


def tabela_html(por_area):
    """A mesma contagem, em HTML, para a secao «Estado e roteiro» do dossie.

    A secao ja foi oitenta linhas digitadas a mao, com o numero de testes de
    cada peca ao lado -- e envelhecia a cada rodada. Uma contagem, dois
    destinos: a duplicata e o que diverge.
    """
    total = sum(por_area.values())
    l = ['  <div class="rolo">', "    <table>",
         '      <thead><tr><th>Área</th><th class="num">testes</th><th class="num">%</th></tr></thead>',
         "      <tbody>"]
    for a, n in por_area.most_common():
        p = 100.0 * n / total
        pct = f"{p:.1f}".replace(".", ",")
        m0, m1 = ("<strong>", "</strong>") if p < 1.5 else ("", "")
        l.append(f'        <tr><td>{m0}{a}{m1}</td>'
                 f'<td class="num">{m0}{n}{m1}</td>'
                 f'<td class="num">{m0}{pct}%{m1}</td></tr>')
    l.append(f'        <tr><td><strong>total</strong></td>'
             f'<td class="num"><strong>{total}</strong></td><td class="num"></td></tr>')
    l += ["      </tbody>", "    </table>", "  </div>"]
    return "\n".join(l)


def gravar_no_dossie(caminho, por_area):
    alvo = pathlib.Path(caminho).resolve()
    txt = alvo.read_text(encoding="utf-8")
    i, j = txt.find(DOSSIE_ABRE), txt.find(DOSSIE_FECHA)
    if i < 0 or j < 0:
        raise SystemExit(f"{alvo} nao tem as marcas cobertura:inicio/fim")
    txt = (txt[:i] + DOSSIE_ABRE + "\n" + tabela_html(por_area) + "\n"
           + DOSSIE_FECHA + txt[j + len(DOSSIE_FECHA):])
    alvo.write_text(txt, encoding="utf-8")
    print(f"{alvo.name}: tabela de cobertura regravada")


def main():
    por_area, sem_teste = medir()
    bloco = tabelas(por_area, sem_teste)
    if "--so-medir" in sys.argv:
        print(bloco)
        return
    for a in sys.argv[1:]:
        if a.endswith(".html"):
            gravar_no_dossie(a, por_area)
    txt = ALVO.read_text(encoding="utf-8")
    if INICIO not in txt or FIM not in txt:
        raise SystemExit(f"{ALVO} nao tem as marcas {INICIO} / {FIM}")
    antes = txt.split(INICIO)[0]
    depois = txt.split(FIM)[1]
    ALVO.write_text(f"{antes}{INICIO}\n{bloco}\n{FIM}{depois}", encoding="utf-8")
    print(f"{ALVO}: {sum(por_area.values())} testes em {len(por_area)} áreas, "
          f"{len(sem_teste)} arquivos sem #[test]")


main()
