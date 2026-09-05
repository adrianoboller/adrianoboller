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
import shlex
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
        if not isinstance(s, dict) or not all(s.get(c) for c in ("arquivo", "tela", "estado")):
            raise ValueError("screenshots.json: cada item precisa de arquivo, tela e estado")
        arq = raiz / "screenshots" / s["arquivo"]
        if arq.is_file():
            itens.append({"path": f"screenshots/{s['arquivo']}", "screen_or_report": s["tela"], "state": s["estado"], "platform": s.get("plataforma", "WINDEV")})
    return itens


EXTENSOES_DE_FONTE = {".php", ".inc", ".c", ".h", ".cpp", ".hpp", ".cc", ".rs", ".py",
                      ".js", ".ts", ".java", ".cs", ".go", ".rb", ".pas", ".cbl", ".clw"}
MAX_ITENS_DE_FONTE = 200


def itens_de_codigo_fonte(p: dict, raiz_evidencias: Path | None) -> list[dict]:
    """Lista o codigo-fonte do legado nao-WX, que e a evidencia central dele.

    Vale para PHP (legado_php.raiz) e para qualquer outra linguagem
    (legado_outra.raiz): C, C++, Clarion, COBOL. O caminho gravado e relativo a
    raiz de evidencias, como todo item do manifesto.
    """
    if raiz_evidencias is None:
        return []
    raizes = []
    for chave in ("legado_php", "legado_outra"):
        bloco = p.get(chave) or {}
        if chave == "legado_php" and not bloco.get("tem"):
            continue
        if chave == "legado_outra" and not (bloco.get("linguagem") or "").strip():
            continue
        alvo = (bloco.get("raiz") or "").strip()
        if alvo:
            raizes.append(alvo)
    itens: list[dict] = []
    for alvo in raizes:
        base = Path(alvo)
        if not base.is_absolute():
            # a raiz do legado e relativa a raiz do PROJETO, e a de evidencias
            # costuma ser ./inputs: resolver contra as duas cobre os dois jeitos
            candidatos = [raiz_evidencias.parent / base, raiz_evidencias / base.name]
        else:
            candidatos = [base]
        pasta = next((c for c in candidatos if c.is_dir()), None)
        if pasta is None:
            continue
        for arq in sorted(pasta.rglob("*")):
            if not arq.is_file() or arq.suffix.lower() not in EXTENSOES_DE_FONTE:
                continue
            try:
                rel = arq.resolve().relative_to(raiz_evidencias.resolve())
            except ValueError:
                continue
            itens.append({"path": str(rel).replace(os.sep, "/"),
                          "language": arq.suffix.lstrip(".").lower(),
                          "lines": len(arq.read_text(encoding="utf-8", errors="replace").splitlines())})
            if len(itens) >= MAX_ITENS_DE_FONTE:
                return itens
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

    # Dados de amostra e resultados esperados (golden master): so entram se os
    # arquivos existirem em dados-de-amostra/. Achado por uma sessao real que
    # viu os arquivos e o manifesto dizendo «missing».
    amostra = (RAIZ_DE_EVIDENCIAS / "dados-de-amostra") if RAIZ_DE_EVIDENCIAS else None
    if amostra and amostra.is_dir():
        arqs = sorted(f for f in amostra.iterdir() if f.is_file())
        if arqs:
            a["sample_data_and_expected_results"]["status"] = "provided"
            a["sample_data_and_expected_results"]["items"] = [{"path": f"dados-de-amostra/{f.name}", "description": "resultado esperado do legado (golden master)" if "esperad" in f.name else "dados sinteticos ou anonimizados de amostra"} for f in arqs]
            a["sample_data_and_expected_results"]["notes"] = "Lidos de dados-de-amostra/; somente dados sinteticos ou anonimizados."

    # Legado que nao e WX: o codigo-fonte E a evidencia, nao um PDF de
    # documentacao. Sem isto o manifesto dizia native_project_sources
    # "missing" com o fonte inteiro ali do lado, e o G0 bloqueava um projeto
    # PHP por falta de PDF que ele nunca teve.
    itens_fonte = itens_de_codigo_fonte(p, RAIZ_DE_EVIDENCIAS)
    if itens_fonte:
        a["native_project_sources"]["status"] = "provided"
        a["native_project_sources"]["items"] = itens_fonte
        a["native_project_sources"]["notes"] = (
            "Codigo-fonte do legado lido de " + str(p.get("legado_php", {}).get("raiz")
            or p.get("legado_outra", {}).get("raiz") or "") + "; e a evidencia central deste projeto.")

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
    # F0: a tela principal do legado como modelo visual. So entra depois de aberta.
    t0 = f.get("F0_tela_modelo", {}) or {}
    if t0.get("status") not in (None, "missing", "not_applicable") or t0.get("arquivos"):
        raiz = Path(f.get("_raiz_de_evidencias", "."))
        linhas += ["", "## Tela modelo (F0)", "", "A tela principal do projeto WX e a referencia visual de toda tela nova: o Impeccable `critique` compara com ela antes de `polish`.", ""]
        linhas += ["| Tela | Papel | Arquivo | Estado |", "| --- | --- | --- | --- |"]
        for a0 in t0.get("arquivos", []) or []:
            arq = a0.get("arquivo", "")
            if arq and not (raiz / arq).is_file():
                raise ValueError(f"tela modelo {arq!r} nao existe em {raiz}")
            linhas.append(f"| {a0.get('tela','')} | {a0.get('papel','')} | {arq} | {'provided' if arq else 'missing'} |")
        linhas += ["", "Preservar:", ""] + ([f"- {x}" for x in t0.get("o_que_preservar", []) or []] or ["- (nada informado)"])
        linhas += ["", "Mudar:", ""] + ([f"- {x}" for x in t0.get("o_que_mudar", []) or []] or ["- (nada informado)"])
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
# A palavra em QUALQUER posicao da chave (senha_do_banco, token_da_api), nao so
# no fim; e os valores tambem sao varridos contra formatos conhecidos de token.
CHAVES_DE_SEGREDO = re.compile(r"(?:^|_|\b)(senha|password|passwd|token|secret|segredo|api_?key|pwd)(?:$|_|\b)", re.IGNORECASE)
VALORES_DE_SEGREDO = re.compile(r"(ghp_[A-Za-z0-9]{20,}|github_pat_[A-Za-z0-9_]{20,}|sk-[A-Za-z0-9_-]{20,}|AKIA[0-9A-Z]{16}|xox[baprs]-[A-Za-z0-9-]{10,}|-----BEGIN [A-Z ]*PRIVATE KEY-----|eyJ[A-Za-z0-9_-]{20,}\.[A-Za-z0-9_-]{20,}\.[A-Za-z0-9_-]{10,})")


def procurar_segredos(obj, caminho: str = "") -> list[str]:
    """Devolve os caminhos de chaves de segredo com valor, e de valores que parecem token."""
    achados: list[str] = []
    if isinstance(obj, dict):
        for k, v in obj.items():
            aqui = f"{caminho}.{k}" if caminho else k
            if CHAVES_DE_SEGREDO.search(k) and not k.endswith("_ref") and v not in (None, "", [], {}, False):
                achados.append(aqui)
            achados += procurar_segredos(v, aqui)
    elif isinstance(obj, list):
        for i, v in enumerate(obj):
            achados += procurar_segredos(v, f"{caminho}[{i}]")
    elif isinstance(obj, str) and VALORES_DE_SEGREDO.search(obj):
        achados.append(caminho + " (valor com formato de token)")
    return achados


# ---------------------------------------------------------------------------
# Validacao do que vira bash, SQL ou YAML. Valor do questionario e escrito pelo
# modelo a partir de anexos e conversa; e vetor de injecao. A regra e recusar
# antes de gravar, nao escapar depois: identificador e identificador.
# ---------------------------------------------------------------------------
RX_IDENT = re.compile(r"^[A-Za-z_][A-Za-z0-9_]{0,62}$")
RX_VAR = re.compile(r"^[A-Za-z_][A-Za-z0-9_]*$")
RX_HOST = re.compile(r"^[A-Za-z0-9]([A-Za-z0-9.-]{0,252}[A-Za-z0-9])?$")
RX_BRANCH = re.compile(r"^[A-Za-z0-9][A-Za-z0-9._/-]{0,120}$")
RX_URL_GITHUB = re.compile(r"^https://github\.com/[A-Za-z0-9_.-]+/[A-Za-z0-9_.-]+(\.git)?/?$")
RX_CAMINHO = re.compile(r"^[A-Za-z0-9_./~-][A-Za-z0-9_./~ -]{0,200}$")
RX_VERSAO = re.compile(r"^[A-Za-z0-9._-]{1,40}$")


def _confere(valor, rx: re.Pattern, onde: str, opcional: bool = True) -> None:
    if valor in (None, ""):
        if opcional:
            return
        raise ValueError(f"{onde}: obrigatorio")
    if not isinstance(valor, str) or not rx.fullmatch(valor):
        raise ValueError(f"{onde}: valor {str(valor)[:40]!r} nao e aceito (so caracteres seguros)")


def _porta(valor, onde: str) -> int:
    if valor in (None, ""):
        return 0
    if isinstance(valor, bool) or not isinstance(valor, int) or not 1 <= valor <= 65535:
        raise ValueError(f"{onde}: porta {valor!r} invalida (inteiro de 1 a 65535)")
    return valor


