#!/usr/bin/env python3
"""Gera as paginas «O que voce pediu», em faixas cortadas pelo TAMANHO
MEDIDO, a partir do docs/PENDENCIAS.md.

    python3 docs/dossie/pagina-dos-pedidos.py [dossie.html]

A pagina nao se digita, e a razao e a mesma do selo do dossie: numero
digitado a mao envelhece calado. A fonte da verdade e uma so:
`docs/PENDENCIAS.md`. Mexeu la, rode isto.

Um argumento cujo nome contenha «dossie» e o dossie (o script grava o PAINEL
da contagem entre as marcas `<!-- pedidos:... -->`), nunca uma das saidas por
faixa -- essas saem de `escolher_faixas()`, nao de uma lista digitada.

Pedido 403, 23/09/2026: ate aqui esta pagina era UMA SO, e cresceu ate
1.304.729 bytes (781 linhas) -- o guarda da republicacao exige LER a versao
publicada inteira antes de aceitar a nova, e isso passou a custar ~580.000
fichas, mais de uma janela de contexto inteira so para REPUBLICAR. Decisao do
dono: partir em faixas contiguas por numero de pedido.

A PRIMEIRA versao deste conserto cortava em faixas FIXAS de 100 pedidos, e o
integrador (papel A) mediu que a ultima nascia com 562,5 KiB -- JA ACIMA do
teto de republicacao (450 KiB), porque pedido recente pesa muito mais que
pedido antigo (60 -> 287 -> 423 -> 562 KiB nas quatro faixas fixas). Corte por
NUMERO redondo de pedido nao e corte por TAMANHO -- e' o pedido 404 de novo,
por outro lado: um numero cravado no codigo (aqui, "100") envelhece calado
assim que a premissa que o justificava muda. A cura, em `escolher_faixas()`:
cortar pelo BYTE acumulado, medido a cada corrida, com o numero de paginas
resultante IMPRESSO (nunca mais fixo em quatro) -- "redondo onde der", nunca
redondo sempre.

Cada faixa e um artefato NOVO na primeira vez que nasce -- sem URL anterior,
sem guarda de leitura -- entao o custo de publicar pela primeira vez e ZERO
nesse sentido. O `docs/dossie/pedidos.html` antigo (a pagina unica) deixou de
ser gerado; este script apaga o arquivo do disco se ele sobrar, e tambem apaga
qualquer `pedidos-XXX-YYY.html` de uma corrida anterior que o corte desta
corrida nao gera mais (nunca deixa um arquivo apontando para uma faixa que
nao existe mais).

`ler()`, `ESTADOS`, `FONTE` e `marcar()` sao a API que outros geradores desta
casa IMPORTAM (nao le pedidos.html, importa o MODULO): `docs/planilha/
planilha-das-atividades.py`, `docs/status/pagina-do-status-do-projeto.py`,
`docs/status/riscos.py`, `docs/pmo/pagina-do-status-do-projeto.py` e a prova
`docs/dossie/prova-do-leitor-de-pedidos.py` (que troca `FONTE` por um arquivo
de mentira). Nenhum deles le arquivo HTML nenhum -- por isso o corte por
tamanho, com numero de paginas variavel, nao exige tocar em nenhum dos cinco.
"""

import html
import pathlib
import re
import sys

RAIZ = pathlib.Path(__file__).resolve().parents[2]
FONTE = RAIZ / "docs" / "PENDENCIAS.md"
PASTA_SAIDA = RAIZ / "docs" / "dossie"

# O arquivo unico que esta pagina gerava ate o pedido 403. Ele DEIXA de ser
# gerado -- e o script apaga o que sobrar no disco, para nunca deixar um
# arquivo velho apontando para uma pagina que ninguem mais publica.
ARQUIVO_ANTIGO = PASTA_SAIDA / "pedidos.html"

# O nome do dossie muda a cada refacao: quem o acha e a varredura da pasta,
# num dono so. Padrao digitado aqui envelhece calado na proxima refacao.
sys.path.insert(0, str(pathlib.Path(__file__).resolve().parent))
from dossie_da_pasta import achar_o_dossie  # noqa: E402

# O estado vem do emoji da primeira coluna da tabela.
ESTADOS = {
    "☑️": ("feito", "Feito"),
    "◐": ("parcial", "Parcial"),
    "☐": ("planejado", "Planejado"),
}

LINHA = re.compile(r"^\|\s*(☑️|◐|☐)\s*\|\s*(\d+)\s*\|\s*(.*?)\s*\|\s*(.*?)\s*\|\s*$")

# Uma linha que TEM numero de pedido mas cujo estado nao e' um dos tres. Ela
# existe porque o pedido 150 passou meses com um `⏳` que nao esta na legenda:
# o regex de cima nao casava, o gerador SEGUIA em silencio, e o pedido sumia
# da pagina. Papel que nao esta cumprindo tem de aparecer como nao cumprindo --
# e um pedido invisivel e' o contrario disso.
QUALQUER = re.compile(r"^\|\s*([^|\s]+)\s*\|\s*(\d+)\s*\|")

