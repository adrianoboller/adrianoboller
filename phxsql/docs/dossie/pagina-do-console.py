#!/usr/bin/env python3
"""A OITAVA pagina -- o console em imagens, a bancada e os tetos medidos.

    python3 docs/dossie/pagina-do-console.py [saida.html]

# Por que ela existe, e o numero que a fez nascer (pedido 411, 23/09/2026)

O dossie chegou a **2.703.573 bytes**, e o guarda da republicacao exige reler
a versao publicada INTEIRA antes de aceitar a nova -- o mesmo teto de ~450 KiB
que partiu a pagina dos pedidos em cinco (pedido 403). Medido antes de mexer:

    secao 18 (o console em imagens)      2.106.613 bytes   77,9% do arquivo
    as 21 imagens `data:` do documento   2.171.386 bytes   80,3%, 4,71x o teto

Nao ha prosa a cortar que resolva isso: o peso e' a galeria. A rota (c) que a
rodada tentou antes -- reduzir as capturas -- morreu medida (rendia 2,4% onde
precisava render 83,0%, erro de 35x), e o dono escolheu a rota que a medicao
apontava: a secao vira pagina propria, no molde exato do pedido 403.

**Conteudo nao se perde -- muda de pagina.** Junto com a 18 vieram a 32 (a
bancada de dez milhoes de linhas e os tres motores a um milhao) e os dois
paineis medidos da 35 (os quatro tetos da trava e a cobertura por area),
porque so' as tres juntas cabiam no teto. No dossie, cada uma virou um
PONTEIRO que diz onde o conteudo foi parar.

# O que este gerador faz de diferente dos outros da pasta

Ele monta a CASCA -- prosa, estrutura, estilo, navegacao -- e **preserva** o
que os outros geradores escreveram dentro das marcas. Sao seis donos escrevendo
nesta pagina:

    capturas:                 docs/dossie/capturas-no-dossie.py
    bancada: / :tabela: /
    :diagnostico:             docs/dossie/numeros-da-bancada.py
    trio:                     docs/dossie/trio-de-motores.py
    tetos:                    docs/dossie/tetos-da-trava.py
    cobertura:                docs/dossie/cobertura-por-area.py
    a numeracao das figuras   docs/dossie/numerar-figuras.py

Se ele REGERASSE os blocos vazios, rodar a casca apagaria o numero medido de
todos eles -- e a pagina anunciaria sucesso mostrando lugar nenhum. Entao ele
so' os transporta. Bloco que ainda nao existe nasce com um aviso visivel
nomeando o gerador que falta, e o comando **sai != 0 dizendo quais**: gerador
que faz menos do que o nome dele promete tem de dizer que fez menos.

# O que ele NAO faz, e por que

**Nao numera as capturas.** As legendas das vinte telas nunca tiveram numero
de figura, por decisao registrada no `numerar-figuras.py`: elas nao sao
referenciadas por numero em lugar nenhum, e numera-las mudaria o significado
das referencias que ja existem.

**Nao publica o tamanho do dossie dentro da pagina.** Seria um numero que muda
toda vez que alguem toca o dossie -- e a pagina ficaria velha por um arquivo
que nao e' dela, num vaivem sem ponto fixo. Os numeros daqui sao funcao pura
das fontes versionadas: o peso das capturas sai dos proprios PNG.
"""

import base64
import importlib.util
import pathlib
import re
import sys

AQUI = pathlib.Path(__file__).resolve().parent
RAIZ = AQUI.parent.parent
sys.path.insert(0, str(AQUI))
from dossie_da_pasta import achar_o_dossie, pagina_do_console  # noqa: E402

# --- O ponto UNICO das URLs publicadas -------------------------------------
#
# Mesmo molde do `URLS_PUBLICADAS` do `pagina-dos-pedidos.py`, e pelo mesmo
# motivo: a navegacao entre as paginas da casa tem de sair de um lugar so.
#
# `console-em-imagens.html` NASCE SEM URL, e isso e' o certo -- ela e' um
# artefato NOVO, nunca publicado. Enquanto a chave estiver vazia, a navegacao
# cai no NOME DO ARQUIVO (que funciona abrindo localmente) em vez de fingir
# que alguma URL antiga serve. Depois de publicar, o integrador preenche aqui.
URLS_PUBLICADAS = {
    "console-em-imagens.html": "",
    "dossie": "https://claude.ai/code/artifact/5c14044e-0dc5-4832-b015-224ab1e40033",
    "testes.html": "https://claude.ai/code/artifact/0c069766-7ff0-437e-b18b-8278f8b96038",
    "graficos.html": "https://claude.ai/code/artifact/b34a216b-a685-4173-8950-9584c911ba07",
    "status.html": "https://claude.ai/code/artifact/51330b6a-7c5c-4f8f-831a-93a9fb7cba9c",
}

# O teto que o pedido 403 mediu e registrou: ~450 KiB de pagina publicada e'
# uma janela de contexto inteira so' para republicar. Constante daqui, para
# nao ser digitada duas vezes na prosa da pagina.
TETO_REPUBLICACAO = 450 * 1024

MARCA = RAIZ / "marca" / "derivados" / "phxsql-simbolo-440.png"

# --- As marcas, e o dono de cada uma ---------------------------------------
#
# (chave, abre, fecha, gerador, o que o bloco mostra)
BLOCOS = [
    ("capturas",
     "<!-- capturas:inicio (gerado por docs/dossie/capturas-no-dossie.py) -->",
     "<!-- capturas:fim -->",
     "docs/dossie/capturas-no-dossie.py",
     "as vinte capturas de tela"),
    ("bancada",
     "<!-- bancada:inicio (gerado por docs/dossie/numeros-da-bancada.py) -->",
     "<!-- bancada:fim -->",
     "docs/dossie/numeros-da-bancada.py",
     "a figura das cinco operacoes contra o MySQL(R)"),
    ("bancada_tabela",
     "<!-- bancada:tabela:inicio -->",
     "<!-- bancada:tabela:fim -->",
     "docs/dossie/numeros-da-bancada.py",
     "a tabela PhxSql x MySQL(R) a dez milhoes de linhas"),
    ("bancada_diagnostico",
     "<!-- bancada:diagnostico:inicio -->",
     "<!-- bancada:diagnostico:fim -->",
     "docs/dossie/numeros-da-bancada.py",
     "o diagnostico de onde a insercao doi"),
    ("trio",
     "<!-- trio:inicio (gerado por docs/dossie/trio-de-motores.py) -->",
     "<!-- trio:fim -->",
     "docs/dossie/trio-de-motores.py",
     "os tres motores a um milhao de linhas"),
    ("tetos",
     "<!-- tetos:inicio (gerado por docs/dossie/tetos-da-trava.py) -->",
     "<!-- tetos:fim -->",
     "docs/dossie/tetos-da-trava.py",
     "os quatro tetos da trava global"),
    ("cobertura",
     "<!-- cobertura:inicio (gerado por docs/dossie/cobertura-por-area.py) -->",
     "<!-- cobertura:fim -->",
     "docs/dossie/cobertura-por-area.py",
     "os testes por area"),
]


def _numerador():
    """A regra da legenda sai do `numerar-figuras.py`, que e o dono dela.

    Por que renumerar AQUI tambem, se aquele roda por ultimo: porque a prosa
    movida trouxe as legendas com o numero que elas tinham no dossie (27 e 28),
    e um molde que recoloca 27 a cada corrida nunca chega a ponto fixo -- o
    portao acusaria VELHO para sempre, alternando com o numerador. Uma SEMENTE
    digitada no molde daria no mesmo na primeira figura que alguem inserisse no
    meio. Entao a casca ja sai numerada pela ordem do proprio documento, e o
    `numerar-figuras.py` continua sendo o dono da regra e o conferidor final.
    """
    caminho = AQUI / "numerar-figuras.py"
    spec = importlib.util.spec_from_file_location("numerar_figuras", caminho)
    mod = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(mod)
    return mod.LEGENDA


def numerar_as_figuras(html: str) -> str:
    legenda = _numerador()
    conta = iter(range(1, len(legenda.findall(html)) + 1))
    return legenda.sub(lambda _: f"<b>Figura {next(conta)}.</b>", html)


def _telas():
    """A lista das telas sai do CODIGO do gerador das capturas, nao daqui.

    «Quando um gerador depende de uma lista, a lista tem de sair do codigo» --
    uma lista digitada aqui diria «vinte capturas» no dia em que o
    `capturar-dossie.mjs` fotografasse a decima primeira tela, e ninguem veria.
    """
    caminho = AQUI / "capturas-no-dossie.py"
    spec = importlib.util.spec_from_file_location("capturas_no_dossie", caminho)
    mod = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(mod)
    return mod.TELAS, mod.TEMAS, mod.CAPTURAS


def peso_das_capturas(telas, temas, pasta):
    """Quantos bytes as capturas ocupam DEPOIS de viradas data URI.

    O numero que justifica esta pagina existir nao pode ser digitado nela: ele
    sai dos proprios PNG versionados, que e' o que entra no HTML.
    """
    total = 0
    faltando = []
    for nome, *_ in telas:
        for tema, _rotulo in temas:
            p = pasta / f"{nome}-{tema}.png"
            if p.exists():
                total += p.stat().st_size
            else:
                faltando.append(p.name)
    # base64 cresce 4/3 e arredonda para cima em blocos de 4.
    return total, (total + 2) // 3 * 4, faltando


def preservar(antigo: str, abre: str, fecha: str):
    """O bloco como esta na pagina de hoje, ou None quando ele ainda nao existe."""
    i = antigo.find(abre)
    if i < 0:
        return None
    j = antigo.find(fecha, i)
    if j < 0:
        return None
    return antigo[i:j + len(fecha)]


