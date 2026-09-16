#!/usr/bin/env python3
"""TELEMETRIA E LOGS -- a secao que o pedido 265 fez nascer.

    python3 docs/status/telemetria-medida.py [pagina.html]   # so esta secao
    ./status-html.sh                                         # a pagina inteira

# Por que este arquivo existe

A setima pagina tinha quatro secoes que NAO NASCIAM, porque *secao so entra com
gerador*. Esta era uma delas, e o que faltava vinha ANTES do gerador: a pasta
`bancada/telemetria/` existia e exercitava o painel de bolhas, mas **nao
gravava `resultados.json`**. Sem ele nao ha a data em que o numero foi medido,
e numero sem a data em que foi medido publica um retrato que nunca existiu.

Entao o pedido 265 comecou pela fonte: `bancada/telemetria/custo.py` mede por
soquete, contra um servidor de verdade, com o interruptor virado em tempo de
execucao -- e grava o resultado com um `quando` por carga. Este arquivo so le.

# O que ele recusa fazer, e por que

* **inventar numero quando a bancada nao rodou** -- sem o `resultados.json` a
  secao aparece como NAO MEDIDA, com o comando para rodar. E a mesma regra das
  bancadas na pagina dos testes: bancada sem resultado nunca some da tabela,
  porque uma pagina que esconde o que nao mediu e a pior de todas.
* **publicar custo que a corrida nao resolveu** -- o arquivo traz `resolvido`
  por carga, e a celula diz «dentro do ruido» quando o teste de sinal nao
  passou. Esta casa ja declarou vencedor dentro do ruido uma vez (pedido 155),
  e a bancada guarda quantos pares votaram de cada lado para quem le julgar o
  veredito em vez de acreditar nele.
* **contar os pontos de captura por uma lista digitada** -- a conta sai da
  varredura do proprio `telemetria.rs` atras do portao
  `if !self.ligada() { return; }`. Lista copiada envelhece calada, e foi o que
  fez o rodape publicar 780 KiB onde a interface tinha 1.032.
"""

import importlib.util
import pathlib
import re
import sys

AQUI = pathlib.Path(__file__).resolve().parent
RAIZ = AQUI.parent.parent
PADRAO = AQUI / "status-do-projeto.html"
RESULTADO = "bancada/telemetria/resultados.json"
COMANDO = ("flock /tmp/phx-cargo.lock cargo build --release -p phxsql-server "
           "--bin phxsqld &amp;&amp; python3 bancada/telemetria/custo.py")

# A secao e escrita DENTRO da pagina, entre estas marcas -- o mesmo desenho do
# `riscos.py` (pedido 264) e do bloco `catracas:` do `docs/QA-PDCA.md`. E' o
# que deixa este gerador rodar sozinho em segundos: a pagina inteira leva
# minutos porque varre `crates/` cinco vezes, e quem so re-rodou a bancada nao
# precisa pagar isso.
MARCA_INICIO = "<!-- telemetria:inicio -->"
MARCA_FIM = "<!-- telemetria:fim -->"

# O portao de cada ponto de captura, no fonte. E' a lei do CLAUDE.md escrita em
# codigo: «instrumentacao desligada custa zero, e o portao que decide isso vem
# ANTES do trabalho».
PORTAO_NO_FONTE = re.compile(r"^\s*if !self\.ligada\(\) \{", re.M)
FONTE_DA_TELEMETRIA = RAIZ / "crates" / "phxsql-server" / "src" / "telemetria.rs"


def importar(caminho, apelido):
    """Importa um gerador pelo CAMINHO: os nomes tem hifen e nao sao modulos."""
    if not caminho.exists():
        raise SystemExit(
            f"falta o modulo irmao {caminho.relative_to(RAIZ)} -- este gerador "
            "nao reescreve o leitor dele. Sem ele nao ha numero, e numero que "
            "falta nao vira zero.")
    spec = importlib.util.spec_from_file_location(apelido, caminho)
    mod = importlib.util.module_from_spec(spec)
    sys.modules[apelido] = mod
    spec.loader.exec_module(mod)
    return mod


PROVAS = importar(RAIZ / "docs" / "dossie" / "pagina-dos-testes.py", "tl_provas")


