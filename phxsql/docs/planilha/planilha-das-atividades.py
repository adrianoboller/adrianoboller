#!/usr/bin/env python3
"""Gera a planilha das atividades do PhxSql, em .xlsx.

Nenhum numero desta planilha se digita. Ela nao tem leitor proprio de nada:
importa os leitores que ja existem -- `pagina-dos-pedidos.py` para o
PENDENCIAS.md, `rollup.py` para o BACKLOG.md e o `achar_gates` do gerador do
painel PMO para o lexico do dono. *Receita duplicada e receita que diverge*, e
uma planilha que contasse os pedidos por conta propria divergiria da pagina
dos pedidos no primeiro pedido com estado esquisito.

O `openpyxl` e ferramenta de TRABALHO, nao dependencia do produto -- a mesma
classe do Playwright e do `strace`, que esta casa ja usa. A petrea de zero
dependencias vale para o motor em Rust, que continua so com a `std`.

    python3 docs/planilha/planilha-das-atividades.py [saida.xlsx]
"""

import datetime
import importlib.util
import json
import pathlib
import subprocess
import sys

RAIZ = pathlib.Path(__file__).resolve().parents[2]
SAIDA_PADRAO = RAIZ / "docs" / "planilha" / "atividades-phxsql.xlsx"


def importar(caminho, apelido):
    """Importa um gerador pelo caminho: os nomes tem hifen e nao sao modulos."""
    spec = importlib.util.spec_from_file_location(apelido, caminho)
    mod = importlib.util.module_from_spec(spec)
    sys.modules[apelido] = mod
    spec.loader.exec_module(mod)
    return mod


PEDIDOS = importar(RAIZ / "docs" / "dossie" / "pagina-dos-pedidos.py", "gen_pedidos")
BOARD = importar(RAIZ / "docs" / "pmo" / "rollup.py", "gen_rollup")
PMO = importar(RAIZ / "docs" / "pmo" / "pagina-do-status-do-projeto.py", "gen_pmo")


def limpo(html):
    """O texto de volta, sem as marcas que o `marcar` dos geradores pos."""
    return PMO.texto_puro(html)


def commits_do_dia(dia):
    """Os commits de UM dia, do proprio git log. Vazio se o git nao responder."""
    try:
        bruto = subprocess.run(
            ["git", "log", "--no-merges", f"--since={dia} 00:00",
             f"--until={dia} 23:59", "--pretty=%h\t%ad\t%s", "--date=format:%H:%M"],
            cwd=RAIZ, capture_output=True, text=True, timeout=30, check=True).stdout
    except Exception:
        return []
    return [l.split("\t", 2) for l in bruto.strip().split("\n") if l.strip()]


def capacidades():
    p = RAIZ / "CAPABILITIES.json"
    if not p.exists():
        return {}
    return json.loads(p.read_text(encoding="utf-8"))


# ----------------------------------------------------------------- escrita

