#!/usr/bin/env python3
"""Escreve a figura e a tabela da secao 17 do dossie a partir da medicao.

Existe por um motivo especifico: os numeros do dossie ja sairam errados duas
vezes por serem DIGITADOS. Numero digitado envelhece calado -- a capa passou
tres lancamentos dizendo 276 testes quando eram 280, e o rodape ficou parado
numa versao inteira.

Aqui o caminho e o contrario: le `bancada/resultados.json`, calcula, e troca o
bloco entre as marcas no HTML. Se ninguem rodar, nada muda; se rodar, o que
sai e o que foi medido.

    python3 docs/dossie/numeros-da-bancada.py

Nao mexe em mais nada do dossie.
"""

import json
import math
import pathlib
import sys

RAIZ = pathlib.Path(__file__).resolve().parents[2]
MEDICAO = RAIZ / "bancada" / "resultados.json"
# Qual dossie reescrever. O nome mudou na 0.15.0 e pode mudar de novo:
# passar o caminho como primeiro argumento evita editar o script a cada vez.
def _alvo():
    for a in sys.argv[1:]:
        if a.endswith(".html"):
            return pathlib.Path(a)
    return RAIZ / "docs" / "dossie" / "dossie-phxsql-0.15.html"


DOSSIE = _alvo()
PENDENCIAS = RAIZ / "docs" / "PENDENCIAS.md"

ABRE = "<!-- bancada:inicio (gerado por docs/dossie/numeros-da-bancada.py) -->"
FECHA = "<!-- bancada:fim -->"

# Da pior para a melhor: a secao conta uma historia, e ela comeca no buraco.
FASES = [
    ("inserir", "inserir", "Inserir {ops}"),
    ("buscar", "buscar", "Buscar {ops} pontuais"),
    ("excluir", "excluir", "Excluir {ops}"),
    ("atualizar", "atualizar", "Atualizar {ops}"),
    ("varrer", "varrer faixa", "Varrer a faixa inteira ({ops} linhas)"),
]

# Geometria da figura. O eixo do empate fica em EIXO; cada dobro de diferenca
# anda PASSO pixels para o lado de quem ganhou.
LARGURA, EIXO, PASSO = 840, 470, 70


def mil(n):
    return f"{n:,}".replace(",", ".")


def dec(x, casas=2):
    return f"{x:,.{casas}f}".replace(",", "\x00").replace(".", ",").replace("\x00", ".")


def carregar():
    dados = json.loads(MEDICAO.read_text())
    por = {}
    for d in dados:
        por.setdefault(d["fase"], {})[d["motor"]] = d
    faltando = [f for f, _, _ in FASES if len(por.get(f, {})) < 2]
    if faltando:
        sys.exit(f"medicao incompleta, faltam os dois motores em: {faltando}")
    return por


def barra(fator_phx_mais_lento):
    """Largura e lado da barra, em escala log2."""
    v = abs(math.log2(fator_phx_mais_lento))
    larg = max(3.0, v * PASSO)
    if fator_phx_mais_lento >= 1.0:  # PhxSql mais devagar: barra para a esquerda
        return EIXO - larg, larg, "esq"
    return EIXO, larg, "dir"