def vazio(abre: str, fecha: str, gerador: str, oque: str) -> str:
    """O bloco que ainda nao foi gerado -- visivel, e nomeando quem o escreve.

    Vazio silencioso seria o pior estado possivel: a pagina abriria inteira,
    bonita, sem o numero, e ninguem veria falta nenhuma.
    """
    return (f'{abre}\n<p class="falta"><b>NAO GERADO</b> — {oque}. '
            f"Rode <code>python3 {gerador}</code>.</p>\n{fecha}")


def link(chave: str) -> str:
    """A URL publicada da pagina, ou o nome do arquivo enquanto ela nao tem."""
    return URLS_PUBLICADAS.get(chave) or chave


def marca_embutida() -> str:
    """O simbolo da marca como data URI.

    Ele SAIU da capa do dossie nesta mesma rodada, e nao por preferencia: com
    o dossie a 460.800 bytes de teto, os 75.394 bytes dele eram a diferenca
    entre caber e nao caber, e nenhuma resolucao menor cabia (o menor derivado
    oficial, o icone de 40x34 px, ainda custa 1.990 bytes em base64, e um
    simbolo de 40 px numa placa de 440 e' a marca mostrada mal). Aqui ele e'
    3,3% do arquivo, e cabe. **A marca mudou de casa, como as secoes.**
    """
    if not MARCA.exists():
        raise SystemExit(f"a marca nao esta em {MARCA} -- ver marca/LEIA-ME.md")
    b64 = base64.b64encode(MARCA.read_bytes()).decode("ascii")
    return ("data:image/png;base64," + b64)


# Os tres PONTEIROS que ficaram no dossie, no lugar das secoes que mudaram de
# casa. O texto deles e do dossie; o LINK e daqui, e o motivo e o mesmo do
# `URLS_PUBLICADAS`: enquanto esta pagina nao tiver URL, os tres tem de cair
# no nome do arquivo, e no dia em que ela tiver, os tres tem de mudar JUNTOS.
# Tres hrefs digitados a mao seriam tres lugares para divergir -- e o pedido
# 403 ja pagou por uma URL velha apontando para conteudo que mudou.
ANCORAS_NO_DOSSIE = ["console:a1", "console:a2", "console:a3"]


def ancora() -> str:
    """O link para esta pagina: a URL publicada, ou o nome do arquivo."""
    from dossie_da_pasta import NOME_DA_PAGINA_DO_CONSOLE
    alvo = link(NOME_DA_PAGINA_DO_CONSOLE)
    return f'<a href="{alvo}">O console em imagens</a>'


def ligar_os_ponteiros() -> int:
    """Regrava o link dos tres ponteiros do dossie. Devolve quantos achou."""
    dossie = achar_o_dossie()
    txt = dossie.read_text(encoding="utf-8")
    achados = 0
    for chave in ANCORAS_NO_DOSSIE:
        abre, fecha = f"<!--{chave}:inicio-->", f"<!--{chave}:fim-->"
        i, j = txt.find(abre), txt.find(fecha)
        if i < 0 or j < 0:
            continue
        txt = txt[:i] + abre + ancora() + fecha + txt[j + len(fecha):]
        achados += 1
    if achados:
        dossie.write_text(txt, encoding="utf-8")
    return achados


def trocar(molde: str, dados: dict) -> str:
    """Substituicao por `@@chave@@`, e NAO por `str.format`.

    O motivo e' a prosa: as secoes que vieram do dossie trazem HTML e codigo
    com `{` e `}` dentro, e `format` obrigaria a dobrar cada um. Dobrar chave
    a mao em 34 KiB de texto movido e' o jeito classico de perder uma linha
    numa mudanca que devia ser um transporte -- e o contrato desta pagina e
    justamente «nada se perde». Duas passadas, porque `@@secao2@@` traz
    `@@bancada@@` dentro.
    """
    for _ in range(2):
        for chave, valor in dados.items():
            molde = molde.replace(f"@@{chave}@@", valor)
    sobrou = re.findall(r"@@(\w+)@@", molde)
    if sobrou:
        raise SystemExit(f"token sem valor no molde: {sorted(set(sobrou))}")
    return molde


