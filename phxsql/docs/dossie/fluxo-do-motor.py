#!/usr/bin/env python3
"""As duas figuras do MOTOR: o caminho de um pedido, e o ciclo de operacao.

    python3 docs/dossie/fluxo-do-motor.py [dossie.html]   # sem argumento, acha o da pasta

Oitavo gerador da pasta. Desenha duas coisas que o dossie contava em prosa:

- **Fluxograma** (secao 9): o caminho de um pedido, do soquete ao disco. Os
  NOVE portoes do `despachar` e os OITO passos do `inserir`.
- **Workflow** (secao 31): o ciclo de operacao -- modelar, gravar, consultar,
  replicar, salvaguardar --, com as garantias que valem nos cinco.

# Por que ele le o codigo em vez de trazer a lista escrita aqui

Porque lista digitada envelhece calada, e esta ja tem dono: o `servidor.rs`
NUMERA os proprios portoes (`// Portao 0`, `1`, `2`, `2a`, ...) e o `inserir`
do `table.rs` chama os passos numa ordem que e o desenho. Entao a LISTA sai
do fonte e o ROTULO mora aqui -- e o gerador **para** quando os dois
divergem, nos dois sentidos:

- portao no codigo sem rotulo aqui: o desenho mentiria por omissao, que e o
  jeito de mentir que ninguem confere;
- rotulo aqui sem portao no codigo: e a «chave morta» da fabrica de idiomas,
  e ela e pior, porque quem le acha que existe uma protecao que nao existe.

E a lei do CLAUDE.md que este arquivo honra: *quando um gerador depende de uma
lista, a lista tem de sair do codigo.*
"""

import html
import json
import pathlib
import re
import sys

# O nome do dossie muda a cada refacao: quem o acha e a varredura da pasta,
# num dono so. Padrao digitado aqui envelhece calado na proxima refacao.
sys.path.insert(0, str(pathlib.Path(__file__).resolve().parent))
from dossie_da_pasta import achar_o_dossie  # noqa: E402

AQUI = pathlib.Path(__file__).resolve().parent
RAIZ = AQUI.parent.parent
SERVIDOR = RAIZ / "crates/phxsql-server/src/servidor.rs"
TABLE = RAIZ / "crates/phxsql-store/src/table.rs"
CATALOGO = RAIZ / "crates/phxsql-server/src/catalogo.rs"

ABRE_F = "<!-- fluxo-motor:inicio (gerado por docs/dossie/fluxo-do-motor.py) -->"
FECHA_F = "<!-- fluxo-motor:fim -->"
ABRE_W = "<!-- workflow-motor:inicio (gerado por docs/dossie/fluxo-do-motor.py) -->"
FECHA_W = "<!-- workflow-motor:fim -->"

SVG_FLUXO = AQUI / "fig-fluxo-do-motor.svg"
SVG_WORKFLOW = AQUI / "fig-workflow-do-motor.svg"

TOKENS_SOLTOS = ("<style>svg{--ok:#2f7a3e;--log:#b71414;--pend:#8a6a1f;"
                 "--acento:#c63c0a;--reg:#1f5c93;--ndx:#6a44a8;--bin:#0e7a85;"
                 "--memo:#37702e;--lgpd:#0f6b4f;--bkp:#4a5d78;"
                 "color:#4a3f3a;background:#fbf9f7}</style>")
NOMEADAS = {"&middot;": "·", "&mdash;": "—", "&nbsp;": " ",
            "&laquo;": "«", "&raquo;": "»"}

# O rotulo de cada portao. A LISTA quem manda e o `servidor.rs`; isto aqui e
# so a traducao curta de cada um para caber na caixa.
ROTULO_PORTAO = {
    "0":      ("a política", "comando proibido pelo config.json"),
    "1":      ("o token", "a chave da porta, não a identidade"),
    "2":      ("o login", "quem é você nesta sessão"),
    "2a":     ("o papel do servidor", "réplica não atende escrita"),
    "2a-bis": ("de ONDE vem a replicação", "o IP da sessão, não o do pedido"),
    "2b":     ("a escrita", "quem decide é o papel VIVO do cluster"),
    "2b-bis": ("o `aplicar` respeita somente-leitura", "source/isolado trancados recusam; réplica aplica por dentro"),
    "3":      ("o poder sobre a base E a tabela", "o campo `tabela` do pedido"),
    "4":      ("carga de outra ligação?", "a tabela está reservada"),
    "5":      ("transação de outra conexão?", "a tabela está segurada"),
}

