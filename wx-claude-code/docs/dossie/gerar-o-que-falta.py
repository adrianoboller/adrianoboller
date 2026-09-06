#!/usr/bin/env python3
"""Gera `docs/o-que-falta.html` a partir de `docs/PENDENCIAS.md`.

A pagina existia desde a 3.18.0 sem gerador: ficou dezenove versoes carimbada
naquela versao, com os numeros do rodape (agentes, skills, testes) digitados a
mao e envelhecendo calados -- 94 agentes, 11 skills e 39 testes quando ja eram
94, 21 e 98. E o defeito que a regra do projeto ja nomeia: numero visivel ou
sai de gerador, ou esta errado e ninguem percebeu ainda.

Agora a fonte e o Markdown, os numeros saem de `numeros.json` (medidos no
repositorio) e a contagem por estado -- falta, parcial, feito -- e feita aqui,
nao digitada.

Uso: python3 docs/dossie/gerar-o-que-falta.py
"""
from __future__ import annotations

import html as H
import json
import re
import sys
from datetime import date

MESES = ("janeiro", "fevereiro", "março", "abril", "maio", "junho", "julho",
         "agosto", "setembro", "outubro", "novembro", "dezembro")
from pathlib import Path

RAIZ = Path(__file__).resolve().parents[2]
FONTE = RAIZ / "docs/PENDENCIAS.md"
SAIDA = RAIZ / "docs/o-que-falta.html"
E = H.escape

ESTADOS = {"falta": ("falta", "g5"), "parcial": ("parcial", "g3"), "feito": ("✓ feito", "feito")}


def ler(texto: str) -> tuple[list[str], list[dict]]:
    """Le a fonte. Item sem estado reconhecido NAO entra -- vira erro.

    O contrario (assumir `falta` no silencio) e o que faz uma lista mentir
    devagar: bastava um erro de digitacao para um item feito voltar a faltar.
    """
    cabeca, itens, sec = [], [], None
    blocos = re.split(r"^## ", texto, flags=re.M)
    cabeca = blocos[0].strip().splitlines()
    for b in blocos[1:]:
        sec = b.splitlines()[0].strip()
        for it in re.split(r"^### ", b, flags=re.M)[1:]:
            linhas = it.strip().splitlines()
            m = re.match(r"(\d+)\.\s+(.*)", linhas[0].strip())
            if not m:
                raise SystemExit(f"item sem numero: {linhas[0]!r}")
            campos = {}
            for l in linhas[1:]:
                c = re.match(r"-\s+([a-z ]+):\s*(.*)", l.strip())
                if c:
                    campos[c.group(1).strip()] = c.group(2).strip()
            estado = campos.get("estado", "").strip("`")
            if estado not in ESTADOS:
                raise SystemExit(f"item {m.group(1)}: estado {estado!r} desconhecido")
            campos["estado"] = estado
            itens.append({"secao": sec, "ordem": int(m.group(1)), "titulo": m.group(2), **campos})
    return cabeca, sorted(itens, key=lambda x: x["ordem"])