def pontos_de_captura():
    """Quantos pontos perguntam pelo portao ANTES do trabalho, contados no fonte.

    Devolve `None` quando o arquivo nao esta onde se espera -- numero que falta
    nao vira zero, e zero aqui diria o contrario do que a lei manda provar.
    """
    if not FONTE_DA_TELEMETRIA.exists():
        return None
    texto = FONTE_DA_TELEMETRIA.read_text(encoding="utf-8")
    return len(PORTAO_NO_FONTE.findall(texto))


def medidas():
    """(dados, quando, veio_do_mtime) da corrida da bancada de telemetria.

    `dados` e `None` quando a bancada nunca rodou -- e ai a secao diz isso.
    """
    dados, caminho = PROVAS.ler_json(RESULTADO)
    if dados is None or "__erro__" in dados:
        return dados, "—", False
    quando, mtime = PROVAS.quando_de(caminho, dados)
    return dados, quando, mtime


def contexto():
    dados, quando, mtime = medidas()
    return {"dados": dados, "quando": quando, "mtime": mtime,
            "pontos": pontos_de_captura()}


def _curto(caminho):
    """O caminho relativo a RAIZ -- ou o absoluto, quando ele nao mora la.

    `relative_to` LEVANTA fora da raiz, e levantar de dentro da montagem de uma
    mensagem troca o diagnostico por outro. O alvo pode vir por argumento, e
    `/tmp/qualquer.html` e um alvo legitimo.
    """
    try:
        return str(caminho.relative_to(RAIZ))
    except ValueError:
        return str(caminho)


# ------------------------------------------------------------------- a secao

def _nao_medida(P, ctx):
    dados = ctx["dados"]
    porque = ("<code>" + P.esc(RESULTADO) + "</code> não existe"
              if dados is None else
              "<code>" + P.esc(RESULTADO) + "</code> está quebrado: "
              + P.esc(dados["__erro__"]))
    return "\n".join([
        P.h2("telemetria", "Telemetria e logs"),
        '<p class="sub">A instrumentação deste motor — o que ela custa ligada, '
        'e o que o log escreve por operação.</p>',
        '<div class="ausente-bloco"><b>NÃO MEDIDA.</b> ' + porque
        + ". A bancada existe e roda sozinha:<br><code>" + COMANDO
        + "</code><br>Enquanto ela não rodar, esta seção diz que não mediu em "
        "vez de publicar o número de outra corrida — bancada sem resultado "
        "nunca some da tabela.</div>",
        P.fonte("<code>" + P.esc(RESULTADO) + "</code>, gravado por "
                "<code>bancada/telemetria/custo.py</code>."),
    ])


def num(x, casas=3):
    """Numero com virgula decimal -- o separador desta casa."""
    return f"{x:.{casas}f}".replace(".", ",")


def _linha_da_carga(P, c):
    """Uma carga: o piso desligado, o custo, o veredito e a data.

    O custo so vira numero quando `resolvido` -- senao a celula diz «dentro do
    ruido» e mostra a contagem dos pares, que e o que permite julgar o veredito
    em vez de acreditar nele.
    """
    sinal = c.get("sinal", {})
    unidade = c["unidade"].split("/")[0]
    if c.get("resolvido"):
        pct = c["custo_pct"]
        custo = ("<b>+" + num(c["custo"]) + "</b> " + P.esc(unidade)
                 + " (" + ("+" if pct > 0 else "") + num(pct, 2) + "%)")
        veredito = '<span class="etiqueta e-tem">resolvido</span>'
    else:
        custo = '<span class="mtime">dentro do ruído</span>'
        veredito = '<span class="etiqueta e-parcial">não resolvido</span>'
    piso = num(c["desligada"]["mediana"])
    return ('<tr><td>' + c["titulo"] + '</td>'
            '<td class="num">' + piso + '</td>'
            '<td class="num">' + custo + '</td>'
            '<td class="num">' + str(sinal.get("a_favor_de_custar", "—")) + "/"
            + str(sinal.get("pares", "—")) + '</td>'
            '<td>' + veredito + '</td>'
            # O `T` do ISO vira espaco: a celula e estreita, e com o `T` no
            # meio a quebra de linha parte a data em «2026-09-» / «16T20:46:50».
            # Nao e maquiagem do dado -- e o mesmo instante, num separador que
            # o navegador sabe quebrar. `nowrap` aqui seria pior: foi uma
            # celula `nowrap` que deu os 3 px de rolagem lateral que a sonda de
            # estouro achou nesta mesma pagina.
            '<td class="mono">'
            + P.esc(str(c.get("quando", "—"))[:19].replace("T", " "))
            + '</td></tr>')