# ============================================================ OS DOIS TETOS
#
# Medido em 23/09/2026, ao fechar o pedido 403: a pagina UNICA publicada tinha
# 1.304.729 bytes, e o guarda da republicacao exige LER a versao publicada
# INTEIRA antes de aceitar a nova -- acima de ~450 KiB isso nao cabe na janela
# de contexto de quem republica. Esse e' o TETO DE REPUBLICACAO: acima dele,
# uma pagina JA publicada fica presa (paga ~3 compactacoes so para ser lida de
# volta).
#
# A primeira versao deste script cortava em faixas FIXAS de 100 pedidos --
# "1-100, 101-200, ..." -- e a ultima faixa (301 em diante, 104 itens) nasceu
# com 575.960 bytes: JA ACIMA do teto de republicacao, medido pelo integrador
# (papel A) na mesma rodada. A causa esta no proprio escalonamento medido:
# 60 -> 287 -> 423 -> 562 KiB. FAIXA FIXA DE 100 PEDIDOS NAO E FAIXA FIXA DE
# BYTES -- os pedidos recentes tem corpo muito maior que os antigos (o proprio
# 403 e 404 estao entre os mais longos do arquivo), e a tendencia continua a
# cada rodada.
#
# A cura, decidida pelo integrador: cortar pelo TAMANHO ACUMULADO, medido, nao
# por numero redondo de pedido. TETO_DE_CORTE e o alvo por pagina ao decidir
# onde cortar -- bem abaixo do teto de republicacao, para que toda faixa (a
# ultima em especial, que so' CRESCE a cada rodada -- as faixas de baixo estao
# fechadas para sempre, porque pedido antigo nao muda de numero) nasca com
# folga antes de estourar.
TETO_DE_REPUBLICACAO_KIB = 450
TETO_DE_CORTE_KIB = 300

TETO_DE_REPUBLICACAO_BYTES = TETO_DE_REPUBLICACAO_KIB * 1024
TETO_DE_CORTE_BYTES = TETO_DE_CORTE_KIB * 1024


def linha_tr(item):
    """A linha <tr> de UM pedido -- a UNICA receita, usada tanto para
    RENDERIZAR quanto para MEDIR o custo em bytes de cada pedido (ver
    `custo_bytes`). Duas receitas da mesma linha e' o mesmo defeito da lista
    de KiB da interface que envelheceu calada: se a medida e o desenho
    divergirem, o corte para de bater com o tamanho real do arquivo."""
    return (f'      <tr data-e="{item["classe"]}">'
            f'<td class="n mono">{item["n"]}</td>'
            f'<td class="st"><span class="pino {item["classe"]}">{item["rotulo"]}</span></td>'
            f'<td class="p">{item["pedido"]}</td>'
            f'<td class="e">{item["estado"]}</td></tr>')


def custo_bytes(item):
    """Quantos bytes este pedido acrescenta as DUAS tabelas da pagina -- a de
    "todos" (sempre) e a de "em aberto" (so quando o pedido nao e' feito).
    Medido codificando a MESMA linha que a pagina vai escrever, nunca uma
    formula aproximada."""
    n = len(linha_tr(item).encode("utf-8")) + 1  # +1 do '\n' de juncao
    return n * 2 if item["classe"] != "feito" else n


def overhead_medido(itens_geral):
    """O peso de uma pagina com as DUAS tabelas vazias -- cabecalho, placar
    geral, navegacao, CSS, JS, rodape. Medido renderizando `corpo()` de
    verdade com uma faixa vazia, nao uma constante digitada -- se o molde da
    pagina crescer (uma secao nova, por exemplo), este numero cresce junto
    sem que ninguem precise lembrar de atualiza-lo aqui."""
    faixa_amostra = [("pedidos-amostra.html", [], None, None, "amostra")]
    txt = corpo(itens_geral, [], "pedidos-amostra.html", None, None,
                "amostra", 1, faixa_amostra)
    return len(txt.encode("utf-8"))


def escolher_faixas(itens_geral):
    """Decide os CORTES pelo tamanho ACUMULADO, medido -- nunca por numero
    redondo digitado (e' o mesmo defeito do pedido 404 por outro lado: um
    corte cravado no codigo envelhece calado assim que o corpo dos pedidos
    muda de tamanho). Devolve uma lista de
    (arquivo, itens_da_faixa, ini, fim_ou_None, rotulo) -- SEMPRE contigua em
    numero de pedido, com a ULTIMA faixa aberta (fim=None), porque so ela
    cresce a cada rodada: as de baixo fecham para sempre assim que um pedido
    novo nasce com numero maior.

    O NUMERO de paginas NAO e mais fixo em quatro -- ele sai desta conta. Se
    o corpo dos pedidos continuar crescendo, a proxima rodada pode gerar
    cinco; se encolher, pode voltar a quatro. A navegacao e a lista de
    arquivos saem desta MESMA decisao, entao nunca ficam desalinhadas dela.

    Onde da, o corte cai num numero REDONDO (multiplo de 10) -- so' para
    baixo, o que e sempre seguro (encolhe a faixa atual, nunca estoura o
    teto). Quando nao ha multiplo de 10 dentro da faixa, o corte fica no
    numero exato -- "redondo onde der", nao redondo sempre.
    """
    itens = sorted(itens_geral, key=lambda i: i["n"])
    overhead = overhead_medido(itens_geral)
    n_total = len(itens)
    faixas = []
    i = 0
    while i < n_total:
        acumulado = overhead
        j = i
        while j < n_total:
            c = custo_bytes(itens[j])
            if acumulado + c > TETO_DE_CORTE_BYTES and j > i:
                break
            acumulado += c
            j += 1
        fim_idx = j  # exclusivo: itens[i:fim_idx] e' esta faixa, por ora

        # Redondo onde der -- so' arredondando PARA BAIXO, dentro da propria
        # faixa (nunca reduz abaixo de 1 item, e nunca aumenta o acumulado).
        if fim_idx < n_total:
            for k in range(fim_idx - 1, i, -1):
                if itens[k - 1]["n"] % 10 == 0:
                    fim_idx = k
                    break

        grupo = itens[i:fim_idx]
        ini_n = grupo[0]["n"]
        aberta = fim_idx >= n_total
        fim_n = None if aberta else grupo[-1]["n"]
        rotulo = f"{ini_n} em diante" if aberta else f"{ini_n}–{fim_n}"
        arquivo = (f"pedidos-{ini_n:03d}-mais.html" if aberta
                   else f"pedidos-{ini_n:03d}-{fim_n:03d}.html")
        faixas.append((arquivo, grupo, ini_n, fim_n, rotulo))
        i = fim_idx
    return faixas