def montar(antigo: str) -> tuple[str, list]:
    telas, temas, pasta = _telas()
    png, b64, faltando = peso_das_capturas(telas, temas, pasta)
    n_capturas = len(telas) * len(temas)
    vezes = b64 / TETO_REPUBLICACAO

    partes = {}
    pendentes = []
    for chave, abre, fecha, gerador, oque in BLOCOS:
        bloco = preservar(antigo, abre, fecha)
        if bloco is None:
            bloco = vazio(abre, fecha, gerador, oque)
            pendentes.append((gerador, oque))
        partes[chave] = bloco

    html = trocar(MOLDE, {
        "secao2": SECAO_2,
        "secao3": SECAO_3,
        "marca": marca_embutida(),
        "n_capturas": str(n_capturas),
        "n_telas": str(len(telas)),
        "n_temas": str(len(temas)),
        "kib_capturas": f"{b64 / 1024:,.0f}".replace(",", "."),
        "kib_png": f"{png / 1024:,.0f}".replace(",", "."),
        "teto_kib": str(TETO_REPUBLICACAO // 1024),
        "vezes": f"{vezes:.2f}".replace(".", ","),
        "url_dossie": link("dossie"),
        "url_testes": link("testes.html"),
        "url_graficos": link("graficos.html"),
        "url_status": link("status.html"),
        **partes,
    })
    html = numerar_as_figuras(html)
    if faltando:
        pendentes.append(("docs/dossie/capturar-dossie.mjs",
                          f"{len(faltando)} PNG de captura faltando em "
                          f"{pasta.relative_to(RAIZ)}"))
    return html, pendentes


# ---------------------------------------------------------------- o molde
#
# Sem `<meta charset>`: o embrulho do visualizador o poe ao publicar, e as
# outras paginas desta casa seguem a mesma regra. Quem abre por `file://` usa
# o `pdf.mjs`/`olhar.mjs`, que imprimem uma copia com o charset na frente.
MOLDE = """<title>O console em imagens — PhxSql</title>
<meta name="viewport" content="width=device-width, initial-scale=1">
<link rel="preconnect" href="https://fonts.googleapis.com">
<link rel="preconnect" href="https://fonts.gstatic.com" crossorigin>
<link rel="stylesheet" href="https://fonts.googleapis.com/css2?family=Exo+2:wght@400;500;600;700&family=Source+Serif+4:opsz,wght@8..60,400;8..60,600&family=IBM+Plex+Mono:wght@400;500&display=swap">
<style>
/* Todo token de cor nasce AQUI, no :root base: cor definida so dentro de
   @media ou [data-theme] nao existe para quem esta no tema "sistema". O fundo
   escuro e o #010418 da marca (marca/LEIA-ME.md), nao um valor inventado. */
:root{
  --papel:#fbf9f7; --papel-2:#f3efec; --papel-3:#e9e3de;
  --tinta:#1a1210; --tinta-2:#4a3f3a; --tinta-3:#7a6d66;
  --linha:#ded6d0; --acento:#c63c0a; --acento-2:#ff8a1c;
  --ok:#2f7a3e; --pend:#8a6a1f; --log:#b71414;
  --sombra:0 1px 2px rgba(26,18,16,.06),0 8px 24px rgba(26,18,16,.05);
  --medida:74ch; --corpo:1180px;
}
@media (prefers-color-scheme:dark){
  :root:not([data-theme="light"]){
    --papel:#010418; --papel-2:#0a1122; --papel-3:#131c31;
    --tinta:#dde2eb; --tinta-2:#a8b0c0; --tinta-3:#7c8598;
    --linha:#1e2940; --acento:#ff8a1c; --acento-2:#ffc43d;
    --ok:#6cc98c; --pend:#ffc43d; --log:#ff5f5f;
    --sombra:0 1px 2px rgba(0,0,0,.5),0 8px 28px rgba(0,0,0,.38);
  }
}
:root[data-theme="dark"]{
  --papel:#010418; --papel-2:#0a1122; --papel-3:#131c31;
  --tinta:#dde2eb; --tinta-2:#a8b0c0; --tinta-3:#7c8598;
  --linha:#1e2940; --acento:#ff8a1c; --acento-2:#ffc43d;
  --ok:#6cc98c; --pend:#ffc43d; --log:#ff5f5f;
  --sombra:0 1px 2px rgba(0,0,0,.5),0 8px 28px rgba(0,0,0,.38);
}
*{box-sizing:border-box}
body{margin:0;background:var(--papel);color:var(--tinta);
  font-family:"Source Serif 4",Georgia,"Times New Roman",serif;
  font-size:17px;line-height:1.62;-webkit-font-smoothing:antialiased;
  overflow-x:hidden}
h1,h2,h3,.rotulo{font-family:"Exo 2","Helvetica Neue",Arial,sans-serif}
code,.mono,.dado{font-family:"IBM Plex Mono",ui-monospace,Menlo,monospace}
/* NADA CENTRALIZA (docs/DESIGN.md §4.1): num monitor duplo o meio da janela
   e a emenda FISICA entre os dois.
   E o `minmax(0,1fr)` nao e enfeite: `1fr` sozinho e `minmax(auto,1fr)`, e
   `auto` NAO desce abaixo do min-content do filho -- foi o que ja pos 1.700 px
   de galeria dentro de uma janela de 390 nesta casa. Medido aqui em
   23/09/2026, com a `.pagina` como bloco simples: a pagina rolava 246 px para
   o lado a 390 px, e o `body{overflow-x:hidden}` nao segurava. So se viu
   exercitando (`sonda-de-estouro.mjs`); lendo o CSS nao aparece. */
.pagina{display:grid;grid-template-columns:minmax(0,1fr);padding:0 20px 90px}
@media (min-width:1060px){.pagina{padding:0 32px 120px}}

header.capa{padding:48px 0 8px;border-bottom:1px solid var(--linha)}
/* A marca vive sobre #010418 -- fundo oficial, medido dos originais. Sobre
   papel claro ela precisa levar esse fundo junto, senao o cilindro, que e
   escuro por dentro, vira um fantasma branco. */
.placa{display:block;width:fit-content;background:#010418;border-radius:12px;
  padding:14px 22px 10px;margin:0 0 26px;
  box-shadow:0 10px 30px rgba(198,60,10,.14),0 1px 0 rgba(255,255,255,.05) inset}
.marca{display:block;width:min(300px,62vw);height:auto}
.selo{display:inline-flex;align-items:center;gap:9px;
  font-family:"IBM Plex Mono",monospace;font-size:11px;letter-spacing:.14em;
  text-transform:uppercase;color:var(--acento);margin-bottom:22px}
.selo::before{content:"";width:26px;height:1px;background:var(--acento)}
h1{font-size:clamp(34px,6.4vw,64px);line-height:.98;font-weight:700;
  letter-spacing:-.035em;margin:0 0 20px;text-wrap:balance;max-width:17ch}
h1 .leve{color:var(--tinta-3);font-weight:500}
.chamada{font-size:clamp(17px,2.1vw,20px);line-height:1.5;color:var(--tinta-2);
  max-width:var(--medida);margin:0 0 26px}
.chamada strong{color:var(--tinta);font-weight:600}

/* A tira de irmas: a mesma ideia da navegacao entre as faixas de pedidos --
   quem chega por uma pagina tem de achar as outras sem voltar ao chat. */
nav.irmas{display:flex;flex-wrap:wrap;gap:8px;margin:0 0 30px}
nav.irmas a{font-family:"Exo 2",sans-serif;font-size:13px;color:var(--tinta-2);
  text-decoration:none;border:1px solid var(--linha);border-radius:99px;
  padding:6px 14px}
nav.irmas a:hover{border-color:var(--acento);color:var(--acento)}

.painel{display:grid;grid-template-columns:repeat(auto-fit,minmax(min(100%,148px),1fr));
  gap:1px;background:var(--linha);border:1px solid var(--linha);
  border-radius:6px;overflow:hidden;margin:0 0 26px;max-width:var(--corpo)}
.painel div{background:var(--papel);padding:15px 16px}
.painel .v{font-family:"Exo 2",sans-serif;font-size:clamp(22px,3vw,27px);
  font-weight:700;letter-spacing:-.02em;line-height:1.05;
  font-variant-numeric:tabular-nums}
.painel .r{font-family:"IBM Plex Mono",monospace;font-size:10px;
  letter-spacing:.11em;text-transform:uppercase;color:var(--tinta-3);margin-top:5px}

section{padding-top:62px;scroll-margin-top:16px}
.rotulo{display:flex;align-items:baseline;gap:12px;margin-bottom:8px;
  max-width:var(--corpo)}
.rotulo .num{font-family:"IBM Plex Mono",monospace;font-size:12px;
  font-weight:500;color:var(--acento);letter-spacing:.05em}
.rotulo .traco{flex:1;height:1px;background:var(--linha)}
h2{font-size:clamp(25px,3.6vw,36px);line-height:1.1;font-weight:700;
  letter-spacing:-.025em;margin:0 0 18px;text-wrap:balance;max-width:22ch}
h3{font-size:19px;font-weight:600;letter-spacing:-.012em;margin:36px 0 12px;
  text-wrap:balance;max-width:34ch}
p{max-width:var(--medida);margin:0 0 17px}
ul{max-width:var(--medida);margin:0 0 17px;padding-left:22px}
li{margin-bottom:7px}
a{color:var(--acento);text-decoration-thickness:1px;text-underline-offset:2px}
strong{font-weight:600}
code{font-size:.86em;background:var(--papel-2);padding:1.5px 5px;
  border-radius:3px;border:1px solid var(--linha);overflow-wrap:anywhere}

.nota{border-left:2px solid var(--acento);background:var(--papel-2);
  padding:14px 18px;margin:22px 0;max-width:var(--medida);
  border-radius:0 4px 4px 0}
.nota p:last-child{margin-bottom:0}
.nota .t{font-family:"IBM Plex Mono",monospace;font-size:10px;
  letter-spacing:.13em;text-transform:uppercase;color:var(--acento);
  display:block;margin-bottom:6px}
/* O bloco que ainda nao foi gerado. Vermelho de proposito: ausencia calada e
   o pior estado que esta pagina pode ter. */
.falta{border-left:3px solid var(--log);background:var(--papel-2);
  padding:13px 17px;margin:22px 0;max-width:var(--medida);
  border-radius:0 4px 4px 0;color:var(--log)}

figure{margin:32px 0;padding:0;max-width:var(--corpo)}
.fig-caixa{border:1px solid var(--linha);border-radius:6px;
  background:var(--papel-2);padding:22px 18px;overflow-x:auto;
  box-shadow:var(--sombra)}
@media (min-width:700px){.fig-caixa{padding:26px 22px}}
figure svg{display:block;width:100%;height:auto;max-width:100%;
  color:var(--tinta-2)}
figcaption{font-size:14px;line-height:1.5;color:var(--tinta-3);margin-top:12px;
  max-width:var(--medida)}
figcaption b{color:var(--tinta-2);font-weight:600}

.rolo{overflow-x:auto;margin:22px 0;border:1px solid var(--linha);
  border-radius:6px;max-width:var(--corpo)}
table{width:100%;border-collapse:collapse;font-size:14.5px;background:var(--papel)}
th,td{padding:9px 14px;text-align:left;border-bottom:1px solid var(--linha);
  vertical-align:top}
thead th{font-family:"IBM Plex Mono",monospace;font-size:10px;
  letter-spacing:.12em;text-transform:uppercase;color:var(--tinta-3);
  font-weight:500;background:var(--papel-2);white-space:nowrap}
tbody tr:last-child td{border-bottom:none}
td.dado,th.dado{font-family:"IBM Plex Mono",monospace;font-size:13px;
  font-variant-numeric:tabular-nums;white-space:nowrap}
td.num{text-align:right;font-family:"IBM Plex Mono",monospace;
  font-variant-numeric:tabular-nums}
.pino{display:inline-block;font-family:"IBM Plex Mono",monospace;
  font-size:10.5px;letter-spacing:.07em;text-transform:uppercase;
  padding:2px 8px;border-radius:99px;border:1px solid currentColor;
  white-space:nowrap}
.pino.ok{color:var(--ok)}
.pino.pend{color:var(--pend)}
.pino.nao{color:var(--tinta-3)}
/* O bloco do trio entrega a tabela dos quatro motores SEM um `.rolo` em volta
   -- e ela tem seis colunas de cabecalho `nowrap`, 636 px de min-content
   medidos. No dossie ela ficava CORTADA pelo `overflow-x:hidden` do corpo: a
   ultima coluna existia e nao dava para chegar nela. Aqui ela rola por dentro,
   que e a regra da casa («nada rola de lado; o largo rola por dentro»). */
table.tab{display:block;overflow-x:auto;max-width:100%}

/* ------------------------------------------------------------ capturas */
/* A galeria e o lugar onde a largura extra REALMENTE vira mais coluna:
   `auto-fill` mede o conteiner, entao numa ultrawide as telas ficam lado a
   lado em vez de esticarem uma a uma. */
.telas{display:grid;grid-template-columns:minmax(0,1fr);gap:26px;
  margin:26px 0;max-width:2800px}
@media (min-width:900px){
  .telas{grid-template-columns:repeat(auto-fill,minmax(420px,1fr))}
}
.tela{margin:0}
.tela img{display:block;width:100%;height:auto;border:1px solid var(--linha);
  border-radius:6px;background:var(--papel-2);box-shadow:var(--sombra)}
.tela figcaption{margin-top:9px;font-size:13.5px}
/* Caixa alta no ROTULO da captura, nunca no dado: e a mesma linha que o
   «Blumenau» virando «BLUMENAU» deixou escrita. */
.tela .qual{font-family:"IBM Plex Mono",monospace;font-size:10px;
  letter-spacing:.12em;text-transform:uppercase;color:var(--acento);
  display:block;margin-bottom:3px}
.tela.larga{grid-column:1/-1}
.tela.larga img{max-width:100%}

footer{margin-top:72px;padding-top:26px;border-top:1px solid var(--linha);
  font-size:13.5px;color:var(--tinta-3)}
footer p{max-width:var(--medida)}
@media (prefers-reduced-motion:reduce){*{animation:none!important;transition:none!important}}

@media print{
  :root{--papel:#fff;--papel-2:#fff;--papel-3:#fff;--tinta:#000;--tinta-2:#222;
    --tinta-3:#555;--linha:#bbb;--acento:#a33208;--sombra:none;
    --medida:none;--corpo:none}
  body{background:#fff;color:#000;font-size:10.5pt;line-height:1.45}
  .pagina{padding:0}
  nav.irmas{display:none!important}
  .placa{box-shadow:none;padding:8px 14px}
  .marca{width:190px}
  h1{font-size:28pt;max-width:none}
  h2{font-size:16pt;max-width:none;break-after:avoid}
  h3{font-size:12pt;max-width:none;break-after:avoid}
  section{padding-top:20pt}
  section > .rotulo{break-after:avoid}
  p,li,figcaption{max-width:none}
  figure,.rolo,.nota,.tela{break-inside:avoid}
  .fig-caixa{overflow:visible;border-color:#ccc}
  .rolo{overflow:visible}
  table{font-size:8.5pt}
  .telas{grid-template-columns:1fr 1fr;gap:14pt}
  .tela img{box-shadow:none}
  a{color:#000;text-decoration:none}
  @page{margin:14mm 13mm}
}
</style>

<div class="pagina">

<header class="capa">
  <div class="placa">
    <img class="marca" width="440" height="262" src="@@marca@@"
         alt="Símbolo do PhxSql: uma fênix de asas abertas envolvendo um cilindro de banco de dados, com trilhas de circuito saindo pelos lados">
  </div>
  <div class="selo">PhxSql · o console, a bancada e os tetos</div>
  <h1>O console em imagens<span class="leve">, e a medição por trás dele</span></h1>

  <p class="chamada">Três coisas que moravam no dossiê e <strong>mudaram de
  página, sem perder uma linha</strong>: as @@n_capturas@@ capturas do Centro de
  Controle, a bancada de dez milhões de registros contra o MySQL(R), e os dois
  painéis medidos do estado — os quatro tetos da trava global e os testes por
  área.</p>

  <div class="painel">
    <div><div class="v">@@n_capturas@@</div><div class="r">capturas</div></div>
    <div><div class="v">@@n_telas@@</div><div class="r">telas</div></div>
    <div><div class="v">@@n_temas@@</div><div class="r">temas</div></div>
    <div><div class="v">@@kib_capturas@@</div><div class="r">KiB embutidos</div></div>
    <div><div class="v">@@vezes@@×</div><div class="r">o teto de republicação</div></div>
  </div>

  <nav class="irmas" aria-label="As outras páginas do projeto">
    <a href="@@url_dossie@@">← Dossiê técnico</a>
    <a href="@@url_testes@@">Dossiê dos testes</a>
    <a href="@@url_graficos@@">Gráficos das bancadas</a>
    <a href="@@url_status@@">Status dos dez recursos</a>
  </nav>

  <div class="nota">
    <span class="t">Por que esta página existe</span>
    <p>O dossiê passou de 2,7 MB, e o guarda da republicação exige reler a
    versão publicada <strong>inteira</strong> antes de aceitar a nova —
    o mesmo teto de <strong>@@teto_kib@@ KiB</strong> que partiu a página dos
    pedidos em cinco. Só a galeria pesa <strong>@@kib_png@@ KiB de PNG</strong>,
    que viram <strong>@@kib_capturas@@ KiB</strong> embutidos: <strong>@@vezes@@×
    o teto</strong>, sozinha. Não havia prosa a cortar que resolvesse isso — o
    peso <em>é</em> a galeria.</p>
    <p>Então a seção saiu do dossiê e virou página, e trouxe junto a bancada e
    os dois painéis medidos do estado, porque só as três juntas faziam o
    dossiê caber. <strong>Nada se perdeu: mudou de endereço</strong>, e no
    dossiê cada uma virou um ponteiro que diz para cá.</p>
  </div>
</header>

<!-- ============================== 1 ============================== -->
<section id="c1">
  <div class="rotulo"><span class="num">01</span><span class="traco"></span></div>
  <h2>O console em imagens, do login à replicação</h2>

  <p>O caminho que o dono pediu, fotografado contra o <strong>servidor de
  verdade</strong>: do login até a replicação, passando pelo painel, pelas
  tabelas, pela grade, pela consulta, pelo diagrama ER, pela telemetria e pelo
  profiler — mais o modo multitela com as quatro telas lado a lado.</p>

  <p>Nada aqui é maquete. Um <code>phxsqld</code> sobe numa porta própria, um
  script popula três tabelas ligadas por chave estrangeira e um segundo banco,
  faz movimento suficiente para os gráficos terem o que mostrar, e o Playwright
  dirige a interface e dispara a foto. O procedimento inteiro está em
  <code>docs/dossie/capturar-dossie.mjs</code>, e as imagens entram nesta página
  por <code>docs/dossie/capturas-no-dossie.py</code> — <strong>captura de tela
  também é número que envelhece</strong>, e por isso ela sai de um gerador em vez
  de ser colada à mão.</p>

  <div class="nota">
    <p>As duas colunas são os <strong>dois temas</strong>, e não duas versões: é
    a mesma página, com o mesmo dado, na mesma sessão. As cinco cores da ação —
    <em>verde inclui, amarelo altera, rosa marca, vermelho exclui de vez, azul
    consulta</em> — escurecem no tema claro, pelo mesmo motivo do vermelhão da
    marca: verde e rosa claros não passam de 4,5:1 sobre papel.</p>
  </div>

@@capturas@@
</section>

<!-- ============================== 2 ============================== -->
@@secao2@@

<!-- ============================== 3 ============================== -->
@@secao3@@

<footer>
  <p>Gerado por <code>docs/dossie/pagina-do-console.py</code>, que monta a
  casca e <strong>preserva</strong> o que os outros geradores escreveram dentro
  das marcas. Nenhum número desta página foi digitado: as capturas saem do
  <code>capturas-no-dossie.py</code>, a bancada do <code>numeros-da-bancada.py</code>,
  os três motores do <code>trio-de-motores.py</code>, os tetos do
  <code>tetos-da-trava.py</code>, os testes por área do
  <code>cobertura-por-area.py</code>, e a numeração das figuras do
  <code>numerar-figuras.py</code>.</p>
  <p>Esta página nasceu da <a href="@@url_dossie@@">seção 18 do dossiê técnico</a>
  em 23/09/2026, quando o arquivo passou do teto de republicação. As legendas
  das capturas não levam número de figura, por decisão registrada: elas não são
  referenciadas por número em lugar nenhum.</p>
</footer>
</div>
"""


# --------------------------------------------------- as secoes movidas
#
# Elas vieram VERBATIM das secoes 32 e 35 do dossie, em 23/09/2026 -- nao
# foram reescritas. A prosa mora aqui porque a pagina NAO se edita: quem
# quiser mudar uma palavra muda este arquivo e roda o gerador, que e a mesma
# disciplina das outras paginas satelites da pasta.
#
# Os `@@...@@` de dentro sao as marcas dos outros geradores, e o `trocar()`
# as resolve na segunda passada.
SECAO_2 = """<section id="c2">
  <div class="rotulo"><span class="num">02</span><span class="traco"></span></div>
  <h2>A bancada: dez milhões de linhas, e os três motores a um milhão</h2>
  <p>Um motor novo só tem duas maneiras de se apresentar: prometendo, ou
  medindo. Esta seção mede. Dez milhões de registros, a mesma tabela dos dois
  lados, os mesmos dados na mesma ordem — sem sorteio, porque comparar motores
  com entradas diferentes não compara nada. Do outro lado, MySQL(R) 8.0.46 na
  mesma máquina, no mesmo disco, na mesma hora.</p>

  <p>Cada fase roda num <strong>processo separado</strong> de propósito. Assim o
  medidor lê <code>/proc/&lt;pid&gt;/io</code> e <code>/proc/&lt;pid&gt;/status</code>
  do processo que fez exatamente aquela fase, sem misturar com as outras — e o
  <code>mysqld</code> recebe o mesmo tratamento, por diferença de contadores.</p>

  <figure>
    <div class="fig-caixa">
@@bancada@@
    </div>
    <figcaption><b>Figura 27.</b> O perfil do motor num desenho: perde onde
    grava, ganha onde lê em ordem. A varredura por faixa é o formato mostrando
    o que ele é — linhas vizinhas no índice são bytes vizinhos no
    <code>.reg</code>.</figcaption>
  </figure>

  <h3>Os números, sem arredondar para o lado bom</h3>

@@bancada_tabela@@

  <p>O espaço ocupado é <em>consequência de projeto, não defeito</em>: o
  <code>.reg</code> é de slot fixo, e slot fixo é o que compra o endereçamento
  O(1) e a ordem de digitação. Um registro curto ocupa o mesmo que um longo.
  Trocar isso por compactação seria trocar exatamente as duas garantias que
  fazem este formato ser este formato.</p>

  <div class="nota">
    <p><strong>Um erro meu, que a bancada quase escondeu.</strong> Na primeira
    montagem o MySQL(R) recebia um <code>WHERE id IN (20.000 ids)</code> e o
    PhxSql fazia 20.000 buscas separadas. Não é o mesmo trabalho: é a
    <em>forma da pergunta</em> dando 41&#215; de vantagem a um lado. Refeito com
    uma instrução por operação nos dois, a busca pontual caiu de 41&#215;
    perdendo para pouco mais de 3&#215;, e a atualização virou empate.</p>
    <p>O segundo erro foi para o outro lado, e demorou mais a aparecer. Na
    varredura por faixa o MySQL(R) recebia <code>COUNT(*) + SUM(valor)</code>
    sobre 1.250.000 linhas e o PhxSql lia 20.000 — mesma pergunta,
    <strong>1,6% do trabalho</strong>. O «5&#215; mais rápido» que essa versão
    produzia não era o motor sendo rápido. Hoje a fase lê a faixa inteira e
    soma, como o outro lado, e a bancada foi refeita do zero.</p>
    <p>Bancada mal montada mente com número, que é a mentira mais convincente
    que existe — e mente para os dois lados.</p>
    <p><strong>A prova de que agora está igual</strong> não é a promessa: é a
    soma. Os dois motores devolvem <code>1.250.000</code> linhas e
    <code>5.576.201.000,00</code> — o mesmo total, até o centavo, calculado por
    dois códigos que não têm uma linha em comum. Trabalho diferente não daria o
    mesmo número.</p>
    <p>E o resultado sobreviveu ao conserto: a varredura continua a favor do
    PhxSql — o número da tabela acima sai da medição, não desta frase.</p>
  </div>

  <div class="nota">
    <p><strong>Esta medição é a terceira, e as três contam a mesma história por
    ângulos diferentes.</strong> A primeira comparava trabalho diferente na
    varredura e foi descartada. A segunda foi honesta e mostrou a inserção
    <strong>20,7&#215;</strong> atrás. Esta veio depois do CRC <em>slice-by-8</em>,
    e a mesma inserção ficou <strong>7,7&#215;</strong> atrás — 2,8&#215; mais
    rápida no mesmo trabalho, mesma máquina. A quarta, depois do cache de páginas,
    trouxe a inserção para <strong>2,6&#215;</strong> atrás.</p>
    <p>O ganho não ficou só na escrita: a exclusão caiu 2,1&#215; e as leituras
    1,5&#215;, porque o CRC estava em <em>todo</em> toque de página, não apenas
    nas gravações. Três das cinco operações agora são do PhxSql.</p>
  </div>

  <h3>Onde a inserção gastava o tempo, e o que estava errado no diagnóstico</h3>

  <p>A bancada da época dizia que a inserção era 7,7× mais lenta que a do
  MySQL(R), e o
  diagnóstico anterior foi <em>«97% de CPU, disco parado, a culpa é da B+tree
  reescrita nó a nó»</em>. Estava certo — para a <strong>biblioteca</strong>,
  que era onde a medição tinha sido feita. Pelo <strong>servidor</strong> a
  conta era outra, e ninguém tinha medido.</p>

  <p>O servidor chamava <code>sincronizar()</code> depois de cada gravação, o
  que é <code>fsync</code> em todos os arquivos da tabela. Medindo os dois lados
  separados, com as mesmas 20.000 linhas na mesma tabela:</p>

  <div class="rolo">
    <table>
      <thead><tr><th>Quando sincroniza</th><th class="num">linhas/s</th><th class="num">ganho</th></tr></thead>
      <tbody>
        <tr><td class="dado">a cada linha — o que o servidor fazia</td><td class="num">1.289</td><td class="num">—</td></tr>
        <tr><td class="dado">a cada 10</td><td class="num">6.990</td><td class="num">5,4×</td></tr>
        <tr><td class="dado">a cada 100</td><td class="num">18.264</td><td class="num">14,2×</td></tr>
        <tr><td class="dado">a cada 1.000</td><td class="num">24.858</td><td class="num">19,3×</td></tr>
        <tr><td class="dado">só no fim</td><td class="num">26.301</td><td class="num">20,4×</td></tr>
      </tbody>
    </table>
  </div>

  <p><strong>95% do tempo era <code>fsync</code>.</strong> O que sobra são 37,5
  µs por linha, e aí sim o diagnóstico antigo vale: 65% deles são os dois
  índices. Ou seja, as duas medições estavam certas — o que estava errado era
  aplicar a conclusão de uma ao caminho da outra.</p>

  <p>O medidor termina repetindo o primeiro caso. Se as duas linhas não
  baterem, a medida está contaminada por cache do sistema de arquivos, e o
  número não vale. Bateram: 1.289 e 1.253.</p>

  <h3>O que exatamente se arrisca ao não sincronizar</h3>

  <p>Vale ser preciso, porque «gravar na memória» soa mais perigoso do que é. O
  <code>write</code> acontece <strong>sempre</strong>, na hora: os bytes vão
  para o sistema operacional em toda gravação, sem buffer nosso. Então outro
  processo que abrir o arquivo <strong>vê o dado imediatamente</strong>,
  sincronizado ou não.</p>

  <p>O <code>fsync</code> protege de <em>uma</em> coisa: o computador perder
  energia antes de o sistema descarregar a página. Em <code>por_lote</code> a
  janela do que se perde é o que entrou nos últimos <code>lote_milissegundos</code>
  — 200 por padrão. Quem não pode perder nem isso põe
  <code>por_operacao</code> e paga os 20×; a escolha é de quem opera, e o
  <code>config.json</code> é onde ela se faz.</p>

  <p>Um relógio de fundo fecha a janela quando ninguém grava. Sem ele, a última
  venda do dia às 18h ficaria sem <code>fsync</code> a noite inteira — e o
  ganho teria sido comprado com um buraco.</p>

  <h3>A fresta entre abrir e gravar</h3>

  <p>Medir a gravação achou uma coisa pior que lentidão. O servidor tomava a
  trava de dados para <em>abrir</em> a tabela, <strong>soltava</strong>, e só
  então tomava de novo para <em>gravar</em>.</p>

  <p>Abrir uma tabela lê o cabeçalho, e o cabeçalho traz o
  <code>slot_count</code> — o contador que decide onde a próxima linha vai.
  Nessa fresta duas operações simultâneas abriam a tabela, as duas guardavam
  <code>slot_count = N</code>, e as duas gravavam no rowid N+1.</p>

  <figure>
    <div class="fig-caixa">
      <svg viewBox="0 0 840 250" role="img" aria-label="Duas operações abrem a tabela, ambas leem slot_count igual a cem, a trava é solta entre abrir e gravar, e as duas gravam no rowid cento e um: a segunda por cima da primeira">
        <g font-family="IBM Plex Mono, monospace" font-size="11.5" fill="currentColor">
          <text x="16" y="18" font-size="10" opacity=".55" letter-spacing=".08em">COMO ERA — A TRAVA SOLTA NO MEIO</text>

          <text x="16" y="46" font-size="10.5" opacity=".7">operação A</text>
          <rect x="96" y="32" width="118" height="26" rx="4" fill="none" stroke="currentColor" stroke-width="1.3"/>
          <text x="155" y="49" text-anchor="middle" font-size="10.5">abre · lê 100</text>
          <rect x="330" y="32" width="150" height="26" rx="4" fill="none" stroke="var(--log)" stroke-width="1.5"/>
          <text x="405" y="49" text-anchor="middle" font-size="10.5" fill="var(--log)">grava no rowid 101</text>

          <text x="16" y="88" font-size="10.5" opacity=".7">operação B</text>
          <rect x="216" y="74" width="118" height="26" rx="4" fill="none" stroke="currentColor" stroke-width="1.3"/>
          <text x="275" y="91" text-anchor="middle" font-size="10.5">abre · lê 100</text>
          <rect x="496" y="74" width="150" height="26" rx="4" fill="none" stroke="var(--log)" stroke-width="1.5"/>
          <text x="571" y="91" text-anchor="middle" font-size="10.5" fill="var(--log)">grava no rowid 101</text>

          <path d="M660 87 L700 87" stroke="var(--log)" stroke-width="1.4"/>
          <text x="706" y="91" font-size="10.5" fill="var(--log)">A sumiu</text>

          <line x1="16" y1="122" x2="824" y2="122" stroke="currentColor" stroke-width=".8" opacity=".25"/>

          <text x="16" y="148" font-size="10" opacity=".55" letter-spacing=".08em">COMO FICOU — UM BLOCO SÓ</text>

          <text x="16" y="176" font-size="10.5" opacity=".7">operação A</text>
          <rect x="96" y="162" width="270" height="26" rx="4" fill="none" stroke="var(--acento)" stroke-width="1.5"/>
          <text x="231" y="179" text-anchor="middle" font-size="10.5" fill="var(--acento)">abre · lê 100 · grava 101</text>

          <text x="16" y="212" font-size="10.5" opacity=".7">operação B</text>
          <rect x="382" y="198" width="270" height="26" rx="4" fill="none" stroke="var(--acento)" stroke-width="1.5"/>
          <text x="517" y="215" text-anchor="middle" font-size="10.5" fill="var(--acento)">abre · lê 101 · grava 102</text>
        </g>
      </svg>
    </div>
    <figcaption><b>Figura 28.</b> O defeito não era a trava faltar — era ela
    cobrir metade do trabalho. Com índice único sobre a coluna, o índice pegava
    e virava «chave duplicada»; foi assim que apareceu. <strong>Sem índice
    único, a linha sumia em silêncio.</strong></figcaption>
  </figure>

  <p>Um teste em <code>tests/tabela.rs</code> deixa o contrato escrito: duas
  aberturas da mesma tabela disputam o mesmo rowid, e por isso quem abre precisa
  serializar. Ele não testa um defeito — testa uma propriedade do formato, para
  que ninguém a redescubra do jeito difícil.</p>

  <h3>O CRC de página inteira, e o cache que o tirou do caminho</h3>

  <p>A rodada anterior parou num lugar desconfortável: <strong>83,5% do tempo de
  uma inserção estava no <code>.ndx</code></strong>, e a conta do CRC
  <em>não fechava</em> — o medidor estimava, por um <code>strace</code> de outro dia,
  ~20 toques de página por linha, e a 2,34&#8201;µs de CRC por página de 4&#8201;KiB
  isso daria ~47&#8201;µs, mais do que os 44,4&#8201;µs medidos no total. Ficou
  registrado como <em>pista aberta</em>, e não como conclusão.</p>

  <p>A conta não fechava porque o número era <strong>citado, e não medido</strong>.
  Hoje o medidor conta os toques por dentro:</p>

  <div class="rolo">
    <table>
      <thead><tr><th>Por linha inserida, com dois índices</th><th class="num">páginas</th></tr></thead>
      <tbody>
        <tr><td>servidas pelo cache</td><td class="num">8,80</td></tr>
        <tr><td>lidas do arquivo</td><td class="num">0,00</td></tr>
        <tr><td>gravadas</td><td class="num">2,06</td></tr>
      </tbody>
    </table>
  </div>

  <p>São <strong>10,86</strong>, e não 20. Antes do cache, as 10,86 passavam
  <em>todas</em> pelo CRC: 25,4&#8201;µs de 44,4 — <strong>57% do tempo de uma
  inserção era CRC-32 de página</strong>. E a raiz da árvore é a mesma página em
  todas as inserções da carga.</p>

  <p>Um cache de páginas <strong>de leitura</strong> no <code>.ndx</code>, com
  despejo por segunda chance, levou a inserção de <strong>44,4 para 18,5&#8201;µs
  por linha (2,40&#215;)</strong> sem mudar formato, sem mudar garantia e sem tocar
  na B+tree. A linha que mais mudou diz o que aconteceu: <em>conferir a chave
  única</em> caiu de 20,5% para 2,3% do tempo — é uma descida na árvore que não
  escreve nada, exatamente o trabalho que o cache serve de graça.</p>

  <div class="nota">
    <span class="t">Era de leitura; desde a 0.18.0 é write-back, e isso é escolha
    avisada</span>
    <p>Segurar página suja em RAM comprou mais (16,4 → 7,5&#8201;µs por linha) e
    trocou a garantia que o cache de leitura dava sozinho: hoje uma queda do
    <em>processo</em> <strong>pode</strong> atrasar o <code>.ndx</code> em relação ao
    <code>.reg</code>, porque a página fica em RAM até o despejo, o fechamento da
    tabela ou o <code>sincronizar</code> — não vai mais ao arquivo a cada chave. A
    troca só ficou aceitável porque passou a se <strong>avisar</strong>: a marca de
    sujo (byte 52 do cabeçalho) vai ao arquivo antes da primeira página suja e só
    sai depois de todas irem ao disco, então quem reabre um <code>.ndx</code>
    atrasado acha a marca levantada e recusa responder até <code>reindexar</code>
    reconstruir — 0,31&#8201;s por milhão de chaves. Detalhe completo em
    <code>docs/FORMATO.md</code>, «A marca de sujo (byte 52)».</p>
    <p>O despejo é por <strong>segunda chance</strong>, e não fila simples: a raiz,
    a página mais visitada de todas, sairia junto com as outras assim que o teto
    enchesse. O teto — 2.048 páginas, 8&#8201;MiB por tabela aberta — saiu de uma
    varredura de quatro tamanhos, e não do chute. É o campo
    <code>recursos.cache_paginas</code> do <code>config.json</code>, que existia
    desde a 0.13.0 <strong>sem nada por trás</strong>.</p>
  </div>

  <h3>E o cabeçalho que reserializava o esquema a cada linha</h3>

  <p>Achado respondendo a uma pergunta sobre outra coisa — «e se o
  <code>.ndx</code> parasse durante a carga?» —, que é onde essas coisas costumam
  aparecer. Toda inserção chamava <code>gravar_cabecalho</code>, e ele fazia
  <strong>cinco coisas, das quais uma era necessária</strong>:</p>

  <div class="rolo">
    <table>
      <thead><tr><th class="num">#</th><th>o que fazia por linha inserida</th><th>precisa?</th></tr></thead>
      <tbody>
        <tr><td class="num">1</td><td>serializar o <strong>esquema inteiro</strong></td><td><span class="pino nao">não</span></td></tr>
        <tr><td class="num">2</td><td>calcular o <strong>CRC-32</strong> desse bloco</td><td><span class="pino nao">não</span></td></tr>
        <tr><td class="num">3</td><td>gravar os 128 bytes de cabeçalho, com os contadores</td><td><span class="pino ok">sim</span></td></tr>
        <tr><td class="num">4</td><td>gravar o <strong>bloco de esquema outra vez</strong>, byte a byte igual</td><td><span class="pino nao">não</span></td></tr>
        <tr><td class="num">5</td><td>perguntar o <strong>tamanho do arquivo</strong></td><td><span class="pino nao">não</span></td></tr>
      </tbody>
    </table>
  </div>

  <p>O esquema é imutável depois que a tabela nasce. Ele passou a ser serializado
  uma vez, no construtor, com o CRC junto; e o caminho quente ganhou um irmão que
  grava <em>só</em> o cabeçalho. O bloco de esquema e o teste de tamanho ficaram
  onde importam: na criação do volume. <strong>Só o <code>.reg</code>: 6,8 → 5,3
  µs por linha (1,27×); com dois índices, 18,5 → 17,0</strong>. Nenhum byte mudou
  de lugar no disco.</p>

  <div class="nota">
    <span class="t">O pedido pedia outra coisa, e a medição mandou</span>
    <p>O item da lista dizia «ordene as chaves do lote antes de inserir no
    <code>.ndx</code>, para chaves vizinhas caírem na mesma folha». O alvo estava
    certo; o mecanismo, não. Medindo antes de escrever código: <strong>a desordem
    das chaves custava 1,06&#215;</strong>. Ordenar teria comprado quase nada — o
    custo não era de localidade, era de reler e recalcular CRC da mesma página.</p>
    <p>Depois do cache a desordem passou a custar <strong>1,19&#215;</strong>: a
    localidade só importa quando não se está pagando CRC de qualquer jeito.
    Ordenar continua não feito, agora com o preço na mesa — implementá-lo exige
    gravar o <code>.reg</code> antes de indexar, e aí uma falha no meio deixa linha
    sem chave, sem como desfazer.</p>
  </div>

  <h3>E o <code>.log</code>, que gravava duas vezes por evento</h3>

  <p>O diário fazia <strong>duas escritas por evento</strong>: os 44 bytes do
  evento em si, e os 64 bytes do cabeçalho com <code>fim</code> e
  <code>qtd_eventos</code>. O evento tem de ir na hora — ele é a história, e é a
  posição de que a replicação depende. O cabeçalho é um <em>contador</em>, e a
  leitura sabe recalculá-lo varrendo os próprios eventos.</p>

  <p>Ele passou a ir no <code>sincronizar</code>: <strong>1,22 → 0,67&#8201;µs por
  evento (1,82&#215;)</strong>, e a inserção completa com dois índices de
  <strong>17,0 para 15,9&#8201;µs</strong>.</p>

  <div class="nota">
    <span class="t">Adiar o contador pediu um caminho de reparo</span>
    <p>Uma queda antes do <code>sincronizar</code> deixaria o cabeçalho atrasado, e
    a próxima gravação escreveria <strong>por cima</strong> dos eventos já
    gravados — evento destruído, e não apenas invisível. Então <code>abrir</code>
    varre para a frente a partir do <code>fim</code> gravado, validando cada evento
    pelo CRC-32 que ele já carrega, e para no primeiro que não confere.</p>
    <p><strong>Segurar os eventos em RAM continua fora</strong>, e a razão não é de
    tamanho — mediu-se 4,2% — e sim de natureza: índice perdido se reconstrói do
    <code>.reg</code>; evento perdido não se reconstrói.</p>
  </div>

  <h3>E o Profiler desligado, que custava 7% de uma carga</h3>

  <p>O ponto de captura fazia o trabalho <strong>antes</strong> de conferir se
  havia o que capturar: dois <code>Json::analisar</code> do corpo inteiro, três
  <code>String</code> e um mutex, para no fim descobrir que o Profiler estava
  desligado e devolver nada. Num <code>inserir_lote</code> de 5.000 linhas isso é
  analisar meio megabyte de JSON <em>duas vezes</em>, para nada. O portão virou um
  <code>AtomicBool</code> lido antes de qualquer trabalho: <strong>40.600 →
  43.450 linhas/s</strong> na carga pela rede.</p>

  <div class="nota">
    <span class="t">Diagnóstico plausível não é diagnóstico medido</span>
    <p>Estava escrito em três lugares deste projeto que «o mutex era o pior pedaço,
    porque ele serializa». Medido, em <code>--example quem-custava</code>: um
    <code>lock</code>/<code>unlock</code> sem disputa custa
    <strong>13,2&#8201;ns</strong>; analisar o corpo de um lote custa
    <strong>3.456&#8201;µs</strong>. São <strong>262.000&#215;</strong> — o mutex
    nunca foi o problema. O errado sobreviveu justamente porque o conserto
    <em>funcionou</em>, por outro motivo.</p>
  </div>

  <h3><code>BULKINSERT</code>: a tabela reservada, e a janela que não fecha</h3>

  <p>Uma carga longa quer duas coisas que o servidor não dava: ninguém mais
  mexendo naquela tabela enquanto ela entra, e <strong>uma sincronização só</strong>,
  no fim. As duas saem da mesma reserva:</p>

  <div class="rolo">
    <table>
      <tbody>
        <tr><td><code>{"op":"bulkinsert",…,"ligado":true}</code></td><td>a tabela passa a ser sua</td></tr>
        <tr><td>… as inserções …</td><td>a janela de durabilidade não fecha</td></tr>
        <tr><td><code>{"op":"bulkinsert",…,"ligado":false}</code></td><td>um <code>fsync</code>, e a tabela volta</td></tr>
      </tbody>
    </table>
  </div>

  <figure>
    <div class="fig-caixa">
      <svg viewBox="0 0 840 250" role="img" aria-label="Sem reserva a janela de durabilidade fecha várias vezes durante a carga, e cada fechamento é um fsync; com a reserva ela não fecha, e a carga inteira termina num fsync só, o que mede 1,53 vezes mais rápido">
        <defs>
          <marker id="setaCg" viewBox="0 0 10 10" refX="9" refY="5" markerWidth="6" markerHeight="6" orient="auto-start-reverse">
            <path d="M0 0 L10 5 L0 10 z" fill="currentColor"/>
          </marker>
        </defs>
        <g font-family="IBM Plex Mono, monospace" font-size="11.5" fill="currentColor">

          <text x="16" y="20" font-size="10" opacity=".55" letter-spacing=".08em">SEM RESERVA — A JANELA FECHA SOZINHA</text>
          <line x1="16" y1="60" x2="700" y2="60" stroke="currentColor" stroke-width="1.4" marker-end="url(#setaCg)"/>
          <text x="16" y="46" font-size="10" opacity=".6">as 5.000 linhas do lote</text>
          <g stroke="currentColor" stroke-width="1.6">
            <line x1="120" y1="50" x2="120" y2="70"/>
            <line x1="236" y1="50" x2="236" y2="70"/>
            <line x1="352" y1="50" x2="352" y2="70"/>
            <line x1="468" y1="50" x2="468" y2="70"/>
            <line x1="584" y1="50" x2="584" y2="70"/>
            <line x1="700" y1="50" x2="700" y2="70"/>
          </g>
          <text x="120" y="86" text-anchor="middle" font-size="10" opacity=".7">fsync</text>
          <text x="236" y="86" text-anchor="middle" font-size="10" opacity=".7">fsync</text>
          <text x="352" y="86" text-anchor="middle" font-size="10" opacity=".7">fsync</text>
          <text x="468" y="86" text-anchor="middle" font-size="10" opacity=".7">fsync</text>
          <text x="584" y="86" text-anchor="middle" font-size="10" opacity=".7">fsync</text>
          <text x="700" y="86" text-anchor="middle" font-size="10" opacity=".7">fsync</text>
          <text x="726" y="64" font-size="12" font-weight="600">43.500/s</text>

          <line x1="16" y1="118" x2="824" y2="118" stroke="currentColor" stroke-width="1" opacity=".25"/>

          <text x="16" y="146" font-size="10" fill="var(--acento)" letter-spacing=".08em">COM A RESERVA — A JANELA NÃO FECHA</text>
          <line x1="16" y1="186" x2="700" y2="186" stroke="var(--acento)" stroke-width="1.6" marker-end="url(#setaCg)"/>
          <text x="16" y="172" font-size="10" opacity=".6">as mesmas 5.000 linhas</text>
          <line x1="700" y1="176" x2="700" y2="196" stroke="var(--acento)" stroke-width="1.8"/>
          <text x="700" y="212" text-anchor="middle" font-size="10" fill="var(--acento)">um fsync</text>
          <text x="726" y="190" font-size="12" font-weight="600" fill="var(--acento)">66.500/s</text>

          <text x="16" y="238" font-size="10" opacity=".6">Os outros que pedirem esta tabela recebem 4002 EM_CARGA na hora, dizendo quem reservou e desde quando.</text>
        </g>
      </svg>
    </div>
    <figcaption>O ganho não vem de escrever menos: vem de <strong>confirmar
    menos vezes</strong>. <strong>1,53&#215; medido</strong> — 43.044 e 44.026
    linhas/s sem reserva contra 65.737 e 67.339 com ela, dois pares de
    corridas.</figcaption>
  </figure>

  <p>Os outros recebem <strong>erro na hora</strong>, e não espera: o novo
  <strong>4002 <code>EM_CARGA</code></strong>, dizendo <strong>quem</strong>
  reservou e <strong>desde quando</strong> — sem isso, «tabela em carga» manda a
  pessoa procurar sozinha quem está segurando. Ele vem com <code>repetir: true</code>,
  e é o <strong>segundo</strong> erro do protocolo que pede nova tentativa (o outro
  é o de E/S): é o que separa «espere um pouco» de «você não pode».</p>

  <div class="nota">
    <span class="t">Contra reserva órfã há duas redes, e não uma</span>
    <p>A <strong>queda da conexão</strong> solta na hora, por qualquer caminho de
    saída. Mas o soquete que fica pendurado <em>vivo</em>, com o cliente morto do
    outro lado, é exatamente o caso em que ela não pega — e aí vale o
    <strong>prazo</strong>, <code>recursos.carga_prazo_min</code>, padrão 30
    minutos, ajustável no <code>config.json</code> e visível na tela de
    configuração, que também lista as cargas em andamento.</p>
    <p>Foi a prova pelo soquete, em <code>bancada/carga/bulkinsert.py</code>, que
    achou o que dez testes unitários não achavam — e o primeiro resultado dela
    <em>acusava</em> a queda da conexão de não soltar. O defeito estava no teste:
    o <code>makefile()</code> do Python segura o descritor, então fechar só o
    soquete deixa o servidor sem ver o fim. <strong>Teste que passa por engano é
    pior que teste que falta.</strong></p>
  </div>

  <div class="nota">
    <span class="t">Não é transação, e o documento repete isso alto</span>
    <p><code>BULKINSERT</code> <strong>reserva a tabela; não desfaz nada</strong>.
    Quem ler «exclusiva até concluir» e entender <code>BEGIN</code> vai perder
    dado. <code>docs/SQL.md</code> registra isso junto com as três coisas que o
    analisador de SQL não vai poder tratar como açúcar sintático: é palavra
    reservada; vale para a <strong>sessão</strong>, e não para o comando, então um
    driver que multiplexa conexões quebra a exclusividade sem avisar; e o
    <code>EM_CARGA</code> tem de virar <em>serialization failure</em> no SQLSTATE,
    e não <em>access denied</em>, senão o driver do outro lado desiste em vez de
    repetir.</p>
  </div>

  <h3>A construção em lote da B+tree, e o item que ela não salvou</h3>

  <p>O <code>reindexar</code> inseria <strong>chave a chave</strong> — uma descida
  na árvore por chave, que é exatamente o trabalho do caminho de dentro feito de
  novo. Era por isso que adiar o índice numa carga comprava 1,02&#215;: o preço
  não sumia, mudava de lugar e continuava o mesmo.</p>

  <p><code>construir_em_lote</code> não desce a árvore nenhuma vez. Ordena as
  chaves, enche as folhas <em>em sequência</em> e monta os níveis de cima por
  cima dos de baixo. Um milhão de chaves:</p>

  <div class="rolo">
    <table>
      <thead><tr><th></th><th class="num">montar</th><th class="num">páginas</th><th class="num">varrer</th></tr></thead>
      <tbody>
        <tr><td>uma a uma (o <code>reindexar</code> de antes)</td><td class="num">7,72&#8201;s</td><td class="num">6.136</td><td class="num">0,036&#8201;s</td></tr>
        <tr><td><strong>em lote</strong></td><td class="num"><strong>0,31&#8201;s</strong></td><td class="num">5.271</td><td class="num">0,028&#8201;s</td></tr>
      </tbody>
    </table>
  </div>

  <p><strong>23&#215; a 25&#215;</strong>, em duas corridas. Todo
  <code>reindexar</code> e todo <em>reparar índice</em> andam nisso.</p>

  <div class="nota">
    <span class="t">80% de enchimento, e o número é medido</span>
    <p>Encher a folha a 70% é a folga clássica e <strong>não compra nada</strong>:
    inserção aleatória já assenta perto de 69% de ocupação sozinha, que é um
    resultado clássico de B-tree. De 90% para cima a folha fica sem folga —
    crescer 10% aloca mais de dois mil páginas e fica <strong>mais lento</strong>
    do que na árvore mais frouxa, e a varredura mais rápida não paga isso. 80% é
    a ocupação mais densa que ainda absorve 10% de crescimento sem alocar uma
    página.</p>
    <p>E a primeira versão do medidor deu 100% de graça, porque as chaves de
    crescimento entravam <strong>acima</strong> da faixa — e chave maior que
    todas vai sempre para a última folha, então a divisão que o enchimento
    deveria provocar nunca acontecia. <em>Medidor com furo mede o furo.</em></p>
  </div>

  <p>Com o lote pronto, adiar o índice virou item de implementar. <strong>Medi
  antes, e o número o derrubou.</strong> O 1,59&#215; vale para tabela vazia, mas
  o <code>reindexar</code> refaz sobre a tabela <em>inteira</em>: carregando M
  numa tabela de 200.000, o ganho é 1,22&#215; quando M dobra a tabela e vira
  <strong>prejuízo abaixo de M&#8239;&#8776;&#8239;N/3</strong> — 0,86&#215; com
  M=40.000, 0,22&#215; com M=4.000. E cobraria marcar <strong>índice suspenso no
  formato</strong>, cujo defeito é busca respondendo errado em silêncio depois de
  uma queda. Ficou fora com o número na mesa.</p>

  <h3>A réplica: a causa registrada apontava para o lado errado do fio</h3>

  <p>Estava escrito em dois documentos que a réplica ficava para trás porque
  «aplicar decodifica a imagem para <code>Value</code> e <strong>reencoda</strong>
  o payload, em vez de gravar os bytes que vieram». A primeira coisa foi medir a
  acusação — e ela custa <strong>0,35&#8201;µs</strong>:
  <code>aplicar_evento</code> são 16,15&#8201;µs contra 15,80 de uma inserção
  local pura. A réplica media <strong>229&#8201;µs por evento</strong>.</p>

  <figure>
    <div class="fig-caixa">
      <svg viewBox="0 0 840 240" role="img" aria-label="Dos 229 microssegundos por evento, o caminho de CPU dos dois lados custa 24 e o resto estava no source, que varria o diário desde o começo a cada lote, e no laço, que dormia depois de toda rodada">
        <defs>
          <marker id="setaR" viewBox="0 0 10 10" refX="9" refY="5" markerWidth="6" markerHeight="6" orient="auto-start-reverse">
            <path d="M0 0 L10 5 L0 10 z" fill="currentColor"/>
          </marker>
        </defs>
        <g font-family="IBM Plex Mono, monospace" font-size="11.5" fill="currentColor">

          <text x="16" y="20" font-size="10" opacity=".55" letter-spacing=".08em">229 µs POR EVENTO — ONDE ELES ESTAVAM</text>

          <rect x="16" y="34" width="88" height="34" rx="4" fill="none" stroke="currentColor" stroke-width="1.4"/>
          <text x="60" y="50" text-anchor="middle" font-size="10">CPU dos</text>
          <text x="60" y="62" text-anchor="middle" font-size="10">dois lados</text>
          <text x="112" y="55" font-size="11" font-weight="600">24 µs</text>

          <rect x="16" y="82" width="600" height="34" rx="4" fill="none" stroke="var(--acento)" stroke-width="1.7"/>
          <text x="30" y="103" font-size="11" fill="var(--acento)">o source varria o diário desde o começo a cada lote</text>
          <text x="624" y="103" font-size="11" font-weight="600" fill="var(--acento)">~180 µs</text>

          <rect x="16" y="130" width="150" height="34" rx="4" fill="none" stroke="currentColor" stroke-width="1.4"/>
          <text x="30" y="151" font-size="11">o laço dormia</text>
          <text x="174" y="151" font-size="11" font-weight="600">o resto</text>

          <line x1="16" y1="184" x2="824" y2="184" stroke="currentColor" stroke-width="1" opacity=".25"/>

          <text x="16" y="208" font-size="10" opacity=".55" letter-spacing=".08em">A ACUSAÇÃO QUE ESTAVA REGISTRADA</text>
          <rect x="16" y="214" width="10" height="16" rx="2" fill="none" stroke="currentColor" stroke-width="1.4"/>
          <text x="36" y="227" font-size="11">reencodar o payload &#8212; 0,35 µs, e não os 229</text>
        </g>
      </svg>
    </div>
    <figcaption>Servir «500 eventos a partir de P» caminhava pelos P anteriores
    lendo o cabeçalho de cada um: 1,11&#8201;µs por evento com P=0, e
    <strong>72,65</strong> com P=90.000. Linear em P, e portanto quadrático no
    total.</figcaption>
  </figure>

  <p>Alcançar 100.000 eventos de 500 em 500 gastava <strong>4,07&#8201;s só do
  lado de quem serve</strong>, com três réplicas fazendo isso ao mesmo tempo sob
  a trava global do master. Uma <strong>marca de posição</strong> levou a
  <strong>0,09&#8201;s — 45&#215;</strong>.</p>

  <div class="nota">
    <span class="t">A marca é uma dica, e é isso que a torna segura</span>
    <p>Uma marca errada faz a leitura começar no lugar errado e o
    <strong>CRC do evento recusar</strong>, ou cair depois do fim do arquivo e
    devolver vazio. Nenhum dos dois entrega evento errado.</p>
    <p>Ela mora no <strong>servidor</strong>, e não na tabela, porque a tabela é
    aberta e fechada a cada pedido — e são pedidos seguidos que ela serve. E são
    <strong>várias por tabela</strong>: um source atende réplicas em posições
    diferentes, e uma marca só seria empurrada para frente pela mais adiantada e
    nunca serviria às outras. Foi essa correção que levou o número de 7.835 para
    17.450.</p>
  </div>

  <div class="rolo">
    <table>
      <thead><tr><th>na bancada dos quatro servidores</th><th class="num">antes</th><th class="num">agora</th></tr></thead>
      <tbody>
        <tr><td>master, com a imagem no diário</td><td class="num">28.914/s</td><td class="num">34.048/s</td></tr>
        <tr><td><strong>aplicação, por réplica (as três em paralelo)</strong></td><td class="num"><strong>4.273/s</strong></td><td class="num"><strong>17.450/s</strong></td></tr>
        <tr><td>alcançar 100.000 eventos</td><td class="num">18,7&#8201;s</td><td class="num">5,7&#8201;s</td></tr>
        <tr><td>exclusão física até as três</td><td class="num">1.952&#8201;ms</td><td class="num">140&#8201;ms</td></tr>
      </tbody>
    </table>
  </div>

  <p><strong>4,08×</strong> por réplica. <strong>A soma das três NÃO vale</strong>
  (erro de aritmética corrigido em 17/09/2026: réplica completa recebe todos os
  eventos, então cada uma atrasa 34.048 − 17.450 = <strong>16.598/s</strong>).
  E um terceiro achado, menor, saiu no caminho:
  <code>bytes_para_hex</code> fazia um <code>format!</code> — e uma alocação de
  <code>String</code> — <strong>por byte</strong> da imagem. Tabela de dígitos no
  lugar: 3,48 → 0,24&#8201;µs por evento, <strong>14,5&#215;</strong>.</p>

  <h3>A inserção ainda é onde o MySQL(R) ganha — e 2,9&#215; já voltaram</h3>

@@bancada_diagnostico@@

@@trio@@

  <p>A bancada inteira está em <code>bancada/</code> — o medidor, os gráficos, o
  registro bruto da carga e o <code>resultados.json</code>. Qualquer número desta
  seção sai de lá; nenhum foi estimado.</p>
</section>"""

SECAO_3 = """<section id="c3">
  <div class="rotulo"><span class="num">03</span><span class="traco"></span></div>
  <h2>Os tetos da trava, e onde os testes estão ralos</h2>

  <p>Os dois painéis medidos que moravam na seção <em>Estado e roteiro</em> do
  dossiê, e o roteiro que ia com eles. O que ficou lá é o <strong>estado dos
  pedidos</strong>, que é o resumo de capa; o que veio para cá é a
  <strong>medição</strong>, que é o que pesa.</p>

<h3>A trava global contra o MVCC: os quatro tetos, medidos</h3>

  <p>Esta era a parcial mais antiga da lista, e em 04/09 ela deixou de ser
  &laquo;medida e sem plano&raquo; para ser <strong>medida com quatro
  n&uacute;meros</strong>. Nenhum deles se digita aqui: saem das corridas cruas
  do medidor, por <code>docs/dossie/tetos-da-trava.py</code> &mdash; e a raz&atilde;o
  de ser assim custou uma hora nesta mesma rodada, quando uma bancada mediu
  quatro vezes a mesma coisa por um campo que o servidor n&atilde;o l&ecirc;.</p>

  @@tetos@@

  <h3>Onde os testes estão, e onde estão ralos</h3>

  <p>O que segue é <code>#[test]</code> contado por arquivo e agrupado por área —
  medido, não lembrado. Não é o mesmo número que o <code>cargo test</code>
  reporta na capa: aquele inclui os <em>doc-tests</em> e não sabe dizer de que
  arquivo cada teste veio. As áreas em <strong>negrito</strong> estão abaixo de
  1,5% do total, que é onde a cobertura é rala.</p>

@@cobertura@@

  <h3>O roteiro, em três lugares e nenhum deles aqui</h3>

  <div class="rolo">
    <table>
      <thead><tr><th>Onde</th><th>O que guarda</th></tr></thead>
      <tbody>
        <tr><td><code>docs/PENDENCIAS.md</code></td><td>a lista dos pedidos, o detalhe de cada parcial, e o que falta de verdade com o lugar do buraco no código</td></tr>
        <tr><td><code>docs/SPRINTS.md</code></td><td>as propostas que quatro leituras de manuais de outros motores produziram — <strong>candidatas esperando um sim</strong>, cada uma com a premissa que pode matá-la antes da primeira linha</td></tr>
        <tr><td><code>docs/DESEMPENHO.md</code></td><td>onde a escrita dói, com o medidor que produz o número — e as hipóteses que <em>morreram</em> medidas, para a mesma ideia não voltar sem medição</td></tr>
      </tbody>
    </table>
  </div>

  <div class="nota">
    <span class="t">A lista do que falta também é palpite até alguém medir</span>
    <p>O pedido que dizia «ordene as chaves do lote antes do <code>.ndx</code>»
    vinha com o alvo certo — os 83,5% estavam mesmo lá. Só que o custo não era de
    <strong>localidade</strong>: era reler do arquivo e recalcular o CRC-32 da
    <em>mesma página</em> a cada descida da árvore. A desordem custava
    <strong>1,06×</strong>; ordenar teria comprado quase nada, e teria custado
    uma garantia. Um cache de páginas comprou <strong>2,40×</strong>.</p>
    <p><em>Medir a premissa do item vem antes de implementar o item</em> —
    inclusive quando o item é nosso.</p>
  </div>
</section>"""


def principal() -> int:
    saida = (pathlib.Path(sys.argv[1]).resolve()
             if len(sys.argv) > 1 and sys.argv[1].endswith(".html")
             else pagina_do_console(exigir=False))
    antigo = saida.read_text(encoding="utf-8") if saida.exists() else ""
    html, pendentes = montar(antigo)
    saida.write_text(html, encoding="utf-8")
    n = len(html.encode("utf-8"))
    print(f"pagina gravada: {saida.relative_to(RAIZ)} ({n:,} bytes)".replace(",", "."))

    ligados = ligar_os_ponteiros()
    if ligados == len(ANCORAS_NO_DOSSIE):
        print(f"  os {ligados} ponteiros do dossie apontam para "
              + (link("console-em-imagens.html") or "o nome do arquivo (sem URL ainda)"))
    else:
        # Ponteiro que sumiu do dossie e conteudo que ficou sem caminho de
        # volta. Isso e VERMELHO, nao um aviso: a lei do gerador que faz menos.
        print(f"FEZ MENOS: achei {ligados} de {len(ANCORAS_NO_DOSSIE)} ponteiros "
              f"no dossie ({', '.join(ANCORAS_NO_DOSSIE)}) -- quem sumiu deixou "
              "uma secao movida sem link de volta.")
        return 1
    if not pendentes:
        print("  todos os blocos vieram dos geradores donos deles")
        return 0
    # Gerador que faz menos do que o nome dele promete tem de DIZER que fez
    # menos -- e nao sob uma linha de exito.
    print(f"FEZ MENOS: {len(pendentes)} bloco(s) desta pagina ainda nao foram "
          "gerados, e aparecem como NAO GERADO nela:")
    for gerador, oque in pendentes:
        print(f"   · {oque} -> rode `python3 {gerador}`")
    return 1


if __name__ == "__main__":
    sys.exit(principal())