def secao(P, ctx):
    dados = ctx["dados"]
    if dados is None or "__erro__" in dados:
        return _nao_medida(P, ctx)

    cargas = dados.get("cargas") or []
    log = dados.get("log") or {}
    fios = dados.get("threads") or {}
    resolvidas = [c for c in cargas if c.get("resolvido")]
    q = ctx["quando"] + (" (mtime)" if ctx["mtime"] else "")

    linhas = [P.h2("telemetria", "Telemetria e logs"),
              '<p class="sub">Instrumentação desligada tem de custar zero, e o '
              '<b>portão que decide isso vem antes do trabalho</b> — é pétrea, '
              'e nasceu do Profiler cobrando 7% desligado. Aqui está o preço '
              'de ligá-la, medido por soquete contra um servidor de verdade.</p>',
              '<div class="kpis">']

    if ctx["pontos"] is not None:
        linhas.append(P.kpi(ctx["pontos"], "pontos de captura, todos abrindo "
                            "pelo portão", ctx["hoje"], classe="feito"))
    for c in resolvidas:
        rotulo = "custo de ligar — <code>" + P.esc(c["chave"]) + "</code>"
        linhas.append(P.kpi(
            "+" + num(c["custo"]) + " " + P.esc(c["unidade"].split("/")[0]),
            rotulo, str(c.get("quando", q))[:10], curto=True))
    if log.get("bytes_por_operacao") is not None:
        linhas.append(P.kpi(
            num(log["bytes_por_operacao"], 1) + " B",
            "de <code>" + P.esc(log.get("arquivo", "acessos.log"))
            + "</code> por operação",
            str(log.get("quando", q))[:10], curto=True))
    if fios.get("registradas") is not None:
        linhas.append(P.kpi(fios["registradas"], "threads com finalidade "
                            "escrita, vivas no servidor",
                            str(fios.get("quando", q))[:10]))
    linhas.append("</div>")

    linhas.append('<div class="rolo"><table><thead><tr>'
                  '<th>carga</th><th class="num">desligada (piso)</th>'
                  '<th class="num">o que ligar custa</th>'
                  '<th class="num">pares a favor</th><th>veredito</th>'
                  '<th>medida em</th></tr></thead><tbody>')
    for c in cargas:
        linhas.append(_linha_da_carga(P, c))
    linhas.append("</tbody></table></div>")

    if dados.get("carga_da_maquina") is not None:
        linhas.append(
            '<p class="fonte"><b>Em que condição:</b> '
            + P.esc(dados.get("maquina", "—")) + ", <code>"
            + P.esc(dados.get("versao", "—")) + "</code>, com a máquina em "
            "carga <b>" + str(dados["carga_da_maquina"]).replace(".", ",")
            + "</b> — outras frentes compilando ao lado. A carga entra no "
            "resultado de propósito: estas sondas medem diferenças de 1%, e "
            "quem lê tem de saber em que condição o número foi tirado sem ter "
            "de acreditar na palavra de ninguém.</p>")

    linhas.append(
        '<div class="nota"><b>Por que o «piso», e não a média.</b> O que a '
        'telemetria acrescenta a um pedido é uma <b>constante</b>: dois '
        '<code>Instant::now()</code>, um <code>fetch_add</code>, um '
        '<code>lock</code>. Num tempo <code>t = base + c</code>, o <code>c</code> '
        'aparece inteiro no <b>menor</b> tempo — e o menor é o único quase sem '
        'vizinho dentro. A média mede o vizinho: a primeira corrida desta '
        'bancada viu a mediana do <code>ping</code> andar de 59 µs para 161 µs '
        'entre duas corridas com dois minutos de diferença, sem uma linha de '
        'código mudar. E o veredito é um <b>teste de sinal</b> sobre as '
        'diferenças par a par, a 3 σ: o número publicado e o veredito saem da '
        '<b>mesma</b> lista, porque numa corrida eles discordaram — 231 de 400 '
        'pares a favor com a estimativa em −0,04%.</div>')

    if log.get("bytes") is not None:
        linhas.append(
            '<div class="nota a"><b>O que o log escreveu nesta corrida.</b> '
            + P.milhar(log["bytes"]) + " bytes de <code>"
            + P.esc(log.get("arquivo", "acessos.log")) + "</code> para "
            + P.milhar(log["operacoes"]) + " operações. O divisor não é "
            "digitado: sai da própria receita da bancada, então mudar o "
            "tamanho de uma carga muda os dois números juntos.</div>")

    linhas.append(P.fonte(
        f'<code>{P.esc(RESULTADO)}</code>, gravado por '
        '<code>bancada/telemetria/custo.py</code> — um servidor só, com o '
        'interruptor virado por <code>telemetria_ligar</code>/'
        '<code>telemetria_desligar</code> a cada par e a ordem alternada, para '
        'que um pico de vizinho caia nos dois lados do par. Os pontos de '
        'captura são contados no próprio <code>telemetria.rs</code>. O desenho '
        'está em <code>docs/TELEMETRIA.md</code>.'))
    return "\n".join(linhas)