# ============================================================ URLS PUBLICADAS
#
# As paginas sao artefatos NOVOS: nao existe URL nenhuma ate o integrador
# publicar cada uma pela primeira vez. Este e' o UNICO lugar do script onde
# essas URLs entram -- depois de publicar, cole aqui pelo NOME exato que o
# script escolheu naquela corrida (impresso no stdout). Ate la, a navegacao
# usa o NOME do arquivo (funciona abrindo local; nao resolve sozinho no
# artefato do Claude, que publica cada HTML isolado, sem pasta ao redor -- por
# isso o link real so' nasce quando alguem preenche esta tabela).
#
# ATENCAO: como os cortes sao MEDIDOS a cada corrida (acima), o nome de uma
# faixa pode mudar entre rodadas se o tamanho dos pedidos mudar o bastante
# para deslocar um corte -- e uma chave aqui que nao bate mais com o arquivo
# gerado simplesmente NAO e usada (a navegacao cai de volta no nome do
# arquivo). Isso e dito, nao escondido: o script imprime os cortes a cada
# corrida exatamente para que essa divergencia apareca.
URLS_PUBLICADAS = {
    # Preenchido pelo integrador em 23/09/2026, DEPOIS de publicar as cinco.
    # A chave e o NOME que o corte escolheu naquela corrida -- se um corte se
    # deslocar, a chave velha para de bater e a navegacao cai no nome do
    # arquivo, em vez de fingir que a URL antiga ainda serve para uma faixa
    # que mudou de conteudo. Corte que se move e' o preco de caber no teto.
    "pedidos-001-190.html": "https://claude.ai/artifact/4jSZ5yZFnjEnbGGbE3i5nz",
    "pedidos-191-260.html": "https://claude.ai/artifact/MAc3CcjfCPbjVm5sQZwYoC",
    "pedidos-261-310.html": "https://claude.ai/artifact/R4iGBeRQt7ao6yffQkmGCp",
    "pedidos-311-350.html": "https://claude.ai/artifact/S1oHX53g9q4oqHSn7Yv3vm",
    "pedidos-351-mais.html": "https://claude.ai/artifact/6iLs2ho6wDKgEF6eja6Kpm",
}


def marcar(t):
    """O pouco de Markdown que as celulas usam, virando HTML.

    Escapa ANTES de converter: senao um `<` do texto viraria tag.
    """
    t = html.escape(t)
    t = re.sub(r"`([^`]+)`", r"<code>\1</code>", t)
    t = re.sub(r"\*\*([^*]+)\*\*", r"<strong>\1</strong>", t)
    t = re.sub(r"(?<!\*)\*([^*]+)\*(?!\*)", r"<em>\1</em>", t)
    return t


