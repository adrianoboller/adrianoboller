#!/usr/bin/env python3
"""Os graficos de desempenho das bancadas, desenhados dos `resultados.json`.

    python3 docs/dossie/graficos-dos-testes.py [saida.html]

Pedido do dono em 05/09/2026. Sai a QUARTA pagina do projeto, e ela obedece
as mesmas duas leis das outras tres: **todo numero visivel sai de um
gerador**, e **cada numero traz a data em que foi medido** -- porque as
bancadas correm em dias diferentes e um painel que as junta calado publica
um retrato que nunca existiu.

# Sem biblioteca de grafico

SVG escrito a mao, como o `bancada/graficos.py` que ja existe. Nao e teimosia:
e a mesma regra de zero dependencias que fez a compilacao cruzada funcionar de
primeira, e barra comparativa e' o desenho certo aqui -- sao poucas series e
poucas medidas, e um grafico mais bonito nao diria mais nada.

# As tres disciplinas do desenho, e as tres saem de erro ja pago

**A faixa aparece.** Cada barra traz o min e o max medidos, e nao so a
mediana. Barra sem faixa faz duas medidas que se cruzam parecerem vencedor e
perdedor -- e esta casa ja declarou vencedor dentro do ruido uma vez, na
pagina da comparacao dos tres motores.

**Vencedor so' se as faixas NAO se cruzam.** E' a regra que o pedido 155
deixou escrita depois daquele erro, e ela vale aqui.

**O que nao e' trabalho igual, o grafico DIZ.** Os tres bracos da bancada de
utilizacao padrao (`sem`, `com`, `largo`) gravam em arquivos diferentes; os
tres motores da comparacao recebem o trabalho por caminhos diferentes (so o
MySQL(R) o recebe como texto por soquete, e o piso disso ja foi 59,6% de uma
barra). Legenda que omite isso deixa a razao mentir sozinha.

# Fase que nao rodou

Aparece como **nao medida**, com o comando para rodar -- nunca como zero.
Zero num grafico e' uma barra que alguem le como resultado.
"""
import datetime
import json
import sys
from pathlib import Path

AQUI = Path(__file__).resolve().parent
RAIZ = AQUI.parent.parent
SAIDA_PADRAO = AQUI / "graficos.html"

# A paleta sai da marca; ver `phxsql/marca/LEIA-ME.md`. O vermelhao escurece
# no tema claro por contraste, e e' por isso que ele nao e' um literal solto.
CORES = ["var(--c1)", "var(--c2)", "var(--c3)"]


def ler(rel):
    p = RAIZ / rel
    if not p.exists():
        return None, p
    try:
        return json.loads(p.read_text(encoding="utf-8")), p
    except (json.JSONDecodeError, UnicodeDecodeError):
        return None, p


def quando(caminho, dados):
    for k in ("medido_em", "quando", "data"):
        if isinstance(dados, dict) and dados.get(k):
            return str(dados[k])[:19], False
    if caminho.exists():
        t = datetime.datetime.fromtimestamp(caminho.stat().st_mtime)
        return t.strftime("%Y-%m-%d %H:%M"), True
    return "—", False


def esc(s):
    return str(s).replace("&", "&amp;").replace("<", "&lt;").replace(">", "&gt;")


def num(v, casas=0):
    if v is None:
        return "—"
    s = f"{v:,.{casas}f}"
    return s.replace(",", " ").replace(".", ",") if casas else s.replace(",", ".")


def cruzam(a, b):
    """As duas faixas se sobrepoem? Se sim, nao ha vencedor a declarar."""
    if not a or not b:
        return True
    return not (a[1] < b[0] or b[1] < a[0])