def bloco(P, ctx):
    """O texto inteiro entre as marcas -- o mesmo que a pagina monta.

    Uma montagem so, e nao duas: a pagina poe as marcas em volta desta mesma
    secao, entao rodar um gerador depois do outro nao muda um byte. Duas
    montagens divergiriam no primeiro espaco em branco, e o portao acusaria a
    pagina como VELHA sem numero nenhum ter mudado.
    """
    return MARCA_INICIO + "\n" + secao(P, ctx) + "\n" + MARCA_FIM


def main():
    alvo = pathlib.Path(sys.argv[1]) if len(sys.argv) > 1 else PADRAO
    if not alvo.exists():
        raise SystemExit(
            f"{alvo} nao existe. Este gerador escreve a secao DENTRO da setima "
            "pagina; a pagina inteira sai de `./status-html.sh`.")
    # Importado aqui, e nao no topo, para nao haver ciclo: a pagina importa
    # ESTE modulo. Daqui so saem os ajudantes de tela dela.
    P = importar(AQUI / "pagina-do-status-do-projeto.py", "tl_pagina")
    ctx = contexto()
    ctx["hoje"] = P.hoje_iso()
    texto = alvo.read_text(encoding="utf-8")
    i, f = texto.find(MARCA_INICIO), texto.find(MARCA_FIM)
    if i < 0 or f < 0:
        raise SystemExit(
            f"{alvo.name} nao tem as marcas {MARCA_INICIO}/{MARCA_FIM}. A "
            "pagina foi gerada por uma versao que ainda nao conhecia esta "
            "secao -- rode `./status-html.sh` uma vez. Escrever sem a marca "
            "seria adivinhar onde a secao entra, e adivinhar e o que esta lei "
            "existe para nao fazer.")
    novo = texto[:i] + bloco(P, ctx) + texto[f + len(MARCA_FIM):]
    if novo == texto:
        print(f"a secao ja estava em dia em {_curto(alvo)}")
    else:
        alvo.write_text(novo, encoding="utf-8")
        print(f"secao escrita em {_curto(alvo)}")

    # Gerador que faz menos do que o nome promete TEM de dizer que fez menos.
    dados = ctx["dados"]
    if dados is None or "__erro__" in dados:
        print(f"  a bancada NAO rodou: {RESULTADO} nao existe ou esta "
              "quebrado -- a secao saiu como NAO MEDIDA, com o comando")
        return 0
    cargas = dados.get("cargas") or []
    resolvidas = [c for c in cargas if c.get("resolvido")]
    print(f"  {len(resolvidas)} de {len(cargas)} cargas resolveram o custo, "
          f"medidas em {ctx['quando']}"
          + (" (mtime)" if ctx["mtime"] else ""))
    for c in cargas:
        if not c.get("resolvido"):
            s = c.get("sinal", {})
            print(f"    - {c['chave']}: DENTRO DO RUIDO "
                  f"({s.get('a_favor_de_custar')}/{s.get('pares')} pares a "
                  "favor) -- a celula diz isso em vez de um numero")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
