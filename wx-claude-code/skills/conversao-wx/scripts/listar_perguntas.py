#!/usr/bin/env python3
"""Lista toda pergunta do questionario com o id que a invoca.

A lista sai do proprio modelo (`templates/questionario.json`), nunca escrita a
mao: item novo no modelo aparece aqui sem ninguem lembrar de editar um .md.
Cada id e o argumento de `/wx-claude-code:pergunta <id>`.

Uso:
  listar_perguntas.py                 # tabela em texto
  listar_perguntas.py --markdown      # tabela markdown (entra no MANUAL)
  listar_perguntas.py --json          # para outro script
  listar_perguntas.py --id F9         # so aquela, com o caminho no JSON
"""
from __future__ import annotations

import argparse
import json
import re
from pathlib import Path

RAIZ = Path(__file__).resolve().parents[3]
MODELO = RAIZ / "skills/conversao-wx/templates/questionario.json"

# Titulo de cada bloco de primeiro nivel. O que nao estiver aqui entra pelo
# proprio nome, sem inventar: melhor um titulo feio que um titulo errado.
BLOCOS = {
    "projeto": ("PROJ", "Identificação do projeto, legado (E/OU) e raiz de evidências"),
    "0_empresa_e_projeto": ("0", "Empresa, diretores, logotipos, prazo, orçamento, riscos, GitHub, aprovador"),
    "A_sql": ("A", "Script SQL da análise HFSQL"),
    "B_pdf_codigos": ("B", "PDF dos códigos WLanguage"),
    "C_pdf_interfaces": ("C", "PDF das interfaces"),
    "D_pdf_queries": ("D", "PDF das queries"),
    "E_pdf_completo": ("E", "PDF completo do projeto"),
    "F_estilo_impeccable": ("F", "Estilo de tela: tela modelo, botões, cores, fundo"),
    "G_help_json": ("G", "Corpus do Help WLanguage (12k)"),
    "H_backend": ("H", "Linguagem de destino do backend e estratégia"),
    "I_frontend": ("I", "Linguagem de destino do frontend e ritmo"),
    "J_economia_de_tokens": ("J", "Economia de tokens e estilo de resposta"),
    "K_ambiente": ("K", "Ambiente: privilégios, Rust, bancos, GitHub, n8n"),
    "L_contexto_e_implantacao": ("L", "Contexto do Claude Code, implantação, hooks, MCP, esqueleto de ERP"),
    "M_artefatos": ("M", "Artefatos e anotações submetidos pelo cliente"),
}
# Subperguntas numeradas dentro de um bloco: 0_1_..., F0_..., K0_..., L1_...
# 0_16_aprovador, F9_vocabulario, K7_n8n, L6_esqueleto: o bloco 0 usa
# underscore entre a letra e o numero, os outros nao.
RX_SUB = re.compile(r"^(?P<pref>0|F|K|L)_?(?P<num>\d+)_(?P<resto>.+)$")


def humano(chave: str) -> str:
    return chave.replace("_", " ").strip().capitalize()


def perguntas(modelo: Path | None = None) -> list[dict]:
    q = json.loads((modelo or MODELO).read_text(encoding="utf-8"))
    saida: list[dict] = []
    for bloco, valor in q.items():
        if bloco in {"schema_version", "respondido_em"}:
            continue
        letra, titulo = BLOCOS.get(bloco, (bloco.split("_")[0].upper(), humano(bloco)))
        saida.append({"id": letra, "bloco": bloco, "caminho": bloco, "titulo": titulo, "nivel": 1})
        if not isinstance(valor, dict):
            continue
        for chave in valor:
            m = RX_SUB.match(chave)
            if not m:
                continue
            saida.append({"id": f"{m.group('pref')}{m.group('num')}" if m.group("pref") != "0" else f"0.{m.group('num')}",
                          "bloco": bloco, "caminho": f"{bloco}.{chave}", "titulo": humano(m.group("resto")), "nivel": 2})
    return saida


def main() -> int:
    p = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    p.add_argument("--markdown", action="store_true")
    p.add_argument("--json", action="store_true")
    p.add_argument("--id")
    p.add_argument("--modelo", type=Path)
    a = p.parse_args()
    itens = perguntas(a.modelo)
    if a.id:
        alvo = a.id.strip().upper().replace(",", ".")
        itens = [i for i in itens if i["id"].upper() == alvo]
        if not itens:
            print(f"erro: id {a.id!r} nao existe; rode sem --id para ver a lista")
            return 2
    if a.json:
        print(json.dumps(itens, ensure_ascii=False, indent=2))
        return 0
    if a.markdown:
        print("| comando | id | pergunta | onde vai no JSON |")
        print("| --- | --- | --- | --- |")
        for i in itens:
            print(f"| `/wx-claude-code:pergunta {i['id']}` | `{i['id']}` | {i['titulo']} | `{i['caminho']}` |")
        return 0
    largura = max((len(i["id"]) for i in itens), default=4)
    for i in itens:
        risco = "  " if i["nivel"] == 1 else "    "
        print(f"{risco}{i['id']:<{largura}}  {i['titulo']}")
    print(f"\n{len(itens)} perguntas. Invoque uma com: /wx-claude-code:pergunta <id>")
    return 0


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except BrokenPipeError:  # `| head` fecha o cano; nao e erro do script
        raise SystemExit(0)
