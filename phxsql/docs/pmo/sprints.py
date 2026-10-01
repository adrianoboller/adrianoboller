#!/usr/bin/env python3
"""Conta o estado de cada sprint de `docs/pmo/SPRINTS.md` pelo `PENDENCIAS.md`.

    python3 docs/pmo/sprints.py

Cada sprint do plano traz uma linha `| **Pedidos** | 290 294 513 ... |` -- a
lista de QUAIS pedidos pertencem a ela, que e decisao de plano e por isso se
escreve. O ESTADO de cada um nao se escreve: sai do `PENDENCIAS.md` pelo
`ler()` do `docs/dossie/pagina-dos-pedidos.py` (importado, nunca uma segunda
copia da regex -- a copia que o pedido 484 achou deixava o `⏸` sumir calado).

O script regrava o bloco entre `<!-- SPRINTS:inicio -->` e
`<!-- SPRINTS:fim -->`: por sprint, quantos pedidos, quantos feitos, quantos
abertos (◐ + ☐) e quais. E mais duas contas que impedem o plano de mentir por
omissao:

* **pedido da sprint que nao existe no PENDENCIAS.md** -- PARA, nomeando o
  numero. Lista com numero inventado e o caso do 150 pelo outro lado.
* **pedido aberto que nao esta em sprint nenhuma** -- sai NOMEADO no bloco.
  Plano que cobre menos do que existe protege menos no dia em que alguem o
  usar como inventario (a mesma licao do portao de permissao).

Faz MENOS do que o nome promete num ponto, e diz: sprint cuja tabela nao tem
a linha `**Pedidos**` (as fatias T6b..T6d do 572 sao pedaços de UM pedido, e
a contagem por pedido nao as distingue) sai no bloco como «sem lista de
pedidos», nunca como zero aberto.
"""

import datetime
import importlib.util
import pathlib
import re
import sys

RAIZ = pathlib.Path(__file__).resolve().parents[2]
FONTE = RAIZ / "docs" / "pmo" / "SPRINTS.md"
ABRE = "<!-- SPRINTS:inicio -->"
FECHA = "<!-- SPRINTS:fim -->"

CABECA_SPRINT = re.compile(r"^##\s+(SPR-\d+)\s+[—-]\s+(.*)$")
LINHA_PEDIDOS = re.compile(r"^\|\s*\*\*Pedidos\*\*\s*\|\s*([0-9 ,]*)\|\s*$")
# Sprint que so existe como linha de tabela (fases 3 e 4) recebe a lista por
# uma linha nomeada: `| **Pedidos SPR-12** | 325 |`.
LINHA_PEDIDOS_DE = re.compile(
    r"^\|\s*\*\*Pedidos\s+(SPR-\d+)\*\*\s*\|\s*([0-9 ,]*)\|\s*$")
LINHA_SPRINT_EM_TABELA = re.compile(r"^\|\s*(SPR-\d+)\s*\|")


def importar_pedidos():
    caminho = RAIZ / "docs" / "dossie" / "pagina-dos-pedidos.py"
    spec = importlib.util.spec_from_file_location("phx_pedidos", caminho)
    mod = importlib.util.module_from_spec(spec)
    sys.modules["phx_pedidos"] = mod
    spec.loader.exec_module(mod)
    return mod


def ler_sprints(texto):
    """[(sprint, titulo, [pedidos] ou None)] na ordem do arquivo."""
    sprints = []
    atual = None
    vistos = set()
    dentro = False
    for n, linha in enumerate(texto.split("\n"), 1):
        # o bloco gerado fica no topo do arquivo: pula-se, nao se le
        if ABRE in linha:
            dentro = True
        if FECHA in linha:
            dentro = False
            continue
        if dentro:
            continue
        c = CABECA_SPRINT.match(linha)
        if c:
            atual = [c.group(1), c.group(2).strip(), None]
            sprints.append(atual)
            vistos.add(c.group(1))
            continue
        d = LINHA_PEDIDOS_DE.match(linha)
        if d:
            alvo = [x for x in sprints if x[0] == d.group(1)]
            if not alvo:
                raise SystemExit(f"SPRINTS.md:{n}: {d.group(1)} sem linha de sprint antes")
            alvo[0][2] = [int(x) for x in re.split(r"[ ,]+", d.group(2).strip()) if x]
            continue
        p = LINHA_PEDIDOS.match(linha)
        if p:
            if atual is None:
                raise SystemExit(f"SPRINTS.md:{n}: linha **Pedidos** fora de sprint")
            atual[2] = [int(x) for x in re.split(r"[ ,]+", p.group(1).strip()) if x]
            continue
        t = LINHA_SPRINT_EM_TABELA.match(linha)
        if t and t.group(1) not in vistos:
            # sprint que so existe como linha de tabela (fases 3 e 4)
            vistos.add(t.group(1))
            titulo = linha.split("|")[2].strip()
            sprints.append([t.group(1), titulo, None])
    return sprints