def validar_entradas(q: dict) -> None:
    """Tudo que entra em shell, SQL, YAML ou JSON de configuracao passa por aqui antes."""
    e = q.get("0_empresa_e_projeto") or {}
    g = e.get("0_15_github") or {}
    _confere(g.get("url"), RX_URL_GITHUB, "0.15 url")
    _confere(g.get("branch"), RX_BRANCH, "0.15 branch")
    _confere(g.get("usuario"), RX_IDENT, "0.15 usuario")
    _confere(g.get("credencial_ref"), RX_VAR, "0.15 credencial_ref")
    _confere(g.get("diretorio_destino"), RX_CAMINHO, "0.15 diretorio_destino")
    # Legado E/OU: um ou mais produtos. WLanguage e o principal e nunca sai do
    # plugin; php e outra entram ao lado, ou sozinhos.
    p = q.get("projeto") or {}
    prods = [str(x).strip().lower() for x in (p.get("produtos") or []) if str(x).strip()]
    desconhecidos = [x for x in prods if x not in PRODUTOS_LEGADO]
    if desconhecidos:
        raise ValueError(f"projeto.produtos {desconhecidos} desconhecido(s); aceitos: {', '.join(sorted(PRODUTOS_LEGADO))}")
    if prods and p.get("principal") and p["principal"].strip().lower() not in prods:
        raise ValueError(f"projeto.principal {p['principal']!r} nao esta em produtos {prods}")
    if "outra" in prods and not ((p.get("legado_outra") or {}).get("linguagem") or "").strip():
        raise ValueError("projeto.produtos inclui 'outra': preencha projeto.legado_outra.linguagem")
    if (p.get("legado_php") or {}).get("tem") and "php" not in prods:
        raise ValueError("projeto.legado_php.tem = true exige 'php' em projeto.produtos")
    k = q.get("K_ambiente") or {}
    k8 = k.get("K8_backup_e_replicacao") or {}
    if k8.get("ativar"):
        b = k8.get("backup") or {}
        rep = k8.get("replicacao") or {}
        for campo, aceitos in (("ferramenta", FERRAMENTAS_BACKUP), ("tipo", {"completo", "completo+incremental", "continuo-wal"}),
                               ("frequencia", {"horaria", "diaria", "semanal"}), ("destino", DESTINOS_BACKUP),
                               ("teste_de_restauracao", {"semanal", "mensal", "trimestral", "nunca"})):
            v = str(b.get(campo) or "").strip().lower()
            if v and v not in aceitos:
                raise ValueError(f"K8 backup.{campo} {v!r} desconhecido (aceitos: {', '.join(sorted(aceitos))})")
        for campo in ("chave_ref", "credencial_destino_ref"):
            _confere(b.get(campo), RX_VAR, f"K8 backup.{campo}")
        _confere(((b.get("alerta_em_falha") or {}).get("destino_ref")), RX_VAR, "K8 backup.alerta_em_falha.destino_ref")
        _confere(b.get("caminho_ou_bucket"), RX_CAMINHO, "K8 backup.caminho_ou_bucket")
        if b.get("cifrado") and not (b.get("chave_ref") or "").strip():
            raise ValueError("K8: backup cifrado exige chave_ref (o NOME da variavel, nunca a chave)")
        for campo in ("retencao_dias", "retencao_mensal_meses"):
            v = b.get(campo)
            if v is not None and (not isinstance(v, int) or isinstance(v, bool) or not 0 < v <= 3650):
                raise ValueError(f"K8 backup.{campo} tem de ser inteiro entre 1 e 3650")
        if rep.get("ativar"):
            if str(rep.get("tipo") or "").strip().lower() not in TIPOS_REPLICACAO:
                raise ValueError(f"K8 replicacao.tipo {rep.get('tipo')!r} desconhecido (aceitos: {', '.join(sorted(TIPOS_REPLICACAO))})")
            if str(rep.get("failover") or "manual").strip().lower() not in {"manual", "automatico"}:
                raise ValueError("K8 replicacao.failover: manual | automatico")
            for r in rep.get("replicas") or []:
                _confere(r.get("nome"), RX_HOST, "K8 replica.nome", opcional=False)
                _confere(r.get("host"), RX_HOST, "K8 replica.host", opcional=False)
                if str(r.get("papel") or "").strip().lower() not in {"primaria", "leitura", "espera"}:
                    raise ValueError(f"K8 replica {r.get('nome')!r}: papel primaria | leitura | espera")
            if rep.get("failover") == "automatico" and str(rep.get("ferramenta_de_failover") or "nenhum").lower() == "nenhum":
                raise ValueError("K8: failover automatico sem ferramenta_de_failover nao existe; escolha patroni, repmgr ou orchestrator")
        obj = k8.get("objetivos") or {}
        for campo in ("rpo_minutos", "rto_minutos"):
            v = obj.get(campo)
            if v is not None and (not isinstance(v, int) or isinstance(v, bool) or v < 0):
                raise ValueError(f"K8 objetivos.{campo} tem de ser inteiro em minutos")
        if isinstance(obj.get("rpo_minutos"), int) and str(b.get("frequencia") or "").lower() == "diaria" and obj["rpo_minutos"] < 1440 \
                and str(b.get("tipo") or "").lower() != "continuo-wal":
            raise ValueError(f"K8: RPO de {obj['rpo_minutos']} min nao cabe em backup diario; use continuo-wal (WAL/binlog) ou aumente o RPO")
    k0 = k.get("K0_privilegios") or {}
    if k0.get("modo") not in (None, "", "sudo", "root", "nenhum"):
        raise ValueError(f"K0 modo {k0.get('modo')!r} desconhecido (sudo | root | nenhum)")
    _confere(k0.get("usuario_root"), RX_IDENT, "K0 usuario_root")
    k1 = k.get("K1_rust") or {}
    _confere(k1.get("versao_minima"), RX_VERSAO, "K1 versao_minima")
    _confere(k1.get("canal"), RX_IDENT, "K1 canal")
    for c in list(k1.get("componentes") or []) + list(k1.get("targets") or []):
        _confere(c, re.compile(r"^[A-Za-z0-9_-]{1,64}$"), "K1 componente/target")
    for chave in ("K2_postgresql", "K3_mysql", "K4_mariadb"):
        kx = k.get(chave) or {}
        _confere(kx.get("versao"), RX_VERSAO, f"{chave} versao")
        _confere(kx.get("host"), RX_HOST, f"{chave} host")
        _porta(kx.get("porta"), f"{chave} porta")
        _confere(kx.get("banco"), RX_IDENT, f"{chave} banco")
        _confere(kx.get("superusuario"), RX_IDENT, f"{chave} superusuario")
        _confere(kx.get("senha_ref"), RX_VAR, f"{chave} senha_ref")
        for pp in kx.get("papeis") or []:
            _confere(pp.get("nome"), RX_IDENT, f"{chave} papel nome", opcional=False)
            _confere(pp.get("senha_ref"), RX_VAR, f"{chave} papel {pp.get('nome')} senha_ref", opcional=False)
    k5 = k.get("K5_supabase") or {}
    _confere(k5.get("anon_key_ref"), RX_VAR, "K5 anon_key_ref")
    _confere(k5.get("service_role_ref"), RX_VAR, "K5 service_role_ref")
    _confere(k5.get("projeto_ref"), re.compile(r"^[A-Za-z0-9-]{1,64}$"), "K5 projeto_ref")
    k6 = k.get("K6_github") or {}
    _confere(k6.get("remote"), RX_IDENT, "K6 remote")
    _confere(k6.get("branch_principal"), RX_BRANCH, "K6 branch_principal")
    if k6.get("visibilidade") not in (None, "", "private", "public", "internal"):
        raise ValueError("K6 visibilidade: private | public | internal")
    k7 = k.get("K7_n8n") or {}
    _confere(k7.get("versao"), RX_VERSAO, "K7 versao")
    _confere(k7.get("host"), RX_HOST, "K7 host")
    _porta(k7.get("porta"), "K7 porta")
    _confere(k7.get("url_publica"), re.compile(r"^https?://[A-Za-z0-9.-]+(:\d+)?(/[A-Za-z0-9._/-]*)?$"), "K7 url_publica")
    _confere(k7.get("timezone"), re.compile(r"^[A-Za-z_]+(/[A-Za-z_+-]+)*$"), "K7 timezone")
    _confere(k7.get("encryption_key_ref"), RX_VAR, "K7 encryption_key_ref")
    bd = k7.get("banco") or {}
    _confere(bd.get("nome"), RX_IDENT, "K7 banco nome"); _confere(bd.get("usuario"), RX_IDENT, "K7 banco usuario"); _confere(bd.get("senha_ref"), RX_VAR, "K7 banco senha_ref")
    adm = k7.get("admin") or {}
    _confere(adm.get("senha_ref"), RX_VAR, "K7 admin senha_ref")
    _confere(adm.get("email"), re.compile(r"^[A-Za-z0-9._%+-]+@[A-Za-z0-9.-]+$"), "K7 admin email")
    itg = k7.get("integracao") or {}
    _confere(itg.get("api_token_ref"), RX_VAR, "K7 api_token_ref"); _confere(itg.get("api_key_ref"), RX_VAR, "K7 api_key_ref")
    l = q.get("L_contexto_e_implantacao") or {}
    imp = l.get("L3_implantacao") or {}
    _porta(imp.get("porta"), "L3 porta")
    _confere(imp.get("dominio"), RX_HOST, "L3 dominio")
    _confere(imp.get("healthcheck"), re.compile(r"^/[A-Za-z0-9._/-]*$"), "L3 healthcheck")
    _confere(imp.get("pasta_de_saida"), RX_CAMINHO, "L3 pasta_de_saida")
    for v in imp.get("variaveis_de_ambiente") or []:
        _confere(v, RX_VAR, "L3 variavel de ambiente", opcional=False)
    l4 = l.get("L4_hooks_do_projeto") or {}
    for c in (l4.get("comando_de_teste"), l4.get("comando_de_lint")):
        if c and ("\n" in c or "\r" in c):
            raise ValueError("L4: comando com quebra de linha nao e aceito")
    raiz = (q.get("projeto") or {}).get("raiz_de_evidencias")
    _confere(raiz, RX_CAMINHO, "projeto.raiz_de_evidencias")


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
    if ref and not RX_VAR.fullmatch(ref):
        raise ValueError(f"credencial_ref {ref!r} nao e nome de variavel de ambiente")
    return {
        "github": {"url": g.get("url", ""), "branch": g.get("branch", "main"), "usuario": g.get("usuario", "")},
        "credencial_ref": ref,
        "senha": "NUNCA AQUI: configure a credencial no ambiente com o nome em credencial_ref",
        "diretorio_destino": g.get("diretorio_destino", ""),
    }


# ---------------------------------------------------------------------------
# Processo de conversao: o que cada peca do WX vira no perfil escolhido, e a
# estrategia. Primeira versao do que o G3 detalha; a tabela completa esta em
# references/perfis-de-destino.md.
# ---------------------------------------------------------------------------

MAPA_BACKEND = {
    "rust": ("Rust", ["funcoes em modulos por dominio, tipos fortes", "struct + impl, traits so para heranca usada", "esquema PostgreSQL migrado por script; sqlx/diesel", "repositorio por arquivo HFSQL com SQL explicito", "uma funcao por query .WDR", "API por caso de uso", "gerador de PDF proprio, comparado pagina a pagina", "std e crates mapeadas no inventario"]),
    "python": ("Python", ["funcoes em pacotes por dominio, type hints e pydantic", "classes e dataclasses", "esquema PostgreSQL; SQLAlchemy", "repositorio por arquivo HFSQL; sessao da ORM", "uma funcao por query .WDR", "API por caso de uso", "ReportLab ou WeasyPrint", "biblioteca padrao"]),
    "csharp-wl": ("C# + WL_C#", ["metodos estaticos com o mesmo nome da funcao WLanguage quando existe na WL_C#", "classes C# quase um para um", "esquema PostgreSQL ou SQL Server; Entity Framework (HFSQL nao esta na WL_C#)", "repositorio por arquivo; leitura por chave vira LINQ", "uma funcao por query .WDR", "API por caso de uso ou Blazor", "QuestPDF ou Report Viewer", "WL_C# com o mesmo nome (Left, DateSys, fFileExist...)"]),
    "go": ("Go", ["funcoes por dominio", "structs", "esquema PostgreSQL; ORM da pilha", "repositorio por arquivo", "uma funcao por query", "API por caso de uso", "biblioteca da pilha", "biblioteca padrao"]),
    "java": ("Java", ["servicos por dominio", "classes", "esquema PostgreSQL; JPA", "repositorio por arquivo", "uma funcao por query", "API por caso de uso", "JasperReports", "biblioteca padrao"]),
    "php": ("PHP 8.3", ["metodos de servico por dominio, strict_types em todo arquivo", "classes PHP; heranca so a que existe", "esquema PostgreSQL ou MySQL por migrations", "repositorio por entidade, sempre com parametro ligado", "metodo de repositorio com SQL explicito", "rota + controller por caso de uso", "template renderizado a PDF, comparado pagina a pagina", "biblioteca padrao e Composer com versao fixada"]),
    "node": ("Node", ["servicos por dominio em TypeScript", "classes", "esquema PostgreSQL; Prisma ou TypeORM", "repositorio por arquivo", "uma funcao por query", "API por caso de uso", "biblioteca da pilha", "biblioteca padrao"]),
}
PECAS = ["Procedures globais e locais", "Classes WLanguage", "Analise HFSQL", "HReadSeek*/HAdd/HModify e navegacao", "Queries .WDR", "Janelas e paginas", "Relatorios .WDE", "Funcoes de string, data, arquivo, JSON"]
ESTRATEGIAS = {
    "traducao-assistida": "cada procedure vira uma funcao no destino, na mesma ordem, com a WL_C# ou um mapa de funcoes; regra preservada literalmente",
    "reescrita-guiada": "o inventario extrai as regras BR-* e o codigo novo nasce delas, nao do codigo velho; o golden master e a unica prova de igualdade",
    "estrangulamento": "o legado continua no ar e cada modulo migra por vez atras de uma fachada; usuarios mudam de tela aos poucos",
    "ondas": "tudo e convertido por ondas (G5) e a virada acontece de uma vez no G7, com paralelo antes",
}


PRODUTOS_LEGADO = {"windev", "webdev", "windev-mobile", "php", "outra"}
# WLanguage e o caso principal do plugin e nao sai dele: converter WINDEV,
# WEBDEV e WINDEV Mobile para outra linguagem e a razao de o plugin existir.
# PHP e "outra" entram ao lado (E/OU), nunca no lugar.
PRODUTOS_WX = {"windev", "webdev", "windev-mobile"}


def mapa_do_perfil(perfil: str, linguagem: str) -> tuple[str, list[str]]:
    """O que cada peca vira no perfil. Perfil desconhecido ou 'outra' cai no
    mapa generico, com o nome da linguagem que o usuario deu: destino livre nao
    pode travar o questionario."""
    if perfil in MAPA_BACKEND:
        return MAPA_BACKEND[perfil]
    nome = (linguagem or perfil or "destino escolhido").strip()
    return (nome, [f"funcoes ou metodos por dominio em {nome}", f"classes ou o equivalente de {nome}; heranca so a que existe",
                   "esquema do banco migrado por script, com as chaves e ligacoes da analise",
                   "repositorio por arquivo HFSQL, com a consulta explicita", "uma funcao por query .WDR",
                   "API ou tela por caso de uso", "gerador de relatorio comparado pagina a pagina",
                   "biblioteca padrao da linguagem, mapeada no inventario"])


def esboco_processo(q: dict) -> str:
    h = q.get("H_backend", {}) or {}
    i = q.get("I_frontend", {}) or {}
    ph = h.get("processo", {}) or {}
    pi = i.get("processo", {}) or {}
    perfil = str(h.get("perfil", "")).lower()
    nome, colunas = mapa_do_perfil(perfil, h.get("linguagem") or "")
    est = ph.get("estrategia", "")
    if est and est not in ESTRATEGIAS:
        raise ValueError(f"estrategia {est!r} desconhecida (aceitas: {', '.join(ESTRATEGIAS)})")
    est_i = pi.get("estrategia", "")
    if est_i and est_i not in ESTRATEGIAS:
        raise ValueError(f"estrategia do frontend {est_i!r} desconhecida (aceitas: {', '.join(ESTRATEGIAS)})")
    linhas = [
        "# Processo de conversao (letras H e I do questionario)", "",
        "Primeira versao, gerada por `aplicar_questionario.py`; o G3 detalha e a `DEC-0001` fecha. Tabela completa em `references/perfis-de-destino.md`.", "",
        f"## Backend: {nome} ({h.get('framework') or 'framework a definir'}, {h.get('banco') or 'banco a definir'})", "",
        f"- Estrategia: **{est or '(pendente)'}**" + (f" — {ESTRATEGIAS[est]}" if est else ""),
        f"- Processo mostrado ao usuario: {ph.get('mostrado') or 'nenhuma'} opcao; mapeamento confirmado: {'sim' if ph.get('mapeamento_confirmado') else 'nao'}",
        f"- O que o usuario quer diferente: {ph.get('quer_diferente') or 'nada'}", "",
        "| Peca do legado | Vira | Gate |", "| --- | --- | --- |",
    ]
    gates = ["G5", "G5", "G3", "G4", "G4", "G4", "G5", "G1"]
    linhas += [f"| {p} | {c} | {g} |" for p, c, g in zip(PECAS, colunas, gates)]
    linhas += ["",
        f"## Frontend: {i.get('linguagem') or i.get('perfil') or '(perfil nao escolhido)'} ({i.get('framework') or 'framework a definir'})", "",
        f"- Estrategia: **{est_i or '(pendente)'}**" + (f" — {ESTRATEGIAS[est_i]}" if est_i else ""),
        f"- Ritmo: {pi.get('telas_por_vez') or 'tela a tela'}; plataformas: {', '.join(i.get('plataformas', []) or []) or '(pendente)'}",
        f"- O que o usuario quer diferente: {pi.get('quer_diferente') or 'nada'}", "",
        "| Peca do legado | Vira | Gate |", "| --- | --- | --- |",
        "| Janela ou pagina | uma rota e um componente por tela, com o mesmo trace_id | G4 piloto, G5 ondas |",
        "| Controles (campo, combo, tabela) | componentes do sistema de design (DESIGN.md), grade virtualizada quando F3 pedir | G4 |",
        "| Eventos de controle | validacao no cliente espelhando a regra do backend; atalhos de F2 | G4 |",
        "| Relatorio na tela | visualizador de PDF gerado pelo backend | G5 |",
        "| Estados (vazio, erro, offline) | os de F7, em todo componente | G6 |", "",
        "## O que nao muda com o processo", "",
        "- Regra de negocio nao muda de comportamento; o golden master compara com o legado.",
        "- O banco de destino e decisao separada, no G3.",
        "- Cada peca convertida carrega o trace_id do inventario.", "",
    ]
    return "\n".join(linhas)


