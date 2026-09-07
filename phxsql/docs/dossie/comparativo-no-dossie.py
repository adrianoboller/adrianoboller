#!/usr/bin/env python3
"""Escreve a tabela comparativa da §33 do dossie, do `resultados.json` medido.

    python3 docs/dossie/comparativo-no-dossie.py [dossie-phxsql-0.18.html]

O setimo gerador da pasta, e ele existe pela lei que fez nascer os seis
anteriores: **todo numero visivel sai de um gerador, ou esta errado e ninguem
percebeu ainda.** A §33 e a secao do «o que este motor nao faz», e ate agora
ela era prosa inteira -- uma lista de ausencias que envelhecia como envelhece
toda lista escrita a mao. A do HFSQL.md envelheceu SEIS vezes.

Aqui a lista sai do `bancada/comparativo/resultados.json`, com a data da
medicao ao lado e a procedencia de cada celula. Refaca com:

    python3 bancada/comparativo/medir.py
    python3 docs/dossie/comparativo-no-dossie.py
"""

import datetime
import html
import json
import pathlib
import sys

AQUI = pathlib.Path(__file__).resolve().parent
RAIZ = AQUI.parent.parent
FONTE = RAIZ / "bancada" / "comparativo" / "resultados.json"
COBERTURA = RAIZ / "bancada" / "cobertura-da-tela" / "resultados.json"
SVG_FLUXO = AQUI / "fig-fluxo-do-medidor.svg"
SVG_WORKFLOW = AQUI / "fig-workflow-da-rodada.svg"

# Os tokens de cor do dossie, para o SVG SOLTO abrir sozinho. Dentro do
# dossie eles ja existem; fora dele, `var(--ok)` sem definicao vira preto
# e a figura perde justamente a distincao que ela existe para mostrar.
# Ele entra DENTRO do `<svg>`: um arquivo `.svg` que comeca com `<style>` nao
# tem `<svg>` como raiz, e deixa de ser SVG -- o navegador mostra texto cru.
TOKENS_SOLTOS = ("<style>svg{--ok:#2f7a3e;--log:#b71414;--pend:#8a6a1f;"
                 "--acento:#c63c0a;color:#4a3f3a;background:#fbf9f7}</style>")


NOMEADAS = {"&middot;": "\u00b7", "&mdash;": "\u2014", "&nbsp;": "\u00a0",
            "&laquo;": "\u00ab", "&raquo;": "\u00bb"}


def solto(svg: str) -> str:
    """O mesmo desenho, agora valido como ARQUIVO e nao so embutido no HTML.

    Duas diferencas, e as duas so aparecem abrindo o arquivo:

    - **`xmlns`**: dentro do HTML o parser assume o namespace SVG; num `.svg`
      solto, sem ele o arquivo nao carrega como imagem -- largura zero, e
      nenhum erro na tela.
    - **entidade NOMEADA**: `&middot;` e `&mdash;` sao HTML, nao XML. UMA
      delas derruba o arquivo inteiro. Viram o caractere.
    """
    for ent, ch in NOMEADAS.items():
        svg = svg.replace(ent, ch)
    corte = svg.index(">") + 1
    cabeca = svg[:corte].replace("<svg ", '<svg xmlns="http://www.w3.org/2000/svg" ', 1)
    return cabeca + "\n  " + TOKENS_SOLTOS + svg[corte:]
PADRAO = AQUI / "dossie-phxsql-0.18.html"

ABRE = "<!-- comparativo:inicio (gerado por docs/dossie/comparativo-no-dossie.py) -->"
FECHA = "<!-- comparativo:fim -->"

COLUNAS = [("phxsql", "PhxSql"), ("hfsql", "HFSQL(R)"),
           ("postgres", "PostgreSQL(R)"), ("cassandra", "Cassandra(R)"),
           ("mysql", "MySQL(R)"), ("sqlite", "SQLite(R)")]

# A marca vira `pino`, que e a classe que o dossie ja usa para estado.
PINO = {"tem": ("ok", "tem"), "meio": ("pend", "meio"),
        "nao": ("mal", "não"), "citado": ("nao", "citado"),
        "sem-motor": ("nao", "—")}