def barras(titulo, sub, series, unidade, casas=0, menor_e_melhor=False,
           faixas=None, vencedor=True):
    """Um grupo de barras horizontais.

    `series`: [(rotulo, valor), ...]. `faixas`: {rotulo: (min, max)} ou None.
    O SVG e' proporcional: `viewBox` fixo e largura 100%, entao ele acompanha
    a coluna em vez de estourar no celular.
    """
    validos = [(r, v) for r, v in series if isinstance(v, (int, float))]
    if not validos:
        return (f'<figure class="g"><figcaption><b>{esc(titulo)}</b>'
                f'<span class="ausente">não medido</span></figcaption></figure>')
    # A escala tem de caber TUDO o que se desenha, e nao so as medianas: o
    # `max` de uma amostra pode ser muitas vezes a maior mediana (o MySQL(R)
    # da fase `buscar` tem mediana 2,48 s e max 17,1 s), e uma escala feita so
    # das medianas manda a faixa para 2.610 num `viewBox` de 640 -- a linha
    # sai do desenho e some, sem erro nenhum. Achado EXERCITANDO a pagina no
    # navegador; lendo o codigo nao aparece.
    tetos = [v for _, v in validos]
    if faixas:
        tetos += [hi for lo, hi in faixas.values() if isinstance(hi, (int, float))]
    maior = max(tetos) or 1
    alt_l, gap, topo = 30, 12, 10
    h = topo + len(series) * (alt_l + gap)
    larg, esq, margem = 640, 168, 110

    # Quem vence: o melhor valor cuja faixa nao cruza a de ninguem.
    campeao = None
    if vencedor and len(validos) > 1:
        ordenados = sorted(validos, key=lambda x: x[1], reverse=not menor_e_melhor)
        primeiro, segundo = ordenados[0], ordenados[1]
        f = faixas or {}
        if not cruzam(f.get(primeiro[0]), f.get(segundo[0])):
            campeao = primeiro[0]

    corpo = []
    for i, (rot, val) in enumerate(series):
        y = topo + i * (alt_l + gap)
        cor = CORES[i % len(CORES)]
        if not isinstance(val, (int, float)):
            corpo.append(
                f'<text x="{esq - 10}" y="{y + 20}" text-anchor="end" '
                f'class="rot">{esc(rot)}</text>'
                f'<text x="{esq + 6}" y="{y + 20}" class="vazio">não medido</text>')
            continue
        w = max(2, (val / maior) * (larg - esq - margem))
        marca = ' class="campeao"' if rot == campeao else ""
        # Onde o numero pode ser escrito: DEPOIS do que estiver mais a direita.
        # A barra e' a mediana e o traco vai ate o max, entao escrever em
        # `esq + w` poe o numero em cima da linha -- e um numero riscado no
        # meio de um trago le-se pior que numero nenhum. Visto na foto da
        # pagina, nao no codigo.
        fim = esq + w
        corpo.append(
            f'<text x="{esq - 10}" y="{y + 20}" text-anchor="end" class="rot">'
            f'{esc(rot)}</text>'
            f'<rect x="{esq}" y="{y}" width="{w:.1f}" height="{alt_l}" rx="3" '
            f'fill="{cor}"{marca}/>')
        if faixas and faixas.get(rot):
            lo, hi = faixas[rot]
            x1 = esq + (lo / maior) * (larg - esq - margem)
            x2 = esq + (hi / maior) * (larg - esq - margem)
            fim = max(fim, x2)
            ym = y + alt_l / 2
            corpo.append(
                f'<line x1="{x1:.1f}" y1="{ym}" x2="{x2:.1f}" y2="{ym}" '
                f'class="faixa"/>'
                f'<line x1="{x1:.1f}" y1="{y + 6}" x2="{x1:.1f}" y2="{y + alt_l - 6}" '
                f'class="faixa"/>'
                f'<line x1="{x2:.1f}" y1="{y + 6}" x2="{x2:.1f}" y2="{y + alt_l - 6}" '
                f'class="faixa"/>')
        corpo.append(
            f'<text x="{fim + 9:.1f}" y="{y + 20}" class="val">'
            f'{num(val, casas)}</text>')

    nota = ('<span class="dica">a barra é a mediana; o traço é min–max. '
            'Contorno = vencedor, e ele só aparece quando as faixas não se '
            'cruzam.</span>') if faixas else ""
    return (
        f'<figure class="g">'
        f'<figcaption><b>{esc(titulo)}</b> <span class="un">{esc(unidade)}</span>'
        f'<div class="sub">{sub}</div>{nota}</figcaption>'
        f'<svg viewBox="0 0 {larg} {h}" role="img" '
        f'aria-label="{esc(titulo)} em {esc(unidade)}">{"".join(corpo)}</svg>'
        f'</figure>')