# ---------------------------------------------------------------------------
# respostas_questionario.md: todas as respostas, legiveis, num arquivo so.
# E o unico gerado que se REGRAVA a cada aplicacao, porque e uma renderizacao
# do questionario.json, nao um documento que o usuario edita.
# ---------------------------------------------------------------------------

ROTULOS = {
    "projeto": "Projeto", "0_empresa_e_projeto": "Bloco 0 · Empresa e projeto", "A_sql": "A · Script SQL",
    "B_pdf_codigos": "B · PDF dos códigos", "C_pdf_interfaces": "C · PDF das interfaces", "D_pdf_queries": "D · PDF das queries",
    "E_pdf_completo": "E · PDF completo", "F_estilo_impeccable": "F · Qualidade das telas (Impeccable)", "G_help_json": "G · Help WLanguage em JSON",
    "H_backend": "H · Backend de destino", "I_frontend": "I · Frontend de destino", "J_economia_de_tokens": "J · Economia de tokens", "K_ambiente": "K · Ambiente e ferramentas", "L_contexto_e_implantacao": "L · Contexto do Claude Code e implantação",
    "M_artefatos": "M · Artefatos e anotações submetidos",
}


def _humano(chave: str) -> str:
    chave = re.sub(r"^0_(\d+)_", lambda m: f"0.{m.group(1)} ", chave)
    chave = re.sub(r"^([FKL])(\d+)_", lambda m: f"{m.group(1)}{m.group(2)} ", chave)
    return chave.replace("_", " ").strip()


def _render(valor, nivel: int) -> list[str]:
    ind = "  " * nivel
    if isinstance(valor, dict):
        out = []
        for k, v in valor.items():
            if k.startswith("_") or k == "observacao" and not v:
                continue
            if isinstance(v, (dict, list)) and v:
                out.append(f"{ind}- **{_humano(k)}**:")
                out += _render(v, nivel + 1)
            else:
                out.append(f"{ind}- {_humano(k)}: {_escalar(v)}")
        return out
    if isinstance(valor, list):
        out = []
        for v in valor:
            if isinstance(v, dict):
                out.append(f"{ind}- " + "; ".join(f"{_humano(k)}: {_escalar(x)}" for k, x in v.items() if not isinstance(x, (dict, list))))
                for k, x in v.items():
                    if isinstance(x, (dict, list)) and x:
                        out.append(f"{ind}  - **{_humano(k)}**:")
                        out += _render(x, nivel + 2)
            else:
                out.append(f"{ind}- {_escalar(v)}")
        return out
    return [f"{ind}- {_escalar(valor)}"]


def _escalar(v) -> str:
    if v is None or v == "" or v == [] or v == {}:
        return "(não informado)"
    if v is True:
        return "sim"
    if v is False:
        return "não"
    return str(v)


def _indice_por_id(q: dict) -> list[str]:
    """Indice id -> pergunta -> estado. Serve para o agente achar sem ler tudo,
    e para ver de relance o que ainda nao foi respondido."""
    try:
        sys.path.insert(0, str(Path(__file__).resolve().parent))
        from listar_perguntas import perguntas  # noqa: E402
        itens = perguntas()
    except Exception:  # sem o modelo por perto, o indice some, o resto fica
        return []
    L = ["## Índice por id", "",
         "| id | pergunta | respondido |", "| --- | --- | --- |"]
    for i in itens:
        no = q
        for parte in i["caminho"].split("."):
            no = (no or {}).get(parte) if isinstance(no, dict) else None
        resp = "—"
        if isinstance(no, dict):
            resp = "sim" if any(_tem_valor(v) for k, v in no.items() if not k.startswith("observacao")) else "não"
        elif no is not None:
            resp = "sim" if _tem_valor(no) else "não"
        L.append(f"| `{i['id']}` | {i['titulo']} | {resp} |")
    L += ["", f"{len(itens)} perguntas. Refaça uma com `/wx-claude-code:pergunta <id>`; a lista completa sai de `listar_perguntas.py`.", ""]
    return L


def _tem_valor(v) -> bool:
    if isinstance(v, bool):
        return True
    if v is None:
        return False
    if isinstance(v, (list, dict)):
        return len(v) > 0
    return str(v).strip() != ""


def respostas_md(q: dict, vazados_removidos: bool = False) -> str:
    p = q.get("projeto", {}) or {}
    ap = (q.get("0_empresa_e_projeto", {}) or {}).get("0_16_aprovador", {}) or {}
    L = ["# Respostas do questionário", "",
         f"Projeto **{p.get('nome') or '(sem nome)'}** · respondido em {q.get('respondido_em') or '(sem data)'} · gerado por `aplicar_questionario.py` de `.wx-migration/questionario.json`.",
         "Este arquivo é regravado a cada aplicação do questionário; para mudar uma resposta, edite o `questionario.json` e reaplique.", "",
         "> **Para os agentes:** toda resposta do questionário está aqui, e cada seção traz o id da pergunta.",
         "> Antes de perguntar qualquer coisa ao usuário, **procure aqui pelo id** (`0.16` aprovador, `F0` tela modelo, `H` destino, `K8` backup, `L6` esqueleto de ERP, `M` artefatos).",
         "> Perguntar de novo o que já foi respondido é o erro mais caro do fluxo. Resposta vazia significa **não respondido** — aí sim pergunte, e só aquele item, com `/wx-claude-code:pergunta <id>`.", ""] + \
        _indice_por_id(q) + [
         "## Aprovador", "",
         f"- Nome: **{ap.get('nome') or p.get('aprovador') or '(pendente)'}**",
         f"- Cargo: {_escalar(ap.get('cargo'))}", f"- E-mail: {_escalar(ap.get('email'))}",
         f"- Aprova: {', '.join(ap.get('aprova', []) or []) or '(não informado)'}", f"- Substituto: {_escalar(ap.get('substituto'))}", "",
         "O aprovador decide nos gates G0 a G7, nas divergências e no aceite; o nome vai para `pmo/plano.json` e para toda sprint.", ""]
    for chave, valor in q.items():
        if chave in ("schema_version", "respondido_em") or not isinstance(valor, (dict, list)):
            continue
        L += [f"## {ROTULOS.get(chave, chave)}", ""] + _render(valor, 0) + [""]
    L += ["## Onde mais procurar", "",
          "- `.wx-migration/questionario.json` — as respostas cruas, para editar.",
          "- `.wx-migration/ambiente/backup-e-replicacao.md` — o plano de backup e replicação (K8) por extenso.",
          "- `artefatos/CATALOGO.md` — o que o cliente mandou por fora (M), com onde usar e hash.",
          "- `INDEX_FILES.md` — o mapa de todos os arquivos do projeto.", ""]
    L += ["## Segredos", "", "Senhas e tokens nunca entram no questionário nem neste arquivo: o `0.15` guarda só o nome da credencial (`credencial_ref`).", ""]
    return "\n".join(L)


# ---------------------------------------------------------------------------
# Letra K: ambiente e ferramentas. Gera ambiente.md, o instalador, os papeis
# de banco em SQL e um .env.exemplo SEM valores. A senha do root e de cada
# papel vive na variavel de ambiente nomeada em senha_ref; nunca aqui.
# ---------------------------------------------------------------------------

FERRAMENTAS_BACKUP = {"pg_dump", "pgbackrest", "barman", "wal-g", "mysqldump", "xtrabackup", "mariabackup", "nativo"}
DESTINOS_BACKUP = {"local", "nfs", "s3", "b2", "ftp", "fita", "outro"}
TIPOS_REPLICACAO = {"streaming", "logica", "mestre-escravo", "mestre-mestre", "galera", "nenhuma"}

NIVEIS_PG = {
    "superuser": "SUPERUSER",
    "owner": "NOSUPERUSER CREATEDB",
    "readwrite": "NOSUPERUSER",
    "readonly": "NOSUPERUSER",
}
NIVEIS_MY = {
    "superuser": "ALL PRIVILEGES ON *.* WITH GRANT OPTION",
    "owner": "ALL PRIVILEGES ON `{banco}`.*",
    "readwrite": "SELECT, INSERT, UPDATE, DELETE, EXECUTE ON `{banco}`.*",
    "readonly": "SELECT ON `{banco}`.*",
}


def _papeis_ok(papeis: list, chave: str) -> None:
    for pp in papeis or []:
        if pp.get("nivel") not in NIVEIS_PG:
            raise ValueError(f"{chave}: nivel {pp.get('nivel')!r} do papel {pp.get('nome')!r} desconhecido (aceitos: {', '.join(NIVEIS_PG)})")
        if not pp.get("senha_ref"):
            raise ValueError(f"{chave}: papel {pp.get('nome')!r} sem senha_ref (nome da variavel de ambiente com a senha)")
        if not pp.get("nome"):
            raise ValueError(f"{chave}: papel sem nome")


def sql_papeis_postgresql(k2: dict) -> str:
    banco = k2.get("banco") or "app"
    L = ["-- Papeis do PostgreSQL gerados do questionario (K2). Senhas vem do ambiente:",
         "-- rode com: envsubst < papeis-postgresql.sql | psql -U $PGUSER -h $PGHOST -d postgres", "",
         f'CREATE DATABASE "{banco}";', ""]
    for pp in k2.get("papeis", []) or []:
        n, nv = f'"{pp["nome"]}"', pp["nivel"]
        L.append(f"CREATE ROLE {n} LOGIN {NIVEIS_PG[nv]} PASSWORD '${{{pp['senha_ref']}}}';")
        if nv == "owner":
            L.append(f'ALTER DATABASE "{banco}" OWNER TO {n};')
        elif nv == "readwrite":
            L += [f'GRANT CONNECT ON DATABASE "{banco}" TO {n};', f"GRANT USAGE ON SCHEMA public TO {n};",
                  f"GRANT SELECT, INSERT, UPDATE, DELETE ON ALL TABLES IN SCHEMA public TO {n};",
                  f"ALTER DEFAULT PRIVILEGES IN SCHEMA public GRANT SELECT, INSERT, UPDATE, DELETE ON TABLES TO {n};"]
        elif nv == "readonly":
            L += [f'GRANT CONNECT ON DATABASE "{banco}" TO {n};', f"GRANT USAGE ON SCHEMA public TO {n};",
                  f"GRANT SELECT ON ALL TABLES IN SCHEMA public TO {n};",
                  f"ALTER DEFAULT PRIVILEGES IN SCHEMA public GRANT SELECT ON TABLES TO {n};"]
        L.append("")
    return "\n".join(L)


def sql_papeis_mysql(k: dict, rotulo: str) -> str:
    banco = k.get("banco") or "app"
    L = [f"-- Papeis do {rotulo} gerados do questionario. Senhas vem do ambiente:",
         f"-- rode com: envsubst < papeis-{rotulo.lower()}.sql | {rotulo.lower()} -u {k.get('superusuario') or 'root'} -p", "",
         f"CREATE DATABASE IF NOT EXISTS `{banco}`;", ""]
    for pp in k.get("papeis", []) or []:
        L += [f"CREATE USER IF NOT EXISTS '{pp['nome']}'@'%' IDENTIFIED BY '${{{pp['senha_ref']}}}';",
              f"GRANT {NIVEIS_MY[pp['nivel']].format(banco=banco)} TO '{pp['nome']}'@'%';", ""]
    L.append("FLUSH PRIVILEGES;")
    return "\n".join(L)