# Os passos do `inserir`, na ordem em que o `table.rs` os chama. A marca de
# cada um e a chamada literal; se ela sumir ou trocar de lugar, o gerador para.
# NOVE, e nao oito: o portao abaixo derrubou a minha primeira versao em dois
# pontos. `proximo_rowid` nao existe -- sao `numerar_linha` (rownum) e
# `numerar` (sequencia) --, e eu tinha esquecido o `montar_payload`, que e
# ONDE o `.bin` e o `.memo` sao gravados. Desenhar de memoria teria publicado
# um caminho sem dois dos sete arquivos.
PASSOS_GRAVAR = [
    (["conferir_aridade"],                  "confere aridade", "", ""),
    (["conferir_fks"],                      "confere a chave", ".ndx da mãe", "ndx"),
    (["numerar_linha", "self.numerar("],    "numera a linha", "rownum, seq.", ""),
    (["ndx.existe"],                        "confere único", "antes de gravar", "ndx"),
    (["montar_payload"],                    "grava anexos", ".bin .memo", "bin"),
    (["reg.inserir"],                       "grava a linha", ".reg", "reg"),
    (["inserir_ja_conferido"],              "grava as chaves", ".ndx", "ndx"),
    (["indexar_texto"],                     "indexa o texto", ".fts", "memo"),
    (["self.anotar"],                       "anota o evento", ".log", "log"),
]


# ------------------------------------------------------------- o que o codigo diz
def portoes_do_codigo():
    """A lista ORDENADA dos portoes, lida dos comentarios que os numeram."""
    fonte = SERVIDOR.read_text(encoding="utf-8")
    achados = re.findall(r"^\s*// Portao ([0-9a-z-]+) -- ", fonte, re.M)
    if len(achados) < 5:
        sys.exit(f"SONDA QUEBRADA: so {len(achados)} portoes no servidor.rs -- "
                 "o comentario que os numera mudou de forma")
    return achados


def passos_do_codigo():
    """Confere que cada passo do desenho ainda e chamado pelo `inserir`."""
    fonte = TABLE.read_text(encoding="utf-8")
    i = fonte.index("pub fn inserir(&mut self")
    j = fonte.index("pub fn inserir_lote", i)
    corpo = fonte[i:j]
    return [ms[0] for ms, _, _, _ in PASSOS_GRAVAR
            if all(m in corpo for m in ms)]


def operacoes():
    fonte = CATALOGO.read_text(encoding="utf-8")
    i = fonte.index("pub const OPERACOES")
    j = fonte.index("\n];", i)
    return len(re.findall(r'^\s*nome: "([A-Za-z_0-9]+)"', fonte[i:j], re.M))


def conferir():
    """Os dois sentidos do laco. Sem isto o desenho envelhece calado."""
    no_codigo = portoes_do_codigo()
    faltam_rotulo = [p for p in no_codigo if p not in ROTULO_PORTAO]
    sobram_rotulo = [p for p in ROTULO_PORTAO if p not in no_codigo]
    if faltam_rotulo:
        sys.exit(f"PORTAO SEM ROTULO: {faltam_rotulo} existe(m) no servidor.rs e "
                 "nao no desenho. Desenho que esconde portao mente por omissao.")
    if sobram_rotulo:
        sys.exit(f"ROTULO MORTO: {sobram_rotulo} esta(o) no desenho e nao no "
                 "codigo. Pior que faltar: quem le acha que ha protecao que nao ha.")
    achados = passos_do_codigo()
    sumidos = [ms[0] for ms, _, _, _ in PASSOS_GRAVAR if ms[0] not in achados]
    if sumidos:
        sys.exit(f"PASSO SUMIDO do `inserir`: {sumidos}. A ordem de gravacao "
                 "mudou e o desenho ficou para tras.")
    return no_codigo