def ler():
    itens = []
    for numero_da_linha, l in enumerate(
            FONTE.read_text(encoding="utf-8").split("\n"), 1):
        m = LINHA.match(l)
        if not m:
            q = QUALQUER.match(l)
            if q and q.group(1) not in ESTADOS:
                raise SystemExit(
                    f"PENDENCIAS.md:{numero_da_linha}: o pedido {q.group(2)} "
                    f"esta marcado {q.group(1)!r}, que nao e' um dos estados "
                    f"({' '.join(ESTADOS)}). Pedido com estado desconhecido "
                    "SUMIA da pagina em silencio -- foi o que aconteceu com o "
                    "150. Declare o estado, ou acrescente o simbolo a legenda.")
            if q:
                # Estado da legenda, numero de pedido, e mesmo assim a linha
                # nao casa: a forma esta torta. Sobrou em 18/09/2026, com o
                # estado CERTO -- onze linhas sem o pipe de fechamento, cinco
                # delas porque o fecho entrou como QUINTA coluna. O leitor
                # contou 369 de 380 e imprimiu tres linhas de exito.
                #
                # A guarda do 150 so olhava o simbolo, entao esta familia
                # passava por baixo dela: estado conhecido + forma torta era
                # exatamente o buraco. Guarda que cobre o motivo e nao o
                # EFEITO deixa a porta irma aberta.
                # CONTAR PIPE NAO DIAGNOSTICA. Escrevi assim primeiro e a
                # mensagem saiu mentindo «5 pipes onde precisam ser 5»: a
                # linha com quinta coluna e SEM fecho tem cinco pipes, como a
                # linha certa. O que separa as duas e' a CELULA depois do
                # ultimo pipe -- vazia na certa, com texto na torta.
                partes = l.rstrip("\n").replace("\\|", "\0").split("|")
                if partes[-1].strip():
                    causa = ("a linha nao fecha com `|` -- depois do ultimo "
                             f"pipe ainda ha texto ({partes[-1].strip()[:40]!r})")
                else:
                    causa = (f"a linha tem {len(partes) - 2} colunas onde "
                             "precisam ser 4")
                raise SystemExit(
                    f"PENDENCIAS.md:{numero_da_linha}: o pedido {q.group(2)} "
                    f"esta marcado {q.group(1)!r} (estado da legenda) mas "
                    f"{causa}. Pedido com a forma torta SUMIA da pagina em "
                    "silencio, como o 150 sumia por simbolo desconhecido. "
                    "Feche a linha com `|`, e ponha o fecho DENTRO da quarta "
                    "coluna em vez de abrir uma quinta.")
        if m:
            # Pipe CRU dentro de uma celula. O regex de cima SOBREVIVE a ele --
            # o grupo 4 esta ancorado no fim da linha, entao engole o pipe a
            # mais e o gerador nao reclama de nada. Quem quebra e' o
            # renderizador do GitHub, que corta a celula ali e joga o resto do
            # texto para colunas que nao existem. Por isso a falta passou
            # despercebida em QUATRO linhas: gerador verde, tabela torta.
            # E a cura e' escapar (`\|`), nunca crase: em tabela do GitHub o
            # corte da celula acontece ANTES do trecho de codigo, entao
            # `a || b` entre crases quebra igual.
            if l.rstrip("\n").replace("\\|", "").count("|") != 5:
                raise SystemExit(
                    f"PENDENCIAS.md:{numero_da_linha}: o pedido {m.group(2)} "
                    "tem pipe CRU dentro de uma celula -- a linha precisa ter "
                    "exatamente 5 pipes (quatro colunas), e tem outro numero. "
                    "O GitHub corta a celula no pipe e o texto vaza para "
                    "colunas que nao existem. Escreva `\\|` no lugar do `|`; "
                    "crase NAO protege.")
            classe, rotulo = ESTADOS[m.group(1)]
            itens.append(
                {
                    "classe": classe,
                    "rotulo": rotulo,
                    "n": int(m.group(2)),
                    "pedido": marcar(m.group(3)),
                    "estado": marcar(m.group(4)),
                }
            )
    if not itens:
        raise SystemExit("nenhuma linha reconhecida em PENDENCIAS.md")
    ns = [i["n"] for i in itens]
    if len(set(ns)) != len(ns):
        raise SystemExit("ha numero de pedido repetido em PENDENCIAS.md")
    return itens


def contas_de(itens):
    return {c: sum(1 for i in itens if i["classe"] == c)
            for c in ("feito", "parcial", "planejado")}