# ------------------------------------------------------------------ figura 1
#
# Coordenadas conferidas contra o `viewBox`: a coluna vermelha comeca em 690 e
# o vertice direito dos losangos esta em 585, entao nenhuma seta sai do quadro
# -- foi assim que a pagina dos graficos perdeu a faixa min-max uma vez.
def fluxograma(n_cap, n_sql, n_sonda):
    L, MEIO, CX, RX = 105, 430, 430, 800   # centros: sonda, canal, principal, vermelho
    return f'''<svg viewBox="0 0 930 700" role="img" aria-label="Fluxograma do medidor comparativo: cada uma das {n_cap} capacidades entra por um portao de corrida que exige a recusa de uma instrucao invalida por todos os motores; depois o caminho se divide entre perguntar ao motor vivo e sondar o codigo, e quatro portoes podem parar a medicao antes de qualquer veredito">
  <defs>
    <marker id="setaFlx" viewBox="0 0 10 10" refX="9" refY="5" markerWidth="6" markerHeight="6" orient="auto-start-reverse">
      <path d="M0 0 L10 5 L0 10 z" fill="currentColor"/>
    </marker>
    <marker id="setaFlxN" viewBox="0 0 10 10" refX="9" refY="5" markerWidth="6" markerHeight="6" orient="auto-start-reverse">
      <path d="M0 0 L10 5 L0 10 z" fill="var(--log)"/>
    </marker>
    <marker id="setaFlxO" viewBox="0 0 10 10" refX="9" refY="5" markerWidth="6" markerHeight="6" orient="auto-start-reverse">
      <path d="M0 0 L10 5 L0 10 z" fill="var(--ok)"/>
    </marker>
  </defs>
  <g font-family="IBM Plex Mono, monospace" font-size="11.5" fill="currentColor">

    <!-- portao da corrida: acontece UMA vez, antes de qualquer celula -->
    <rect x="200" y="16" width="460" height="44" rx="4" fill="none" stroke="var(--acento)" stroke-width="1.6"/>
    <text x="{MEIO}" y="34" text-anchor="middle" font-size="10" fill="var(--acento)" letter-spacing="1.4">PORTÃO DA CORRIDA &#8212; UMA VEZ, ANTES DE TUDO</text>
    <text x="{MEIO}" y="50" text-anchor="middle">CREATE ZZZZ tem de ser RECUSADO pelos quatro</text>
    <path d="M660 38 L{RX-110} 38" stroke="var(--log)" stroke-width="1.4" marker-end="url(#setaFlxN)"/>
    <rect x="690" y="14" width="220" height="48" rx="4" fill="var(--log)" fill-opacity=".16" stroke="var(--log)" stroke-width="1.5"/>
    <text x="{RX}" y="30" text-anchor="middle" font-size="9.5" fill="var(--log)">se algum aceitar,</text>
    <text x="{RX}" y="44" text-anchor="middle" font-size="10.5" fill="var(--log)" font-weight="600">O MEDIDOR MENTE</text>
    <text x="{RX}" y="57" text-anchor="middle" font-size="9.5" opacity=".8">quem falha é ele, não o motor</text>

    <path d="M{MEIO} 60 L{MEIO} 86" stroke="currentColor" stroke-width="1.4" marker-end="url(#setaFlx)"/>

    <!-- entrada -->
    <rect x="280" y="88" width="300" height="40" rx="4" fill="none" stroke="currentColor" stroke-width="1.4"/>
    <text x="{CX}" y="113" text-anchor="middle">uma das {n_cap} capacidades</text>
    <path d="M{CX} 128 L{CX} 144" stroke="currentColor" stroke-width="1.4" marker-end="url(#setaFlx)"/>

    <!-- divisao: motor vivo ou sonda de codigo -->
    <polygon points="{CX},146 585,176 {CX},206 275,176" fill="none" stroke="currentColor" stroke-width="1.4"/>
    <text x="{CX}" y="180" text-anchor="middle">SQL alcança este motor?</text>
    <text x="{CX + 12}" y="230" text-anchor="start" font-size="10" opacity=".75">sim &#8212; {n_sql} linhas</text>
    <path d="M{CX} 206 L{CX} 246" stroke="currentColor" stroke-width="1.4" marker-end="url(#setaFlx)"/>
    <path d="M275 176 L{L} 176 L{L} 246" fill="none" stroke="currentColor" stroke-width="1.4" marker-end="url(#setaFlx)"/>
    <text x="150" y="168" font-size="10" opacity=".75">não &#8212; {n_sonda}</text>

    <!-- lane da sonda de codigo -->
    <rect x="15" y="246" width="180" height="44" rx="4" fill="none" stroke="currentColor" stroke-width="1.3" opacity=".85"/>
    <text x="{L}" y="264" text-anchor="middle" font-size="10.5">citar(): arquivo, linha</text>
    <text x="{L}" y="280" text-anchor="middle" font-size="10.5">E o trecho citado</text>
    <path d="M{L} 290 L{L} 306" stroke="currentColor" stroke-width="1.3" marker-end="url(#setaFlx)"/>
    <polygon points="{L},308 195,336 {L},364 15,336" fill="none" stroke="currentColor" stroke-width="1.3"/>
    <text x="{L}" y="340" text-anchor="middle" font-size="10.5">lista veio vazia?</text>
    <path d="M{L} 364 L{L} 404" stroke="var(--log)" stroke-width="1.4" marker-end="url(#setaFlxN)"/>
    <text x="{L + 14}" y="382" text-anchor="middle" font-size="10" fill="var(--log)">sim</text>
    <rect x="15" y="406" width="180" height="40" rx="4" fill="var(--log)" fill-opacity=".16" stroke="var(--log)" stroke-width="1.5"/>
    <text x="{L}" y="422" text-anchor="middle" font-size="10.5" fill="var(--log)" font-weight="600">SONDA QUEBRADA</text>
    <text x="{L}" y="437" text-anchor="middle" font-size="10" opacity=".8">vazio não é ausência</text>
    <path d="M195 336 L230 336 L230 644 L274 644" fill="none" stroke="currentColor" stroke-width="1.3" marker-end="url(#setaFlx)"/>
    <text x="238" y="330" font-size="10" opacity=".75">não</text>

    <!-- lane do motor vivo -->
    <rect x="280" y="246" width="300" height="44" rx="4" fill="none" stroke="currentColor" stroke-width="1.4"/>
    <text x="{CX}" y="264" text-anchor="middle">recompila o phxsqld</text>
    <text x="{CX}" y="281" text-anchor="middle" font-size="10" opacity=".75">binário velho mede o passado</text>
    <path d="M{CX} 290 L{CX} 306" stroke="currentColor" stroke-width="1.4" marker-end="url(#setaFlx)"/>

    <polygon points="{CX},306 585,336 {CX},366 275,336" fill="none" stroke="currentColor" stroke-width="1.4"/>
    <text x="{CX}" y="340" text-anchor="middle">a mesa nasceu?</text>
    <path d="M585 336 L{RX-110} 336" stroke="var(--log)" stroke-width="1.4" marker-end="url(#setaFlxN)"/>
    <text x="620" y="330" font-size="10" fill="var(--log)">não</text>
    <rect x="690" y="316" width="220" height="40" rx="4" fill="var(--log)" fill-opacity=".16" stroke="var(--log)" stroke-width="1.5"/>
    <text x="{RX}" y="332" text-anchor="middle" font-size="10.5" fill="var(--log)" font-weight="600">MESA NÃO POSTA</text>
    <text x="{RX}" y="347" text-anchor="middle" font-size="10" opacity=".8">recusa não lida vira ausência</text>
    <path d="M{CX} 366 L{CX} 396" stroke="currentColor" stroke-width="1.4" marker-end="url(#setaFlx)"/>

    <polygon points="{CX},396 585,426 {CX},456 275,426" fill="none" stroke="currentColor" stroke-width="1.4"/>
    <text x="{CX}" y="430" text-anchor="middle">lê de volta v = 42?</text>
    <path d="M585 426 L{RX-110} 426" stroke="var(--log)" stroke-width="1.4" marker-end="url(#setaFlxN)"/>
    <text x="620" y="420" font-size="10" fill="var(--log)">não</text>
    <rect x="690" y="406" width="220" height="40" rx="4" fill="var(--log)" fill-opacity=".16" stroke="var(--log)" stroke-width="1.5"/>
    <text x="{RX}" y="422" text-anchor="middle" font-size="10.5" fill="var(--log)" font-weight="600">LEITOR QUEBRADO</text>
    <text x="{RX}" y="437" text-anchor="middle" font-size="10" opacity=".8">o instrumento, não o motor</text>
    <path d="M{CX} 456 L{CX} 478" stroke="currentColor" stroke-width="1.4" marker-end="url(#setaFlx)"/>

    <rect x="280" y="478" width="300" height="44" rx="4" fill="none" stroke="currentColor" stroke-width="1.4"/>
    <text x="{CX}" y="496" text-anchor="middle">exercita o EFEITO</text>
    <text x="{CX}" y="513" text-anchor="middle" font-size="10" opacity=".75">nunca o «aceitou» da resposta</text>
    <path d="M{CX} 522 L{CX} 538" stroke="currentColor" stroke-width="1.4" marker-end="url(#setaFlx)"/>

    <polygon points="{CX},538 585,568 {CX},598 275,568" fill="none" stroke="currentColor" stroke-width="1.4"/>
    <text x="{CX}" y="572" text-anchor="middle">o caso legítimo passa?</text>
    <path d="M585 568 L{RX-110} 568" stroke="var(--log)" stroke-width="1.4" marker-end="url(#setaFlxN)"/>
    <text x="620" y="562" font-size="10" fill="var(--log)">não</text>
    <rect x="690" y="548" width="220" height="40" rx="4" fill="var(--log)" fill-opacity=".16" stroke="var(--log)" stroke-width="1.5"/>
    <text x="{RX}" y="564" text-anchor="middle" font-size="10.5" fill="var(--log)" font-weight="600">VEREDITO ANULADO</text>
    <text x="{RX}" y="579" text-anchor="middle" font-size="10" opacity=".8">recusa sozinha não prova</text>
    <path d="M{CX} 598 L{CX} 622" stroke="var(--ok)" stroke-width="1.6" marker-end="url(#setaFlxO)"/>

    <rect x="274" y="624" width="312" height="42" rx="4" fill="var(--ok)" fill-opacity=".12" stroke="var(--ok)" stroke-width="1.6"/>
    <text x="{CX}" y="643" text-anchor="middle" font-size="11.5">
      <tspan fill="var(--ok)" font-weight="600">tem</tspan><tspan opacity=".55"> · </tspan><tspan fill="var(--pend)" font-weight="600">meio</tspan><tspan opacity=".55"> · </tspan><tspan fill="var(--log)" font-weight="600">não</tspan>
    </text>
    <text x="{CX}" y="659" text-anchor="middle" font-size="10" opacity=".8">com a recusa e a procedência guardadas</text>

    <text x="15" y="688" font-size="10.5" opacity=".65">Todo portão PARA a medição. Nenhum devolve «não sei» — porque «não achei» e «não sei olhar» produzem a mesma frase, e ela convence nas duas.</text>
  </g>
</svg>'''