# --------------------------------------------------------------- os graficos

def g_tres_motores():
    d, p = ler("bancada/comparacao/um-milhao.json")
    if not d:
        return ['<div class="ausente-bloco">Os três motores a um milhão — '
                '<b>não medido</b>. Rode <code>python3 bancada/comparacao/medir.py'
                '</code>.</div>'], ("—", False)
    q = quando(p, d)
    nomes = {"phxsql": "PhxSql", "mysql": "MySQL®", "sqlite": "SQLite®"}
    out = []
    for fase, dados in d.get("fases", {}).items():
        series, faixas = [], {}
        for motor, v in dados.items():
            rot = nomes.get(motor, motor)
            series.append((rot, v.get("mediana_s")))
            if v.get("min_s") is not None:
                faixas[rot] = (v["min_s"], v["max_s"])
        n = d.get("linhas") if fase == "inserir" else d.get(
            "operacoes_por_fase_pontual")
        out.append(barras(
            f"{fase.capitalize()} — {num(n)} linhas",
            "Os três na <b>mesma rodada</b>, intercalados: medidas de dias "
            "diferentes carregam o ambiente junto. E o MySQL® é o único que "
            "recebe o trabalho como <b>texto por soquete</b> — o piso disso já "
            "foi 59,6% de uma barra desta bancada.",
            series, "segundos (menor é melhor)", 2, menor_e_melhor=True,
            faixas=faixas))
    return out, q


def g_utilizacao():
    d, p = ler("bancada/utilizacao-padrao/resultado.json")
    if not d:
        return ['<div class="ausente-bloco">Utilização padrão — <b>não medido</b>. '
                'Rode <code>python3 bancada/utilizacao-padrao/medir.py</code>.'
                '</div>'], ("—", False)
    q = quando(p, d)
    rot = {"sem": "sem blob", "com": "com Bin/Memo", "largo": "largo (Str fixo)"}
    lados = d.get("lados", {})
    sub = ("Os três braços <b>não são o mesmo trabalho</b>: <code>com</code> "
           "grava em arquivos que <code>sem</code> nem abre, e <code>largo</code> "
           "existe justamente para separar o peso do <b>pedido</b> do peso dos "
           "<b>arquivos externos</b> — ele declara as mesmas colunas como "
           "<code>Str(n)</code>, e o pedido no fio sai byte a byte igual ao de "
           "<code>com</code>.")
    fio = [(rot.get(k, k), v.get("fio_por_linha")) for k, v in lados.items()]
    disco = [(rot.get(k, k), v.get("disco_por_linha") or
              (v.get("disco", {}) or {}).get("por_linha")) for k, v in lados.items()]
    out = [barras("Bytes no fio, por linha", sub, fio, "bytes/linha", 1,
                  menor_e_melhor=True, vencedor=False)]
    if any(isinstance(v, (int, float)) for _, v in disco):
        out.append(barras("Bytes em disco, por linha", sub, disco, "bytes/linha",
                          1, menor_e_melhor=True, vencedor=False))
    return out, q


def g_carga():
    d, p = ler("bancada/carga/resultados.json")
    if not d:
        return ['<div class="ausente-bloco">Carga pela rede — <b>não medido</b>. '
                'Rode <code>python3 bancada/carga/medir.py 20000</code>.</div>'], ("—", False)
    q = quando(p, d)
    series = [("uma a uma", d.get("uma_a_uma_por_s")),
              (f"em lote de {num(d.get('por_lote'))}", d.get("lote_por_s") or
               d.get("em_lote_por_s"))]
    return [barras(
        f"Carga de {num(d.get('linhas'))} linhas pela rede",
        "A diferença é <b>viagem de rede</b>, não gravação: os dois lados "
        "gravam as mesmas linhas nos mesmos índices, no mesmo servidor.",
        series, "linhas por segundo (maior é melhor)", 0, vencedor=False)], q