def instalador(k: dict, e: dict) -> str:
    k1, k2, k3, k4, k5, k6 = (k.get(x, {}) or {} for x in ("K1_rust", "K2_postgresql", "K3_mysql", "K4_mariadb", "K5_supabase", "K6_github"))
    g = (e.get("0_15_github", {}) or {}) if e else {}
    k0 = k.get("K0_privilegios", {}) or {}
    modo_priv = k0.get("modo", "sudo"); root_user = k0.get("usuario_root") or "root"
    L = ["#!/usr/bin/env bash", "# Instalador do ambiente, gerado do questionario (letra K). Idempotente: cada",
         "# passo confere antes de agir. Senhas e tokens vem de variaveis de ambiente",
         "# (veja .env.exemplo); este arquivo nao contem nenhum.", "set -euo pipefail", "",
         "falta() { echo \"FALTA: $1\"; FALTOU=1; }", "FALTOU=0", "",
         f"# --- Privilegios (K0: modo {modo_priv}). Instalar pacote e servico exige root; o resto nao.", "SUDO=\"\"",
         "if [ \"$(id -u)\" -ne 0 ]; then"]
    if modo_priv == "sudo":
        L += ["  if command -v sudo >/dev/null; then SUDO=\"sudo\"; sudo -v || { echo 'sudo recusado'; exit 3; }",
              f"  else echo 'sem sudo: entrando como {root_user} (su) para os passos que exigem root'; exec su {root_user} -c 'cd \"$PWD\" && bash \"$(readlink -f \"$0\")\" \"$@\"' -- \"$@\"; fi"]
    elif modo_priv == "root":
        L += [f"  echo 'entrando como {root_user} (su)'; exec su {root_user} -c 'cd \"$PWD\" && bash \"$(readlink -f \"$0\")\" \"$@\"' -- \"$@\""]
    else:
        L += ["  echo 'K0 = nenhum: passos que exigem root serao marcados como FALTA'; SUDO=\"__sem_root__\""]
    L += ["fi", "priv() { if [ \"$SUDO\" = __sem_root__ ]; then falta \"precisa de root: $*\"; else $SUDO \"$@\"; fi; }", ""]
    if k1.get("instalar_ou_atualizar"):
        L += ["# --- Rust / Cargo (minimo " + str(k1.get("versao_minima", "")) + ", canal " + k1.get("canal", "stable") + ")",
              "if ! command -v rustup >/dev/null; then curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --default-toolchain " + k1.get("canal", "stable") + "; source \"$HOME/.cargo/env\"; fi",
              "rustup update " + k1.get("canal", "stable"),
              *(f"rustup component add {c}" for c in k1.get("componentes", []) or []),
              *(f"rustup target add {t}" for t in k1.get("targets", []) or []),
              f"cargo --version | grep -Eo '[0-9]+\\.[0-9]+' | awk -F. -v maj=$(echo {k1.get('versao_minima','0')} | cut -d. -f1) -v min=$(echo {k1.get('versao_minima','0')} | cut -d. -f2) '($1>maj)||($1==maj&&$2>=min){{ok=1}} END{{exit !ok}}' || falta \"cargo abaixo de {k1.get('versao_minima','')}: a estavel de hoje pode ser mais antiga; o minimo do questionario e o que vale\"", ""]
    if k3.get("instalar_ou_atualizar") and k4.get("instalar_ou_atualizar") and k3.get("porta") == k4.get("porta"):
        L += ["echo 'AVISO: MySQL e MariaDB marcados na mesma porta; instale um deles ou mude a porta no questionario.'", ""]
    for kx, nome, pacote, cli, sql in ((k2, "PostgreSQL", ("postgresql-" + str(k2["versao"])) if k2.get("versao") else "postgresql", "psql", "papeis-postgresql.sql"),
                                       (k3, "MySQL", "mysql-server", "mysql", "papeis-mysql.sql"),
                                       (k4, "MariaDB", "mariadb-server", "mariadb", "papeis-mariadb.sql")):
        if kx.get("instalar_ou_atualizar"):
            L += [f"# --- {nome} {kx.get('versao', '')} em {kx.get('host', 'localhost')}:{kx.get('porta', '')}",
                  f"if ! command -v {cli} >/dev/null; then if command -v apt-get >/dev/null; then priv apt-get update && priv apt-get install -y {pacote}; else falta '{nome}: instale pelo gerenciador da sua plataforma'; fi; fi",
                  f": \"${{{kx.get('senha_ref', 'SENHA')}:?defina {kx.get('senha_ref', 'SENHA')} (senha do {kx.get('superusuario', 'root')}) no ambiente}}\"",
                  *(f": \"${{{pp['senha_ref']}:?defina {pp['senha_ref']} (senha do papel {pp['nome']})}}\"" for pp in kx.get("papeis", []) or []),
                  (f"PGPASSWORD=\"${{{kx.get('senha_ref')}}}\" envsubst < \"$(dirname \"$0\")/{sql}\" | psql -h {kx.get('host', 'localhost')} -p {kx.get('porta', 5432)} -U {kx.get('superusuario', 'postgres')} -d postgres"
                   if nome == "PostgreSQL" else
                   f"envsubst < \"$(dirname \"$0\")/{sql}\" | {cli} -h {kx.get('host', 'localhost')} -P {kx.get('porta', 3306)} -u {kx.get('superusuario', 'root')} -p\"${{{kx.get('senha_ref')}}}\""),
                  ""]
    if k5.get("instalar_ou_atualizar"):
        L += ["# --- Supabase", *( ["if ! command -v supabase >/dev/null; then npm install -g supabase || falta 'supabase CLI (npm)'; fi"] if k5.get("cli_local") else []),
              f": \"${{{k5.get('anon_key_ref', 'SUPABASE_ANON_KEY')}:?defina a anon key no ambiente}}\"",
              f": \"${{{k5.get('service_role_ref', 'SUPABASE_SERVICE_ROLE_KEY')}:?defina a service role key no ambiente}}\"",
              f"echo \"Supabase: projeto {k5.get('projeto_url') or '(url pendente)'} (ref {k5.get('projeto_ref') or '?'})\"", ""]
    k7 = k.get("K7_n8n", {}) or {}
    if k7.get("instalar"):
        bd = k7.get("banco", {}) or {}; adm = k7.get("admin", {}) or {}; itg = k7.get("integracao", {}) or {}
        L += [f"# --- n8n ({k7.get('modo', 'docker')}, versao {k7.get('versao', 'latest')}) em {k7.get('host', 'localhost')}:{k7.get('porta', 5678)}",
              f": \"${{{k7.get('encryption_key_ref', 'N8N_ENCRYPTION_KEY')}:?defina a chave de criptografia do n8n (openssl rand -hex 32)}}\"",
              f": \"${{{adm.get('senha_ref', 'N8N_ADMIN_PASSWORD')}:?defina a senha do admin do n8n}}\"",
              *( [f": \"${{{bd.get('senha_ref', 'N8N_DB_PASSWORD')}:?defina a senha do usuario {bd.get('usuario', 'n8n')} do banco do n8n}}\""] if bd.get("tipo") == "postgresql" else []),
              *( [f": \"${{{itg.get('api_token_ref', 'PROJETO_API_TOKEN')}:?defina o token da API do projeto para o n8n}}\""] if itg.get("api_do_projeto_url") else [])]
        if k7.get("modo") == "docker":
            L += ["if ! command -v docker >/dev/null; then if command -v apt-get >/dev/null; then priv apt-get update && priv apt-get install -y docker.io docker-compose-v2; else falta 'docker'; fi; fi",
                  "priv systemctl enable --now docker 2>/dev/null || true",
                  "(cd \"$(dirname \"$0\")/n8n\" && priv docker compose up -d)"]
        elif k7.get("modo") == "npm":
            L += ["if ! command -v n8n >/dev/null; then npm install -g n8n@" + str(k7.get("versao", "latest")) + " || falta 'n8n (npm)'; fi",
                  "echo 'n8n via npm: rode `n8n start` com as variaveis de n8n/n8n.env carregadas'"]
        else:
            L += [f"echo 'n8n cloud: use {k7.get('url_publica') or '(url pendente)'}; nada a instalar'"]
        if bd.get("tipo") == "postgresql":
            L += [f"PGPASSWORD=\"${{{k2.get('senha_ref', 'PGPASSWORD')}}}\" envsubst < \"$(dirname \"$0\")/n8n/banco-n8n.sql\" | psql -h {k2.get('host', 'localhost')} -p {k2.get('porta', 5432)} -U {k2.get('superusuario', 'postgres')} -d postgres || falta 'banco do n8n (K2 precisa estar instalado)'"]
        L += [f"curl -fsS http://{k7.get('host', 'localhost')}:{k7.get('porta', 5678)}/healthz >/dev/null 2>&1 && echo 'n8n respondendo' || falta 'n8n nao respondeu em /healthz (pode levar alguns segundos apos subir)'", ""]
    if k6.get("ligar_projeto"):
        url = g.get("url", ""); br = k6.get("branch_principal", "main"); rem = k6.get("remote", "origin")
        L += ["# --- GitHub", f": \"${{{g.get('credencial_ref') or 'GITHUB_TOKEN'}:?defina o token do GitHub no ambiente}}\"",
              "cd " + shlex.quote(g.get('diretorio_destino') or '.'), "git rev-parse --is-inside-work-tree >/dev/null 2>&1 || git init -b " + br,
              *( [f"if command -v gh >/dev/null; then gh repo view {url.replace('https://github.com/', '')} >/dev/null 2>&1 || gh repo create {url.replace('https://github.com/', '')} --{k6.get('visibilidade', 'private')}; else falta 'gh (para criar o repositorio)'; fi"] if k6.get("criar_repositorio_se_nao_existir") and url else []),
              f"git remote get-url {rem} >/dev/null 2>&1 || git remote add {rem} {url or '<URL do 0.15>'}",
              *( [f"mkdir -p .github/workflows && [ -f .github/workflows/ci.yml ] || cat > .github/workflows/ci.yml <<'YML'\nname: ci\non: [push, pull_request]\njobs:\n  build:\n    runs-on: ubuntu-latest\n    steps:\n      - uses: actions/checkout@v4\n      - run: cargo build --locked && cargo test --locked\nYML"] if k6.get("ci") == "github-actions" and k1.get("instalar_ou_atualizar") else []),
              *( [f"echo 'Protecao da branch {br}: configure em Settings > Branches (exige gh api ou a interface).'"] if k6.get("proteger_branch") else []),
              ""]
    L += ["[ \"$FALTOU\" = 0 ] && echo 'ambiente OK' || { echo 'ambiente incompleto (veja FALTA acima)'; exit 3; }"]
    return "\n".join(L) + "\n"


def env_exemplo(k: dict, e: dict) -> str:
    refs = []
    for chave in ("K2_postgresql", "K3_mysql", "K4_mariadb"):
        kx = k.get(chave, {}) or {}
        if kx.get("instalar_ou_atualizar"):
            refs.append((kx.get("senha_ref"), f"senha do {kx.get('superusuario', 'root')} do {chave[3:]}"))
            refs += [(pp["senha_ref"], f"senha do papel {pp['nome']} ({pp['nivel']})") for pp in kx.get("papeis", []) or []]
    k5 = k.get("K5_supabase", {}) or {}
    if k5.get("instalar_ou_atualizar"):
        refs += [(k5.get("anon_key_ref"), "Supabase anon key"), (k5.get("service_role_ref"), "Supabase service role key (nunca no frontend)")]
    k7 = k.get("K7_n8n", {}) or {}
    if k7.get("instalar"):
        refs += [(k7.get("encryption_key_ref"), "chave de criptografia do n8n (openssl rand -hex 32)"), ((k7.get("admin") or {}).get("senha_ref"), "senha do admin do n8n")]
        if (k7.get("banco") or {}).get("tipo") == "postgresql":
            refs.append(((k7.get("banco") or {}).get("senha_ref"), "senha do usuario do banco do n8n"))
        if (k7.get("integracao") or {}).get("api_do_projeto_url"):
            refs.append(((k7.get("integracao") or {}).get("api_token_ref"), "token que o n8n usa na API do projeto"))
    g = (e or {}).get("0_15_github", {}) or {}
    if (k.get("K6_github", {}) or {}).get("ligar_projeto"):
        refs.append((g.get("credencial_ref") or "GITHUB_TOKEN", "token do GitHub"))
    L = ["# Copie para .env (fora do repositorio: .env esta no .gitignore) e preencha.",
         "# Este exemplo nao tem valores de proposito: senha nunca em texto puro no repositorio.", ""]
    L += [f"# {desc}\n{ref}=" for ref, desc in refs if ref]
    return "\n".join(L) + "\n"


def n8n_compose(k7: dict, k2: dict) -> str:
    bd = k7.get("banco", {}) or {}; adm = k7.get("admin", {}) or {}
    pg = bd.get("tipo") == "postgresql"
    L = ["# n8n gerado do questionario (K7). Valores sensiveis vem do .env ao lado (copie de ../.env.exemplo).",
         "services:", "  n8n:", f"    image: docker.n8n.io/n8nio/n8n:{k7.get('versao', 'latest')}", "    restart: unless-stopped",
         f"    ports: [\"{k7.get('porta', 5678)}:5678\"]", "    environment:",
         f"      - N8N_HOST={k7.get('host', 'localhost')}", f"      - N8N_PORT=5678", f"      - N8N_PROTOCOL={'https' if str(k7.get('url_publica', '')).startswith('https') else 'http'}",
         *( [f"      - WEBHOOK_URL={k7['url_publica'].rstrip('/')}/"] if k7.get("url_publica") else []),
         f"      - GENERIC_TIMEZONE={k7.get('timezone', 'America/Sao_Paulo')}", f"      - TZ={k7.get('timezone', 'America/Sao_Paulo')}",
         f"      - N8N_ENCRYPTION_KEY=${{{k7.get('encryption_key_ref', 'N8N_ENCRYPTION_KEY')}}}",
         "      - N8N_BASIC_AUTH_ACTIVE=true", f"      - N8N_BASIC_AUTH_USER={adm.get('email') or 'admin'}", f"      - N8N_BASIC_AUTH_PASSWORD=${{{adm.get('senha_ref', 'N8N_ADMIN_PASSWORD')}}}"]
    if pg:
        host = "host.docker.internal" if k2.get("host", "localhost") in ("localhost", "127.0.0.1") and bd.get("reusar_k2", True) else k2.get("host", "localhost")
        L += ["      - DB_TYPE=postgresdb", f"      - DB_POSTGRESDB_HOST={host}", f"      - DB_POSTGRESDB_PORT={k2.get('porta', 5432)}",
              f"      - DB_POSTGRESDB_DATABASE={bd.get('nome', 'n8n')}", f"      - DB_POSTGRESDB_USER={bd.get('usuario', 'n8n')}", f"      - DB_POSTGRESDB_PASSWORD=${{{bd.get('senha_ref', 'N8N_DB_PASSWORD')}}}",
              "    extra_hosts: [\"host.docker.internal:host-gateway\"]"]
    L += ["    volumes:", "      - n8n_data:/home/node/.n8n", "volumes:", "  n8n_data:"]
    return "\n".join(L) + "\n"


