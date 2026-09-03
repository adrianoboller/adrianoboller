#!/usr/bin/env python3
"""Aplica as respostas do questionario A-J ao espaco de trabalho .wx-migration/.

Le o questionario.json, gera o manifesto de entradas, a configuracao de conversao,
o CLAUDE.md do projeto e o esboco de DESIGN.md. Nunca sobrescreve arquivo que ja
exista: o questionario pode ser reaplicado sem apagar o que o usuario editou.

Por que um script e nao o modelo escrevendo os JSON: o manifesto tem schema, e um
campo fora do enum derruba o pre-flight inteiro. Aqui o mapeamento e um so.
"""

from __future__ import annotations

import argparse
import json
import os
import re
import sys
from datetime import date
from pathlib import Path

STATUS = {"provided", "partial", "missing", "not_applicable"}
MODOS = {
    "inventario": "inventory",
    "inventário": "inventory",
    "inventory": "inventory",
    "plano": "plan",
    "plan": "plan",
    "piloto": "pilot",
    "pilot": "pilot",
    "completo": "complete",
    "complete": "complete",
}

# O template CLAUDE.md ja traz a secao «Estilo de resposta»; a letra J acrescenta
# so o bloco de economia, para nao repetir a mesma regra duas vezes no arquivo.
ESTILO_DE_RESPOSTA = """
## Economia de tokens (letra J do questionário)

O estilo de resposta acima vale para a sessão inteira: direto ao ponto, frases
curtas, um assunto por parágrafo, problema em uma linha, solução em passos.

- Não releia arquivo grande que já está no contexto; cite o trecho.
- Saída de comando longa vai para arquivo em `.wx-migration/logs/` e volta como localizador.
- Anexos e o corpus do Help são consultados por índice, nunca abertos inteiros.
- `/wx-claude-code:laudo-tokens` mede o consumo; nada muda sem aprovação.
"""


def write_new(destination: Path, payload: str) -> str:
    flags = os.O_WRONLY | os.O_CREAT | os.O_EXCL
    if hasattr(os, "O_NOFOLLOW"):
        flags |= os.O_NOFOLLOW
    destination.parent.mkdir(parents=True, exist_ok=True)
    try:
        descriptor = os.open(destination, flags, 0o600)
    except FileExistsError:
        return f"SKIPPED {destination} (já existe)"
    with os.fdopen(descriptor, "w", encoding="utf-8") as stream:
        stream.write(payload)
    return f"CREATED {destination}"


def status_de(bloco: dict) -> str:
    valor = str(bloco.get("status", "missing")).strip().lower()
    if valor not in STATUS:
        raise ValueError(f"status inválido: {valor!r} (aceitos: {sorted(STATUS)})")
    if valor == "provided" and not bloco.get("arquivos"):
        raise ValueError("status 'provided' sem nenhum arquivo listado")
    return valor


RAIZ_DE_EVIDENCIAS: Path | None = None


def contar_paginas(pdf: Path) -> int | None:
    """Mede page_count: pypdf quando existe; senao, conta objetos /Type /Page no
    binario (funciona para PDFs de texto comuns; PDF cifrado ou incremental pode
    escapar, e ai o campo fica ausente e o pre-flight cobra)."""
    try:
        from pypdf import PdfReader  # type: ignore
        return len(PdfReader(str(pdf)).pages)
    except Exception:  # noqa: BLE001 - qualquer falha cai no contador bruto
        pass
    try:
        dados = pdf.read_bytes()
    except OSError:
        return None
    n = len(re.findall(rb"/Type\s*/Page(?![s/\w])", dados))
    return n or None


def itens_pdf(bloco: dict, escopo: list[str]) -> list[dict]:
    itens = []
    for caminho in bloco.get("arquivos", []):
        item = {"path": caminho, "content_scope": escopo}
        if bloco.get("pesquisavel") is not None:
            item["searchable"] = bool(bloco["pesquisavel"])
        if RAIZ_DE_EVIDENCIAS is not None:
            pdf = RAIZ_DE_EVIDENCIAS / caminho
            if pdf.is_file():
                paginas = contar_paginas(pdf)
                if paginas:
                    item["page_count"] = paginas
        itens.append(item)
    return itens