def montar(saida):
    from openpyxl import Workbook
    from openpyxl.styles import Alignment, Border, Font, PatternFill, Side
    from openpyxl.utils import get_column_letter
    from openpyxl.worksheet.table import Table, TableStyleInfo

    itens = PEDIDOS.ler()
    board = BOARD.ler()
    gates = PMO.achar_gates(itens)
    cap = capacidades()
    agora = datetime.datetime.now(datetime.timezone.utc)
    hoje = agora.strftime("%Y-%m-%d")

    # A marca manda: fundo #010418, vermelhao #C63C0A. No claro do Excel o
    # vermelhao escurece, pela mesma razao de contraste ja decidida na tela.
    MARCA = "FF010418"
    ACENTO = "FFC63C0A"
    CABECA = PatternFill("solid", fgColor=MARCA)
    BRANCO = Font(color="FFFFFFFF", bold=True, size=11)
    FINO = Side(style="thin", color="FFDED6D0")
    GRADE = Border(left=FINO, right=FINO, top=FINO, bottom=FINO)
    TOPO = Alignment(vertical="top", wrap_text=True)
    CORES = {"Feito": "FF2F7A3E", "Parcial": "FF8A6A1F", "Planejado": "FF7A6D66",
             "aberto": "FF1F5C93", "entregue": "FF2F7A3E",
             "fechado": "FF2F7A3E", "parado": "FFB5257F"}

    wb = Workbook()

    def folha(nome, cabecalhos, linhas, larguras, cor_col=None):
        ws = wb.create_sheet(nome)
        ws.append(cabecalhos)
        for c in ws[1]:
            c.fill, c.font = CABECA, BRANCO
            c.alignment = Alignment(vertical="center", wrap_text=True)
        for l in linhas:
            ws.append(l)
        for i, w in enumerate(larguras, 1):
            ws.column_dimensions[get_column_letter(i)].width = w
        for linha in ws.iter_rows(min_row=2):
            for c in linha:
                c.alignment, c.border = TOPO, GRADE
            if cor_col is not None:
                v = str(linha[cor_col].value or "")
                if v in CORES:
                    linha[cor_col].font = Font(color=CORES[v], bold=True)
        ws.freeze_panes = "A2"
        if linhas:
            ref = f"A1:{get_column_letter(len(cabecalhos))}{len(linhas) + 1}"
            t = Table(displayName=f"t_{nome.replace(' ', '_').replace('-', '_')}",
                      ref=ref)
            t.tableStyleInfo = TableStyleInfo(
                name="TableStyleLight1", showRowStripes=True)
            ws.add_table(t)
        ws.sheet_properties.tabColor = ACENTO.replace("FF", "")
        return ws

    # ---------------------------------------------------------- 1. resumo
    ws = wb.active
    ws.title = "Resumo"
    ws["A1"] = "PhxSql — planilha das atividades"
    ws["A1"].font = Font(bold=True, size=16, color=ACENTO)
    ws["A2"] = (f"Gerada em {agora.strftime('%d/%m/%Y %H:%M')} UTC por "
                "docs/planilha/planilha-das-atividades.py. Nenhum numero foi "
                "digitado: todos saem do PENDENCIAS.md, do BACKLOG.md, do "
                "CAPABILITIES.json e do git log.")
    ws["A2"].alignment = Alignment(wrap_text=True, vertical="top")
    ws.merge_cells("A2:D4")

    por_estado = {}
    for i in itens:
        por_estado[i["rotulo"]] = por_estado.get(i["rotulo"], 0) + 1
    por_board = {}
    for b in board:
        por_board[b["estado"]] = por_board.get(b["estado"], 0) + 1

    resumo = [
        ("Pedidos do dono", "", ""),
        ("  total", len(itens), "PENDENCIAS.md"),
    ]
    for rot in ("Feito", "Parcial", "Planejado"):
        resumo.append((f"  {rot.lower()}", por_estado.get(rot, 0), "PENDENCIAS.md"))
    resumo += [
        ("", "", ""),
        ("Board PMO", "", ""),
        ("  total", len(board), "pmo/BACKLOG.md"),
    ]
    for est in sorted(por_board):
        resumo.append((f"  {est}", por_board[est], "pmo/BACKLOG.md"))
    resumo += [
        ("", "", ""),
        ("Travados por decisao do dono", len(gates), "PENDENCIAS.md x lexico"),
        ("", "", ""),
        ("O motor, medido", "", ""),
    ]
    quando = cap.get("medido_em", "")
    for rotulo, chave in (("  versao", "versao"), ("  testes", "testes"),
                          ("  operacoes do protocolo", "operacoes"),
                          ("  linhas de Rust", "linhas_rust"),
                          ("  crates", "crates"),
                          ("  dependencias externas", "dependencias_externas")):
        if chave in cap:
            resumo.append((rotulo, cap[chave], f"CAPABILITIES.json ({quando})"))

    ws["A6"], ws["B6"], ws["C6"] = "o que", "quanto", "de onde saiu"
    for c in ws[6]:
        if c.column <= 3:
            c.fill, c.font = CABECA, BRANCO
    for n, (a, b, c) in enumerate(resumo, 7):
        ws.cell(n, 1, a)
        ws.cell(n, 2, b)
        ws.cell(n, 3, c)
        if b == "" and a:
            ws.cell(n, 1).font = Font(bold=True, color=ACENTO)
    for col, w in ((1, 34), (2, 14), (3, 40)):
        ws.column_dimensions[get_column_letter(col)].width = w
    ws.sheet_properties.tabColor = ACENTO.replace("FF", "")

    # --------------------------------------------------------- 2. pedidos
    trava = {g["n"]: ", ".join(g["frases"]) for g in gates}
    folha("Pedidos", ["Nº", "Estado", "O que voce pediu", "Onde esta",
                      "Trava com o dono"],
          [[i["n"], i["rotulo"], limpo(i["pedido"]), limpo(i["estado"]),
            trava.get(i["n"], "")] for i in itens],
          [7, 12, 58, 110, 26], cor_col=1)

    # ----------------------------------------------------------- 3. board
    folha("Board PMO", ["Item", "Pilar", "Escalao", "Estado"],
          [[b["id"], b["pilar"], b["escalao"], b["estado"]] for b in board],
          [14, 26, 12, 14], cor_col=3)

    # ------------------------------------------------------- 4. decisoes
    folha("Decisoes do dono",
          ["Nº", "Estado", "O que voce pediu", "Frase que travou",
           "Trecho que casou"],
          [[g["n"], g["rotulo"], limpo(g["pedido"]), ", ".join(g["frases"]),
            g["trecho"]] for g in gates],
          [7, 12, 58, 26, 90], cor_col=1)

    # --------------------------------------------------------- 5. rodada
    folha(f"Rodada {hoje}", ["Commit", "Hora", "Assunto"],
          commits_do_dia(hoje), [12, 9, 110])

    saida.parent.mkdir(parents=True, exist_ok=True)
    wb.save(saida)
    return itens, board, gates


def main():
    saida = pathlib.Path(sys.argv[1]) if len(sys.argv) > 1 else SAIDA_PADRAO
    itens, board, gates = montar(saida)
    kb = saida.stat().st_size / 1024
    print(f"planilha gravada: {saida}  ({kb:.0f} KiB)")
    print(f"  pedidos   {len(itens)}")
    print(f"  board     {len(board)} itens")
    print(f"  decisoes  {len(gates)} travadas com o dono")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