def n8n_banco_sql(k7: dict) -> str:
    bd = k7.get("banco", {}) or {}
    return (f"-- Banco do n8n no PostgreSQL de K2. Senha do ambiente: envsubst antes do psql.\n"
            f"CREATE ROLE \"{bd.get('usuario', 'n8n')}\" LOGIN PASSWORD '${{{bd.get('senha_ref', 'N8N_DB_PASSWORD')}}}';\n"
            f"CREATE DATABASE \"{bd.get('nome', 'n8n')}\" OWNER \"{bd.get('usuario', 'n8n')}\";\n")


def n8n_integracao_md(k7: dict) -> str:
    itg = k7.get("integracao", {}) or {}
    endereco = k7.get("url_publica") or f"http://{k7.get('host', 'localhost')}:{k7.get('porta', 5678)}"
    L = ["# Integração do n8n com o projeto (K7)", "",
         f"- n8n em {endereco} ({k7.get('modo', 'docker')}, {k7.get('versao', 'latest')}), fuso {k7.get('timezone', '')}",
         f"- API do projeto: {itg.get('api_do_projeto_url') or '(pendente)'}, token em `${itg.get('api_token_ref', '')}` (credencial «Header Auth» no n8n, nunca dentro do fluxo)",
         f"- Eventos que o projeto emite: {', '.join(itg.get('eventos_do_projeto', []) or []) or '(pendente)'}", "",
         "## Webhooks que o n8n expõe (o backend chama)", "", "| nome | método | caminho | o que faz |", "| --- | --- | --- | --- |"]
    L += [f"| {w.get('nome', '')} | {w.get('metodo', 'POST')} | {w.get('caminho', '')} | {w.get('o_que_faz', '')} |" for w in itg.get("webhooks", []) or []] or ["| (nenhum) | | | |"]
    L += ["", "## Fluxos iniciais", "", "| fluxo | gatilho | ação |", "| --- | --- | --- |"]
    L += [f"| {f.get('nome', '')} | {f.get('gatilho', '')} | {f.get('acao', '')} |" for f in itg.get("fluxos_iniciais", []) or []] or ["| (nenhum) | | |"]
    L += ["", "## O que o projeto precisa ter", "",
          "- Um cliente HTTP que chame cada webhook acima quando o evento acontecer, com retry e idempotência (o n8n pode receber duas vezes).",
          "- Uma rota de API por ação dos fluxos, autenticada pelo token acima; o n8n é um cliente como outro qualquer.",
          "- Cada webhook e cada rota vira um item `INT-*` na matriz de rastreabilidade, com teste de ponta a ponta no G4.", ""]
    return "\n".join(L)


def plano_de_backup(k8: dict, nome_do_projeto: str) -> str:
    """Plano de backup e replicacao (K8), do jeito que foi respondido.

    Escrito para ser lido na hora ruim: o que roda, onde cai, com que chave e,
    o que mais falta em plano de backup, **quando foi a ultima restauracao
    testada**. Backup nunca restaurado e hipotese, nao backup."""
    b = k8.get("backup") or {}
    r = k8.get("replicacao") or {}
    o = k8.get("objetivos") or {}
    leg = k8.get("legado") or {}
    al = b.get("alerta_em_falha") or {}
    rpo = o.get("rpo_minutos")
    rto = o.get("rto_minutos")
    testada = str(b.get("restauracao_testada_em") or "").strip()
    cif = f"sim, chave em `{b.get('chave_ref') or '?'}`" if b.get("cifrado") else "**nao**"
    fora = "sim" if b.get("fora_do_servidor") else "**nao** - um incendio leva o banco e o backup juntos"
    L = [f"# Backup e replicacao - {nome_do_projeto}", "",
         "Gerado do item **K8** do questionario por `aplicar_questionario.py`. Para mudar, refaca a pergunta com `/wx-claude-code:pergunta K8` e reaplique.", "",
         "## Objetivos declarados", "",
         f"- **RPO {rpo if rpo is not None else '(nao informado)'} min**: quanto dado a empresa aceita perder.",
         f"- **RTO {rto if rto is not None else '(nao informado)'} min**: quanto tempo aceita ficar fora do ar.",
         f"- {o.get('observacao') or 'sem observacao'}", "",
         "## Backup", "", "| item | resposta |", "| --- | --- |",
         f"| ativo | {'sim' if k8.get('ativar') else 'nao'} |",
         f"| ferramenta | {b.get('ferramenta') or '(a definir)'} |",
         f"| tipo | {b.get('tipo') or '(a definir)'} |",
         f"| frequencia | {b.get('frequencia') or '(a definir)'}" + (f" as {b['hora']}" if b.get("hora") else "") + " |",
         f"| destino | {b.get('destino') or '(a definir)'}" + (f" - {b['caminho_ou_bucket']}" if b.get("caminho_ou_bucket") else "") + " |",
         f"| fora do servidor do banco | {fora} |",
         f"| cifrado | {cif} |",
         f"| compressao | {b.get('compressao') or 'nenhuma'} |",
         f"| inclui arquivos (anexos, imagens) | {'sim' if b.get('inclui_arquivos') else 'nao, so o banco'} |",
         f"| retencao | {b.get('retencao_dias') or '?'} dias, {b.get('retencao_mensal_meses') or '?'} meses de mensais |",
         f"| teste de restauracao | {b.get('teste_de_restauracao') or 'nunca'} |",
         f"| ultima restauracao testada | {testada or '**nunca** - enquanto for isso, o backup e hipotese'} |",
         f"| janela de manutencao | {b.get('janela_de_manutencao') or '(nao informada)'} |",
         f"| responsavel | {b.get('responsavel') or '(nao informado)'} |",
         f"| alerta em falha | {al.get('canal') or 'nenhum'} para `{al.get('destino_ref') or '-'}` |", "",
         "Senha e chave aparecem aqui **so pelo nome da variavel**; os valores ficam no `.env`, fora do repositorio.", "",
         "## Replicacao", ""]
    if not r.get("ativar"):
        L += ["Nao ha replicacao nesta entrega. Consequencia declarada: a queda do banco derruba o sistema, e a volta depende do backup. O RTO acima e o do restore, nao o de um failover.", ""]
    else:
        ff = r.get("ferramenta_de_failover")
        L += ["| item | resposta |", "| --- | --- |",
              f"| tipo | {r.get('tipo')} |",
              f"| sincrona | {'sim (nenhuma transacao confirmada se perde; escrita mais lenta)' if r.get('sincrona') else 'nao (perda possivel igual ao lag)'} |",
              f"| lag maximo aceito | {r.get('lag_maximo_segundos')} s |",
              f"| failover | {r.get('failover')}" + (f" com {ff}" if ff and ff != "nenhum" else "") + " |",
              f"| monitoramento | {r.get('monitoramento') or '(nao informado)'} |",
              f"| responsavel | {r.get('responsavel') or '(nao informado)'} |", "",
              "| replica | host | papel | regiao |", "| --- | --- | --- | --- |"]
        linhas = [f"| {x.get('nome','')} | `{x.get('host','')}` | {x.get('papel','')} | {x.get('regiao','')} |" for x in (r.get("replicas") or [])]
        L += linhas or ["| (nenhuma declarada) | | | |"]
        L.append("")
        if not r.get("sincrona"):
            L += ["Replica assincrona nao e backup: ela copia o `DROP TABLE` junto, em segundos. Quem protege de erro humano e o backup acima.", ""]
    L += ["## Como era no legado", "",
          f"- Hoje: {leg.get('como_e_hoje') or '(nao informado)'}",
          f"- Backup do HFSQL: {leg.get('backup_do_hfsql') or '(nao informado)'}",
          f"- O que nao pode parar: {leg.get('o_que_nao_pode_parar') or '(nao informado)'}",
          f"- Ja perdeu dado: {leg.get('ja_perdeu_dado') or '(nao informado)'}", "",
          "Isto entra na migracao: a virada (G7) so acontece com backup do legado feito e **restaurado num ambiente separado**, nao so copiado.", "",
          "## O que ainda falta fazer", "",
          "- [ ] Escrever o script em `scripts/backup/` com a ferramenta escolhida.",
          "- [ ] Rodar uma restauracao completa num banco vazio e anotar a data em `K8.backup.restauracao_testada_em`.",
          "- [ ] Ligar o alerta de falha e provocar uma falha para ver se ele chega.",
          "- [ ] Medir quanto tempo a restauracao leva de verdade e comparar com o RTO declarado."]
    if r.get("ativar"):
        L.append("- [ ] Testar o failover fora do horario comercial e cronometrar.")
    L.append("")
    return "\n".join(L)


def esboco_ambiente(k: dict, e: dict) -> str:
    def sim(b): return "sim" if b else "não"
    k1, k2, k3, k4, k5, k6 = (k.get(x, {}) or {} for x in ("K1_rust", "K2_postgresql", "K3_mysql", "K4_mariadb", "K5_supabase", "K6_github"))
    g = (e or {}).get("0_15_github", {}) or {}
    L = ["# Ambiente e ferramentas (letra K do questionario)", "",
         "Gerado por `aplicar_questionario.py`. Instalar: `bash .wx-migration/ambiente/instalar-ambiente.sh` depois de preencher o `.env` a partir de `.env.exemplo`. Medir: `verificar_ambiente.py`.", "",
         "| Ferramenta | Pedido | Versão | Detalhes |", "| --- | --- | --- | --- |",
         f"| Rust / Cargo | {sim(k1.get('instalar_ou_atualizar'))} | mínima {k1.get('versao_minima') or '—'} ({k1.get('canal', 'stable')}) | componentes: {', '.join(k1.get('componentes', []) or []) or '—'}; targets: {', '.join(k1.get('targets', []) or []) or '—'} |",
         f"| PostgreSQL | {sim(k2.get('instalar_ou_atualizar'))} | {k2.get('versao') or '—'} | {k2.get('host', '')}:{k2.get('porta', '')}, banco `{k2.get('banco') or '?'}`, superusuário `{k2.get('superusuario', '')}` (senha em `${k2.get('senha_ref', '')}`) |",
         f"| MySQL | {sim(k3.get('instalar_ou_atualizar'))} | {k3.get('versao') or '—'} | {k3.get('host', '')}:{k3.get('porta', '')}, superusuário `{k3.get('superusuario', '')}` (senha em `${k3.get('senha_ref', '')}`) |",
         f"| MariaDB | {sim(k4.get('instalar_ou_atualizar'))} | {k4.get('versao') or '—'} | {k4.get('host', '')}:{k4.get('porta', '')}, superusuário `{k4.get('superusuario', '')}` (senha em `${k4.get('senha_ref', '')}`) |",
         f"| Supabase | {sim(k5.get('instalar_ou_atualizar'))} | CLI local: {sim(k5.get('cli_local'))} | projeto {k5.get('projeto_url') or '—'}; chaves em `${k5.get('anon_key_ref', '')}` e `${k5.get('service_role_ref', '')}` |",
         f"| GitHub | {sim(k6.get('ligar_projeto'))} | — | {g.get('url') or '(URL no 0.15)'}, remote `{k6.get('remote', 'origin')}`, branch `{k6.get('branch_principal', 'main')}`, criar se não existir: {sim(k6.get('criar_repositorio_se_nao_existir'))}, {k6.get('visibilidade', 'private')}, CI: {k6.get('ci') or '—'}, proteger branch: {sim(k6.get('proteger_branch'))} |", ""]
    for kx, nome in ((k2, "PostgreSQL"), (k3, "MySQL"), (k4, "MariaDB")):
        if kx.get("papeis"):
            L += [f"## Papéis do {nome}", "", "| papel | nível | senha em |", "| --- | --- | --- |"] + [f"| {pp['nome']} | {pp['nivel']} | `${pp['senha_ref']}` |" for pp in kx["papeis"]] + [""]
    k0 = k.get("K0_privilegios", {}) or {}; k7 = k.get("K7_n8n", {}) or {}
    L.insert(6, f"| Privilégios | — | — | modo {k0.get('modo', 'sudo')}: {'sudo antes do que exige root; sem sudo, entra como ' + (k0.get('usuario_root') or 'root') + ' com su' if k0.get('modo', 'sudo') == 'sudo' else ('entra como ' + (k0.get('usuario_root') or 'root') + ' com su' if k0.get('modo') == 'root' else 'nada que exija root')} |")
    itg = k7.get("integracao", {}) or {}
    L.insert(12, f"| n8n | {sim(k7.get('instalar'))} | {k7.get('versao') or '—'} ({k7.get('modo', 'docker')}) | {k7.get('url_publica') or (str(k7.get('host', '')) + ':' + str(k7.get('porta', '')))}, banco {(k7.get('banco') or {}).get('tipo', '—')}, admin {(k7.get('admin') or {}).get('email') or '?'}, {len(itg.get('webhooks', []) or [])} webhooks, {len(itg.get('fluxos_iniciais', []) or [])} fluxos; detalhes em `ambiente/n8n/integracao.md` |")
    L += ["## Regra", "", "Nenhuma senha, token ou chave fica no questionário, neste arquivo, no instalador ou no SQL: só o **nome** da variável de ambiente. O `.env` real fica fora do repositório.", ""]
    return "\n".join(L)