def itens_screenshots(raiz: Path) -> list[dict]:
    """Le screenshots/screenshots.json na raiz de evidencias: uma lista de
    {arquivo, tela, estado, plataforma}. Sem o sidecar, o grupo fica missing e o
    pre-flight cobra; adivinhar tela e estado pelo nome do arquivo seria inventar."""
    sidecar = raiz / "screenshots" / "screenshots.json"
    if not sidecar.is_file():
        return []
    lista = json.loads(sidecar.read_text(encoding="utf-8"))
    itens = []
    for s in lista:
        arq = raiz / "screenshots" / s["arquivo"]
        if arq.is_file():
            itens.append({"path": f"screenshots/{s['arquivo']}", "screen_or_report": s["tela"], "state": s["estado"], "platform": s.get("plataforma", "WINDEV")})
    return itens


def montar_manifesto(q: dict, modelo: dict, projeto: Path) -> dict:
    m = json.loads(json.dumps(modelo))
    p = q.get("projeto", {})
    # O pre-flight resolve evidence_root a partir da pasta do manifesto
    # (.wx-migration/), nao da raiz do projeto: o caminho gravado e relativo a ela.
    raiz = Path(p.get("raiz_de_evidencias") or "./inputs")
    if not raiz.is_absolute():
        raiz = projeto / raiz
    m["evidence_root"] = os.path.relpath(raiz.resolve(strict=False), (projeto / ".wx-migration").resolve(strict=False)).replace(os.sep, "/")
    global RAIZ_DE_EVIDENCIAS
    RAIZ_DE_EVIDENCIAS = raiz.resolve(strict=False)
    m["project"].update(
        {
            "name": p.get("nome", ""),
            "products": p.get("produtos", []),
            "wx_version": p.get("wx_versao", ""),
            "wx_update": p.get("wx_update", ""),
            "source_language": p.get("idioma", "pt-BR"),
            "human_approver": p.get("aprovador", ""),
        }
    )
    a = m["artifacts"]

    sql = q.get("A_sql", {})
    a["sql_scripts"]["status"] = status_de(sql)
    a["sql_scripts"]["items"] = [
        {
            "path": caminho,
            "dialect": sql.get("dialeto", ""),
            "database_version": sql.get("versao_do_banco", ""),
            "encoding": sql.get("encoding", "utf-8"),
            "collation": sql.get("collation", ""),
            "charset": sql.get("charset", ""),
            "timezone": sql.get("timezone", ""),
        }
        for caminho in sql.get("arquivos", [])
    ]
    if sql.get("observacao"):
        a["sql_scripts"]["notes"] = sql["observacao"]

    completo = q.get("E_pdf_completo", {})
    escopo_completo = ["code", "events", "ui", "queries", "business_rules", "reports", "integrations"]

    def grupo(chave_q: str, chave_m: str, escopo: list[str]) -> None:
        bloco = q.get(chave_q, {})
        estado = status_de(bloco)
        itens = itens_pdf(bloco, escopo)
        # O PDF completo cobre o que falta nos PDFs separados, mas como 'partial':
        # a cobertura existe, a separacao que o WX faz por tipo nao.
        if estado == "missing" and status_de(completo) in {"provided", "partial"}:
            estado = "partial"
            itens = itens_pdf(completo, escopo_completo)
            a[chave_m]["notes"] = "Coberto apenas pelo PDF completo (letra E); confira a extração por tipo."
        a[chave_m]["status"] = estado
        a[chave_m]["items"] = itens
        if bloco.get("observacao"):
            a[chave_m]["notes"] = bloco["observacao"]

    grupo("B_pdf_codigos", "code_documents", ["code", "events"])
    grupo("C_pdf_interfaces", "ui_documents", ["ui", "reports"])
    grupo("D_pdf_queries", "query_documents", ["queries"])

    estado_completo = status_de(completo)
    a["business_rule_documents"]["status"] = "partial" if estado_completo in {"provided", "partial"} else "missing"
    a["business_rule_documents"]["items"] = itens_pdf(completo, escopo_completo)
    if estado_completo in {"provided", "partial"}:
        a["business_rule_documents"]["notes"] = (
            "Regras extraídas do PDF completo (letra E); confirme cada regra com o responsável de negócio."
        )

    shots = itens_screenshots(RAIZ_DE_EVIDENCIAS) if RAIZ_DE_EVIDENCIAS else []
    if shots:
        a["screenshots"]["status"] = "provided"
        a["screenshots"]["items"] = shots
        a["screenshots"]["notes"] = "Lidos de screenshots/screenshots.json (tela, estado, plataforma declarados pelo usuário)."

    g = q.get("G_help_json", {})
    m["project"]["wlanguage_help_version"] = str(g.get("versao_do_help", ""))
    if not g.get("usar_corpus_do_plugin", True):
        a["wlanguage_help_json"] = {
            "status": "not_applicable",
            "notes": "Usuário optou por não usar o corpus do plugin (letra G).",
            "items": [],
        }
    override = g.get("override_da_versao", {})
    if override.get("arquivos"):
        a["wlanguage_help_json"]["override"] = {
            "status": status_de(override),
            "version": override.get("versao", ""),
            "items": [{"path": caminho} for caminho in override["arquivos"]],
        }
    return m


