#!/usr/bin/env python3
"""Gera o rollup do board de PMO a partir de `docs/pmo/BACKLOG.md`.

    python3 docs/pmo/rollup.py

O board tem quatro tabelas de item (Pilar 1, Pilar 2, Pilar 3, Governança) e
uma pergunta que hoje só se responde contando na mão: quantos itens estão
abertos, entregues/fechados ou parados, por pilar -- e quantos itens de
escalão **forte** ainda estão abertos. Número digitado à mão envelhece
calado, a mesma lei do selo do dossiê; este script conta e regrava o bloco
entre as marcas `<!-- ROLLUP:inicio -->` / `<!-- ROLLUP:fim -->`, logo abaixo
do título do board.

## Como o estado de cada linha é lido

O board não usa um emoji fixo de estado (como o `PENDENCIAS.md`) -- a coluna
de status é prosa livre: "aberto", "aberto (pesquisa primeiro)", "**entregue
16/09/2026** pela via (b) ...", "**fechado nesta rodada**". A regra explícita
é: o estado é a PRIMEIRA PALAVRA da célula, depois de tirar o `**` de ênfase.
É o que faz a linha do P1-ISO -- "**entregue** ... Sombra/MVCC continua
**parada**" -- classificar como entregue-fechado, e não como parado só porque
a palavra "parada" aparece depois, fora de contexto: o que decide é o começo
da frase, não qualquer palavra dela.

    aberto        -> "aberto"
    entregue      -> "entregue-fechado"
    fechado       -> "entregue-fechado"
    parado/parada -> "parado"

Primeira palavra fora dessa lista é estado **desconhecido**, e o script PARA
nomeando a linha (arquivo:linha, item, célula) -- nunca adivinha. É a mesma
regra do `docs/dossie/pagina-dos-pedidos.py` para o símbolo de estado que não
está na legenda: item com estado que ninguém sabe ler não pode sumir da
conta.

## Idempotência, e o carimbo de data

Rodar duas vezes sem mexer no `BACKLOG.md` produz o MESMO bloco -- exceto o
carimbo "gerado em", que é relógio de parede (a mesma família das páginas do
dossiê que carregam "gerado em HH:MM" e que o portão dos geradores confere em
modo `sem-carimbo`, mascarando só a data/hora na comparação).
"""

import datetime
import pathlib
import re
import sys

RAIZ = pathlib.Path(__file__).resolve().parents[2]
FONTE = RAIZ / "docs" / "pmo" / "BACKLOG.md"

ABRE = "<!-- ROLLUP:inicio -->"
FECHA = "<!-- ROLLUP:fim -->"

# A palavra que decide o estado e' SEMPRE a primeira da celula, apos tirar o
# `**` de enfase. Lista explicita, para nao virar um regex que "parece"
# reconhecer tudo: o que nao esta aqui e' desconhecido, e o script para.
PALAVRA_ESTADO = {
    "aberto": "aberto",
    "entregue": "entregue-fechado",
    "fechado": "entregue-fechado",
    "parado": "parado",
    "parada": "parado",
}

ESTADOS = ("aberto", "entregue-fechado", "parado")
ROTULO_ESTADO = {
    "aberto": "aberto",
    "entregue-fechado": "entregue/fechado",
    "parado": "parado",
}

# Uma linha de item: `| P1-SQL-6 | ... | ... | **forte** | ... | aberto |` ou
# `| GOV-1 | ... | ... | **leve** | aberto |` (a tabela de Governanca nao tem
# a coluna "depende"). O ID e' capturado a parte; o resto da linha vira
# celulas por split de "|".
LINHA_ITEM = re.compile(r"^\|\s*((?:P1|P2|P3|GOV)-[^\s|]+)\s*\|(.*)\|\s*$")

# Os quatro titulos de pilar/governanca que dividem o board em secoes. So'
# entram aqui os `##` que introduzem uma TABELA de item -- os demais titulos
# ("Como esta pagina fecha um item" etc.) nao casam e nao mudam o pilar atual.
CABECALHO_PILAR = re.compile(r"^##\s+(Pilar\s+\d[^*]*|Governan[cç]a[^*]*)")


def celulas(resto: str):
    """As celulas do meio de uma linha de tabela (sem as pontas vazias)."""
    return [c.strip() for c in resto.split("|")]


def primeira_palavra(texto: str) -> str:
    t = re.sub(r"^\*+", "", texto.strip())
    m = re.match(r"[A-Za-zÀ-ÿ]+", t)
    return m.group(0).lower() if m else ""


def limpar_pilar(titulo: str) -> str:
    """"Pilar 2 — E-mail P2P" -> "Pilar 2"; "Governança / transversal" ->
    "Governança". O rotulo curto e' o que entra na tabela do rollup."""
    t = titulo.strip()
    t = re.sub(r"\s*[—\-/].*$", "", t)
    return t.strip()