def g_replicacao():
    d, p = ler("bancada/replicacao/resultados.json")
    if not d:
        return ['<div class="ausente-bloco">Replicação — <b>não medido</b>. Rode '
                '<code>python3 bancada/replicacao/montar.py</code> e '
                '<code>medir.py</code>.</div>'], ("—", False)
    q = quando(p, d)
    out = [barras(
        "Master e réplica",
        "Duas medidas de coisas diferentes no mesmo par: o master <b>grava "
        "linha</b>, a réplica <b>aplica evento</b>. Não é uma razão — é o que "
        "cada lado sustenta.",
        [("master grava", d.get("master_linhas_s")),
         ("réplica aplica", d.get("replica_eventos_s"))],
        "por segundo (maior é melhor)", 0, vencedor=False)]
    atraso = d.get("atraso_ms")
    if isinstance(atraso, dict):
        out.append(barras(
            "Atraso até a réplica ter a linha",
            "Medido por operação, com quatro servidores no ar.",
            [(k, v) for k, v in atraso.items()], "milissegundos (menor é melhor)",
            0, menor_e_melhor=True, vencedor=False))
    return out, q


def g_fts():
    """O indice de texto contra a varredura.

    Este e o unico bloco em que a faixa vem PRONTA do `resultados.json`: a
    bancada do `.fts` grava `{min, mediana, max}` de cada numero, porque uma
    corrida nao e medicao. Os outros blocos derivam a faixa das rodadas.
    """
    d, p = ler("bancada/fts/resultados.json")
    if not d:
        return ['<div class="ausente-bloco">Índice de texto — <b>não medido</b>. '
                'Rode <code>python3 bancada/fts/medir.py 1000000 20</code>.</div>'], ("—", False)
    q = quando(p, d)
    fx = lambda c: (d[c]["min"], d[c]["max"]) if isinstance(d.get(c), dict) else None
    med = lambda c: d[c]["mediana"] if isinstance(d.get(c), dict) else d.get(c)
    linhas = num(d.get("linhas"))
    out = [barras(
        f"Procurar uma palavra em {linhas} linhas",
        "As duas faixas respondem a <b>mesma pergunta</b> e devolvem o "
        "<b>mesmo conjunto</b> de rowids — o medidor aborta se não baterem. "
        "Metade das palavras procuradas não existe, de propósito: palavra "
        "inexistente é o caso em que o índice ganha mais.",
        [("varredura", med("us_varredura")), ("índice .fts", med("us_indice"))],
        "microssegundos por busca (menor é melhor)", 0, menor_e_melhor=True,
        faixas={"varredura": fx("us_varredura"), "índice .fts": fx("us_indice")})]
    out.append(barras(
        "O que a busca custa na GRAVAÇÃO",
        "O ganho de cima é pago aqui, e publicar só um dos dois contaria a "
        "metade que nos favorece. São "
        f"<b>{num(d.get('escrita_chaves_por_linha'), 1)} chaves por linha</b>, "
        "contadas pelo medidor.",
        [("sem índice de texto", med("escrita_sem_indice_us")),
         ("com índice de texto", med("escrita_com_indice_us"))],
        "microssegundos por linha inserida (menor é melhor)", 1,
        menor_e_melhor=True,
        faixas={"sem índice de texto": fx("escrita_sem_indice_us"),
                "com índice de texto": fx("escrita_com_indice_us")}))
    return out, q


BLOCOS = [
    ("Os três motores, a um milhão de linhas", g_tres_motores,
     "bancada/comparacao/"),
    ("Índice de texto — o .fts contra a varredura", g_fts, "bancada/fts/"),
    ("Utilização padrão — 20.000 linhas em tabela complexa", g_utilizacao,
     "bancada/utilizacao-padrao/"),
    ("Carga pela rede — uma a uma contra o lote", g_carga, "bancada/carga/"),
    ("Replicação — quatro servidores", g_replicacao, "bancada/replicacao/"),
]


