#!/usr/bin/env python3
"""Aplica as respostas do questionario (bloco 0 e letras A-J) ao espaco de trabalho .wx-migration/.

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
    # O perfil C# + WL_C# entra como framework para que o especialista de funcoes
    # padrao saiba consultar resources/wl-csharp/funcoes.json.
    if str(h.get("perfil", "")).lower() in {"csharp-wl", "c#-wl", "wl_c#"} and "WL_C#" not in frameworks:
        frameworks.append("WL_C#")
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
    if h.get("perfil") or i.get("perfil"):
        c["target"]["architecture"] = f"perfil backend {h.get('perfil') or '?'} / frontend {i.get('perfil') or '?'} (references/perfis-de-destino.md)"
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
    def sec(titulo: str, chave: str, campos: list[tuple[str, str]]) -> None:
        d = f.get(chave)
        if not d:
            linhas.extend([f"## {titulo}", "", "(sem resposta na letra F; o Impeccable pergunta antes de agir)", ""])
            return
        linhas.extend([f"## {titulo}", ""])
        for rotulo, k in campos:
            v = d.get(k)
            if isinstance(v, list):
                v = ", ".join(map(str, v)) or "(nenhum)"
            if isinstance(v, bool):
                v = "sim" if v else "não"
            linhas.append(f"- {rotulo}: {v if v not in (None, '') else '(pendente)'}")
        linhas.append("")

    sec("Interação e teclado (F2 → harden, polish)", "F2_teclado", [("Atalhos a preservar", "atalhos_a_preservar"), ("Enter avança campo", "enter_avanca_campo"), ("Ordem de tabulação", "ordem_de_tabulacao")])
    sec("Grids (F3 → shape, layout, audit)", "F3_grids", [("Linhas por tela", "linhas_por_tela"), ("Colunas fixas", "colunas_fixas"), ("Ordenar e filtrar por coluna", "ordenar_e_filtrar_por_coluna"), ("Edição na célula", "edicao_na_celula"), ("Totais no rodapé", "totais_no_rodape"), ("Exportar", "exportar"), ("Imprimir grade", "imprimir_grade")])
    sec("Formulários (F4 → harden, clarify)", "F4_formularios", [("Validação", "validacao"), ("Preservar mensagens do legado", "preservar_mensagens_do_legado"), ("Obrigatório marcado como", "obrigatorio_marcado_como"), ("Máscaras", "mascaras"), ("Autocompletar", "autocompletar")])
    sec("Números, datas e moeda (F5 → typeset, harden)", "F5_formatos", [("Locale", "locale"), ("Decimais de moeda", "decimais_moeda"), ("Decimais de quantidade", "decimais_quantidade"), ("Negativo", "negativo"), ("Fuso", "fuso")])
    sec("Relatórios e impressão (F6 → layout, harden)", "F6_impressao", [("Telas que imprimem", "telas_que_imprimem"), ("Papel", "papel"), ("PDF", "pdf"), ("Etiquetas", "etiquetas")])
    sec("Estados e erros (F7 → onboard, harden, critique)", "F7_estados", [("Vazio", "vazio"), ("Carregando", "carregando"), ("Sem permissão", "sem_permissao"), ("Offline", "offline"), ("Erro do servidor", "erro_do_servidor"), ("Confirmar ação destrutiva", "confirmar_destrutivo")])
    sec("Acessibilidade (F8 → audit, adapt)", "F8_acessibilidade", [("WCAG", "wcag"), ("Leitor de tela", "leitor_de_tela"), ("Daltonismo", "daltonismo"), ("Alvo de toque mínimo (px)", "toque_minimo_px")])
    ACOES = ["incluir", "alterar", "excluir", "gravar", "selecionar", "voltar", "cancelar", "duplicar"]
    v = f.get("F9_vocabulario_dos_botoes") or {}
    ic = f.get("F11_icones") or {}
    co = f.get("F12_cores_das_acoes") or {}
    if v or ic or co:
        linhas += ["## Botões: vocabulário, ícone e cor por ação (F9, F11, F12 → harden, polish)", "",
                   f"- Estilo dos rótulos: {v.get('estilo', '(pendente)')}; caixa: {v.get('caixa', '(pendente)')}",
                   f"- Ícones: {'sim' if ic.get('usar') else 'não'}" + (f", biblioteca {ic.get('biblioteca')}, {ic.get('tamanho_px')} px, {'com' if ic.get('com_texto') else 'sem'} texto" if ic.get('usar') else ""),
                   f"- Regra de cor: {co.get('regra', 'contorno; preenchimento só no hover')}", "",
                   "| Ação | Rótulo | Ícone | Cor | Contraste medido |", "| --- | --- | --- | --- | --- |"]
        for a in ACOES:
            linhas.append(f"| {a} | {(v.get('rotulos') or {}).get(a, '(pendente)')} | {(ic.get('por_acao') or {}).get(a, '—')} | {(co.get('por_acao') or {}).get(a, '(pendente)')} | (medir, mínimo 4,5:1) |")
        linhas.append("")
        msgs = v.get("mensagens") or {}
        if msgs:
            linhas += ["Mensagens padrão (texto exato, como o usuário definiu):", ""] + [f"- {k}: «{m}»" for k, m in msgs.items()] + [""]
        linhas += ["Regra: o rótulo é o que o usuário definiu, letra por letra; o agente não «melhora» texto de botão. Texto de mensagem do legado que exista no PDF de código tem precedência sobre o padrão acima.", ""]
    po = f.get("F10_posicao_dos_botoes") or {}
    if po:
        linhas += ["## Posição dos botões (F10 → layout, shape)", "",
                   f"- Barra da grade: {po.get('da_grade', '(pendente)')} da grade, alinhada à {po.get('alinhamento', '(pendente)')}",
                   f"- Barra dos campos: {po.get('dos_campos', '(pendente)')} dos campos, alinhada à {po.get('alinhamento', '(pendente)')}",
                   f"- Ordem na barra: {', '.join(po.get('ordem', [])) or '(pendente)'}",
                   f"- Gravar e cancelar: {po.get('gravar_e_cancelar', '(pendente)')}",
                   "- A mesma posição em todas as telas; barra que muda de lugar é defeito.", ""]
    fu = f.get("F13_fundo_das_telas") or {}
    if fu:
        linhas += ["## Fundo das telas (F13 → colorize, adapt)", "",
                   f"- Tipo: {fu.get('tipo', '(pendente)')}",
                   f"- Cor (claro): {fu.get('cor', '(pendente)')}; cor (escuro): {fu.get('cor_escuro', '(pendente)')}",
                   f"- Textura ou imagem: {fu.get('textura_ou_imagem') or 'nenhuma'}" + (f", opacidade {fu.get('opacidade_da_textura')}" if fu.get('textura_ou_imagem') else ""),
                   "- Textura nunca reduz o contraste do texto abaixo de 4,5:1; imagem de fundo só fora da área de dados.", ""]
    linhas += ["## Critério de pronto de uma tela", "", "Passou por `/impeccable polish` e `/impeccable audit`, atende as seções acima que a afetam, e foi aberta no navegador ao lado do screenshot do legado no mesmo estado.", ""]
    if f.get("observacao"):
        linhas += ["## Observação do usuário", "", f.get("observacao"), ""]
    return "\n".join(linhas)


def esboco_product(q: dict) -> str:
    f = q.get("F_estilo_impeccable", {})
    o = f.get("F1_operacao", {}) or {}
    p = q.get("projeto", {})
    return "\n".join([
        "# PRODUCT.md — esboço do questionário (letra F1)", "",
        "Preenchido por `aplicar_questionario.py`; `/impeccable init` completa e não sobrescreve.", "",
        f"- Produto: {p.get('nome') or '(pendente)'} ({', '.join(p.get('produtos', [])) or 'WX'}), convertido do legado WINDEV",
        f"- Modo Impeccable: **Operate** (ERP: o visitante completa tarefas; escaneabilidade e consistência acima de expressão)",
        f"- Quem opera: {o.get('perfil_do_usuario') or '(pendente)'}",
        f"- Horas por dia na tela: {o.get('horas_por_dia') or '(pendente)'}",
        f"- Ambiente: {o.get('ambiente') or '(pendente)'}",
        f"- Tela típica: {o.get('tela_tipica') or '(pendente)'}",
        f"- Plataformas: {', '.join(q.get('I_frontend', {}).get('plataformas', [])) or '(pendente)'}",
        "- Restrição que não muda: campos, textos, validações e fluxo vêm do legado; o visual pode mudar, o comportamento não.", "",
    ])


# ---------------------------------------------------------------------------
# Bloco 0: empresa e projeto. Vai para empresa.md, para o PMO e para entrega.json.
# ---------------------------------------------------------------------------

# Chaves que nunca podem carregar valor no questionario: a senha se configura
# na maquina (variavel de ambiente, gh auth, credential manager), e o arquivo
# guarda so o NOME dessa referencia. Regra do projeto: senha nunca em texto puro.
CHAVES_DE_SEGREDO = re.compile(r"(senha|password|passwd|token|secret|segredo|api_?key)$", re.IGNORECASE)


def procurar_segredos(obj, caminho: str = "") -> list[str]:
    """Devolve os caminhos das chaves de segredo que vieram com valor preenchido."""
    achados: list[str] = []
    if isinstance(obj, dict):
        for k, v in obj.items():
            aqui = f"{caminho}.{k}" if caminho else k
            if CHAVES_DE_SEGREDO.search(k) and not k.endswith("_ref") and v not in (None, "", [], {}):
                achados.append(aqui)
            achados += procurar_segredos(v, aqui)
    elif isinstance(obj, list):
        for i, v in enumerate(obj):
            achados += procurar_segredos(v, f"{caminho}[{i}]")
    return achados


def anexo_verificado(bloco: dict, raiz: Path) -> tuple[str, str]:
    """(status, caminho) de um anexo unico: provided so se o arquivo abrir de verdade."""
    arq = (bloco or {}).get("arquivo") or ""
    st = (bloco or {}).get("status", "missing")
    if st == "not_applicable":
        return st, ""
    if not arq:
        return "missing", ""
    if not (raiz / arq).is_file():
        raise ValueError(f"anexo {arq!r} marcado como {st} mas nao existe em {raiz}")
    return "provided", arq


def _tabela(cabecalho: list[str], linhas: list[list[str]]) -> list[str]:
    if not linhas:
        return ["(nao informado)", ""]
    return ["| " + " | ".join(cabecalho) + " |", "| " + " | ".join("---" for _ in cabecalho) + " |"] + ["| " + " | ".join(str(c) for c in l) + " |" for l in linhas] + [""]


def esboco_empresa(q: dict, raiz: Path) -> str:
    e = q.get("0_empresa_e_projeto", {}) or {}
    sh = e.get("0_1_softhouse", {}) or {}
    en = e.get("0_3_endereco", {}) or {}
    desc = e.get("0_8_descricao_do_software", {}) or {}
    st_emp, arq_emp = anexo_verificado(e.get("0_4_logotipo_da_empresa"), raiz)
    st_sw, arq_sw = anexo_verificado(e.get("0_5_logotipo_do_software"), raiz)
    endereco = ", ".join(x for x in [
        " ".join(x for x in [en.get("logradouro", ""), en.get("numero", "")] if x), en.get("complemento", ""), en.get("bairro", ""),
        " - ".join(x for x in [en.get("cidade", ""), en.get("uf", "")] if x), en.get("cep", ""), en.get("pais", "")] if x)
    linhas = [
        "# Empresa e projeto (bloco 0 do questionario)", "",
        "Preenchido por `aplicar_questionario.py` a partir de `.wx-migration/questionario.json`. Logotipo so conta como `provided` depois de o arquivo abrir.", "",
        "## Softhouse", "",
        f"- Razao social: {sh.get('razao_social') or '(pendente)'}",
        f"- Nome fantasia: {sh.get('nome_fantasia') or '(pendente)'}",
        f"- CNPJ: {sh.get('cnpj') or '(nao informado)'}",
        f"- Endereco: {endereco or '(pendente)'}",
        f"- Logotipo da empresa: {arq_emp or '-'} ({st_emp})",
        f"- Logotipo do software: {arq_sw or '-'} ({st_sw})", "",
        "## Solicitacao", "", sh.get("solicitacao") or "(pendente)", "",
        "## Diretores", "",
        *_tabela(["nome", "cargo", "contato"], [[d.get("nome", ""), d.get("cargo", ""), d.get("email", "")] for d in e.get("0_2_diretores", []) or []]),
        "## Finalidade", "", e.get("0_6_finalidade") or "(pendente)", "",
        "## Objetivos", "", *([f"{i}. {o}" for i, o in enumerate(e.get("0_7_objetivos", []) or [], 1)] or ["(pendente)"]), "",
        "## Descricao do software", "", desc.get("descricao") or "(pendente)", "",
        "Recursos:", "", *([f"- {r}" for r in desc.get("recursos", []) or []] or ["- (pendente)"]), "",
        f"Modulos: {', '.join(desc.get('modulos', []) or []) or '(pendente)'}", "",
        "## Pessoal envolvido", "",
        *_tabela(["nome", "papel", "empresa", "contato"], [[x.get("nome", ""), x.get("papel", ""), x.get("empresa", ""), x.get("contato", "")] for x in e.get("0_14_pessoal_envolvido", []) or []]),
        "## Onde esta o resto", "",
        "- Organograma, fluxograma, cronograma, orcamento e riscos: `pmo/` (o PMO le de la).",
        "- Repositorio de destino e diretorio: `entrega.json` (sem senha: so o nome da credencial).", "",
    ]
    return "\n".join(linhas)


def esboco_organograma(e: dict, raiz: Path) -> str:
    o = e.get("0_9_organograma", {}) or {}
    st, arq = anexo_verificado(o, raiz)
    linhas = ["# Organograma do projeto", "", f"Fonte: {'arquivo `' + arq + '` na raiz de evidencias' if arq else 'posicoes informadas no questionario'} ({st}).", ""]
    linhas += _tabela(["papel", "nome", "responde a"], [[p.get("papel", ""), p.get("nome", ""), p.get("responde_a", "")] for p in o.get("posicoes", []) or []])
    return "\n".join(linhas)


def esboco_fluxograma(e: dict, raiz: Path) -> str:
    f = e.get("0_10_fluxograma", {}) or {}
    st, arq = anexo_verificado(f, raiz)
    etapas = f.get("etapas", []) or []
    linhas = ["# Fluxograma do processo principal", "", f"Fonte: {'arquivo `' + arq + '`' if arq else 'etapas informadas no questionario'} ({st}).", ""]
    if etapas:
        linhas += ["```mermaid", "flowchart LR"] + [f"  e{i}[{et}]" for i, et in enumerate(etapas, 1)] + [f"  e{i} --> e{i+1}" for i in range(1, len(etapas))] + ["```", ""]
    else:
        linhas += ["(nao informado)", ""]
    return "\n".join(linhas)


def esboco_cronograma(e: dict) -> str:
    c = e.get("0_11_cronograma", {}) or {}
    linhas = ["# Cronograma", "", f"- Inicio: {c.get('inicio') or '(pendente)'}", f"- **Prazo final de entrega: {c.get('prazo_final') or '(pendente)'}**", "", "## Marcos", ""]
    linhas += _tabela(["marco", "data", "gate"], [[m.get("marco", ""), m.get("data", ""), m.get("gate", "")] for m in c.get("marcos", []) or []])
    linhas += ["O `pmo.py status` compara o prazo final com a data de hoje; marco com gate preenche `previsto_para` em `plano.json` no `pmo.py iniciar`.", ""]
    return "\n".join(linhas)


def riscos_iniciais(e: dict) -> str:
    """Mesmo cabecalho do pmo.py iniciar, ja com as linhas do questionario (RSK-001...)."""
    rows = [[f"RSK-{i:03d}", r.get("risco", ""), r.get("probabilidade", ""), r.get("impacto", ""), r.get("resposta", ""), r.get("dono", ""), date.today().isoformat()] for i, r in enumerate(e.get("0_13_riscos", []) or [], 1)]
    cab = "| id | risco | prob. | impacto | resposta | dono | data |\n| --- | --- | --- | --- | --- | --- | --- |\n"
    corpo = "".join("| " + " | ".join(str(c) for c in r) + " |\n" for r in rows)
    return ("# RAID da conversao\n\n## Riscos\n\n" + cab + corpo + "\n"
            "## Premissas\n\n| id | premissa | quem confirma | ate |\n| --- | --- | --- | --- |\n\n"
            "## Issues\n\n| id | o que aconteceu | efeito | tratamento | dono | data |\n| --- | --- | --- | --- | --- | --- |\n\n"
            "## Dependencias\n\n| id | dependemos de | para | quando | dono |\n| --- | --- | --- | --- | --- |\n")


def montar_projeto_pmo(e: dict) -> dict:
    c = e.get("0_11_cronograma", {}) or {}
    o = e.get("0_12_orcamento", {}) or {}
    sh = e.get("0_1_softhouse", {}) or {}
    return {
        "softhouse": sh.get("nome_fantasia") or sh.get("razao_social") or "",
        "solicitacao": sh.get("solicitacao", ""),
        "inicio": c.get("inicio", ""),
        "prazo_final": c.get("prazo_final", ""),
        "marcos": c.get("marcos", []) or [],
        "orcamento_financeiro": {"valor": o.get("valor"), "moeda": o.get("moeda", "BRL"), "base": o.get("base", ""), "aprovado_por": o.get("aprovado_por", "")},
        "riscos_iniciais": len(e.get("0_13_riscos", []) or []),
        "pessoas": len(e.get("0_14_pessoal_envolvido", []) or []),
        "fonte": "questionario.json bloco 0",
    }


def montar_entrega(e: dict) -> dict:
    g = e.get("0_15_github", {}) or {}
    ref = g.get("credencial_ref", "")
    if ref and not re.fullmatch(r"[A-Za-z_][A-Za-z0-9_./:-]*", ref):
        raise ValueError(f"credencial_ref {ref!r} nao parece nome de variavel ou de segredo")
    return {
        "github": {"url": g.get("url", ""), "branch": g.get("branch", "main"), "usuario": g.get("usuario", "")},
        "credencial_ref": ref,
        "senha": "NUNCA AQUI: configure a credencial no ambiente com o nome em credencial_ref",
        "diretorio_destino": g.get("diretorio_destino", ""),
    }


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
    vazados = procurar_segredos(q)
    if vazados:
        raise ValueError("senha ou token em texto puro no questionario (" + ", ".join(vazados) + "); guarde so o nome da credencial em credencial_ref e apague o valor")
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
        saida.append(write_new(projeto / "PRODUCT.md", esboco_product(q)))

    e = q.get("0_empresa_e_projeto")
    if e:
        raiz = (projeto / q.get("projeto", {}).get("raiz_de_evidencias", "./inputs")).resolve()
        saida += [
            write_new(wx / "empresa.md", esboco_empresa(q, raiz)),
            write_new(wx / "pmo" / "projeto.json", json.dumps(montar_projeto_pmo(e), ensure_ascii=False, indent=2) + "\n"),
            write_new(wx / "pmo" / "organograma.md", esboco_organograma(e, raiz)),
            write_new(wx / "pmo" / "fluxograma.md", esboco_fluxograma(e, raiz)),
            write_new(wx / "pmo" / "cronograma.md", esboco_cronograma(e)),
            write_new(wx / "pmo" / "riscos.md", riscos_iniciais(e)),
            write_new(wx / "entrega.json", json.dumps(montar_entrega(e), ensure_ascii=False, indent=2) + "\n"),
        ]

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