def ler():
    linhas = FONTE.read_text(encoding="utf-8").split("\n")
    pilar_atual = None
    itens = []
    vistos = {}
    for n, l in enumerate(linhas, 1):
        cp = CABECALHO_PILAR.match(l)
        if cp:
            pilar_atual = limpar_pilar(cp.group(1))
            continue
        m = LINHA_ITEM.match(l)
        if not m:
            continue
        id_item = m.group(1).strip()
        cel = celulas(m.group(2))
        if len(cel) < 3:
            continue
        if pilar_atual is None:
            raise SystemExit(
                f"BACKLOG.md:{n}: item {id_item} aparece antes de qualquer "
                "cabecalho de Pilar/Governanca -- nao sei a que secao "
                "atribui-lo.")
        if id_item in vistos:
            raise SystemExit(
                f"BACKLOG.md:{n}: item {id_item} repetido (ja visto na "
                f"linha {vistos[id_item]}).")
        vistos[id_item] = n
        # escalao e' sempre a 4a celula (id, entrega, dono, escalao, [depende,]
        # status): a coluna que falta na tabela de Governanca e' "depende", e
        # ela vem DEPOIS do escalao, nunca antes -- entao a posicao do
        # escalao nao muda entre as duas formas de tabela.
        escalao = primeira_palavra(cel[2])
        status_txt = cel[-1]
        palavra = primeira_palavra(status_txt)
        if palavra not in PALAVRA_ESTADO:
            raise SystemExit(
                f"BACKLOG.md:{n}: item {id_item} tem estado {palavra!r} na "
                f"celula {status_txt!r}, que nao e' um dos reconhecidos "
                f"({', '.join(sorted(set(PALAVRA_ESTADO)))}). Estado "
                "desconhecido nao se adivinha -- comece a celula por aberto, "
                "entregue, fechado ou parado.")
        itens.append({
            "id": id_item,
            "pilar": pilar_atual,
            "escalao": escalao,
            "estado": PALAVRA_ESTADO[palavra],
        })
    if not itens:
        raise SystemExit("nenhum item reconhecido em BACKLOG.md")
    return itens


def montar_bloco(itens) -> str:
    agora = datetime.datetime.now(datetime.timezone.utc)
    carimbo = agora.strftime("%d/%m/%Y %H:%M")

    pilares = []
    for it in itens:
        if it["pilar"] not in pilares:
            pilares.append(it["pilar"])

    def contar(lista):
        return {e: sum(1 for i in lista if i["estado"] == e) for e in ESTADOS}

    linhas_tabela = [
        "| pilar | " + " | ".join(ROTULO_ESTADO[e] for e in ESTADOS) + " | total |",
        "|---|---|---|---|---|",
    ]
    total = contar(itens)
    for p in pilares:
        do_pilar = [i for i in itens if i["pilar"] == p]
        c = contar(do_pilar)
        linhas_tabela.append(
            f"| {p} | " + " | ".join(str(c[e]) for e in ESTADOS)
            + f" | {len(do_pilar)} |")
    linhas_tabela.append(
        "| **total** | " + " | ".join(f"**{total[e]}**" for e in ESTADOS)
        + f" | **{len(itens)}** |")

    forte_abertos = sorted(
        i["id"] for i in itens if i["escalao"] == "forte" and i["estado"] == "aberto")

    partes = [
        f"*Gerado por `docs/pmo/rollup.py` em {carimbo} UTC — não conte à "
        "mão; o estado sai da última coluna de cada tabela abaixo, e o "
        "escalão da coluna `escalão`.*",
        "",
        *linhas_tabela,
        "",
        f"**itens em forte abertos:** {len(forte_abertos)}"
        + (" — " + ", ".join(forte_abertos) if forte_abertos else " (nenhum)"),
    ]
    return "\n".join(partes)


def regravar(bloco: str) -> bool:
    txt = FONTE.read_text(encoding="utf-8")
    i, j = txt.find(ABRE), txt.find(FECHA)
    if i < 0 or j < 0:
        raise SystemExit(
            f"{FONTE}: faltam as marcas {ABRE!r}/{FECHA!r}. Acrescente-as "
            "logo abaixo do titulo do board (uma linha em branco depois do "
            "`# Board de controle...`) antes de rodar este gerador.")
    novo = txt[: i] + ABRE + "\n" + bloco + "\n" + FECHA + txt[j + len(FECHA):]
    mudou = novo != txt
    if mudou:
        FONTE.write_text(novo, encoding="utf-8")
    return mudou


def main() -> int:
    itens = ler()
    bloco = montar_bloco(itens)
    mudou = regravar(bloco)

    contas = {e: sum(1 for i in itens if i["estado"] == e) for e in ESTADOS}
    forte_abertos = [i["id"] for i in itens if i["escalao"] == "forte" and i["estado"] == "aberto"]

    print(f"{len(itens)} itens do board: {contas['aberto']} abertos, "
          f"{contas['entregue-fechado']} entregues/fechados, "
          f"{contas['parado']} parados")
    print(f"itens em forte abertos: {len(forte_abertos)}"
          + (" -- " + ", ".join(forte_abertos) if forte_abertos else ""))
    print(f"BACKLOG.md {'regravado' if mudou else 'ja estava atualizado (idempotente)'}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
