#!/usr/bin/env python3
"""Escreve o questionario.json deste exemplo a partir do exemplo WX.

Os dois projetos respondem as mesmas 60 perguntas; o que muda e o legado (PHP
procedural em vez de WINDEV), o banco (MySQL em vez de HFSQL), as evidencias
(codigo-fonte legivel em vez de PDF) e as regras de negocio. Derivar em vez de
copiar mantem os dois exemplos respondendo o MESMO questionario: se uma
pergunta nova entrar, ela entra nos dois.

Uso: python3 exemplos/faturamento-php/gerar-questionario.py
"""
from __future__ import annotations

import json
from pathlib import Path

AQUI = Path(__file__).resolve().parent
BASE = AQUI.parent / "estoque-wx/questionario.json"


def main() -> int:
    q = json.loads(BASE.read_text(encoding="utf-8"))

    q["respondido_em"] = "2026-09-05"
    p = q["projeto"]
    p["nome"] = "FATURAMENTO"
    p["produtos"] = ["php"]
    p["principal"] = "php"
    p["wx_versao"] = ""
    p["wx_update"] = ""
    p["legado_php"] = {
        "tem": True,
        "raiz": "./inputs/legado-php",
        "versao": "7.4",
        "framework": "nenhum",
        "estilo": "procedural",
        "observacao": "PHP procedural com mysqli e HTML no meio do codigo; sem composer, sem autoload, sem teste",
    }
    p["raiz_de_evidencias"] = "./inputs"

    b0 = q["0_empresa_e_projeto"]
    b0["0_1_softhouse"]["solicitacao"] = (
        "Converter o FATURAMENTO (PHP 7.4 procedural, 2009) para Rust, mantendo as regras do financeiro ao centavo.")
    b0["0_6_finalidade"] = (
        "Emitir faturas e controlar titulos a receber de uma distribuidora, com multa e juros no atraso.")
    b0["0_7_objetivos"] = [
        "Sair do PHP procedural sem perder nenhuma regra do financeiro",
        "Ter teste automatizado onde hoje nao existe nenhum",
        "Trocar a concatenacao de SQL por consulta parametrizada",
    ]
    d = b0["0_8_descricao_do_software"]
    d["descricao"] = ("Sistema de faturamento com cadastro de clientes, pedidos a faturar, emissao de fatura com "
                      "desconto por forma de pagamento, parcelamento e baixa de titulo com multa e juros.")
    d["recursos"] = ["Emissao de fatura a partir dos pedidos a faturar",
                     "Desconto por forma de pagamento",
                     "Parcelamento em ate 12 vezes",
                     "Baixa de titulo com multa e juros pro rata die",
                     "Bloqueio de cliente inadimplente"]
    d["modulos"] = ["cadastros", "faturamento", "financeiro"]
    b0["0_10_fluxograma"]["etapas"] = ["Pedido a faturar", "Conferencia do cliente", "Emissao da fatura",
                                       "Geracao dos titulos", "Baixa com encargos"]
    b0["0_13_riscos"] = [
        {"risco": "Regra do financeiro so existe no codigo, sem documento", "probabilidade": "alta",
         "impacto": "alto", "mitigacao": "golden master capturado rodando o proprio legado"},
        {"risco": "Arredondamento diferente entre PHP e Rust", "probabilidade": "media", "impacto": "alto",
         "mitigacao": "valores em centavos inteiros no destino e caso de teste por regra"},
        {"risco": "SQL concatenado esconde injecao ja explorada", "probabilidade": "media", "impacto": "alto",
         "mitigacao": "inventario das consultas no G2 antes de reescrever"},
    ]
    b0["0_15_github"]["url"] = "https://github.com/adrianoboller/faturamento-rs"
    b0["0_15_github"]["diretorio_destino"] = "./faturamento-rs"

    a = q["A_sql"]
    a.update({"status": "provided", "arquivos": ["banco.sql"], "dialeto": "MySQL",
              "versao_do_banco": "5.7", "encoding": "utf-8", "collation": "utf8mb4_general_ci",
              "charset": "utf8mb4", "timezone": "America/Sao_Paulo",
              "observacao": "DDL exportado do banco de producao, sem dados"})

    # O legado aqui e codigo-fonte legivel: nao ha PDF de documentacao nenhum.
    # Dizer 'missing' seria mentira; e not_applicable mesmo, e o G0 tem de
    # aceitar isso sem exigir PDF de um projeto que nunca teve documentacao.
    for letra, nota in (("B_pdf_codigos", "o codigo-fonte PHP esta em inputs/legado-php/, legivel, sem PDF"),
                        ("C_pdf_interfaces", "as telas sao HTML dentro dos proprios .php"),
                        ("D_pdf_queries", "as consultas estao concatenadas no PHP, nao ha catalogo"),
                        ("E_pdf_completo", "o sistema nunca teve documentacao")):
        q[letra].update({"status": "not_applicable", "arquivos": [], "observacao": nota})
        q[letra].pop("pesquisavel", None)

    f = q["F_estilo_impeccable"]
    f["preservar_ou_redesenhar"] = "preservar"
    f["F0_tela_modelo"] = {"status": "not_applicable", "arquivos": [],
                           "o_que_preservar": ["a sequencia da tela de emissao: cliente, forma, parcelas, gerar",
                                               "os rotulos que o financeiro le em voz alta ao telefone"],
                           "o_que_mudar": ["a tabela sem estilo, que hoje e um <table border=1>",
                                           "as mensagens de erro que hoje saem por die()"],
                           "observacao": "nao ha screenshot; o modelo e o proprio fatura_gerar.php"}
    f["F1_operacao"].update({"perfil_do_usuario": "faturamento e financeiro",
                             "ambiente": "escritorio", "tela_tipica": "1920x1080"})
    f["F3_grids"]["linhas_por_tela"] = "titulos em aberto ate 5 mil linhas"
    f["F4_formularios"]["mascaras"] = ["cnpj", "moeda", "data"]

    h = q["H_backend"]
    h.update({"perfil": "rust_axum", "linguagem": "Rust", "framework": "Axum"})

    return escrever(q)


def escrever(q: dict) -> int:
    alvo = AQUI / "questionario.json"
    alvo.write_text(json.dumps(q, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
    print(f"ok {alvo}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