# ---------------------------------------------------------------------------
# Letra L: contexto para o Claude Code e implantacao. A licao da aula de
# vibe coding e que o resultado depende do CONTEXTO da primeira sessao:
# kickoff + DESIGN.md + prints + skills + mapa de arquivos. Aqui isso vira
# arquivo, montado das respostas, sem nenhum segredo.
# ---------------------------------------------------------------------------

DOCKERFILES = {
    "rust": "FROM rust:1-slim AS build\nWORKDIR /app\nCOPY . .\nRUN cargo build --release --locked\n\nFROM debian:bookworm-slim\nRUN apt-get update && apt-get install -y --no-install-recommends ca-certificates && rm -rf /var/lib/apt/lists/*\nCOPY --from=build /app/target/release/{bin} /usr/local/bin/{bin}\nEXPOSE {porta}\nHEALTHCHECK CMD curl -fsS http://localhost:{porta}{health} || exit 1\nCMD [\"{bin}\"]\n",
    "python": "FROM python:3.12-slim\nWORKDIR /app\nCOPY requirements.txt .\nRUN pip install --no-cache-dir -r requirements.txt\nCOPY . .\nEXPOSE {porta}\nHEALTHCHECK CMD python -c \"import urllib.request; urllib.request.urlopen('http://localhost:{porta}{health}')\" || exit 1\nCMD [\"uvicorn\", \"app.main:app\", \"--host\", \"0.0.0.0\", \"--port\", \"{porta}\"]\n",
    "php": "FROM composer:2 AS vendor\nWORKDIR /app\nCOPY composer.json composer.lock ./\nRUN composer install --no-dev --no-scripts --no-interaction --prefer-dist\n\nFROM php:8.3-fpm-alpine\nRUN docker-php-ext-install pdo_pgsql opcache\nWORKDIR /app\nCOPY --from=vendor /app/vendor ./vendor\nCOPY . .\nEXPOSE {porta}\nHEALTHCHECK CMD php -r \"exit(@file_get_contents('http://localhost:{porta}{health}') === false ? 1 : 0);\"\nCMD [\"php\", \"-S\", \"0.0.0.0:{porta}\", \"-t\", \"public\"]\n",
    "csharp-wl": "FROM mcr.microsoft.com/dotnet/sdk:8.0 AS build\nWORKDIR /src\nCOPY . .\nRUN dotnet publish -c Release -o /out\n\nFROM mcr.microsoft.com/dotnet/aspnet:8.0\nWORKDIR /app\nCOPY --from=build /out .\nEXPOSE {porta}\nENV ASPNETCORE_URLS=http://+:{porta}\nCMD [\"dotnet\", \"{bin}.dll\"]\n",
    "node": "FROM node:22-alpine\nWORKDIR /app\nCOPY package*.json ./\nRUN npm ci --omit=dev\nCOPY . .\nRUN npm run build\nEXPOSE {porta}\nCMD [\"npm\", \"start\"]\n",
    "go": "FROM golang:1.23 AS build\nWORKDIR /src\nCOPY . .\nRUN CGO_ENABLED=0 go build -o /out/{bin} ./...\n\nFROM gcr.io/distroless/static\nCOPY --from=build /out/{bin} /{bin}\nEXPOSE {porta}\nCMD [\"/{bin}\"]\n",
    "java": "FROM eclipse-temurin:21-jdk AS build\nWORKDIR /src\nCOPY . .\nRUN ./mvnw -q package -DskipTests\n\nFROM eclipse-temurin:21-jre\nCOPY --from=build /src/target/*.jar /app.jar\nEXPOSE {porta}\nCMD [\"java\", \"-jar\", \"/app.jar\"]\n",
}
MCPS = {
    "supabase": {"command": "npx", "args": ["-y", "@supabase/mcp-server-supabase@latest"], "env": {"SUPABASE_ACCESS_TOKEN": "${SUPABASE_ACCESS_TOKEN}"}},
    "postgresql": {"command": "npx", "args": ["-y", "@modelcontextprotocol/server-postgres", "${DATABASE_URL}"]},
    "github": {"command": "npx", "args": ["-y", "@modelcontextprotocol/server-github"], "env": {"GITHUB_PERSONAL_ACCESS_TOKEN": "${GITHUB_TOKEN}"}},
    "playwright": {"command": "npx", "args": ["-y", "@playwright/mcp@latest"]},
}


def _slug(nome: str) -> str:
    return re.sub(r"[^a-z0-9]+", "-", (nome or "app").lower()).strip("-") or "app"


def prompt_kickoff(q: dict) -> str:
    p = q.get("projeto", {}) or {}; e = q.get("0_empresa_e_projeto", {}) or {}; a = q.get("A_sql", {}) or {}
    h = q.get("H_backend", {}) or {}; i = q.get("I_frontend", {}) or {}; l = q.get("L_contexto_e_implantacao", {}) or {}
    k = q.get("K_ambiente", {}) or {}; ap = e.get("0_16_aprovador", {}) or {}
    v1 = (l.get("L1_requisitos_da_v1") or {}); imp = l.get("L3_implantacao") or {}
    est_h = ((h.get("processo") or {}).get("estrategia")) or "(pendente)"
    L = [f"# Kickoff — {p.get('nome') or 'projeto'}: conversão do legado {', '.join(p.get('produtos', []) or ['WX'])} {p.get('wx_versao', '')} para {h.get('linguagem') or h.get('perfil') or '?'} + {i.get('linguagem') or i.get('perfil') or '?'}", "",
         "Cole este prompt na primeira sessão do Claude Code no projeto de destino, com `DESIGN.md`, `INDEX_FILES.md` e as capturas de tela anexadas. Gerado por `aplicar_questionario.py`; edite o `questionario.json` para mudar.", "",
         "## O que você é e o que não é", "",
         "Você é o engenheiro de conversão deste projeto. **Isto não é vibe coding**: as regras de negócio vêm do legado WINDEV, cada uma com origem localizável na matriz `.wx-migration/traceability.csv`, e a igualdade se prova com o golden master. Nunca invente regra; registre lacuna em `.wx-migration/gaps.md`.", "",
         "## Contexto", "",
         f"- Solicitante: {(e.get('0_1_softhouse') or {}).get('nome_fantasia') or (e.get('0_1_softhouse') or {}).get('razao_social') or '(pendente)'}; aprovador: **{ap.get('nome') or p.get('aprovador') or '(pendente)'}**; prazo final {((e.get('0_11_cronograma') or {}).get('prazo_final')) or '(pendente)'}.",
         f"- Finalidade: {e.get('0_6_finalidade') or '(pendente)'}",
         f"- Legado: {', '.join(p.get('produtos', []) or ['WX'])} {p.get('wx_versao', '')}, banco {a.get('dialeto') or '?'} {a.get('versao_do_banco', '')}, encoding {a.get('encoding', '')}, collation {a.get('collation') or '?'}, fuso {a.get('timezone', '')}. **Datas, decimais e fuso seguem o legado.**",
         f"- Destino: backend **{h.get('linguagem') or h.get('perfil') or '?'}** ({h.get('framework') or 'framework a definir'}, banco {h.get('banco') or '?'}); frontend **{i.get('linguagem') or i.get('perfil') or '?'}** ({i.get('framework') or '?'}), plataformas {', '.join(i.get('plataformas', []) or []) or '?'}.",
         f"- Estratégia de conversão: **{est_h}** (detalhe em `.wx-migration/processo-de-conversao.md`).",
         f"- Implantação: {imp.get('alvo') or 'nenhum'}" + (f", {imp.get('dominio')}" if imp.get("dominio") else "") + f", porta {imp.get('porta', '')}, healthcheck `{imp.get('healthcheck', '/health')}`; variáveis de ambiente (só nomes): {', '.join(imp.get('variaveis_de_ambiente', []) or []) or 'nenhuma'}.", "",
         "## Requisitos da primeira versão (v1)", ""]
    L += [f"{n}. {r}" for n, r in enumerate(v1.get("itens", []) or [], 1)] or ["(nenhum informado em L1: use o inventário do G1 como escopo)"]
    if v1.get("fora_da_v1"):
        L += ["", "Fora da v1: " + "; ".join(v1["fora_da_v1"]) + "."]
    L += ["", "## Como trabalhar", "",
          "1. Leia `CLAUDE.md`, `INDEX_FILES.md` e `.wx-migration/respostas_questionario.md` antes de qualquer arquivo de código.",
          "2. Siga os gates: `/wx-claude-code:converter` faz o G0 (pré-flight dos anexos) antes de escrever código.",
          "3. Telas seguem `DESIGN.md` (tela modelo, vocabulário dos botões, cores, fundo) e passam por `/wx-claude-code:estilo-telas`.",
          f"4. Teste com `{((l.get('L4_hooks_do_projeto') or {}).get('comando_de_teste')) or '(comando de teste pendente)'}`; o hook do projeto roda isso ao parar.",
          f"5. Commits na convenção `{((k.get('K6_github') or {}).get('convencao_de_commits')) or 'conventional'}`; nunca commite `.env`.",
          "6. Segredos só por nome de variável de ambiente; nunca em código, log ou resposta.", ""]
    return "\n".join(L)


def prompt_prototipacao(q: dict) -> str:
    f = q.get("F_estilo_impeccable", {}) or {}; l = q.get("L_contexto_e_implantacao", {}) or {}; p = q.get("projeto", {}) or {}
    pr = l.get("L2_prototipacao") or {}; t0 = f.get("F0_tela_modelo") or {}; f9 = f.get("F9_vocabulario_dos_botoes") or {}
    f10 = f.get("F10_posicao_dos_botoes") or {}; f12 = f.get("F12_cores_das_acoes") or {}; f13 = f.get("F13_fundo_das_telas") or {}; f1 = f.get("F1_operacao") or {}
    pal = f.get("paleta") or {}
    L = [f"# Prompt de prototipação — {p.get('nome') or 'projeto'} ({pr.get('ferramenta') or 'ferramenta a definir'})", "",
         "Cole na ferramenta de protótipo (Google Stitch, Figma Make…) junto com as capturas da tela modelo. O `DESIGN.md` que voltar de lá **complementa** o `DESIGN.md` do questionário; não o substitui.", "",
         f"Desenhe as telas de um sistema de gestão (ERP) chamado {p.get('nome') or '…'}, para {f1.get('perfil_do_usuario') or 'operadores que ficam horas na tela'}, em {f1.get('ambiente') or 'ambiente de escritório'}, tela típica {f1.get('tela_tipica') or '1366×768'}. É a conversão de um sistema WINDEV: a estrutura das telas, os campos e a ordem de tabulação já existem e devem ser preservados; o visual pode mudar.", "",
         "Telas prioritárias: " + (", ".join(pr.get("telas_prioritarias", []) or []) or "as da tela modelo") + ".", "",
         "Tela modelo (referência visual): " + (", ".join(f"{a0.get('tela', '')} ({a0.get('papel', '')}, {a0.get('arquivo', '')})" for a0 in t0.get("arquivos", []) or []) or "nenhuma captura informada") + ".",
         "Preservar: " + ("; ".join(t0.get("o_que_preservar", []) or []) or "(nada informado)") + ". Pode mudar: " + ("; ".join(t0.get("o_que_mudar", []) or []) or "(nada informado)") + ".", "",
         f"Direção: {f.get('preservar_ou_redesenhar', 'preservar')}; tema {f.get('tema', 'ambos')}; densidade {f.get('densidade', 'compacta')}; tipografia {f.get('tipografia') or 'a definir'}.",
         "Paleta: " + (", ".join(f"{k} {v}" for k, v in pal.items() if v) or "a definir") + ".",
         f"Botões: estilo {f9.get('estilo') or '?'} em {f9.get('caixa') or '?'}; rótulos " + (", ".join(f"{k}={v}" for k, v in (f9.get("rotulos") or {}).items()) or "?") + f". Posição: {f10.get('barra_da_grade') or '?'} para a grade, {f10.get('barra_do_formulario') or '?'} para o formulário.",
         "Cores por ação: " + (", ".join(f"{k} {v}" for k, v in (f12.get("cores") or {}).items()) or "padrão do plugin") + f"; {f12.get('preenchimento') or 'contorno'}.",
         f"Fundo: {f13.get('tipo') or 'cor'} {f13.get('cor') or ''}; escuro {f13.get('cor_escuro') or ''}.", "",
         "Entregue: cada tela em estado normal, vazio e com erro; exporte o design system como DESIGN.md.", ""]
    return "\n".join(L)