def main() -> int:
    cabeca, itens = ler(FONTE.read_text(encoding="utf-8"))
    n = json.loads((RAIZ / "docs/dossie/numeros.json").read_text(encoding="utf-8"))
    hoje = date.today()
    conta = {e: sum(1 for i in itens if i["estado"] == e) for e in ESTADOS}
    proprios = sum(1 for i in itens if i["tamanho"].startswith("5"))
    css = re.search(r"<style>(.*?)</style>", SAIDA.read_text(encoding="utf-8"), re.S).group(1)
    # O lead vem de um bloco marcado, nao de "o que sobrar do cabecalho": filtrar
    # por exclusao deixou a instrucao de nao editar vazar para a pagina, com
    # markdown cru e tudo. E o unico texto da fonte que leva acento, porque e
    # texto de interface.
    m = re.search(r"<!-- lead:.*?-->(.*?)<!-- fim do lead -->", FONTE.read_text(encoding="utf-8"), re.S)
    if not m:
        raise SystemExit("PENDENCIAS.md sem o bloco de lead")
    lead = " ".join(m.group(1).split())
    L = ['<title>O que falta no WX Claude Code</title>',
         '<link rel="stylesheet" href="https://fonts.googleapis.com/css2?family=Exo+2:wght@500;700;800&family=Source+Serif+4:opsz,wght@8..60,400;8..60,600&family=JetBrains+Mono:wght@400;600&display=swap">',
         f"<style>{css}</style>", '<div class="wrap">', "<header>",
         f' <div class="eyebrow">WX Claude Code {n["versao"]} · levantamento técnico · {hoje.day} de {MESES[hoje.month - 1]} de {hoje.year}</div>',
         " <h1>O que falta para atender quem migra do WX</h1>",
         f' <p class="lead">{E(lead)} Esta página é gerada de <code>docs/PENDENCIAS.md</code>; os números saem de <code>numeros.json</code>, medidos no repositório.</p>',
         f' <div class="kpis"><div><b>{conta["falta"]}</b><small>itens que faltam</small></div>'
         f'<div><b>{conta["parcial"]}</b><small>começados, com o que falta dito</small></div>'
         f'<div><b>{proprios}</b><small>do tamanho de um projeto próprio</small></div>'
         f'<div><b>{n["agentes"]} · {n["skills"]} · {n["testes"]}</b><small>agentes · skills · testes hoje</small></div></div>',
         "</header>"]
    sec = None
    for it in itens:
        if it["secao"] != sec:
            if sec is not None:
                L.append("</tbody></table></div>")
            sec = it["secao"]
            L += [f"<h2>{E(sec)}</h2>", '<div class="scroll"><table><thead><tr>',
                  "<th>o que falta</th><th>estado</th><th>hoje</th><th>o que é preciso construir</th>"
                  "<th>tamanho</th><th>ordem</th></tr></thead><tbody>"]
        rot, cls = ESTADOS[it["estado"]]
        # item feito nao tem tamanho: repetir "feito" nas duas colunas so ocupa
        # espaco e some com a informacao de porte que a coluna existe para dar
        tam = "—" if it["tamanho"].startswith("✓") else it["tamanho"]
        medido = f'<div class="pq"><i>medido:</i> {E(it["medido"])}</div>' if it.get("medido") else ""
        L.append(f'<tr><td><b>{E(it["titulo"])}</b><div class="pq"><i>Por que importa:</i> '
                 f'{E(it.get("por que importa", ""))}</div>{medido}</td>'
                 f'<td class="g {cls}">{rot}</td><td>{E(it.get("hoje", ""))}</td>'
                 f'<td>{E(it.get("construir", ""))}</td>'
                 f'<td class="g g{tam[0] if tam[0].isdigit() else "3"}">{E(tam)}</td>'
                 f'<td class="n">{it["ordem"]}</td></tr>')
    L += ["</tbody></table></div>", "<h2>Tamanhos</h2>",
          '<p class="nota"><b>1 pequeno</b>: horas, sem mudar formato. <b>2 médio</b>: dias, um script ou'
          ' uma referência. <b>3 grande</b>: semanas, um módulo novo com testes e prova real.'
          ' <b>4 muito grande</b>: um mês ou mais, com projeto real para medir. <b>5 projeto próprio</b>:'
          ' transpilador, leitor nativo, executor do legado e o piloto; cada um tem roteiro, versão e'
          ' dossiê próprios.</p>', "</div>", ""]
    SAIDA.write_text("\n".join(L), encoding="utf-8")
    print(f"ok {SAIDA} ({conta['falta']} faltam, {conta['parcial']} parciais, {conta['feito']} feitos)")
    return 0


if __name__ == "__main__":
    sys.exit(main())