def figura(por):
    linhas = []
    y = 60
    for fase, rotulo, _ in FASES:
        p, m = por[fase]["PhxSql"], por[fase]["MySQL"]
        fator = p["segundos"] / m["segundos"] if m["segundos"] else 1.0
        x, larg, lado = barra(fator)
        cor = "var(--log)" if lado == "esq" else "var(--ok)"
        empate = 0.95 <= fator <= 1.05
        opac = ".85" if abs(math.log2(fator)) > 1.5 else ".6"

        linhas.append(f'          <text x="16" y="{y + 16}" font-size="11.5">{rotulo}</text>')
        if empate:
            linhas.append(
                f'          <rect x="{EIXO - 2:.0f}" y="{y}" width="4" height="22" '
                f'fill="currentColor" opacity=".45"/>'
            )
            linhas.append(
                f'          <text x="{EIXO + 12}" y="{y + 16}" font-size="11.5" opacity=".7">'
                f'{dec(p["segundos"])}&#8201;s contra {dec(m["segundos"])}&#8201;s</text>'
            )
        else:
            linhas.append(
                f'          <rect x="{x:.0f}" y="{y}" width="{larg:.0f}" height="22" '
                f'fill="{cor}" opacity="{opac}"/>'
            )
            texto = dec(fator if fator >= 1 else 1 / fator, 1) + "&#215;"
            if lado == "esq":
                linhas.append(
                    f'          <text x="{x - 8:.0f}" y="{y + 16}" text-anchor="end" '
                    f'fill="{cor}" font-size="11.5">{texto}</text>'
                )
            else:
                linhas.append(
                    f'          <text x="{x + larg + 8:.0f}" y="{y + 16}" '
                    f'fill="{cor}" font-size="11.5">{texto}</text>'
                )
        y += 34

    fundo = y + 12
    alto = fundo + 76
    return f'''      <svg viewBox="0 0 {LARGURA} {alto}" role="img" aria-label="Cinco operações comparadas com o MySQL(R) em escala logarítmica: barra à esquerda do eixo quando o PhxSql é mais devagar, à direita quando é mais rápido">
        <g font-family="IBM Plex Mono, monospace" font-size="11.5" fill="currentColor">

          <text x="16" y="26" font-size="11" opacity=".65">mais devagar que o MySQL(R)</text>
          <text x="824" y="26" text-anchor="end" font-size="11" opacity=".65">mais rápido</text>
          <line x1="{EIXO}" y1="34" x2="{EIXO}" y2="{y - 6}" stroke="currentColor" stroke-width="1.3" opacity=".55"/>
          <text x="{EIXO}" y="{y + 12}" text-anchor="middle" font-size="10" opacity=".55">empate</text>

{chr(10).join(linhas)}

          <line x1="16" y1="{fundo + 22}" x2="824" y2="{fundo + 22}" stroke="currentColor" stroke-width="1" opacity=".25"/>
          <text x="16" y="{fundo + 44}" font-size="11" opacity=".6">Escala logarítmica: cada dobro de diferença anda o mesmo tanto. Sem ela a inserção esmagaria o resto do desenho.</text>
          <text x="16" y="{fundo + 62}" font-size="11" opacity=".6">Tempo de relógio, uma passada, mesma máquina e mesmo disco, os dois motores com os mesmos dados na mesma ordem.</text>
        </g>
      </svg>'''


def pino(fator, ops_iguais=True):
    if 0.95 <= fator <= 1.05:
        return "empate"
    if fator > 1:
        return f'<span class="pino pend">MySQL(R) {dec(fator, 1)}&#215;</span>'
    return f'<span class="pino ok">PhxSql {dec(1 / fator, 1)}&#215;</span>'


def tabela(por):
    linhas = []
    for fase, _, titulo in FASES:
        p, m = por[fase]["PhxSql"], por[fase]["MySQL"]
        fator = p["segundos"] / m["segundos"] if m["segundos"] else 1.0
        linhas.append(
            f'        <tr><td>{titulo.format(ops=mil(p["operacoes"]))}</td>'
            f'<td class="num">{dec(p["segundos"])}&#8201;s</td>'
            f'<td class="num">{dec(m["segundos"])}&#8201;s</td>'
            f"<td>{pino(fator)}</td></tr>"
        )

    ins_p, ins_m = por["inserir"]["PhxSql"], por["inserir"]["MySQL"]
    esc = ins_m["escrito_mb"] / max(ins_p["escrito_mb"], 0.1)
    linhas.append(
        f"        <tr><td>Escrito no disco durante a carga</td>"
        f'<td class="num">{dec(ins_p["escrito_mb"] / 1024)}&#8201;GiB</td>'
        f'<td class="num">{dec(ins_m["escrito_mb"] / 1024)}&#8201;GiB</td>'
        f'<td><span class="pino ok">PhxSql {dec(esc, 0)}&#215;</span></td></tr>'
    )

    disco = {d["motor"]: d["bytes"] for d in json.loads(MEDICAO.read_text()) if d["fase"] == "disco"}
    if len(disco) == 2:
        f = disco["PhxSql"] / disco["MySQL"]
        linhas.append(
            f"        <tr><td>Ocupado no disco no fim</td>"
            f'<td class="num">{dec(disco["PhxSql"] / 1073741824)}&#8201;GiB</td>'
            f'<td class="num">{dec(disco["MySQL"] / 1073741824)}&#8201;GiB</td>'
            f"<td>{pino(f)}</td></tr>"
        )

    pico_p = max(por[f]["PhxSql"]["pico_rss_mb"] for f, _, _ in FASES)
    pico_m = max(por[f]["MySQL"]["pico_rss_mb"] for f, _, _ in FASES)
    linhas.append(
        f"        <tr><td>Pico de memória</td>"
        f'<td class="num">{mil(round(pico_p))}&#8201;MiB</td>'
        f'<td class="num">{mil(round(pico_m))}&#8201;MiB</td>'
        f"<td>{pino(pico_p / pico_m)}</td></tr>"
    )

    return f'''  <div class="rolo">
    <table>
      <thead><tr><th>Operação</th><th class="num">PhxSql</th><th class="num">MySQL(R)</th><th>Quem ganha</th></tr></thead>
      <tbody>
{chr(10).join(linhas)}
      </tbody>
    </table>
  </div>'''