def index_files(q: dict, projeto: Path) -> str:
    """Mapa de arquivos: uma linha por arquivo dizendo o que e e quando abrir. Regravado sempre."""
    wx = projeto / ".wx-migration"
    p = q.get("projeto", {}) or {}
    raiz = p.get("raiz_de_evidencias", "./inputs")
    fixos = [
        ("CLAUDE.md", "regras do projeto; leia primeiro"),
        ("artefatos/CATALOGO.md", "o que o cliente mandou por fora, por tipo, com onde usar e hash; regravado por arquivar_artefato.py"),
        ("artefatos/LEIA-ME.md", "como submeter um artefato e por que a pasta é somente leitura"),
        ("INDEX_FILES.md", "este mapa; regravado a cada aplicação do questionário"),
        ("DESIGN.md", "sistema de design: tela modelo, botões, cores, fundo (letra F)"),
        ("PRODUCT.md", "quem opera e em que condições (F1)"),
        (".claude/settings.json", "hooks do projeto (teste ao parar, lint ao editar) e permissões"),
        (".claude/skills/regras-do-legado/SKILL.md", "como tratar regra de negócio do legado: origem, matriz, golden master"),
        (".claude/skills/legado-para-destino/SKILL.md", "o que cada peça do WX vira no destino e a estratégia escolhida"),
        (".mcp.json", "servidores MCP do projeto (sem chaves)"),
        ("Dockerfile", "imagem do serviço para a implantação (L3)"),
        ("docker-compose.yml", "serviço + banco para rodar local ou no painel"),
        (".wx-migration/questionario.json", "as respostas brutas; edite aqui e reaplique"),
        (".wx-migration/respostas_questionario.md", "as respostas legíveis, aprovador no topo; consulte antes de perguntar"),
        (".wx-migration/prompts/kickoff.md", "prompt da primeira sessão de conversão"),
        (".wx-migration/prompts/prototipacao.md", "prompt para a ferramenta de protótipo de telas"),
        (".wx-migration/wx-inputs.manifest.json", "manifesto dos anexos que o pré-flight lê"),
        (".wx-migration/conversion.config.json", "modo, destino, fidelidade"),
        (".wx-migration/traceability.csv", "a matriz: todo BR-/QRY-/UI-/INT-/RPT-/DB- com estado"),
        (".wx-migration/gaps.md", "lacunas GAP-*; o que falta para seguir"),
        (".wx-migration/empresa.md", "softhouse, diretores, endereço, logotipos, objetivos, pessoal"),
        (".wx-migration/processo-de-conversao.md", "o que cada peça vira, gate a gate, e a estratégia"),
        (".wx-migration/entrega.json", "GitHub, branch, usuário, nome da credencial, diretório"),
        (".wx-migration/ambiente.md", "ferramentas pedidas em K e onde a senha de cada uma fica"),
        (".wx-migration/ambiente/instalar-ambiente.sh", "instalador idempotente (sudo/root resolvido em K0)"),
        (".wx-migration/ambiente/backup-e-replicacao.md", "plano de backup e replicação (K8): RPO, RTO, retenção, e quando a restauração foi testada"),
        (".wx-migration/ambiente/.env.exemplo", "nomes das variáveis; copie para .env fora do repositório"),
        (".wx-migration/ambiente/n8n/integracao.md", "webhooks, fluxos e o que o projeto expõe ao n8n"),
        (".wx-migration/pmo/relatorio.md", "relatório de onze seções do PMO; gerado ao fechar sprint"),
        (".wx-migration/pmo/painel.html", "o mesmo em HTML"),
        (".wx-migration/pmo/backlog.md", "backlog priorizado com o papel dono de cada item"),
        (".wx-migration/pmo/base_de_conhecimento.md", "ciclos PDCA fechados, frutíferos ou não"),
    ]
    if ((q.get("L_contexto_e_implantacao") or {}).get("L6_esqueleto_erp") or {}).get("gerar"):
        sys.path.insert(0, str(Path(__file__).resolve().parent))
        import esqueleto_erp  # noqa: E402
        fixos += esqueleto_erp.entradas_index(q)
    L = ["# INDEX_FILES.md — mapa do projeto", "",
         f"Gerado por `aplicar_questionario.py` em {date.today().isoformat()}. Uma linha por arquivo: o que é e quando abrir. Não abra tudo; ache aqui o arquivo certo. Arquivos marcados «ausente» ainda não existem.", "",
         "| arquivo | o que é | estado |", "| --- | --- | --- |"]
    for rel, desc in fixos:
        L.append(f"| `{rel}` | {desc} | {'existe' if (projeto / rel).exists() else 'ausente'} |")
    L += ["", f"## Evidências do legado (`{raiz}/`, somente leitura)", "", "| arquivo | grupo |", "| --- | --- |"]
    for letra, grupo in (("A_sql", "script SQL"), ("B_pdf_codigos", "PDF dos códigos"), ("C_pdf_interfaces", "PDF das interfaces"), ("D_pdf_queries", "PDF das queries"), ("E_pdf_completo", "PDF completo")):
        for arq in (q.get(letra, {}) or {}).get("arquivos", []) or []:
            L.append(f"| `{raiz}/{arq}` | {grupo} |")
    t0 = ((q.get("F_estilo_impeccable") or {}).get("F0_tela_modelo") or {})
    for a0 in t0.get("arquivos", []) or []:
        L.append(f"| `{raiz}/{a0.get('arquivo', '')}` | tela modelo {a0.get('tela', '')} ({a0.get('papel', '')}) |")
    ss = projeto / raiz / "screenshots" / "screenshots.json"
    if ss.is_file():
        try:
            n = len(json.loads(ss.read_text(encoding="utf-8")))
            L.append(f"| `{raiz}/screenshots/` | {n} capturas listadas em `screenshots.json` |")
        except (OSError, json.JSONDecodeError):
            pass
    L += ["", "## Fontes técnicas (RAG)", "",
          "- Corpus WLanguage 12k do plugin, por tema: `query_wlanguage_help.py --group GG-SS-TT --query …` (semântica técnica, nunca regra de negócio).",
          "- Perfis e processo de conversão: `references/perfis-de-destino.md` do plugin.",
          "- Documentos deste projeto: os de `.wx-migration/` acima; procure por id (`BR-012`, `GAP-003`) antes de ler inteiro.", ""]
    return "\n".join(L)


def leia_me_artefatos(q: dict, pasta: str) -> str:
    """Instrucoes da pasta de artefatos. O CATALOGO.md e do arquivar_artefato.py;
    aqui fica so o que nao muda a cada submissao, para nao haver dois donos do
    mesmo arquivo."""
    m = q.get("M_artefatos") or {}
    itens = m.get("itens") or []
    L = [f"# {pasta}/ — artefatos submetidos", "",
         "O que o cliente manda fora da evidência do projeto WX: anotação de reunião, PDF com as classes OOP, `.sql` de consultas, modelo de relatório impresso, manual, contrato de API, código PHP, dado de amostra.",
         "", "## Como submeter", "",
         "Um por vez, sempre pelo script (nunca copiando à mão): ele confere segredo, calcula o SHA-256, recusa sobrescrever um arquivo já arquivado com outro conteúdo, e regrava `CATALOGO.md` e `registro.json`.", "",
         "```bash", 'python3 "${CLAUDE_PLUGIN_ROOT}/skills/conversao-wx/scripts/arquivar_artefato.py" \\',
         "  --project-root . --arquivo ~/notas-da-reuniao.txt \\",
         '  --tipo anotacao --onde-usar "G1: regras ditadas pelo cliente; cada uma vira BR-*" \\',
         '  --descricao "reunião de 01/09 com o gerente" --origem cliente \\',
         "  --questionario .wx-migration/questionario.json", "```", "",
         "`--onde-usar` é obrigatório: artefato sem destino declarado vira arquivo que ninguém abre. `--confidencial` marca o que não pode ser copiado para documento gerado nem para resposta — cita-se o arquivo, não o conteúdo.", "",
         "## Regras desta pasta", "",
         "- **Somente leitura para o agente.** Um hook recusa escrita aqui; só o script arquiva.",
         "- **`CATALOGO.md` e `registro.json` não se editam**: saem do script, dos fatos.",
         "- **Nenhum segredo.** O script recusa arquivo de texto com token ou chave privada; senha nunca em texto puro, nem dentro de um anexo.",
         "- **Não confunda com `inputs/`**: lá fica a evidência do projeto WX que o G0 inventaria; aqui, o que o cliente mandou por fora.", ""]
    if itens:
        L += ["## Declarados no questionário (bloco M)", "", "| arquivo | tipo | onde usar |", "| --- | --- | --- |"]
        L += [f"| `{i.get('arquivo','')}` | `{i.get('tipo','')}` | {i.get('onde_usar','')} |" for i in itens]
        L += ["", "Estes vieram do `questionario.json`. O que já foi arquivado aparece em `CATALOGO.md` com o hash; o que está só aqui ainda precisa ser submetido pelo script.", ""]
    return "\n".join(L)


def settings_do_projeto(l4: dict) -> dict:
    hooks = {}
    if l4.get("testar_ao_parar") and l4.get("comando_de_teste"):
        hooks["Stop"] = [{"hooks": [{"type": "command", "command": "bash .claude/hooks/testar.sh", "timeout": 600, "statusMessage": "Testes do projeto"}]}]
    if l4.get("lint_ao_editar") and l4.get("comando_de_lint"):
        hooks["PostToolUse"] = [{"matcher": "Edit|Write|MultiEdit", "hooks": [{"type": "command", "command": "bash .claude/hooks/lint.sh", "timeout": 120, "statusMessage": "Lint"}]}]
    return {"permissions": {"allow": ["Read", "Glob", "Grep", "Bash(git status:*)", "Bash(git diff:*)", "Bash(git log:*)"], "deny": ["Read(./.env)", "Read(./.env.*)", "Read(./**/.env)"]}, "hooks": hooks}


def skill_regras_do_legado(q: dict) -> str:
    return ("---\nname: regras-do-legado\ndescription: Como tratar toda regra de negócio vinda do WINDEV neste projeto: origem localizável, matriz, golden master. Use antes de implementar ou mudar qualquer BR-*.\n---\n\n"
            "# Regras do legado\n\n"
            "1. Toda regra tem um `BR-*` em `.wx-migration/traceability.csv` com origem (`source_locator`) no PDF de código ou no SQL. Sem origem, não é regra: é `GAP-*`.\n"
            "2. Hierarquia de evidências (do `CLAUDE.md`): decisão humana > comportamento observado > código e SQL do legado > regra documentada > telas > Help WLanguage.\n"
            "3. A prova é o golden master (`golden.py comparar`) com os dados de amostra; «parece igual» não conta.\n"
            "4. Conflito entre evidências para o item, registra `DEC-*` e pede ao aprovador " + f"**{(((q.get('0_empresa_e_projeto') or {}).get('0_16_aprovador') or {}).get('nome')) or (q.get('projeto') or {}).get('aprovador') or '(pendente)'}**.\n"
            "5. Precisão numérica, nulidade, datas, fuso, collation e transações seguem o legado, salvo `DEC-*` aprovada.\n")