def montar():
    secoes = []
    for titulo, fn, pasta in BLOCOS:
        graficos, (q, mtime) = fn()
        marca = ' <span class="mtime">(mtime)</span>' if mtime else ""
        secoes.append(
            f'<h2>{esc(titulo)}</h2>'
            f'<p class="sub">Medido em <span class="mono">{esc(q)}</span>{marca} '
            f'· <code>{esc(pasta)}</code></p>'
            f'{"".join(graficos)}')
    return TEMPLATE.format(
        agora=datetime.datetime.now().strftime("%d/%m/%Y %H:%M"),
        secoes="\n".join(secoes))


TEMPLATE = """<title>Gráficos de desempenho do PhxSql</title>
<link rel="preconnect" href="https://fonts.googleapis.com">
<link rel="preconnect" href="https://fonts.gstatic.com" crossorigin>
<link rel="stylesheet" href="https://fonts.googleapis.com/css2?family=Exo+2:wght@400;500;600;700&family=Source+Serif+4:opsz,wght@8..60,400;8..60,600&family=IBM+Plex+Mono:wght@400;500&display=swap">
<style>
:root{{
  --papel:#fbf9f7; --papel-2:#f3efec; --tinta:#1a1210; --tinta-2:#4a3f3a;
  --tinta-3:#7a6d66; --linha:#ded6d0; --acento:#c63c0a; --falta:#8a6a1f;
  --c1:#c63c0a; --c2:#4a6fa5; --c3:#5c7a52;
}}
@media (prefers-color-scheme:dark){{
  :root:not([data-theme="light"]){{
    --papel:#040814; --papel-2:#0a1122; --tinta:#dde2eb; --tinta-2:#a8b0c0;
    --tinta-3:#7c8598; --linha:#1e2940; --acento:#ff8a1c; --falta:#d5a83c;
    --c1:#ff8a1c; --c2:#6f9fe0; --c3:#7fb36e;
  }}
}}
:root[data-theme="dark"]{{
  --papel:#040814; --papel-2:#0a1122; --tinta:#dde2eb; --tinta-2:#a8b0c0;
  --tinta-3:#7c8598; --linha:#1e2940; --acento:#ff8a1c; --falta:#d5a83c;
  --c1:#ff8a1c; --c2:#6f9fe0; --c3:#7fb36e;
}}
*{{box-sizing:border-box}}
body{{margin:0;background:var(--papel);color:var(--tinta);
  font-family:"Source Serif 4",Georgia,serif;font-size:16px;line-height:1.55;
  -webkit-font-smoothing:antialiased}}
h1,h2,.rotulo{{font-family:"Exo 2","Helvetica Neue",Arial,sans-serif}}
code,.mono{{font-family:"IBM Plex Mono",ui-monospace,Menlo,monospace}}
code{{font-size:.86em;background:var(--papel-2);padding:1px 4px;border-radius:3px;
  color:var(--tinta-2)}}
.envelope{{max-width:900px;margin:0 auto;padding:0 20px 80px}}
header{{padding:52px 0 26px;border-bottom:1px solid var(--linha)}}
.rotulo{{font-size:10.5px;letter-spacing:.18em;text-transform:uppercase;
  color:var(--acento);font-weight:600;margin-bottom:12px}}
h1{{font-size:clamp(28px,5vw,42px);font-weight:700;line-height:1.08;margin:0 0 14px;
  letter-spacing:-.015em;text-wrap:balance}}
.chamada{{max-width:64ch;color:var(--tinta-2);font-size:17px;margin:0}}
h2{{font-size:20px;font-weight:600;margin:46px 0 4px;letter-spacing:-.01em}}
h2 + .sub{{color:var(--tinta-3);font-size:13px;margin:0 0 18px}}
figure.g{{margin:0 0 30px;padding:16px 18px;border:1px solid var(--linha);
  border-radius:7px;background:var(--papel-2)}}
figcaption{{margin-bottom:12px}}
figcaption b{{font-family:"Exo 2",sans-serif;font-size:15px}}
.un{{color:var(--tinta-3);font-size:12.5px;margin-left:6px}}
figcaption .sub{{color:var(--tinta-2);font-size:13.5px;margin-top:5px;
  max-width:70ch;line-height:1.45}}
.dica{{display:block;color:var(--tinta-3);font-size:12px;margin-top:6px;
  font-style:italic}}
svg{{width:100%;height:auto;display:block}}
svg text{{font-family:"IBM Plex Mono",monospace;font-size:12px;fill:var(--tinta-2)}}
svg text.rot{{fill:var(--tinta)}}
svg text.val{{fill:var(--tinta);font-weight:500}}
svg text.vazio{{fill:var(--falta);font-style:italic}}
svg rect{{opacity:.82}}
svg rect.campeao{{opacity:1;stroke:var(--tinta);stroke-width:1.5}}
svg line.faixa{{stroke:var(--tinta);stroke-width:1.4;opacity:.55}}
.ausente-bloco{{border-left:3px solid var(--falta);background:var(--papel-2);
  padding:13px 17px;border-radius:0 5px 5px 0;margin:0 0 22px;
  color:var(--falta);font-size:14.5px}}
.mtime{{color:var(--falta);font-size:11px}}
.nota{{border-left:3px solid var(--acento);background:var(--papel-2);
  padding:14px 18px;border-radius:0 5px 5px 0;margin:26px 0;font-size:15px;
  color:var(--tinta-2);max-width:68ch}}
.nota b{{color:var(--tinta)}}
footer{{margin-top:52px;padding-top:20px;border-top:1px solid var(--linha);
  color:var(--tinta-3);font-size:13.5px;max-width:68ch}}
@media (prefers-reduced-motion:reduce){{*{{transition:none!important}}}}
</style>
<div class="envelope">
<header>
  <div class="rotulo">PhxSql · desempenho medido</div>
  <h1>Os gráficos das bancadas</h1>
  <p class="chamada">Cada barra sai de um <code>resultados.json</code> gravado
  por uma bancada que rodou, e traz a data em que rodou. Onde há três medições,
  o traço mostra o <b>min–max</b> — e o vencedor só é contornado quando as
  faixas <b>não se cruzam</b>.</p>
</header>

<div class="nota">
  <b>Por que a faixa aparece, e por que ela decide o vencedor.</b> Esta casa já
  declarou vencedor dentro do ruído uma vez, no gráfico da comparação dos três
  motores — a mediana dizia uma coisa e as faixas se cruzavam. Desde então a
  regra é esta, e o desenho a obedece: <b>barra sem faixa não declara nada</b>.
</div>

{secoes}

<div class="nota">
  <b>O que estes gráficos não comparam.</b> Barras lado a lado convidam a ler
  razão onde às vezes só há duas medidas diferentes. Onde os lados não fazem o
  mesmo trabalho — os três braços da utilização padrão, os três motores da
  comparação — a legenda de cada figura diz o que muda, em vez de deixar a
  razão falar sozinha. É a regra do <code>bancada/LEIA-ME.md</code>:
  <b>bancada compara trabalho igual, não só pergunta igual</b>, e os dois erros
  já cometidos aqui saíram do mesmo lugar e apontaram para lados opostos.
</div>

<footer>
  Gerado por <code>docs/dossie/graficos-dos-testes.py</code> em {agora}, sem
  biblioteca de gráfico — SVG escrito à mão, como o resto do projeto. Nenhum
  número foi digitado: todos saem dos <code>resultados.json</code> das
  bancadas. Bancada que não rodou aparece como <b>não medida</b>, nunca como
  zero — zero num gráfico é uma barra que alguém lê como resultado.
</footer>
</div>
"""


def principal():
    saida = Path(sys.argv[1]) if len(sys.argv) > 1 else SAIDA_PADRAO
    html = montar()
    saida.write_text(html, encoding="utf-8")
    print(f"pagina gravada: {saida} ({len(html.encode()):,} bytes)".replace(",", "."))
    for titulo, fn, pasta in BLOCOS:
        _, (q, _) = fn()
        print(f"   · {titulo}: {q}")


if __name__ == "__main__":
    principal()