def taxas_por_milhao():
    """Primeiro e ultimo milhao, do registro bruto da carga.

    A taxa impressa no log e ACUMULADA. A do decimo milhao sozinho sai da
    diferenca entre as duas ultimas marcas -- e e ela que mostra a queda.
    """
    log = RAIZ / "bancada" / "carga-10-milhoes.log"
    if not log.exists():
        return None
    marcas = []
    for linha in log.read_text().splitlines():
        if "PhxSql" in linha and " em " in linha:
            partes = linha.split()
            feitos = int(partes[1].replace(".", ""))
            segundos = float(partes[3].rstrip("s"))
            marcas.append((feitos, segundos))
    if len(marcas) < 2:
        return None
    primeiro = marcas[0][0] / marcas[0][1]
    d_linhas = marcas[-1][0] - marcas[-2][0]
    d_tempo = marcas[-1][1] - marcas[-2][1]
    return primeiro, (d_linhas / d_tempo if d_tempo > 0 else 0), len(marcas)


def diagnostico(por):
    p_, m_ = por["inserir"]["PhxSql"], por["inserir"]["MySQL"]
    taxa_p = p_["operacoes"] / p_["segundos"]
    taxa_m = m_["operacoes"] / m_["segundos"]
    cpu = p_["cpu_s"] / p_["segundos"] * 100

    texto = f'''  <p>{mil(round(taxa_p))} linhas por segundo contra {mil(round(taxa_m))}. O reflexo é
  culpar o disco — mas os contadores dizem outra coisa:
  <strong>{dec(p_["cpu_s"], 0)}&#8201;s de CPU para {dec(p_["segundos"], 0)}&#8201;s de relógio</strong>
  ({dec(cpu, 0)}%), e <strong>{dec(p_["lido_mb"], 1)}&#8201;MiB lidos</strong>. O processo passou o
  tempo inteiro calculando, não esperando.</p>'''

    t = taxas_por_milhao()
    if t:
        primeiro, ultimo, quantas = t
        if ultimo and primeiro > ultimo:
            queda = (1 - ultimo / primeiro) * 100
            texto += f'''

  <p>E piora com o tamanho: o primeiro milhão entra a {mil(round(primeiro))}/s, o
  {"décimo" if quantas >= 10 else str(quantas) + "º"} a {mil(round(ultimo))}/s — {dec(queda, 0)}% mais devagar no fim do que no
  começo. Taxa que cai conforme a tabela cresce, com o disco parado, é
  assinatura de estrutura de índice: a B+tree do <code>.ndx</code> reescrita nó
  a nó a cada linha, sem lote. É ali que uma rodada dedicada renderia, e não em
  recurso novo.</p>'''
    return texto


def resumo_md(por):
    """O mesmo diagnostico, em Markdown, para o PENDENCIAS.md."""
    ins_p, ins_m = por["inserir"]["PhxSql"], por["inserir"]["MySQL"]
    taxa_p = ins_p["operacoes"] / ins_p["segundos"]
    taxa_m = ins_m["operacoes"] / ins_m["segundos"]
    fator = ins_p["segundos"] / ins_m["segundos"]
    cpu = ins_p["cpu_s"] / ins_p["segundos"] * 100

    def f(fase):
        a, b = por[fase]["PhxSql"], por[fase]["MySQL"]
        return a["segundos"] / b["segundos"]

    disco = {
        d["motor"]: d["bytes"]
        for d in json.loads(MEDICAO.read_text())
        if d["fase"] == "disco"
    }

    t = taxas_por_milhao()
    queda = ""
    if t and t[1] and t[0] > t[1]:
        queda = (
            f" E **piora com o tamanho**: o primeiro milhao entra a "
            f"{mil(round(t[0]))}/s, o ultimo a {mil(round(t[1]))}/s \u2014 "
            f"{dec((1 - t[1] / t[0]) * 100, 0)}% mais devagar no fim do que no comeco."
        )
        queda = (queda.replace("primeiro milhao", "primeiro milhão")
                      .replace("o ultimo", "o último")
                      .replace("comeco", "começo"))

    var_p, var_m = por["varrer"]["PhxSql"], por["varrer"]["MySQL"]
    atu_p, atu_m = por["atualizar"]["PhxSql"], por["atualizar"]["MySQL"]

    return (
        "A bancada de 10 milhões achou um buraco só, e é grande.\n\n"
        f"**A inserção é o ponto fraco do motor.** {mil(round(taxa_p))} linhas/s contra\n"
        f"{mil(round(taxa_m))} do MySQL(R) — **{dec(fator, 1)}× mais devagar**. E o\n"
        f"diagnóstico é incômodo: **{dec(ins_p['cpu_s'], 0)} s de CPU para "
        f"{dec(ins_p['segundos'], 0)} s de relógio** ({dec(cpu, 0)}%), com\n"
        f"**{dec(ins_p['lido_mb'], 1)} MiB lidos do disco**. Não é disco, é processador — a\n"
        f"B+tree do `.ndx` reescrita nó a nó a cada linha, sem lote.{queda}\n\n"
        f"Nas outras quatro o motor se defende: a varredura por faixa é\n"
        f"**{dec(1 / f('varrer'), 1)}× mais rápida** ({dec(var_p['segundos'])} s contra "
        f"{dec(var_m['segundos'])} s), lendo as\n"
        f"{mil(var_p['operacoes'])} linhas dos dois lados e chegando à mesma soma; a\n"
        f"atualização empata ({dec(atu_p['segundos'])} s contra {dec(atu_m['segundos'])} s); a busca\n"
        f"pontual é {dec(f('buscar'), 1)}× mais devagar e a exclusão {dec(f('excluir'), 1)}×. E escreve muito\n"
        f"menos: {dec(ins_p['escrito_mb'] / 1024)} GiB contra {dec(ins_m['escrito_mb'] / 1024)} GiB na carga.\n\n"
        f"Contrapartida honesta: **ocupa {dec(disco['PhxSql'] / 1073741824)} GiB em disco contra\n"
        f"{dec(disco['MySQL'] / 1073741824)} GiB**, porque o `.reg` é de slot fixo — o preço do\n"
        "endereçamento O(1) e da ordem de digitação.\n\n"
        "Se algum dia sobrar uma rodada para o motor em vez de para recurso novo, é\n"
        "aqui que ela rende.\n\n"
        "*(Gerado por `docs/dossie/numeros-da-bancada.py` — não edite à mão.)*"
    )