CABECA = """<meta charset="utf-8">
<title>{titulo}</title>
<link rel="preconnect" href="https://fonts.googleapis.com">
<link rel="preconnect" href="https://fonts.gstatic.com" crossorigin>
<link rel="stylesheet" href="https://fonts.googleapis.com/css2?family=Exo+2:wght@400;500;600;700&family=Source+Serif+4:opsz,wght@8..60,400;8..60,600&family=IBM+Plex+Mono:wght@400;500;600&display=swap">
<style>
:root{
  --papel:#fbf9f7; --papel-2:#f3efec; --papel-3:#e9e3de;
  --tinta:#1a1210; --tinta-2:#4a3f3a; --tinta-3:#7a6d66;
  --linha:#ded6d0;
  --acento:#c63c0a;
  --feito:#2f7a3e; --parcial:#8a6a1f; --planejado:#7a6d66;
  --sombra:0 1px 2px rgba(26,18,16,.06),0 8px 24px rgba(26,18,16,.05);
}
@media (prefers-color-scheme:dark){
  :root:not([data-theme="light"]){
    --papel:#040814; --papel-2:#0a1122; --papel-3:#131c31;
    --tinta:#dde2eb; --tinta-2:#a8b0c0; --tinta-3:#7c8598;
    --linha:#1e2940;
    --acento:#ff8a1c;
    --feito:#5cbf74; --parcial:#d5a83c; --planejado:#7c8598;
    --sombra:0 1px 2px rgba(0,0,0,.4),0 8px 24px rgba(0,0,0,.3);
  }
}
:root[data-theme="dark"]{
  --papel:#040814; --papel-2:#0a1122; --papel-3:#131c31;
  --tinta:#dde2eb; --tinta-2:#a8b0c0; --tinta-3:#7c8598;
  --linha:#1e2940;
  --acento:#ff8a1c;
  --feito:#5cbf74; --parcial:#d5a83c; --planejado:#7c8598;
  --sombra:0 1px 2px rgba(0,0,0,.4),0 8px 24px rgba(0,0,0,.3);
}
*{box-sizing:border-box}
body{
  margin:0;background:var(--papel);color:var(--tinta);
  font-family:"Source Serif 4",Georgia,"Times New Roman",serif;
  font-size:16px;line-height:1.55;
  -webkit-font-smoothing:antialiased;
}
h1,h2,h3,.rotulo,.pino,.filtros button{font-family:"Exo 2","Helvetica Neue",Arial,sans-serif}
code,.mono,.num{font-family:"IBM Plex Mono",ui-monospace,Menlo,monospace}
code{
  font-size:.86em;background:var(--papel-2);
  padding:1px 4px;border-radius:3px;color:var(--tinta-2);
}
.envelope{max-width:1080px;margin:0 auto;padding:0 20px 80px}

header{padding:52px 0 28px;border-bottom:1px solid var(--linha)}
.rotulo{
  font-size:10.5px;letter-spacing:.18em;text-transform:uppercase;
  color:var(--acento);font-weight:600;margin-bottom:12px;
}
h1{
  font-size:clamp(30px,5vw,46px);font-weight:700;line-height:1.08;
  margin:0 0 14px;letter-spacing:-.015em;text-wrap:balance;
}
h1 .x{color:var(--acento)}
.chamada{
  max-width:64ch;color:var(--tinta-2);font-size:17px;margin:0 0 4px;
}

.faixas{
  display:flex;flex-wrap:wrap;gap:8px;margin:22px 0 0;
}
.faixas a{
  font-family:"IBM Plex Mono",monospace;font-size:12.5px;font-weight:500;
  text-decoration:none;color:var(--tinta-2);
  border:1px solid var(--linha);border-radius:999px;
  padding:6px 14px;
}
.faixas a:hover{border-color:var(--acento);color:var(--acento)}
.faixas a.ativa{
  border-color:var(--acento);color:var(--acento);background:var(--papel-2);
  font-weight:600;
}

.placar{
  display:grid;grid-template-columns:repeat(auto-fit,minmax(150px,1fr));
  gap:10px;margin:30px 0 0;
}
.placar .c{
  border:1px solid var(--linha);border-radius:6px;padding:14px 16px;
  background:var(--papel-2);
}
.placar .v{
  font-family:"Exo 2",sans-serif;font-size:34px;font-weight:700;
  line-height:1;font-variant-numeric:tabular-nums;
}
.placar .r{
  font-family:"IBM Plex Mono",monospace;font-size:10px;
  letter-spacing:.13em;text-transform:uppercase;color:var(--tinta-3);
  margin-top:7px;
}
.placar .feito .v{color:var(--feito)}
.placar .parcial .v{color:var(--parcial)}
.placar .planejado .v{color:var(--planejado)}

.faixa-info{
  margin:16px 0 0;padding:12px 16px;border-left:3px solid var(--acento);
  background:var(--papel-2);border-radius:0 5px 5px 0;
  font-size:14.5px;color:var(--tinta-2);
}
.faixa-info .rotulo-inline{
  font-family:"Exo 2",sans-serif;font-weight:600;color:var(--tinta);
  font-size:12px;text-transform:uppercase;letter-spacing:.08em;
  margin-right:8px;
}
.faixa-info strong{color:var(--tinta);font-variant-numeric:tabular-nums}
.faixa-info .feito{color:var(--feito)}
.faixa-info .parcial{color:var(--parcial)}
.faixa-info .planejado{color:var(--planejado)}

h2{
  font-size:22px;font-weight:600;margin:52px 0 6px;letter-spacing:-.01em;
  scroll-margin-top:74px;
}
h2 + .sub{color:var(--tinta-3);font-size:15px;margin:0 0 20px;max-width:64ch}

.barra{
  position:sticky;top:0;z-index:5;background:var(--papel);
  border-bottom:1px solid var(--linha);
  padding:12px 0;margin:0 0 4px;
  display:flex;flex-wrap:wrap;gap:10px;align-items:center;
}
.filtros{display:flex;flex-wrap:wrap;gap:6px}
.filtros button{
  font-size:12.5px;font-weight:500;cursor:pointer;
  background:none;color:var(--tinta-2);
  border:1px solid var(--linha);border-radius:999px;
  padding:5px 13px;
}
.filtros button:hover{border-color:var(--acento);color:var(--acento)}
.filtros button[aria-pressed="true"]{
  border-color:var(--acento);color:var(--acento);
  background:color-mix(in srgb,var(--acento) 9%,transparent);
}
.filtros button:focus-visible,input:focus-visible{
  outline:2px solid var(--acento);outline-offset:2px;
}
input[type="search"]{
  font-family:"Source Serif 4",Georgia,serif;font-size:14px;
  background:var(--papel-2);color:var(--tinta);
  border:1px solid var(--linha);border-radius:5px;
  padding:6px 11px;min-width:190px;flex:1;max-width:280px;
}
.conta{
  font-family:"IBM Plex Mono",monospace;font-size:11px;
  color:var(--tinta-3);margin-left:auto;white-space:nowrap;
}

.rolo{overflow-x:auto;-webkit-overflow-scrolling:touch}
table{border-collapse:collapse;width:100%;min-width:660px}
/* Cabecalho NAO grudento: `.rolo` tem `overflow-x:auto`, e isso faz dele um
   contexto de rolagem proprio -- o `position:sticky` do `thead` passava a se
   medir por ele e caia POR CIMA da primeira linha. Quem gruda e a barra de
   filtro, que e onde esta a contagem. */
thead th{
  font-family:"IBM Plex Mono",monospace;font-weight:500;
  font-size:10px;letter-spacing:.13em;text-transform:uppercase;
  color:var(--tinta-3);text-align:left;
  padding:12px 12px 8px;border-bottom:1px solid var(--linha);
  background:var(--papel);
}
tbody td{
  padding:14px 12px;border-bottom:1px solid var(--linha);
  vertical-align:top;font-size:15px;
}
tbody tr:hover td{background:var(--papel-2)}
td.n{
  font-size:12px;color:var(--tinta-3);text-align:right;
  font-variant-numeric:tabular-nums;width:44px;white-space:nowrap;
}
td.p{width:34%;color:var(--tinta)}
td.e{color:var(--tinta-2);font-size:14.5px}
th.st,td.st{width:104px}

/* A forma tambem carrega o estado, e nao so a cor: cheio, meio, vazio. */
.pino{
  display:inline-flex;align-items:center;gap:6px;
  font-size:11px;font-weight:600;letter-spacing:.03em;white-space:nowrap;
}
.pino::before{
  content:"";width:9px;height:9px;border-radius:50%;
  border:1.5px solid currentColor;flex:none;
}
.pino.feito{color:var(--feito)}
.pino.feito::before{background:currentColor}
.pino.parcial{color:var(--parcial)}
.pino.parcial::before{background:linear-gradient(90deg,currentColor 50%,transparent 50%)}
.pino.planejado{color:var(--planejado)}

.vazio{padding:38px 12px;color:var(--tinta-3);text-align:center;font-style:italic}

.nota{
  border-left:3px solid var(--acento);background:var(--papel-2);
  padding:14px 18px;border-radius:0 5px 5px 0;margin:26px 0;
  font-size:15px;color:var(--tinta-2);max-width:66ch;
}
.nota .t{
  display:block;font-family:"Exo 2",sans-serif;font-weight:600;
  color:var(--tinta);font-size:14px;margin-bottom:5px;
}
footer{
  margin-top:56px;padding-top:22px;border-top:1px solid var(--linha);
  color:var(--tinta-3);font-size:13.5px;max-width:66ch;
}
@media (prefers-reduced-motion:reduce){*{transition:none!important;animation:none!important}}
</style>"""