def montar(sprints, itens):
    por_n = {i["n"]: i for i in itens}
    faltam = sorted({n for _, _, ps in sprints if ps for n in ps if n not in por_n})
    if faltam:
        raise SystemExit(
            "SPRINTS.md cita pedido(s) que nao existem no PENDENCIAS.md: "
            + ", ".join(map(str, faltam)))
    agora = datetime.datetime.now(datetime.timezone.utc)
    linhas = [
        ABRE,
        f"_Contado do `PENDENCIAS.md` por `python3 docs/pmo/sprints.py`, gerado em "
        f"{agora:%d/%m/%Y %H:%M} UTC. Não se edita: muda a linha `**Pedidos**` da sprint "
        "ou o estado no `PENDENCIAS.md`, e roda o gerador._",
        "",
        "| Sprint | Pedidos | ☑️ feitos | ◐ | ☐ | ⏸ | Abertos (◐ + ☐) |",
        "|---|---:|---:|---:|---:|---:|---|",
    ]
    nas_sprints = set()
    tot = {"feito": 0, "parcial": 0, "planejado": 0, "depois": 0}
    for sprint, titulo, ps in sprints:
        if ps is None:
            linhas.append(f"| {sprint} · {titulo} | — | — | — | — | — | "
                          "sem lista de pedidos (não contada) |")
            continue
        conta = {"feito": 0, "parcial": 0, "planejado": 0, "depois": 0}
        abertos = []
        for n in ps:
            if n in nas_sprints:
                continue
            nas_sprints.add(n)
            classe = por_n[n]["classe"]
            conta[classe] += 1
            tot[classe] += 1
            if classe == "parcial":
                abertos.append(f"{n}◐")
            elif classe == "planejado":
                abertos.append(f"{n}☐")
        total = sum(conta.values())
        linhas.append(
            f"| {sprint} · {titulo} | {total} | {conta['feito']} | {conta['parcial']} | "
            f"{conta['planejado']} | {conta['depois']} | "
            f"{(str(len(abertos)) + ': ' + ' '.join(abertos)) if abertos else '0'} |")
    soma = sum(tot.values())
    linhas.append(
        f"| **Total nas sprints** | {soma} | {tot['feito']} | {tot['parcial']} | "
        f"{tot['planejado']} | {tot['depois']} | {tot['parcial'] + tot['planejado']} |")
    fora = sorted(i["n"] for i in itens
                  if i["classe"] in ("parcial", "planejado") and i["n"] not in nas_sprints)
    linhas += [
        "",
        f"**Abertos (◐ + ☐) no `PENDENCIAS.md` fora de sprint nenhuma: {len(fora)}** — "
        + (", ".join(map(str, fora)) if fora else "nenhum") + ".",
        FECHA,
    ]
    return "\n".join(linhas)


def main():
    pedidos = importar_pedidos()
    itens = pedidos.ler()
    texto = FONTE.read_text(encoding="utf-8")
    if texto.count(ABRE) != 1 or texto.count(FECHA) != 1:
        raise SystemExit(f"{FONTE.name}: as marcas {ABRE} / {FECHA} tem de existir uma vez cada")
    sprints = ler_sprints(texto)
    bloco = montar(sprints, itens)
    ini = texto.index(ABRE)
    fim = texto.index(FECHA) + len(FECHA)
    FONTE.write_text(texto[:ini] + bloco + texto[fim:], encoding="utf-8")
    sem_lista = [s for s, _, ps in sprints if ps is None]
    print(f"SPRINTS.md: {len(sprints) - len(sem_lista)} sprints contadas pelo PENDENCIAS.md")
    if sem_lista:
        print(f"NAO CONTADAS (sem linha **Pedidos**): {', '.join(sem_lista)}")


if __name__ == "__main__":
    main()
