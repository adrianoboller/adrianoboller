#!/usr/bin/env python3
"""Escreve o questionario.json do mini CRUD de clientes (PHP + MySQL -> Rust + React).

Derivado do exemplo WX, como o de faturamento: os tres exemplos respondem as
MESMAS 60 perguntas. O que muda aqui e o tamanho -- uma tabela, quatro regras
-- e o destino com frontend: Rust no backend e React no frontend, ambos sobre
o mesmo MySQL. E o "primeiro projeto" do video: pequeno o bastante para ir do
questionario ao golden master numa sentada, real o bastante para ter regra que
o golden pega.

Uso: python3 exemplos/clientes-php-mysql/gerar-questionario.py
"""
from __future__ import annotations

import json
from pathlib import Path

AQUI = Path(__file__).resolve().parent
BASE = AQUI.parent / "estoque-wx/questionario.json"


def main() -> int:
    q = json.loads(BASE.read_text(encoding="utf-8"))
    q["respondido_em"] = "2026-09-08"
    p = q["projeto"]
    p.update({"nome": "CLIENTES", "produtos": ["php"], "principal": "php", "wx_versao": "", "wx_update": "",
              "raiz_de_evidencias": "./inputs",
              "legado_php": {"tem": True, "raiz": "./inputs/legado-php", "versao": "5.6 (roda no 8.4)",
                             "framework": "nenhum", "estilo": "procedural",
                             "observacao": "quatro funcoes procedurais com mysqli; uma tabela; sem teste"}})
    b0 = q["0_empresa_e_projeto"]
    b0["0_1_softhouse"].update({"razao_social": "Loja do Bairro Ltda", "nome_fantasia": "Loja do Bairro",
                                "solicitacao": "Converter o cadastro de clientes (PHP procedural de 2011, MySQL) "
                                               "para Rust no backend e React no frontend, mantendo o mesmo MySQL."})
    b0["0_6_finalidade"] = "Cadastro de clientes de uma loja: incluir, alterar, desativar e listar, com limite de credito."
    b0["0_7_objetivos"] = ["Sair do PHP de 2011 sem perder as quatro regras do cadastro",
                           "Ter uma tela React que a loja consiga usar no celular do balcao",
                           "Provar com golden master que o Rust responde igual ao PHP"]
    d = b0["0_8_descricao_do_software"]
    d.update({"descricao": "CRUD de clientes com e-mail unico, limite de credito com duas casas, nome normalizado "
                           "e exclusao que desativa em vez de apagar.",
              "recursos": ["Incluir cliente", "Alterar cliente", "Desativar cliente (excluir logico)",
                           "Listar ativos e listar todos"],
              "modulos": ["clientes"]})
    b0["0_10_fluxograma"]["etapas"] = ["Incluir", "Alterar", "Desativar", "Listar"]
    b0["0_13_riscos"] = [
        {"risco": "Regras so existem no codigo PHP", "probabilidade": "alta", "impacto": "alto",
         "mitigacao": "golden master capturado rodando o proprio legado contra o MySQL"},
        {"risco": "Arredondamento do limite (meio para cima) diferente no Rust", "probabilidade": "media",
         "impacto": "alto", "mitigacao": "caso de teste com 1500.005 e 999.994, os dois lados"},
        {"risco": "PHP 8.4 mudou o mysqli para lancar excecao; o legado trata retorno", "probabilidade": "alta",
         "impacto": "medio", "mitigacao": "mysqli_report(OFF) no db.php, documentado"},
    ]
    b0["0_15_github"].update({"url": "https://github.com/adrianoboller/clientes-rs", "diretorio_destino": "./clientes-rs"})
    q["A_sql"].update({"status": "provided", "arquivos": ["banco.sql"], "dialeto": "MySQL",
                       "versao_do_banco": "MariaDB 10.11 (compativel MySQL 8)", "encoding": "utf-8",
                       "collation": "utf8mb4_general_ci", "charset": "utf8mb4", "timezone": "America/Sao_Paulo",
                       "observacao": "uma tabela: clientes"})
    for letra, nota in (("B_pdf_codigos", "o codigo PHP esta em inputs/legado-php/, legivel"),
                        ("C_pdf_interfaces", "o legado nao tem tela: era chamado por um painel antigo"),
                        ("D_pdf_queries", "quatro consultas, todas dentro de clientes.php"),
                        ("E_pdf_completo", "nunca houve documentacao")):
        q[letra].update({"status": "not_applicable", "arquivos": [], "observacao": nota})
        q[letra].pop("pesquisavel", None)
    f = q["F_estilo_impeccable"]
    f["preservar_ou_redesenhar"] = "redesenhar"
    f["F0_tela_modelo"] = {"status": "not_applicable", "arquivos": [],
                           "o_que_preservar": ["os quatro campos, nessa ordem: nome, e-mail, limite, ativo"],
                           "o_que_mudar": ["tudo: o legado nao tem tela propria"],
                           "observacao": "a tela React nasce nova, sobre o DESIGN.md"}
    f["F1_operacao"].update({"perfil_do_usuario": "balconista", "ambiente": "balcao", "tela_tipica": "celular e 1366x768"})
    f["F3_grids"]["linhas_por_tela"] = "ate 500 clientes"
    f["F4_formularios"]["mascaras"] = ["moeda", "email"]
    q["H_backend"].update({"perfil": "rust", "linguagem": "Rust", "framework": "tiny_http + mysql (crates)",
                           "banco": "MySQL 8 / MariaDB 10.11, o mesmo do legado", "interface": "servico-tcp"})
    q["I_frontend"].update({"perfil": "react", "linguagem": "TypeScript", "framework": "React 19 + Vite",
                            "plataformas": ["web", "celular pelo navegador"]})
    alvo = AQUI / "questionario.json"
    alvo.write_text(json.dumps(q, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
    print(f"ok {alvo}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