def skill_legado_para_destino(q: dict) -> str:
    h = q.get("H_backend", {}) or {}; i = q.get("I_frontend", {}) or {}
    ph = h.get("processo") or {}; pi = i.get("processo") or {}
    return ("---\nname: legado-para-destino\ndescription: O que cada peça do projeto WINDEV vira em " + f"{h.get('linguagem') or h.get('perfil') or 'destino'} + {i.get('linguagem') or i.get('perfil') or 'frontend'}" + " e a estratégia de conversão escolhida. Use ao converter procedure, classe, tela, query ou relatório.\n---\n\n"
            f"# Legado → destino\n\n- Backend: **{h.get('linguagem') or h.get('perfil') or '?'}** ({h.get('framework') or '?'}, {h.get('banco') or '?'}), estratégia **{ph.get('estrategia') or '(pendente)'}**.\n"
            f"- Frontend: **{i.get('linguagem') or i.get('perfil') or '?'}** ({i.get('framework') or '?'}), estratégia **{pi.get('estrategia') or '(pendente)'}**, ritmo {pi.get('telas_por_vez') or 'tela a tela'}.\n"
            f"- O que o usuário quer diferente: {ph.get('quer_diferente') or 'nada'}; {pi.get('quer_diferente') or 'nada'}.\n\n"
            "A tabela peça a peça, com o gate de cada uma, está em `.wx-migration/processo-de-conversao.md`; leia antes de converter. Funções WLanguage: consulte o corpus por tema com `query_wlanguage_help.py --group`, e, no perfil C#, a WL_C# pelo nome da função.\n")


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
    validar_entradas(q)
    if not q.get("respondido_em"):
        q["respondido_em"] = date.today().isoformat()
    ap = (q.get("0_empresa_e_projeto", {}) or {}).get("0_16_aprovador", {}) or {}
    if ap.get("nome") and not q.get("projeto", {}).get("aprovador"):
        q.setdefault("projeto", {})["aprovador"] = ap["nome"]

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
    # Marca d'agua: com licenca valida, o arquivo gerado diz para quem foi emitido.
    try:
        sys.path.insert(0, str(Path(__file__).resolve().parent))
        from licenca import verificar_instalada  # noqa: E402
        lic = verificar_instalada()
    except Exception:  # sem modulo ou sem chave publica: segue sem marca
        lic = {"status": "ausente"}
    marca = f"\n<!-- Gerado sob a licenca WX Claude Code n. {lic['id']} para {lic['cliente']} -->\n" if lic.get("status") == "valida" else ""
    ap_nome = ap.get("nome") or q.get("projeto", {}).get("aprovador") or "(pendente)"
    claude_md = claude_md.rstrip("\n") + "\n\n## Respostas do questionário\n\n" + \
        f"Todas as respostas do questionário (bloco 0 e letras A a J) estão em `.wx-migration/respostas_questionario.md`; o aprovador do projeto é **{ap_nome}**. " + \
        "Consulte esse arquivo antes de perguntar de novo algo que já foi respondido; para mudar uma resposta, edite `.wx-migration/questionario.json` e reaplique `aplicar_questionario.py`.\n\n" + \
        "## Mapa de arquivos\n\n`INDEX_FILES.md` na raiz diz o que é cada arquivo e quando abrir; ache lá antes de ler diretórios inteiros. A primeira sessão começa por `.wx-migration/prompts/kickoff.md`. Skills do projeto em `.claude/skills/`; hooks de teste e lint em `.claude/settings.json`.\n" + marca
    if ((q.get("L_contexto_e_implantacao") or {}).get("L6_esqueleto_erp") or {}).get("gerar"):
        sys.path.insert(0, str(Path(__file__).resolve().parent))
        import esqueleto_erp  # noqa: E402
        claude_md = claude_md.rstrip("\n") + "\n\n" + esqueleto_erp.secao_claude_md(q)
    _m = q.get("M_artefatos") or {}
    if _m.get("itens"):
        _p = (_m.get("pasta") or "./artefatos").strip("./") or "artefatos"
        _linhas = "\n".join(f"| `{i.get('arquivo','')}` | `{i.get('tipo','')}` | {i.get('onde_usar','')} |" for i in _m["itens"])
        claude_md = claude_md.rstrip("\n") + "\n\n## Artefatos submetidos\n\n" + \
            f"{len(_m['itens'])} artefatos **declarados** no questionário, fora da evidência do WX. Declarado não é arquivado: o que já foi submetido está em `{_p}/CATALOGO.md`, com o hash de cada arquivo. " + \
            f"O que aparece só na tabela abaixo ainda **não existe em `{_p}/`** — não use nem cite o conteúdo dele; peça a submissão por `arquivar_artefato.py`. " + \
            "A pasta é **somente leitura**: um hook recusa escrita, e só o script arquiva. Artefato marcado confidencial se cita pelo nome, nunca copiando o conteúdo.\n\n" + \
            "| arquivo | tipo | onde usar |\n| --- | --- | --- |\n" + _linhas + "\n"
    if q.get("J_economia_de_tokens", {}).get("ativar") and q["J_economia_de_tokens"].get("instalar_estilo_no_claude_md", True):
        claude_md = claude_md.rstrip("\n") + "\n" + ESTILO_DE_RESPOSTA
    saida.append(write_new(projeto / "CLAUDE.md", claude_md))

    if q.get("F_estilo_impeccable", {}).get("ativar"):
        q["F_estilo_impeccable"]["_raiz_de_evidencias"] = str((projeto / q.get("projeto", {}).get("raiz_de_evidencias", "./inputs")).resolve())
        saida.append(write_new(projeto / "DESIGN.md", esboco_design(q)))
        saida.append(write_new(projeto / "PRODUCT.md", esboco_product(q)))

    if q.get("H_backend", {}).get("perfil") or q.get("H_backend", {}).get("processo", {}).get("estrategia"):
        saida.append(write_new(wx / "processo-de-conversao.md", esboco_processo(q)))

    k = q.get("K_ambiente")
    if k:
        for chave in ("K2_postgresql", "K3_mysql", "K4_mariadb"):
            _papeis_ok((k.get(chave, {}) or {}).get("papeis", []), chave)
        amb = wx / "ambiente"
        e0 = q.get("0_empresa_e_projeto") or {}
        saida += [write_new(wx / "ambiente.md", esboco_ambiente(k, e0)),
                  write_new(amb / "instalar-ambiente.sh", instalador(k, e0)),
                  write_new(amb / ".env.exemplo", env_exemplo(k, e0))]
        k8 = k.get("K8_backup_e_replicacao") or {}
        if k8:
            saida.append(write_new(amb / "backup-e-replicacao.md", plano_de_backup(k8, (q.get("projeto") or {}).get("nome") or "projeto")))
        if (k.get("K2_postgresql", {}) or {}).get("instalar_ou_atualizar"):
            saida.append(write_new(amb / "papeis-postgresql.sql", sql_papeis_postgresql(k["K2_postgresql"])))
        if (k.get("K3_mysql", {}) or {}).get("instalar_ou_atualizar"):
            saida.append(write_new(amb / "papeis-mysql.sql", sql_papeis_mysql(k["K3_mysql"], "MySQL")))
        if (k.get("K4_mariadb", {}) or {}).get("instalar_ou_atualizar"):
            saida.append(write_new(amb / "papeis-mariadb.sql", sql_papeis_mysql(k["K4_mariadb"], "MariaDB")))
        k7 = k.get("K7_n8n", {}) or {}
        if k7.get("instalar"):
            if k7.get("modo") not in ("docker", "npm", "cloud"):
                raise ValueError(f"K7_n8n.modo {k7.get('modo')!r} desconhecido (docker | npm | cloud)")
            saida.append(write_new(amb / "n8n" / "docker-compose.yml", n8n_compose(k7, k.get("K2_postgresql", {}) or {})))
            if (k7.get("banco") or {}).get("tipo") == "postgresql":
                saida.append(write_new(amb / "n8n" / "banco-n8n.sql", n8n_banco_sql(k7)))
            saida.append(write_new(amb / "n8n" / "integracao.md", n8n_integracao_md(k7)))
        try:
            os.chmod(amb / "instalar-ambiente.sh", 0o755)
        except OSError:
            pass

    e = q.get("0_empresa_e_projeto")
    if e:
        raiz = (projeto / q.get("projeto", {}).get("raiz_de_evidencias", "./inputs")).resolve()
        saida += [
            write_new(wx / "empresa.md", esboco_empresa(q, raiz) + marca),
            write_new(wx / "pmo" / "projeto.json", json.dumps(montar_projeto_pmo(e), ensure_ascii=False, indent=2) + "\n"),
            write_new(wx / "pmo" / "organograma.md", esboco_organograma(e, raiz)),
            write_new(wx / "pmo" / "fluxograma.md", esboco_fluxograma(e, raiz)),
            write_new(wx / "pmo" / "cronograma.md", esboco_cronograma(e)),
            write_new(wx / "pmo" / "riscos.md", riscos_iniciais(e)),
            write_new(wx / "entrega.json", json.dumps(montar_entrega(e), ensure_ascii=False, indent=2) + "\n"),
        ]

    l = q.get("L_contexto_e_implantacao")
    if l:
        imp = l.get("L3_implantacao") or {}; l4 = l.get("L4_hooks_do_projeto") or {}; l5 = l.get("L5_mcp_e_skills") or {}
        if imp.get("alvo") not in (None, "", "easypanel-vps", "docker", "windows-servico", "iis", "cloud", "nenhum"):
            raise ValueError(f"L3_implantacao.alvo {imp.get('alvo')!r} desconhecido")
        for m in l5.get("mcps", []) or []:
            if m not in MCPS:
                raise ValueError(f"L5 mcp {m!r} desconhecido (aceitos: {', '.join(MCPS)})")
        saida += [write_new(wx / "prompts" / "kickoff.md", prompt_kickoff(q)),
                  write_new(wx / "prompts" / "prototipacao.md", prompt_prototipacao(q)),
                  write_new(projeto / ".claude" / "settings.json", json.dumps(settings_do_projeto(l4), ensure_ascii=False, indent=2) + "\n"),
                  write_new(projeto / ".claude" / "skills" / "regras-do-legado" / "SKILL.md", skill_regras_do_legado(q)),
                  write_new(projeto / ".claude" / "skills" / "legado-para-destino" / "SKILL.md", skill_legado_para_destino(q))]
        if l4.get("comando_de_teste"):
            saida.append(write_new(projeto / ".claude" / "hooks" / "testar.sh", "#!/usr/bin/env bash\n# Roda ao parar: o erro aparece aqui, nao em producao.\nset -o pipefail\n" + l4["comando_de_teste"] + " 2>&1 | tail -40\n"))
        if l4.get("comando_de_lint"):
            saida.append(write_new(projeto / ".claude" / "hooks" / "lint.sh", "#!/usr/bin/env bash\nset -o pipefail\n" + l4["comando_de_lint"] + " 2>&1 | tail -20\n"))
        l6 = l.get("L6_esqueleto_erp") or {}
        if l6.get("gerar"):
            import esqueleto_erp  # noqa: E402  (mesma pasta; sys.path ja tem o diretorio do script)
            for rel, corpo in esqueleto_erp.arquivos(q).items():
                saida.append(write_new(projeto / rel, corpo))
            saida.append(write_new(projeto / "docs" / "skills-recomendadas.md", esqueleto_erp.skills_recomendadas(q)))
        if l5.get("mcps"):
            saida.append(write_new(projeto / ".mcp.json", json.dumps({"mcpServers": {m: MCPS[m] for m in l5["mcps"]}}, ensure_ascii=False, indent=2) + "\n"))
        perfil = str((q.get("H_backend") or {}).get("perfil", "")).lower()
        if imp.get("dockerfile") and perfil in DOCKERFILES:
            saida.append(write_new(projeto / "Dockerfile", DOCKERFILES[perfil].format(bin=_slug((q.get("projeto") or {}).get("nome")), porta=imp.get("porta", 8080), health=imp.get("healthcheck", "/health"))))
        if imp.get("docker_compose"):
            k2 = ((q.get("K_ambiente") or {}).get("K2_postgresql") or {})
            vars_ = "\n".join(f"      - {v}=${{{v}}}" for v in imp.get("variaveis_de_ambiente", []) or [])
            comp = ("# Servico + banco, gerado do questionario (L3). Valores vem do .env (nunca versionado).\nservices:\n  app:\n    build: .\n    restart: unless-stopped\n"
                    f"    ports: [\"{imp.get('porta', 8080)}:{imp.get('porta', 8080)}\"]\n    environment:\n{vars_ or '      - RUST_LOG=info'}\n")
            if k2.get("instalar_ou_atualizar"):
                comp += (f"    depends_on: [db]\n  db:\n    image: postgres:{k2.get('versao', '16')}\n    restart: unless-stopped\n    environment:\n      - POSTGRES_DB={k2.get('banco') or 'app'}\n      - POSTGRES_USER={k2.get('superusuario', 'postgres')}\n      - POSTGRES_PASSWORD=${{{k2.get('senha_ref', 'PGPASSWORD')}}}\n    volumes: [db_data:/var/lib/postgresql/data]\nvolumes:\n  db_data:\n")
            saida.append(write_new(projeto / "docker-compose.yml", comp))
        for sh in (projeto / ".claude" / "hooks").glob("*.sh"):
            try:
                os.chmod(sh, 0o755)
            except OSError:
                pass
        gi = projeto / ".gitignore"
        if not gi.exists():
            saida.append(write_new(gi, "# nunca versionar segredos nem gerados\n.env\n.env.*\n!.env.exemplo\n.claude/settings.local.json\n__pycache__/\n*.pyc\ntarget/\nnode_modules/\n.claude/worktrees/\n"))

    # Artefatos (bloco M): a pasta e o LEIA-ME saem daqui; o CATALOGO.md e o
    # registro.json sao do arquivar_artefato.py, que e quem conhece os hashes.
    m = q.get("M_artefatos") or {}
    if m:
        art = (projeto / (m.get("pasta") or "./artefatos")).resolve()
        if projeto.resolve() not in art.parents:
            raise ValueError(f"M_artefatos.pasta {m.get('pasta')!r} tem de ficar dentro do projeto")
        art.mkdir(parents=True, exist_ok=True)
        saida.append(write_new(art / "LEIA-ME.md", leia_me_artefatos(q, art.name)))
        for tipo in sorted({(i.get("tipo") or "outro") for i in (m.get("itens") or [])}):
            if not re.fullmatch(r"[a-z][a-z-]{1,30}", tipo):
                raise ValueError(f"M_artefatos: tipo {tipo!r} invalido")
            (art / tipo).mkdir(exist_ok=True)
            saida.append(write_new(art / tipo / ".gitkeep", ""))

    # As respostas legiveis: regravadas sempre, porque sao renderizacao do JSON.
    resp = wx / "respostas_questionario.md"
    resp.write_text(respostas_md(q) + "\n", encoding="utf-8")
    saida.append(f"UPDATED {resp}")

    # O mapa de arquivos: regravado sempre, por ultimo, para refletir o que existe.
    idx = projeto / "INDEX_FILES.md"
    idx.write_text("", encoding="utf-8")
    idx.write_text(index_files(q, projeto) + "\n", encoding="utf-8")
    saida.append(f"UPDATED {idx}")

    for linha in saida:
        print(linha)
    print(f"modo={config['mode']} destino={config['target']['language'] or '(vazio)'} ui={config['fidelity']['ui']}")
    return 0


# Registro das operacoes do plugin (.wx-migration/logs/): sem projeto por
# perto, nao grava nada; falha de registro nunca derruba a operacao.
try:
    import registro
except ImportError:  # rodando de outro diretorio
    sys.path.insert(0, str(Path(__file__).resolve().parent))
    import registro


if __name__ == "__main__":
    try:
        sys.exit(registro.envolver(__file__, main))
    except (OSError, ValueError, json.JSONDecodeError) as exc:
        print(f"erro: {exc}", file=sys.stderr)
        sys.exit(2)