def main():
    por = carregar()
    ins = por["inserir"]
    taxa_p = ins["PhxSql"]["operacoes"] / ins["PhxSql"]["segundos"]
    taxa_m = ins["MySQL"]["operacoes"] / ins["MySQL"]["segundos"]

    bloco = (
        ABRE
        + "\n"
        + figura(por)
        + "\n"
        + FECHA
        + "\n"
    )

    html = DOSSIE.read_text()
    i, j = html.find(ABRE), html.find(FECHA)
    if i < 0 or j < 0:
        sys.exit("as marcas bancada:inicio/bancada:fim nao estao no dossie")
    html = html[:i] + bloco + html[j + len(FECHA) + 1:]

    ABRE_T = "<!-- bancada:tabela:inicio -->"
    FECHA_T = "<!-- bancada:tabela:fim -->"
    i, j = html.find(ABRE_T), html.find(FECHA_T)
    if i < 0 or j < 0:
        sys.exit("as marcas bancada:tabela nao estao no dossie")
    html = html[:i] + ABRE_T + "\n" + tabela(por) + "\n" + FECHA_T + html[j + len(FECHA_T):]

    ABRE_D = "<!-- bancada:diagnostico:inicio -->"
    FECHA_D = "<!-- bancada:diagnostico:fim -->"
    i, j = html.find(ABRE_D), html.find(FECHA_D)
    if i < 0 or j < 0:
        sys.exit("as marcas bancada:diagnostico nao estao no dossie")
    html = html[:i] + ABRE_D + "\n" + diagnostico(por) + "\n" + FECHA_D + html[j + len(FECHA_D):]

    DOSSIE.write_text(html)

    # O PENDENCIAS.md repete o diagnostico da insercao. Numero repetido em dois
    # lugares e numero que um dia diverge: gerado tambem.
    md = PENDENCIAS.read_text()
    ABRE_P = "<!-- pendencias:insercao:inicio -->"
    FECHA_P = "<!-- pendencias:insercao:fim -->"
    i, j = md.find(ABRE_P), md.find(FECHA_P)
    if i >= 0 and j >= 0:
        md = md[:i] + ABRE_P + "\n" + resumo_md(por) + "\n" + FECHA_P + md[j + len(FECHA_P):]
        PENDENCIAS.write_text(md)
        print(f"  e o resumo do {PENDENCIAS.name}")

    print(f"secao 17 refeita a partir de {MEDICAO.name}")
    print(f"  inserir: PhxSql {taxa_p:,.0f}/s  MySQL(R) {taxa_m:,.0f}/s"
          .replace(",", "."))
    for fase, rotulo, _ in FASES:
        p, m = por[fase]["PhxSql"], por[fase]["MySQL"]
        print(f"  {rotulo:<14} {p['segundos']:8.2f}s  x  {m['segundos']:8.2f}s"
              f"   ({p['segundos'] / m['segundos']:.2f}x)")


if __name__ == "__main__":
    main()