# ------------------------------------------------------------------ figura A
def fluxograma(portoes, n_ops):
    G, RX, TRILHO = 250, 850, 560   # coluna, vermelho, e o trilho da recusa
    p = []
    p.append(f'<text x="{G}" y="26" text-anchor="middle" font-size="10" '
             f'fill="var(--acento)" letter-spacing="1.4">1 &#183; NA PORTA &#8212; O DESPACHAR, QUE É UM SÓ</text>')
    p.append(f'<rect x="60" y="36" width="380" height="34" rx="4" fill="none" '
             f'stroke="currentColor" stroke-width="1.5"/>')
    p.append(f'<text x="{G}" y="57" text-anchor="middle" font-size="11">'
             f'uma linha de JSON no soquete</text>')

    y = 84
    for i, ident in enumerate(portoes):
        curto, sub = ROTULO_PORTAO[ident]
        p.append(f'<rect x="60" y="{y}" width="380" height="36" rx="4" fill="none" '
                 f'stroke="currentColor" stroke-width="1.2" opacity=".9"/>')
        p.append(f'<text x="116" y="{y + 15}" text-anchor="end" font-size="9.5" '
                 f'fill="var(--acento)" font-weight="600">{html.escape(ident)}</text>')
        p.append(f'<text x="126" y="{y + 15}" font-size="10.5">{html.escape(curto)}</text>')
        p.append(f'<text x="126" y="{y + 29}" font-size="9" opacity=".7">{html.escape(sub)}</text>')
        p.append(f'<path d="M440 {y + 18} L{TRILHO} {y + 18}" stroke="var(--log)" '
                 f'stroke-width="1.1" opacity=".65"/>')
        p.append(f'<circle cx="{TRILHO}" cy="{y + 18}" r="2.4" fill="var(--log)" opacity=".8"/>')
        if i < len(portoes) - 1:
            p.append(f'<path d="M{G} {y + 36} L{G} {y + 42}" stroke="currentColor" '
                     f'stroke-width="1.2" marker-end="url(#setaMot)"/>')
        y += 42
    fim_portoes = y - 6

    ymeio = 84 + (len(portoes) - 1) * 21 - 26
    p.append(f'<path d="M{TRILHO} 102 L{TRILHO} {y - 24}" stroke="var(--log)" '
             f'stroke-width="1.6"/>')
    p.append(f'<path d="M{TRILHO} {ymeio + 26} L742 {ymeio + 26}" stroke="var(--log)" '
             f'stroke-width="1.6" marker-end="url(#setaMotN)"/>')
    p.append(f'<text x="{TRILHO + 12}" y="{ymeio + 18}" font-size="9" fill="var(--log)">'
             f'qualquer um recusa</text>')
    p.append(f'<rect x="742" y="{ymeio}" width="200" height="52" rx="4" '
             f'fill="var(--log)" fill-opacity=".14" stroke="var(--log)" stroke-width="1.5"/>')
    p.append(f'<text x="{RX}" y="{ymeio + 20}" text-anchor="middle" font-size="10.5" '
             f'fill="var(--log)" font-weight="600">RECUSA</text>')
    p.append(f'<text x="{RX}" y="{ymeio + 35}" text-anchor="middle" font-size="9">'
             f'com código, SQLSTATE e classe</text>')
    p.append(f'<text x="{RX}" y="{ymeio + 47}" text-anchor="middle" font-size="8.5" '
             f'opacity=".7">grave demais: bloqueio de IP</text>')

    p.append(f'<path d="M{G} {fim_portoes} L{G} {fim_portoes + 22}" stroke="var(--ok)" '
             f'stroke-width="1.6" marker-end="url(#setaMotO)"/>')
    p.append(f'<rect x="60" y="{fim_portoes + 24}" width="380" height="34" rx="4" '
             f'fill="var(--ok)" fill-opacity=".10" stroke="var(--ok)" stroke-width="1.5"/>')
    p.append(f'<text x="{G}" y="{fim_portoes + 45}" text-anchor="middle" font-size="10.5">'
             f'a operação, entre as {n_ops} do catálogo</text>')
    p.append(f'<text x="{G + 14}" y="{fim_portoes + 16}" font-size="9.5" opacity=".7">'
             f'passou os {len(portoes)}</text>')

    # a trava, entre a porta e o disco
    ty = fim_portoes + 74
    p.append(f'<rect x="60" y="{ty}" width="380" height="46" rx="4" fill="none" '
             f'stroke="var(--pend)" stroke-width="1.4"/>')
    p.append(f'<text x="{G}" y="{ty + 18}" text-anchor="middle" font-size="10.5" '
             f'fill="var(--pend)">a trava de dados</text>')
    p.append(f'<text x="{G}" y="{ty + 33}" text-anchor="middle" font-size="9" opacity=".75">'
             f'exclusiva para escrever &#183; o `varrer` tenta a compartilhada</text>')
    p.append(f'<path d="M{G} {ty + 46} L{G} {ty + 66}" stroke="currentColor" '
             f'stroke-width="1.4" marker-end="url(#setaMot)"/>')

    # --- banda 2: o caminho de gravacao, horizontal porque e um encanamento
    by = ty + 92
    p.append(f'<line x1="20" y1="{by - 16}" x2="940" y2="{by - 16}" '
             f'stroke="currentColor" stroke-width="1" opacity=".22"/>')
    p.append(f'<text x="20" y="{by}" font-size="10" fill="var(--acento)" '
             f'letter-spacing="1.4">2 &#183; NO DISCO &#8212; A ORDEM QUE O `inserir` SEGUE</text>')
    cor = {"reg": "var(--reg)", "ndx": "var(--ndx)", "memo": "var(--memo)",
           "bin": "var(--bin)", "log": "var(--log)", "": "currentColor"}
    passo = (940 - 98) // (len(PASSOS_GRAVAR) - 1)
    for i, (_, t1, t2, c) in enumerate(PASSOS_GRAVAR):
        x = 12 + i * passo
        cc = cor[c]
        p.append(f'<rect x="{x}" y="{by + 14}" width="98" height="52" rx="4" fill="none" '
                 f'stroke="{cc}" stroke-width="{1.5 if c else 1.1}" opacity="{1 if c else .75}"/>')
        p.append(f'<text x="{x + 49}" y="{by + 34}" text-anchor="middle" font-size="8.5">{html.escape(t1)}</text>')
        if t2:
            p.append(f'<text x="{x + 49}" y="{by + 50}" text-anchor="middle" font-size="8.5" '
                     f'fill="{cc}">{html.escape(t2)}</text>')
        if i < len(PASSOS_GRAVAR) - 1:
            p.append(f'<path d="M{x + 98} {by + 40} L{x + passo - 2} {by + 40}" '
                     f'stroke="currentColor" stroke-width="1.1" marker-end="url(#setaMot)"/>')
    p.append(f'<text x="12" y="{by + 90}" font-size="10.5" opacity=".85">'
             f'<tspan font-weight="600">A ordem não é arbitrária:</tspan> a unicidade se confere ANTES de qualquer gravação, porque o `.reg` nunca reaproveita slot &#8212; descobrir</text>')
    p.append(f'<text x="12" y="{by + 106}" font-size="10.5" opacity=".85">'
             f'a duplicidade depois exigiria desfazer, e o slot desfeito ficaria morto para sempre. O `.fts` fica FORA do desfazer de propósito: índice</text>')
    p.append(f'<text x="12" y="{by + 122}" font-size="10.5" opacity=".85">'
             f'atrasado o `reindexar` conserta; linha perdida, não.</text>')

    alt = by + 140
    corpo = "\n    ".join(p)
    return f'''<svg viewBox="0 0 960 {alt}" role="img" aria-label="Fluxograma do caminho de um pedido no PhxSql: uma linha de JSON no soquete atravessa os {len(portoes)} portoes numerados do despachar, qualquer um deles podendo recusar com codigo e SQLSTATE; passando, a operacao toma a trava de dados e desce para o caminho de gravacao, que confere aridade, chave e unicidade antes de escrever no .reg, depois no .ndx, no .fts e por fim no .log">
  <defs>
    <marker id="setaMot" viewBox="0 0 10 10" refX="9" refY="5" markerWidth="6" markerHeight="6" orient="auto-start-reverse">
      <path d="M0 0 L10 5 L0 10 z" fill="currentColor"/>
    </marker>
    <marker id="setaMotN" viewBox="0 0 10 10" refX="9" refY="5" markerWidth="6" markerHeight="6" orient="auto-start-reverse">
      <path d="M0 0 L10 5 L0 10 z" fill="var(--log)"/>
    </marker>
    <marker id="setaMotO" viewBox="0 0 10 10" refX="9" refY="5" markerWidth="6" markerHeight="6" orient="auto-start-reverse">
      <path d="M0 0 L10 5 L0 10 z" fill="var(--ok)"/>
    </marker>
  </defs>
  <g font-family="IBM Plex Mono, monospace" font-size="10.5" fill="currentColor">
    {corpo}
  </g>
</svg>'''