# ------------------------------------------------------------------ figura 2
def workflow(n_cap, n_ops, n_tela):
    def cab(x, w, txt):
        return (f'<text x="{x + w // 2}" y="30" text-anchor="middle" font-size="10" '
                f'fill="var(--acento)" letter-spacing="1.3">{txt}</text>'
                f'<line x1="{x}" y1="38" x2="{x + w}" y2="38" stroke="var(--acento)" stroke-width="1" opacity=".45"/>')

    def caixa(x, y, w, h, l1, l2, cor="currentColor", larg="1.3", op=""):
        f = f' fill="{cor}" fill-opacity="{op}"' if op else ' fill="none"'
        s = (f'<rect x="{x}" y="{y}" width="{w}" height="{h}" rx="4"{f} '
             f'stroke="{cor}" stroke-width="{larg}"/>'
             f'<text x="{x + w // 2}" y="{y + 20}" text-anchor="middle" font-size="10.5">{l1}</text>')
        if l2:
            s += (f'<text x="{x + w // 2}" y="{y + 35}" text-anchor="middle" '
                  f'font-size="9.5" opacity=".75">{l2}</text>')
        return s

    def seta(x1, y1, x2, y2, cor="currentColor"):
        return (f'<path d="M{x1} {y1} L{x2} {y2}" stroke="{cor}" stroke-width="1.3" '
                f'marker-end="url(#setaWf)"/>')

    C1, C2, C3, C4 = 20, 262, 502, 742
    W1, W2, W3, W4 = 200, 198, 198, 176
    p = []
    p.append(cab(C1, W1, "1 &#183; MEDIR"))
    p.append(cab(C2, W2, "2 &#183; O QUE FICA GRAVADO"))
    p.append(cab(C3, W3, "3 &#183; OS GERADORES"))
    p.append(cab(C4, W4, "4 &#183; O QUE O DONO VÊ"))

    # coluna 1 -- os medidores, e a prova que os guarda
    p.append(caixa(C1, 52, W1, 44, "comparativo/medir.py", "4 motores vivos + sonda"))
    p.append(caixa(C1, 116, W1, 44, "cobertura-da-tela/medir.py", "as 2 listas saem do código"))
    p.append(caixa(C1, 214, W1, 58, "prova-dos-portoes.py", "repõe o defeito, exige a parada",
                   "var(--ok)", "1.6", ".10"))
    p.append(f'<path d="M{C1 + W1 // 2} 214 L{C1 + W1 // 2} 168" stroke="var(--ok)" '
             f'stroke-width="1.5" marker-end="url(#setaWfO)"/>')
    p.append(f'<text x="{C1 + W1 // 2 + 8}" y="192" font-size="9.5" fill="var(--ok)">guarda os dois</text>')
    p.append(f'<text x="{C1}" y="292" font-size="9.5" opacity=".75">e exige que SEM defeito o medidor</text>')
    p.append(f'<text x="{C1}" y="305" font-size="9.5" opacity=".75">vá até o fim — senão um que parasse</text>')
    p.append(f'<text x="{C1}" y="318" font-size="9.5" opacity=".75">sempre passaria em tudo.</text>')

    # coluna 2 -- o que sobrevive a sessao
    p.append(caixa(C2, 52, W2, 44, "comparativo/", f"resultados.json · {n_cap} linhas"))
    p.append(caixa(C2, 116, W2, 44, "cobertura-da-tela/", f"resultados.json · {n_ops} ops"))
    p.append(caixa(C2, 180, W2, 44, "docs/PENDENCIAS.md", "este é escrito à mão"))
    p.append(seta(C1 + W1, 74, C2 - 6, 74))
    p.append(seta(C1 + W1, 138, C2 - 6, 138))

    # coluna 3 -- geradores
    for i, (y, nome) in enumerate([(52, "documento.py"), (116, "comparativo-no-dossie.py"),
                                   (180, "pagina-dos-pedidos.py"), (244, "pagina-dos-testes.py")]):
        p.append(caixa(C3, y, W3, 44, nome, ""))
    p.append(seta(C2 + W2, 74, C3 - 6, 74))
    p.append(f'<path d="M{C2 + W2} 82 L{C3 - 20} 82 L{C3 - 20} 138 L{C3 - 6} 138" fill="none" stroke="currentColor" stroke-width="1.3" marker-end="url(#setaWf)"/>')
    p.append(f'<path d="M{C2 + W2} 90 L{C3 - 34} 90 L{C3 - 34} 266 L{C3 - 6} 266" fill="none" stroke="currentColor" stroke-width="1.3" marker-end="url(#setaWf)"/>')
    p.append(f'<path d="M{C2 + W2} 146 L{C3 - 27} 146 L{C3 - 27} 274 L{C3 - 6} 274" fill="none" stroke="currentColor" stroke-width="1.3" marker-end="url(#setaWf)"/>')
    p.append(seta(C2 + W2, 202, C3 - 6, 202))

    # coluna 4 -- as saidas
    for y, nome, sub in [(52, "docs/COMPARATIVO.md", "não se edita"),
                         (116, "dossiê §33", "artefato 5c14044e"),
                         (180, "pedidos.html", "artefato d6c8f13c"),
                         (244, "testes.html", "artefato 0c069766")]:
        p.append(caixa(C4, y, W4, 44, nome, sub))
    for y in (74, 138, 202, 266):
        p.append(seta(C3 + W3, y, C4 - 6, y))

    p.append(f'<line x1="{C1}" y1="336" x2="918" y2="336" stroke="currentColor" stroke-width="1" opacity=".22"/>')
    p.append(f'<text x="{C1}" y="358" font-size="11" opacity=".85"><tspan font-weight="600">A regra que fecha o desenho:</tspan> nada da coluna 4 se edita. Quem quiser mudar uma palavra mexe na coluna 3, e quem quiser</text>')
    p.append(f'<text x="{C1}" y="375" font-size="11" opacity=".85">mudar um número mexe na coluna 1 — porque número digitado à mão envelhece calado, e a receita do número envelhece junto.</text>')
    p.append(f'<text x="{C1}" y="399" font-size="10.5" opacity=".6">Medido nesta rodada: {n_cap} capacidades, {n_ops} operações no catálogo, {n_tela} alcançadas pela tela. Os três saem da coluna 1, nenhum do teclado.</text>')

    corpo = "\n    ".join(p)
    return f'''<svg viewBox="0 0 930 415" role="img" aria-label="Diagrama de workflow da rodada: dois medidores gravam resultados JSON, quatro geradores leem esses arquivos e o PENDENCIAS escrito a mao, e produzem quatro saidas que ninguem edita; a prova dos portoes guarda os medidores repondo cada defeito">
  <defs>
    <marker id="setaWf" viewBox="0 0 10 10" refX="9" refY="5" markerWidth="6" markerHeight="6" orient="auto-start-reverse">
      <path d="M0 0 L10 5 L0 10 z" fill="currentColor"/>
    </marker>
    <marker id="setaWfO" viewBox="0 0 10 10" refX="9" refY="5" markerWidth="6" markerHeight="6" orient="auto-start-reverse">
      <path d="M0 0 L10 5 L0 10 z" fill="var(--ok)"/>
    </marker>
  </defs>
  <g font-family="IBM Plex Mono, monospace" font-size="10.5" fill="currentColor">
    {corpo}
  </g>
</svg>'''