def cabeca(titulo):
    return CABECA.replace("{titulo}", titulo)


def navegacao(arquivo_atual, faixas):
    """A barra que liga as faixas. Usa a URL publicada quando ela ja foi
    preenchida em URLS_PUBLICADAS; senao cai no NOME do arquivo -- que
    funciona abrindo localmente, e destrava o link real assim que alguem
    completar a tabela apos publicar. `faixas` vem da MESMA decisao de
    `escolher_faixas()` que gerou as paginas, entao nunca aponta para um
    arquivo que esta corrida nao escreveu."""
    pedacos = []
    for arquivo, _grupo, _ini, _fim, rotulo in faixas:
        ativa = arquivo == arquivo_atual
        href = URLS_PUBLICADAS.get(arquivo) or arquivo
        classe = ' class="ativa"' if ativa else ""
        aria = ' aria-current="page"' if ativa else ""
        pedacos.append(f'<a href="{href}"{classe}{aria}>{rotulo}</a>')
    rotulo_nav = f"As {len(faixas)} páginas de pedidos, por faixa"
    return f'<nav class="faixas" aria-label="{rotulo_nav}">' + "".join(pedacos) + "</nav>"


def corpo(itens_geral, itens_faixa, arquivo_atual, ini, fim, rotulo, indice, faixas):
    n_geral = len(itens_geral)
    cg = contas_de(itens_geral)

    n_faixa = len(itens_faixa)
    cf = contas_de(itens_faixa)
    n_paginas = len(faixas)

    abertos = [i for i in itens_faixa if i["classe"] != "feito"]

    if abertos:
        estados = []
        if cf["parcial"]:
            estados.append(f"{cf['parcial']} pela metade")
        if cf["planejado"]:
            estados.append(f"{cf['planejado']} sem começar")
        abertura = (
            f"Os {len(abertos)} que não fecharam nesta faixa "
            f"({' e '.join(estados)}), com o motivo de cada um. O motivo "
            f"importa mais que o estado: uns esperam trabalho, outros "
            f"esperam uma decisão sua, e outros esperam coisa de fora deste "
            f"repositório."
        )
    else:
        abertura = "Nenhum pedido em aberto nesta faixa."

    def linhas(ls):
        return "\n".join(linha_tr(i) for i in ls)

    return f"""<div class="envelope">
<header>
  <div class="rotulo">PhxSql · o dossiê do que foi pedido — página {indice} de {n_paginas}</div>
  <h1>Pedidos {rotulo}<br>de {n_geral} ao todo</h1>
  <p class="chamada">Uma linha por pedido seu, na ordem em que você pediu. O
  estado é <strong>medido contra o código</strong>, não contra a lembrança — foi
  assim que a chave estrangeira saiu de «pronto» para «parcial», e o Centro de
  Controle de «pronto» para «só navega».</p>
  <p class="chamada">Esta lista deixou de caber numa página só (pedido 403,
  23/09/2026): a publicada tinha 1.304.729 bytes, e republicá-la exigia reler
  a versão antiga inteira. Ela virou <strong>{n_paginas} páginas</strong>,
  cortadas pelo <strong>tamanho medido</strong> de cada faixa de número de
  pedido (nunca por número redondo) — esta é a faixa
  <strong>{rotulo}</strong>.</p>

  {navegacao(arquivo_atual, faixas)}

  <div class="placar">
    <div class="c"><div class="v">{n_geral}</div><div class="r">pedidos, ao todo</div></div>
    <div class="c feito"><div class="v">{cg['feito']}</div><div class="r">feitos</div></div>
    <div class="c parcial"><div class="v">{cg['parcial']}</div><div class="r">parciais</div></div>
    <div class="c planejado"><div class="v">{cg['planejado']}</div><div class="r">planejados</div></div>
  </div>

  <div class="faixa-info">
    <span class="rotulo-inline">Nesta página</span>
    <strong>{n_faixa}</strong> pedidos (faixa {rotulo}) ·
    <span class="feito">{cf['feito']} feitos</span> ·
    <span class="parcial">{cf['parcial']} parciais</span> ·
    <span class="planejado">{cf['planejado']} planejados</span>
  </div>
</header>

<h2>O que está aberto, nesta faixa</h2>
<p class="sub">{abertura}</p>
<div class="rolo">
  <table>
    <thead><tr><th class="n">#</th><th class="st">estado</th><th>o que você pediu</th><th>onde está</th></tr></thead>
    <tbody>
{linhas(abertos)}
    </tbody>
  </table>
</div>

<div class="nota">
  <span class="t">Por que «parcial» e não «feito»</span>
  Meio caminho andado continua sendo meio caminho, e a lista diz <em>qual</em>
  metade — porque é a metade que falta que decide se o pedido serve para alguma
  coisa hoje. O estado sai da primeira coluna do
  <code>docs/PENDENCIAS.md</code>, que é medido contra o código; esta página
  não tem opinião própria sobre nenhum item.
</div>

<h2 id="todos">Os {n_faixa} desta faixa ({rotulo}), na ordem em que você pediu</h2>
<p class="sub">A numeração é a sequência real dos seus pedidos — ela diz
<em>quando</em> cada coisa foi pedida, e é por isso que vale mantê-la. As
outras {n_paginas - 1} faixas estão no menu acima.</p>

<div class="barra">
  <div class="filtros" role="group" aria-label="Filtrar por estado">
    <button type="button" data-f="todos" aria-pressed="true">Todos</button>
    <button type="button" data-f="feito" aria-pressed="false">Feitos</button>
    <button type="button" data-f="parcial" aria-pressed="false">Parciais</button>
    <button type="button" data-f="planejado" aria-pressed="false">Planejados</button>
  </div>
  <input type="search" id="busca" placeholder="procurar nesta faixa…" aria-label="Procurar nos pedidos desta faixa">
  <span class="conta" id="conta"></span>
</div>

<div class="rolo">
  <table id="tudo">
    <thead><tr><th class="n">#</th><th class="st">estado</th><th>o que você pediu</th><th>o que existe hoje</th></tr></thead>
    <tbody>
{linhas(itens_faixa)}
    </tbody>
  </table>
  <div class="vazio" id="vazio" hidden>Nenhum pedido bate com isso.</div>
</div>

<footer>
  Gerado de <code>docs/PENDENCIAS.md</code> por
  <code>docs/dossie/pagina-dos-pedidos.py</code> — a lista não se digita, pela
  mesma razão que o selo do dossiê não se digita. Esta é a página
  <strong>{indice} de {n_paginas}</strong> (faixa {rotulo}); as outras estão no
  menu do topo. O dossiê técnico, com o formato byte a byte e a bancada
  medida, é uma página à parte.
</footer>
</div>

<script>
(function(){{
  const linhas = Array.from(document.querySelectorAll('#tudo tbody tr'));
  const botoes = Array.from(document.querySelectorAll('.filtros button'));
  const busca  = document.getElementById('busca');
  const conta  = document.getElementById('conta');
  const vazio  = document.getElementById('vazio');
  let filtro = 'todos';

  // Sem isto, quem digita «indice» nao acha «indice» com acento -- e em
  // portugues isso e a busca falhando calada, nao uma sutileza.
  const achatar = t => t.normalize('NFD').replace(/[̀-ͯ]/g, '').toLowerCase();
  linhas.forEach(tr => tr.dataset.busca = achatar(tr.textContent));

  function aplicar(){{
    const q = achatar(busca.value.trim());
    let vistos = 0;
    for (const tr of linhas) {{
      const okEstado = filtro === 'todos' || tr.dataset.e === filtro;
      const okTexto  = !q || tr.dataset.busca.includes(q);
      const mostra = okEstado && okTexto;
      tr.hidden = !mostra;
      if (mostra) vistos++;
    }}
    conta.textContent = vistos + ' de ' + linhas.length;
    vazio.hidden = vistos !== 0;
  }}

  botoes.forEach(b => b.addEventListener('click', () => {{
    filtro = b.dataset.f;
    botoes.forEach(o => o.setAttribute('aria-pressed', String(o === b)));
    aplicar();
  }}));
  busca.addEventListener('input', aplicar);
  aplicar();
}})();
</script>"""