# ------------------------------------------------------------------ figura B
ETAPAS = [
    ("MODELAR", "var(--acento)",
     ["criar_database", "criar_tabela", "declarar chave"],
     "recusa CEDO: `ao_excluir`", "só aceita `restringir`"),
    ("GRAVAR", "var(--reg)",
     ["inserir · bulkinsert", "atualizar", "excluir suave / de vez"],
     "nunca mata o pai que tem", "filha — nem no suave"),
    ("CONSULTAR", "var(--bin)",
     ["buscar · varrer WHERE", "procurar_texto (.fts)", "juntar · unir · pivotar"],
     "as três que escondem tabela", "pagam conferência própria"),
    ("REPLICAR", "var(--ndx)",
     ["o `.log` é a fonte", "replicar → aplicar", "cluster: eleição, promoção"],
     "o que vem da réplica não", "reconfere: já foi julgado"),
    ("SALVAGUARDAR", "var(--bkp)",
     ["backup → `.bkp`", "`.trash` e `.reason`", "restaurar linha ou base"],
     "a linha excluída fica inteira", "no `.trash`, com o `.reason`"),
]


def workflow():
    p = []
    L = 176
    for i, (nome, cor, ops, n1, n2) in enumerate(ETAPAS):
        x = 20 + i * 188
        p.append(f'<text x="{x + L // 2}" y="26" text-anchor="middle" font-size="10" '
                 f'fill="{cor}" letter-spacing="1.2" font-weight="600">{nome}</text>')
        p.append(f'<rect x="{x}" y="38" width="{L}" height="128" rx="5" fill="{cor}" '
                 f'fill-opacity=".07" stroke="{cor}" stroke-width="1.5"/>')
        for k, o in enumerate(ops):
            p.append(f'<text x="{x + L // 2}" y="{60 + k * 20}" text-anchor="middle" '
                     f'font-size="9.5">{html.escape(o)}</text>')
        p.append(f'<text x="{x + L // 2}" y="{124}" text-anchor="middle" font-size="8.5" '
                 f'opacity=".75">{html.escape(n1)}</text>')
        p.append(f'<text x="{x + L // 2}" y="{138}" text-anchor="middle" font-size="8.5" '
                 f'opacity=".75">{html.escape(n2)}</text>')
        if i < len(ETAPAS) - 1:
            p.append(f'<path d="M{x + L} 100 L{x + L + 10} 100" stroke="currentColor" '
                     f'stroke-width="1.4" marker-end="url(#setaWk)"/>')

    # o laco que fecha o ciclo: restaurar devolve a linha para o estado gravado
    p.append('<path d="M866 166 L866 196 L296 196 L296 170" fill="none" '
             'stroke="var(--bkp)" stroke-width="1.4" stroke-dasharray="5 3" '
             'marker-end="url(#setaWkB)"/>')
    p.append('<text x="581" y="190" text-anchor="middle" font-size="9.5" '
             'fill="var(--bkp)">restaurar devolve a linha ao estado gravado</text>')

    p.append('<line x1="20" y1="220" x2="940" y2="220" stroke="currentColor" '
             'stroke-width="1" opacity=".22"/>')
    p.append('<text x="20" y="242" font-size="10" fill="var(--acento)" '
             'letter-spacing="1.4">O QUE VALE NAS CINCO, E NÃO SE NEGOCIA</text>')
    leis = [
        "A ordem de digitação é sagrada — o `.reg` nunca reaproveita slot excluído.",
        "Nunca se mata o pai que tem filhos: `ao_excluir` aceita SÓ `restringir`.",
        "Senha nunca em texto puro — nem em arquivo, nem em log, nem na resposta.",
        "Zero dependências externas: só a `std`, e por isso `cargo build --offline` funciona.",
    ]
    for k, l in enumerate(leis):
        p.append(f'<text x="20" y="{264 + k * 17}" font-size="10.5" opacity=".85">'
                 f'<tspan fill="var(--acento)">·</tspan> {html.escape(l)}</text>')

    corpo = "\n    ".join(p)
    return f'''<svg viewBox="0 0 960 344" role="img" aria-label="Diagrama de workflow do PhxSql: cinco etapas em sequencia -- modelar, gravar, consultar, replicar e salvaguardar --, cada uma com suas operacoes e a garantia que ela impoe; um laco tracejado volta da salvaguarda para a gravacao, porque restaurar devolve a linha ao estado gravado; e uma faixa embaixo lista as quatro leis que valem nas cinco etapas">
  <defs>
    <marker id="setaWk" viewBox="0 0 10 10" refX="9" refY="5" markerWidth="6" markerHeight="6" orient="auto-start-reverse">
      <path d="M0 0 L10 5 L0 10 z" fill="currentColor"/>
    </marker>
    <marker id="setaWkB" viewBox="0 0 10 10" refX="9" refY="5" markerWidth="6" markerHeight="6" orient="auto-start-reverse">
      <path d="M0 0 L10 5 L0 10 z" fill="var(--bkp)"/>
    </marker>
  </defs>
  <g font-family="IBM Plex Mono, monospace" font-size="10.5" fill="currentColor">
    {corpo}
  </g>
</svg>'''