def bloco(d, c, n_fig):
    linhas = d["linhas"]
    vivos = d["motores_vivos"]
    quando = datetime.datetime.fromisoformat(d["quando"]).strftime("%d/%m/%Y")
    faltam = sum(1 for l in linhas if l["phxsql"][0] in ("nao", "meio"))
    por_sql = sum(1 for l in linhas if l["como"] == "SQL no motor vivo")

    t = [ABRE]
    t.append(f'  <p><strong>{faltam} de {len(linhas)} capacidades</strong> '
             f'faltam ou estão pela metade aqui, medido em <strong>{quando}</strong>. '
             f'<strong>{por_sql}</strong> destas linhas foram perguntadas '
             f'<strong>por SQL a motores vivos nesta máquina</strong> — a mesma '
             f'instrução na língua de cada um — e o resto por sonda de código '
             f'com arquivo, linha e o trecho citado. HFSQL(R) e Cassandra(R) '
             f'entram <strong>citados</strong>, porque não há motor nem folha '
             f'nesta sessão: célula citada não é medida, e a tabela diz isso '
             f'em cada uma. O documento inteiro, com a recusa que cada motor '
             f'devolveu, é o <code>docs/COMPARATIVO.md</code>.</p>')
    t.append('  <div class="rolo">')
    t.append('    <table>')
    t.append('      <thead><tr><th>capacidade</th><th>como</th>'
             + "".join(f'<th>{html.escape(n)}</th>' for _, n in COLUNAS)
             + '</tr></thead>')
    t.append('      <tbody>')
    for l in linhas:
        como = "SQL" if l["como"] == "SQL no motor vivo" else "sonda"
        cels = "".join(
            f'<td><span class="pino {PINO[l[c][0]][0]}">{PINO[l[c][0]][1]}</span></td>'
            for c, _ in COLUNAS)
        # O titulo vem com crase de Markdown; aqui ela vira <code>.
        titulo = html.escape(l["titulo"]).replace("`", "\x00")
        partes = titulo.split("\x00")
        titulo = "".join(p if i % 2 == 0 else f"<code>{p}</code>"
                         for i, p in enumerate(partes))
        t.append(f'        <tr><td>{titulo}</td><td class="dado">{como}</td>{cels}</tr>')
    t.append('      </tbody>')
    t.append('    </table>')
    t.append('  </div>')
    versoes = ", ".join(f"{n} {html.escape(vivos[c].split()[0])}"
                        for c, n in COLUNAS if c in vivos)
    t.append(f'  <p class="dado" style="font-size:13px">Responderam: {versoes}.</p>')

    # --- as duas figuras, com os rotulos vindo do MEDIDOR e nao do teclado
    por_sonda = len(linhas) - por_sql
    t.append('  <figure>')
    t.append('    <div class="fig-caixa">')
    t.append("    " + fluxograma(len(linhas), por_sql, por_sonda))
    t.append('    </div>')
    t.append(f'    <figcaption><b>Figura {n_fig}.</b> O caminho de <em>uma</em> '
             'célula até o veredito. Os quatro portões vermelhos não devolvem '
             '«não sei» — eles <strong>param</strong>, e é essa a diferença que '
             'a tabela acima depende: «não achei» e «não sei olhar» produzem a '
             'mesma frase, e ela convence nas duas.</figcaption>')
    t.append('  </figure>')

    t.append('  <figure>')
    t.append('    <div class="fig-caixa">')
    t.append("    " + workflow(len(linhas), c["operacoes"], c["alcancadas_pela_tela"]))
    t.append('    </div>')
    t.append(f'    <figcaption><b>Figura {n_fig + 1}.</b> A rodada inteira, da '
             'medição à página. Nada da coluna 4 se edita: quem quiser mudar uma '
             'palavra mexe na coluna 3, e quem quiser mudar um número mexe na '
             'coluna 1.</figcaption>')
    t.append('  </figure>')

    t.append(FECHA)
    return "\n".join(t)