ABRE_C = "<!-- pedidos:contagem:inicio -->"
FECHA_C = "<!-- pedidos:contagem:fim -->"


def gravar_contagem(itens):
    """Escreve a contagem DE VOLTA no PENDENCIAS.md, entre as marcas.

    A linha «123 feitos . 5 parciais . 4 planejados» ficou digitada no proprio
    arquivo que a produz, e passou rodadas errada -- com a tabela logo acima
    dizendo outra coisa. Somar aqui e escrever la e a mesma receita do
    `numeros-da-bancada.py`: quem conta e quem sabe contar.

    Nao mexe no pedido 403: continua contando TODOS os itens, sem faixa.
    """
    md = FONTE.read_text(encoding="utf-8")
    i, j = md.find(ABRE_C), md.find(FECHA_C)
    if i < 0 or j < 0:
        return False
    c = contas_de(itens)
    bloco = (
        f"**{len(itens)} pedidos: {c['feito']} feitos · {c['parcial']} parciais · "
        f"{c['planejado']} planejados.**\n\n"
        "*(Gerado por `docs/dossie/pagina-dos-pedidos.py` — não conte à mão. A\n"
        "conta sai da primeira coluna da tabela acima, e é a mesma que as\n"
        "páginas dos pedidos mostram: se discordarem, é porque alguém digitou\n"
        "uma delas.)*"
    )
    md = md[:i] + ABRE_C + "\n" + bloco + "\n" + FECHA_C + md[j + len(FECHA_C):]
    FONTE.write_text(md, encoding="utf-8")
    return True


