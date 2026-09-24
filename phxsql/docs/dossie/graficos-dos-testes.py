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
CORES = ["var(--c1)", "var(--c2)", "var(--c3)", "var(--c4)"]


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
           faixas=None, vencedor=True, pares=False, grupo=0):
    """Um grupo de barras horizontais.

    `series`: [(rotulo, valor), ...]. `faixas`: {rotulo: (min, max)} ou None.
    `grupo`: generaliza `pares` para N barras que se comparam entre si (os
    quatro lados de um nivel: PhxZip e 7-Zip, com um fio e com varios).
    `pares`: a serie vem em DUPLAS que se comparam entre si (nivel 1 do
    PhxZip contra nivel 1 do 7-Zip, ...): a cor e a do lado da dupla, e o
    vencedor se decide DENTRO de cada dupla -- declarar um campeao so entre
    seis barras compararia o nivel 1 de um com o nivel 9 do outro.
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
    # A calha dos rotulos sai do rotulo MAIS LONGO, nao de um numero fixo:
    # com 168 fixos, «1.000 insercoes em lote» e «1 linha com memo de 200 KB»
    # saiam cortados a esquerda («.000 insercoes», «nha com memo») -- o
    # `text-anchor="end"` escreve para tras e o que passa do 0 some, sem
    # erro. Plex Mono a 12px avanca ~7,2 por caractere; 7,3 + 10 de folga.
    larg, margem = 640, 110
    esq = max(168, 10 + round(7.3 * max(len(r) for r, _ in series)))

    # Quem vence: o melhor valor cuja faixa nao cruza a de ninguem.
    def vence(grupo):
        grupo = [(r, v) for r, v in grupo if isinstance(v, (int, float))]
        if len(grupo) < 2:
            return None
        ordenados = sorted(grupo, key=lambda x: x[1], reverse=not menor_e_melhor)
        primeiro, segundo = ordenados[0], ordenados[1]
        if primeiro[1] == segundo[1]:
            return None
        f = faixas or {}
        if not cruzam(f.get(primeiro[0]), f.get(segundo[0])):
            return primeiro[0]
        return None

    g = grupo or (2 if pares else 0)
    campeoes = set()
    if vencedor and g:
        for k in range(0, len(series) - 1, g):
            c = vence(series[k:k + g])
            if c:
                campeoes.add(c)
    elif vencedor:
        c = vence(validos)
        if c:
            campeoes.add(c)
    # Com grupo de 4 a ordem das cores e PhxZip, PhxZip varios fios, 7-Zip,
    # 7-Zip varios fios: laranja e verde do nosso lado, azul e petroleo do dele.
    cor_de = ((lambda i: CORES[[0, 2, 1, 3][i % 4]]) if g == 4 else
              (lambda i: CORES[i % 2]) if g == 2 else
              (lambda i: CORES[i % len(CORES)]))

    corpo = []
    for i, (rot, val) in enumerate(series):
        y = topo + i * (alt_l + gap)
        cor = cor_de(i)
        if not isinstance(val, (int, float)):
            corpo.append(
                f'<text x="{esq - 10}" y="{y + 20}" text-anchor="end" '
                f'class="rot">{esc(rot)}</text>'
                f'<text x="{esq + 6}" y="{y + 20}" class="vazio">não medido</text>')
            continue
        w = max(2, (val / maior) * (larg - esq - margem))
        marca = ' class="campeao"' if rot in campeoes else ""
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

    # A variante estreita: mesma escala (`maior`), mesmo campeao, mesmas
    # faixas -- so muda ONDE o rotulo fica. Nada de calha a esquerda, entao a
    # barra ganha a largura toda menos a margem do numero.
    e_larg, e_margem, e_alt, e_rot, e_gap, e_topo = 360, 76, 22, 16, 14, 4
    e_h = e_topo + len(series) * (e_rot + e_alt + e_gap)
    estreito = []
    for i, (rot, val) in enumerate(series):
        y = e_topo + i * (e_rot + e_alt + e_gap)
        yb = y + e_rot
        cor = cor_de(i)
        estreito.append(f'<text x="0" y="{y + 11}" class="rot">{esc(rot)}</text>')
        if not isinstance(val, (int, float)):
            estreito.append(f'<text x="0" y="{yb + 15}" class="vazio">não medido</text>')
            continue
        w = max(2, (val / maior) * (e_larg - e_margem))
        marca = ' class="campeao"' if rot in campeoes else ""
        fim = w
        estreito.append(
            f'<rect x="0" y="{yb}" width="{w:.1f}" height="{e_alt}" rx="3" '
            f'fill="{cor}"{marca}/>')
        if faixas and faixas.get(rot):
            lo, hi = faixas[rot]
            x1 = (lo / maior) * (e_larg - e_margem)
            x2 = (hi / maior) * (e_larg - e_margem)
            fim = max(fim, x2)
            ym = yb + e_alt / 2
            estreito.append(
                f'<line x1="{x1:.1f}" y1="{ym}" x2="{x2:.1f}" y2="{ym}" '
                f'class="faixa"/>'
                f'<line x1="{x1:.1f}" y1="{yb + 4}" x2="{x1:.1f}" y2="{yb + e_alt - 4}" '
                f'class="faixa"/>'
                f'<line x1="{x2:.1f}" y1="{yb + 4}" x2="{x2:.1f}" y2="{yb + e_alt - 4}" '
                f'class="faixa"/>')
        estreito.append(
            f'<text x="{fim + 8:.1f}" y="{yb + 15}" class="val">'
            f'{num(val, casas)}</text>')

    nota = ('<span class="dica">a barra é a mediana; o traço é min–max. '
            'Contorno = vencedor, e ele só aparece quando as faixas não se '
            'cruzam.</span>') if faixas else ""
    return (
        f'<figure class="g">'
        f'<figcaption><b>{esc(titulo)}</b> <span class="un">{esc(unidade)}</span>'
        f'<div class="sub">{sub}</div>{nota}</figcaption>'
        # Duas figuras, uma visivel por vez (CSS, abaixo de 700px). A LARGA e a
        # de sempre; a ESTREITA poe o rotulo EM CIMA da barra num viewBox de
        # 360. Sem ela, a 400px o viewBox de 640 escalava a ~330 e o texto de
        # 12px virava ~6px; num rolo com piso, os numeros ficavam atras da
        # rolagem -- grafico sem numero le pior que tabela sem coluna. As duas
        # foram vistas na captura; nenhuma aparece lendo o codigo.
        f'<svg class="largo" viewBox="0 0 {larg} {h}" role="img" '
        f'aria-label="{esc(titulo)} em {esc(unidade)}">{"".join(corpo)}</svg>'
        f'<svg class="estreito" viewBox="0 0 {e_larg} {e_h}" role="img" '
        f'aria-label="{esc(titulo)} em {esc(unidade)}">{"".join(estreito)}</svg>'
        f'</figure>')


# --------------------------------------------------------------- os graficos

def g_tres_motores():
    d, p = ler("bancada/comparacao/um-milhao.json")
    if not d:
        return ['<div class="ausente-bloco">Os quatro motores a um milhão — '
                '<b>não medido</b>. Rode <code>python3 bancada/comparacao/medir.py'
                '</code>.</div>'], ("—", False)
    q = quando(p, d)
    nomes = {"phxsql": "PhxSql", "mysql": "MySQL®", "mariadb": "MariaDB®",
             "sqlite": "SQLite®"}
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
            "Os quatro na <b>mesma rodada</b>, intercalados: medidas de dias "
            "diferentes carregam o ambiente junto. E a família MySQL recebe o "
            "trabalho como <b>texto</b> — o MySQL® por soquete, o MariaDB® por "
            "TCP, cujo piso é maior; os dois pisos são medidos à parte.",
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


def g_registro():
    """O Regedit (Registro do Windows) contra os bancos, em us/op.

    Duas fontes, dois metais, duas datas -- e a legenda diz isso alto. O
    Regedit foi medido no Windows do dono (bancada/registro, 11/09); os
    bancos no Linux da corrida do milhao (08/09). Nenhum vencedor e'
    contornado aqui DE PROPOSITO: cross-hardware nao decide campeao, e a
    escrita ainda por cima nao e' trabalho igual (lote contra por-chave).
    Os us/op saem derivados dos `resultados.json` -- nao ha numero digitado.
    """
    reg, _ = ler("bancada/registro/resultados.json")
    dbs, _ = ler("bancada/comparacao/um-milhao.json")
    if not reg or not dbs:
        falta = []
        if not reg:
            falta.append("o Registro (<code>bench-registro.exe</code> no "
                         "Windows → <code>bancada/registro/resultados.json</code>)")
        if not dbs:
            falta.append("os bancos (<code>python3 bancada/comparacao/medir.py</code>)")
        return (['<div class="ausente-bloco">Regedit contra os bancos — '
                 '<b>não medido</b>: falta ' + " e ".join(falta) + '.</div>'],
                ("—", False))

    nomes = {"phxsql": "PhxSql", "mysql": "MySQL®", "mariadb": "MariaDB®",
             "sqlite": "SQLite®"}
    corridas = reg.get("corridas_representativas", [])
    fases = dbs.get("fases", {})
    ops = dbs.get("operacoes_por_fase_pontual") or 20000
    linhas = dbs.get("linhas") or 1000000

    def db_us(fase, por):
        """us/op de uma fase dos bancos: mediana e faixa (segundos → µs)."""
        serie, faixa = [], {}
        for motor, v in fases.get(fase, {}).items():
            rot = nomes.get(motor, motor)
            med = v.get("mediana_s")
            serie.append((rot, med / por * 1e6
                          if isinstance(med, (int, float)) else None))
            lo, hi = v.get("min_s"), v.get("max_s")
            if isinstance(lo, (int, float)) and isinstance(hi, (int, float)):
                faixa[rot] = (lo / por * 1e6, hi / por * 1e6)
        serie.sort(key=lambda x: (x[1] is None, x[1] or 0))
        return serie, faixa

    out = []

    # --- LEITURA: a colmeia quente do Registro contra o `buscar` dos bancos ---
    leituras = [c["leitura_us_op"] for c in corridas
                if isinstance(c.get("leitura_us_op"), (int, float))]
    serie, faixa = db_us("buscar", ops)
    if leituras:
        serie = [("Regedit (Windows)", sum(leituras) / len(leituras))] + serie
        faixa["Regedit (Windows)"] = (min(leituras), max(leituras))
        serie.sort(key=lambda x: (x[1] is None, x[1] or 0))
    out.append(barras(
        "Leitura por operação",
        "O Regedit lê de uma colmeia quente; os bancos são o <code>buscar</code> "
        f"da corrida do milhão ({num(ops)} operações, segundos ÷ operações). "
        "<b>Metais diferentes</b>: Regedit no Windows do dono (11/09), bancos no "
        "Linux da bancada (08/09) — por isso nenhuma barra é contornada como "
        "vencedora, mesmo quando as faixas não se cruzam.",
        serie, "microssegundos por leitura (menor é melhor)", 1,
        menor_e_melhor=True, faixas=faixa, vencedor=False))

    # --- ESCRITA: a metade que engana, e a legenda diz por quê ---
    escritas = []
    for c in corridas:
        for k in ("escrita_preguicosa_us_op", "escrita_duravel_us_op"):
            if isinstance(c.get(k), (int, float)):
                escritas.append(c[k])
    serie2, faixa2 = db_us("inserir", linhas)
    if escritas:
        serie2 = serie2 + [("Regedit (Windows)", sum(escritas) / len(escritas))]
        faixa2["Regedit (Windows)"] = (min(escritas), max(escritas))
    carga, _ = ler("bancada/carga/resultados.json")
    ref = ""
    if carga and isinstance(carga.get("uma_a_uma_por_s"), (int, float)):
        us1 = 1e6 / carga["uma_a_uma_por_s"]
        ref = (f" Para referência: o próprio PhxSql gravando <b>uma a uma</b> pela "
               f"rede faz ~{num(us1, 0)} µs/op (<code>bancada/carga</code>, "
               f"{str(carga.get('quando'))[:10]}) — a mesma ordem do Registro. "
               "É o LOTE que derruba o insert dos bancos para a casa de µs.")
    out.append(barras(
        "Escrita por operação",
        "Aqui os lados <b>não fazem o mesmo trabalho</b>, e é a metade que engana: "
        f"o insert dos bancos é em <b>lote</b> (uma sincronização por fase, "
        f"amortizada sobre {num(linhas)} linhas), enquanto a escrita do Registro "
        "é <b>por chave</b> — preguiçosa e durável, 20k×64B e 50k×128B. Comparar "
        "os dois FAVORECE os bancos." + ref,
        serie2, "microssegundos por escrita (menor é melhor)", 1,
        menor_e_melhor=True, faixas=faixa2, vencedor=False))

    q = (f"Regedit {str(reg.get('medido_em'))[:10]} (Windows) · bancos "
         f"{str(dbs.get('medido_em'))[:10]} (Linux)")
    return out, (q, False)


def g_phxzip():
    """PhxZip contra o 7-Zip: tamanho e tempo, nivel a nivel.

    Tudo sai do `bancada/phxzip/comparar-7z.json`. Trabalho igual: o 7-Zip
    num fio e sem o filtro BCJ (`-mf=off -mmt=1`), os dois descompactando
    para pasta; e o arquivo do PhxZip so conta depois de o 7-Zip abri-lo com
    o conteudo igual (o medidor recusa se nao abrir).
    """
    d, p = ler("bancada/phxzip/comparar-7z.json")
    if not d:
        return ['<div class="ausente-bloco">PhxZip × 7-Zip — <b>não medido</b>. '
                'Rode <code>python3 bancada/phxzip/comparar-7z.py</code>.</div>'], ("—", False)
    q = quando(p, d)
    out = []
    nomes = {"texto": "Texto", "misto": "Texto + imagem + executável",
             "grande": "Todos os documentos + os binários do 7-Zip"}
    for corpo, c in d.get("corpos", {}).items():
        niveis = c.get("niveis", {})
        arquivos = ", ".join(a["nome"] for a in c.get("arquivos", []))

        mt = d.get("fios_mt")
        lados = [("phxzip", "PhxZip"), ("phxzip_mt", f"PhxZip {mt} fios"),
                 ("7zip", "7-Zip"), ("7zip_mt", f"7-Zip {mt} fios")]
        lados = [l for l in lados if any(l[0] in m for m in niveis.values())]
        g = len(lados)

        def serie(chave):
            s, f = [], {}
            for n, m in niveis.items():
                for quem, rot in lados:
                    r = f"nível {n} · {rot}"
                    v = m.get(quem, {}).get(chave)
                    if isinstance(v, dict):
                        s.append((r, v.get("mediana")))
                        f[r] = (v.get("min"), v.get("max"))
                    else:
                        s.append((r, v))
            return s, f

        def razoes(chave, sub=None, a_de="phxzip", b_de="7zip"):
            partes = []
            for n, m in niveis.items():
                a, b = m.get(a_de, {}).get(chave), m.get(b_de, {}).get(chave)
                if sub:
                    a, b = (a or {}).get(sub), (b or {}).get(sub)
                if isinstance(a, (int, float)) and isinstance(b, (int, float)) and b:
                    partes.append(f"nível {n}: <b>{num(a / b, 3)}×</b>")
            return " · ".join(partes)

        titulo = f"{nomes.get(corpo, corpo)} — {num(c.get('bytes'))} bytes"
        s, _ = serie("bytes")
        out.append(barras(
            f"{titulo}: tamanho do .7z", f"{esc(arquivos)}. PhxZip ÷ 7-Zip, um "
            f"fio: {razoes('bytes')} (abaixo de 1 = o PhxZip gera menor)."
            + (f" O corte em blocos dos fios custa: {razoes('bytes', None, 'phxzip_mt', 'phxzip')}."
               if g == 4 else ""),
            s, "bytes (menor é melhor)", 0, menor_e_melhor=True, grupo=g))
        s, f = serie("compactar_s")
        out.append(barras(
            f"{titulo}: tempo para compactar", f"PhxZip ÷ 7-Zip, um fio: "
            f"{razoes('compactar_s', 'mediana')} (acima de 1 = o PhxZip é mais lento)."
            + (f" Com {mt} fios dos dois lados: "
               f"{razoes('compactar_s', 'mediana', 'phxzip_mt', '7zip_mt')}."
               f" O que os fios deram ao PhxZip (vários ÷ um): "
               f"{razoes('compactar_s', 'mediana', 'phxzip_mt', 'phxzip')}."
               if g == 4 else ""),
            s, "segundos (menor é melhor)", 3, menor_e_melhor=True, faixas=f,
            grupo=g))
        s, f = serie("descompactar_s")
        out.append(barras(
            f"{titulo}: tempo para descompactar", f"PhxZip ÷ 7-Zip: "
            f"{razoes('descompactar_s', 'mediana')}. A descompactação do PhxZip "
            f"é num fio só nos dois lados dele.",
            s, "segundos (menor é melhor)", 3, menor_e_melhor=True, faixas=f,
            grupo=g))
    out.append(f'<p class="sub">{esc(d.get("sete_zip", ""))} · '
               f'<code>{esc(d.get("comando_7z", ""))}</code> contra '
               f'<code>{esc(d.get("comando_phxzip", ""))}</code> · '
               f'{esc(d.get("maquina", ""))} · {d.get("corridas")} corridas por medida.</p>')
    return out, q


BLOCOS = [
    ("Os quatro motores, a um milhão de linhas", g_tres_motores,
     "bancada/comparacao/"),
    ("Regedit (Registro do Windows) contra os bancos, em µs/op", g_registro,
     "bancada/registro/ + bancada/comparacao/"),
    ("Índice de texto — o .fts contra a varredura", g_fts, "bancada/fts/"),
    ("Utilização padrão — 20.000 linhas em tabela complexa", g_utilizacao,
     "bancada/utilizacao-padrao/"),
    ("Carga pela rede — uma a uma contra o lote", g_carga, "bancada/carga/"),
    ("Replicação — quatro servidores", g_replicacao, "bancada/replicacao/"),
    ("PhxZip × 7-Zip — tamanho e velocidade", g_phxzip, "bancada/phxzip/"),
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


TEMPLATE = """<meta charset="utf-8">
<title>Gráficos de desempenho do PhxSql</title>
<link rel="preconnect" href="https://fonts.googleapis.com">
<link rel="preconnect" href="https://fonts.gstatic.com" crossorigin>
<link rel="stylesheet" href="https://fonts.googleapis.com/css2?family=Exo+2:wght@400;500;600;700&family=Source+Serif+4:opsz,wght@8..60,400;8..60,600&family=IBM+Plex+Mono:wght@400;500&display=swap">
<style>
:root{{
  --papel:#fbf9f7; --papel-2:#f3efec; --tinta:#1a1210; --tinta-2:#4a3f3a;
  --tinta-3:#6b5e57; --linha:#ded6d0; --acento:#c63c0a; --falta:#8a6a1f;
  --c1:#c63c0a; --c2:#4a6fa5; --c3:#5c7a52; --c4:#0e7a85;
}}
/* O fundo escuro e o #010418 da marca (marca/LEIA-ME.md, DESIGN.md §1.1). Era
   #040814 -- um valor que nao esta em documento nenhum, e a marca manda
   sobre paleta inventada. Medido: --tinta sobre ele da 15,65:1. */
@media (prefers-color-scheme:dark){{
  :root:not([data-theme="light"]){{
    --papel:#010418; --papel-2:#0a1122; --tinta:#dde2eb; --tinta-2:#a8b0c0;
    --tinta-3:#7c8598; --linha:#1e2940; --acento:#ff8a1c; --falta:#d5a83c;
    --c1:#ff8a1c; --c2:#6f9fe0; --c3:#7fb36e; --c4:#3fc8d4;
  }}
}}
:root[data-theme="dark"]{{
  --papel:#010418; --papel-2:#0a1122; --tinta:#dde2eb; --tinta-2:#a8b0c0;
  --tinta-3:#7c8598; --linha:#1e2940; --acento:#ff8a1c; --falta:#d5a83c;
  --c1:#ff8a1c; --c2:#6f9fe0; --c3:#7fb36e; --c4:#3fc8d4;
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
/* Abaixo de 700px a figura LARGA (viewBox 640) escalaria a menos de 0,95 e
   o texto de 12px cairia abaixo de 11px; entra a ESTREITA, com o rotulo em
   cima da barra. Uma so e visivel por vez. */
svg.estreito{{display:none}}
@media (max-width:700px){{svg.largo{{display:none}}svg.estreito{{display:block}}}}
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