def montar_config(q: dict, modelo: dict) -> dict:
    c = json.loads(json.dumps(modelo))
    p = q.get("projeto", {})
    modo = str(p.get("modo", "inventario")).strip().lower()
    if modo not in MODOS:
        raise ValueError(f"modo inválido: {modo!r} (aceitos: inventario, plano, piloto, completo)")
    c["mode"] = MODOS[modo]
    h = q.get("H_backend", {})
    i = q.get("I_frontend", {})
    frameworks = [x for x in (h.get("framework", ""), i.get("framework", "")) if x]
    linguagens = [x for x in (h.get("linguagem", ""), i.get("linguagem", "")) if x]
    c["target"].update(
        {
            "language": " + ".join(dict.fromkeys(linguagens)),
            "frameworks": frameworks,
            "database": h.get("banco", ""),
            "platforms": i.get("plataformas", []),
            "deployment": h.get("implantacao", ""),
            "minimum_versions": h.get("versoes_minimas", {}),
        }
    )
    c["scale"]["supported_browsers_devices"] = i.get("navegadores_e_dispositivos", [])
    f = q.get("F_estilo_impeccable", {})
    if f.get("ativar"):
        escolha = str(f.get("preservar_ou_redesenhar", "preservar")).lower()
        c["fidelity"]["ui"] = "redesign" if escolha.startswith("redesen") else "behavioral"
    c["acceptance"]["approver"] = p.get("aprovador", "")
    c["governance"]["decision_owner"] = p.get("aprovador", "")
    return c