def main():
    alvo = pathlib.Path(sys.argv[1]) if len(sys.argv) > 1 else PADRAO
    if not FONTE.exists():
        sys.exit(f"falta {FONTE} -- rode antes: python3 bancada/comparativo/medir.py")
    if not COBERTURA.exists():
        sys.exit(f"falta {COBERTURA} -- rode antes: "
                 "python3 bancada/cobertura-da-tela/medir.py")
    d = json.loads(FONTE.read_text(encoding="utf-8"))
    c = json.loads(COBERTURA.read_text(encoding="utf-8"))
    texto = alvo.read_text(encoding="utf-8")
    i, j = texto.find(ABRE), texto.find(FECHA)
    if i < 0 or j < 0:
        sys.exit(f"{alvo.name} nao tem as marcas comparativo:inicio/fim")

    # O NUMERO DA FIGURA sai do proprio arquivo, contando as legendas ANTES
    # deste bloco. Digitado, ele apontaria para a figura errada no dia em que
    # alguem acrescentasse uma figura acima -- e ninguem perceberia, porque
    # legenda errada nao quebra nada.
    n_fig = texto[:i].count("<b>Figura ") + 1

    linhas = d["linhas"]
    por_sql = sum(1 for l in linhas if l["como"] == "SQL no motor vivo")
    SVG_FLUXO.write_text(
        solto(fluxograma(len(linhas), por_sql, len(linhas) - por_sql)),
        encoding="utf-8")
    SVG_WORKFLOW.write_text(
        solto(workflow(len(linhas), c["operacoes"], c["alcancadas_pela_tela"])),
        encoding="utf-8")

    novo = texto[:i] + bloco(d, c, n_fig) + texto[j + len(FECHA):]
    alvo.write_text(novo, encoding="utf-8")
    print(f"{alvo.name}: bloco comparativo com {len(d['linhas'])} capacidades")
    print(f"  figuras {n_fig} e {n_fig + 1}; SVG solto em "
          f"{SVG_FLUXO.name} e {SVG_WORKFLOW.name}")


if __name__ == "__main__":
    main()