DOSSIE_ABRE = "<!-- pedidos:inicio (gerado por docs/dossie/pagina-dos-pedidos.py) -->"
DOSSIE_FECHA = "<!-- pedidos:fim -->"


def gravar_no_dossie(caminho, itens):
    """O painel da secao «Estado e roteiro» do dossie.

    Continua sendo a contagem GERAL, dos itens inteiros -- o painel do dossie
    e a contagem de volta no PENDENCIAS.md nao mudam com o pedido 403; so a
    pagina HTML de baixo, que virou quatro, mudou.
    """
    alvo = pathlib.Path(caminho).resolve()
    txt = alvo.read_text(encoding="utf-8")
    i, j = txt.find(DOSSIE_ABRE), txt.find(DOSSIE_FECHA)
    if i < 0 or j < 0:
        raise SystemExit(f"{alvo} nao tem as marcas pedidos:inicio/fim")
    c = contas_de(itens)
    fichas = [
        (len(itens), "pedidos, ao todo"),
        (c["feito"], "feitos"),
        (c["parcial"], "parciais"),
        (c["planejado"], "planejados"),
    ]
    bloco = "\n" + "\n".join(
        f'    <div><div class="v">{v}</div><div class="r">{r}</div></div>'
        for v, r in fichas) + "\n  "
    txt = txt[:i] + DOSSIE_ABRE + bloco + DOSSIE_FECHA + txt[j + len(DOSSIE_FECHA):]
    alvo.write_text(txt, encoding="utf-8")
    print(f"{alvo.name}: painel dos pedidos regravado")


def gravar_faixas(itens):
    """Escreve as paginas por faixa (o NUMERO delas sai de `escolher_faixas`,
    nao e mais fixo em quatro). Devolve (faixas, saidas) -- `saidas` e a lista
    de (caminho, bytes) para o resumo final e para o relatorio do teto."""
    faixas = escolher_faixas(itens)
    saidas = []
    for indice, (arquivo, grupo, ini, fim, rotulo) in enumerate(faixas, 1):
        caminho = PASTA_SAIDA / arquivo
        titulo = f"Pedidos {rotulo} do PhxSql ({indice}/{len(faixas)})"
        conteudo = (cabeca(titulo) + "\n"
                    + corpo(itens, grupo, arquivo, ini, fim, rotulo, indice, faixas)
                    + "\n")
        caminho.write_text(conteudo, encoding="utf-8")
        saidas.append((caminho, len(conteudo.encode("utf-8"))))
    return faixas, saidas


def apagar_faixas_orfas(faixas_atuais):
    """Como os cortes sao MEDIDOS a cada corrida, uma rodada anterior pode ter
    escrito um `pedidos-XXX-YYY.html` que esta corrida nao gerou mais (o corte
    se deslocou). Sem isto, um arquivo orfao ficaria no disco apontando para
    uma faixa que nao existe mais -- o mesmo defeito que motivou apagar o
    `pedidos.html` antigo, so que por outro caminho."""
    atuais = {arquivo for arquivo, *_ in faixas_atuais}
    apagados = []
    for caminho in sorted(PASTA_SAIDA.glob("pedidos-*.html")):
        if caminho.name not in atuais:
            caminho.unlink()
            apagados.append(caminho)
    return apagados


def main():
    argumentos = [a for a in sys.argv[1:] if not a.startswith("--")]
    dossies = [a for a in argumentos if "dossie" in pathlib.Path(a).name]
    if not dossies:
        dossies = [achar_o_dossie()]

    itens = ler()
    for d in dossies:
        gravar_no_dossie(d, itens)

    faixas, saidas = gravar_faixas(itens)
    orfas = apagar_faixas_orfas(faixas)

    # O pedido 403 tambem manda apagar o arquivo unico antigo do disco --
    # nunca deixar um arquivo velho apontando para uma pagina que ninguem
    # mais publica. Idempotente: se ja sumiu, nao faz nada.
    if ARQUIVO_ANTIGO.exists():
        ARQUIVO_ANTIGO.unlink()
        print(f"{ARQUIVO_ANTIGO.relative_to(RAIZ)}: apagado (a pagina unica "
              "deixou de ser gerada -- pedido 403)")
    for caminho in orfas:
        print(f"{caminho.relative_to(RAIZ)}: apagado (o corte desta rodada "
              "nao gera mais esta faixa)")

    contas = contas_de(itens)
    print(f"{len(itens)} pedidos: {contas['feito']} feitos, "
          f"{contas['parcial']} parciais, {contas['planejado']} planejados")
    print(f"cortes escolhidos nesta corrida ({len(faixas)} pagina(s), "
          f"alvo {TETO_DE_CORTE_KIB} KiB, teto de republicacao "
          f"{TETO_DE_REPUBLICACAO_KIB} KiB):")
    for (arquivo, grupo, ini, fim, rotulo), (caminho, n_bytes) in zip(faixas, saidas):
        kib = n_bytes / 1024
        folga = TETO_DE_REPUBLICACAO_BYTES - n_bytes
        aviso = "" if kib <= TETO_DE_REPUBLICACAO_KIB else "  *** ACIMA DO TETO DE REPUBLICACAO ***"
        print(f"  {arquivo:<28} {len(grupo):>3} pedidos ({rotulo:<14}) "
              f"{n_bytes:>8} bytes  {kib:>7.1f} KiB  "
              f"folga ate o teto: {folga / 1024:>7.1f} KiB{aviso}")
    if gravar_contagem(itens):
        print(f"contagem gravada de volta em {FONTE.name}")


if __name__ == "__main__":
    main()