def esboco_design(q: dict) -> str:
    f = q.get("F_estilo_impeccable", {})
    pal = f.get("paleta", {})
    linhas = ["# DESIGN.md — esboço do questionário (letra F)", ""]
    linhas.append("Preenchido por `aplicar_questionario.py`; o Impeccable completa em `/wx-claude-code:estilo-telas`.")
    linhas.append("")
    linhas.append(f"- Direção: **{f.get('preservar_ou_redesenhar', 'preservar')}** o visual do WX")
    linhas.append(f"- Tema: {f.get('tema', 'ambos')}")
    linhas.append(f"- Tipografia: {f.get('tipografia') or '(a definir, com fallback real)'}")
    linhas.append(f"- Densidade: {f.get('densidade', 'compacta')}")
    if f.get("marca"):
        linhas.append(f"- Marca a respeitar: {f['marca']}")
    linhas += ["", "## Tokens de cor", "", "| Papel | Valor | Contraste medido |", "| --- | --- | --- |"]
    for papel in ("principal", "secundaria", "fundo", "texto", "acao", "erro", "aviso", "sucesso"):
        linhas.append(f"| {papel} | {pal.get(papel) or '(pendente)'} | (medir, mínimo 4,5:1 em texto) |")
    linhas += [
        "",
        "## Cores da ação",
        "",
        "Verde inclui, amarelo altera, rosa marca, vermelho exclui de vez, azul consulta.",
        "Sempre contorno; o preenchimento só no `hover`. No tema claro, escurecer para passar de 4,5:1.",
        "",
        "## Regras",
        "",
        "- Texto de interface não muda a caixa do dado gravado.",
        "- Componente novo se abre no navegador e se olha antes de ser dado como pronto.",
        "- Toda decisão visual tem origem: resposta F, marca do cliente ou `DEC-*`.",
        "",
    ]
    if f.get("observacao"):
        linhas += ["## Observação do usuário", "", f.get("observacao"), ""]
    return "\n".join(linhas)


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--questionario", required=True, type=Path)
    parser.add_argument("--project-root", required=True, type=Path)
    parser.add_argument("--plugin-root", required=True, type=Path, help="raiz do plugin (CLAUDE_PLUGIN_ROOT)")
    args = parser.parse_args()

    projeto = args.project_root.resolve(strict=True)
    skill = (args.plugin_root / "skills" / "conversao-wx").resolve(strict=True)
    modelos = skill / "templates"
    q = json.loads(args.questionario.read_text(encoding="utf-8"))
    if not q.get("respondido_em"):
        q["respondido_em"] = date.today().isoformat()

    manifesto = montar_manifesto(q, projeto=projeto, modelo=json.loads((modelos / "wx-inputs.manifest.json").read_text(encoding="utf-8")))
    config = montar_config(q, json.loads((modelos / "conversion.config.json").read_text(encoding="utf-8")))
    manifesto["$schema"] = str(skill / "schemas" / "wx-inputs.schema.json")
    config["$schema"] = str(skill / "schemas" / "conversion-config.schema.json")

    wx = projeto / ".wx-migration"
    saida = [
        write_new(wx / "wx-inputs.manifest.json", json.dumps(manifesto, ensure_ascii=False, indent=2) + "\n"),
        write_new(wx / "conversion.config.json", json.dumps(config, ensure_ascii=False, indent=2) + "\n"),
        write_new(wx / "gaps.md", "# Lacunas (GAP-*)\n\n| id | escopo | severidade | artefato necessário | responsável | desbloqueio |\n| --- | --- | --- | --- | --- | --- |\n"),
        write_new(wx / "traceability.csv", (modelos / "traceability.csv").read_text(encoding="utf-8")),
    ]

    claude_md = (modelos / "CLAUDE.md").read_text(encoding="utf-8")
    if q.get("J_economia_de_tokens", {}).get("ativar") and q["J_economia_de_tokens"].get("instalar_estilo_no_claude_md", True):
        claude_md = claude_md.rstrip("\n") + "\n" + ESTILO_DE_RESPOSTA
    saida.append(write_new(projeto / "CLAUDE.md", claude_md))

    if q.get("F_estilo_impeccable", {}).get("ativar"):
        saida.append(write_new(projeto / "DESIGN.md", esboco_design(q)))

    for linha in saida:
        print(linha)
    print(f"modo={config['mode']} destino={config['target']['language'] or '(vazio)'} ui={config['fidelity']['ui']}")
    return 0


if __name__ == "__main__":
    try:
        sys.exit(main())
    except (OSError, ValueError, json.JSONDecodeError) as exc:
        print(f"erro: {exc}", file=sys.stderr)
        sys.exit(2)