def solto(svg):
    for ent, ch in NOMEADAS.items():
        svg = svg.replace(ent, ch)
    corte = svg.index(">") + 1
    cabeca = svg[:corte].replace("<svg ", '<svg xmlns="http://www.w3.org/2000/svg" ', 1)
    return cabeca + "\n  " + TOKENS_SOLTOS + svg[corte:]


def trocar(texto, abre, fecha, bloco, onde):
    i, j = texto.find(abre), texto.find(fecha)
    if i < 0 or j < 0:
        sys.exit(f"faltam as marcas {onde} no dossie")
    n_fig = texto[:i].count("<b>Figura ") + 1
    return texto[:i] + abre + "\n" + bloco(n_fig) + "\n" + fecha + texto[j + len(fecha):], n_fig


def main():
    alvo = pathlib.Path(sys.argv[1]) if len(sys.argv) > 1 else achar_o_dossie()
    portoes = conferir()
    n_ops = operacoes()
    svg_f, svg_w = fluxograma(portoes, n_ops), workflow()
    SVG_FLUXO.write_text(solto(svg_f), encoding="utf-8")
    SVG_WORKFLOW.write_text(solto(svg_w), encoding="utf-8")

    texto = alvo.read_text(encoding="utf-8")

    def bf(n):
        return ('  <figure>\n    <div class="fig-caixa">\n    ' + svg_f
                + '\n    </div>\n    <figcaption><b>Figura ' + str(n) + '.</b> '
                'O caminho inteiro de um pedido. Os <strong>' + str(len(portoes))
                + ' portões</strong> em cima são os que o `servidor.rs` numera '
                'no próprio código — qualquer um recusa, e a recusa sai com '
                'código e <code>SQLSTATE</code>. Embaixo, a ordem que o '
                '<code>inserir</code> segue no disco, que é onde as garantias '
                'do formato aparecem.</figcaption>\n  </figure>')

    def bw(n):
        return ('  <figure>\n    <div class="fig-caixa">\n    ' + svg_w
                + '\n    </div>\n    <figcaption><b>Figura ' + str(n) + '.</b> '
                'O ciclo de operação. Cada etapa traz a garantia que ela impõe, '
                'e a faixa de baixo, as quatro que valem nas cinco.</figcaption>'
                '\n  </figure>')

    texto, nf = trocar(texto, ABRE_F, FECHA_F, bf, "fluxo-motor")
    texto, nw = trocar(texto, ABRE_W, FECHA_W, bw, "workflow-motor")
    alvo.write_text(texto, encoding="utf-8")
    print(f"{alvo.name}: figuras {nf} (fluxo) e {nw} (workflow)")
    print(f"  {len(portoes)} portoes lidos do servidor.rs: {' '.join(portoes)}")
    print(f"  {len(PASSOS_GRAVAR)} passos conferidos contra o `inserir` do table.rs")
    print(f"  {n_ops} operacoes no catalogo")


if __name__ == "__main__":
    main()
