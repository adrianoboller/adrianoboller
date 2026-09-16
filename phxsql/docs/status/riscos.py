#!/usr/bin/env python3
"""RISCOS e DIVIDA TECNICA -- as duas secoes que o pedido 264 fez nascer.

    python3 docs/status/riscos.py [pagina.html]   # escreve as duas secoes
    ./status-html.sh                              # a pagina inteira, com elas

# Por que este arquivo existe

Ate 16/09/2026 a setima pagina tinha VINTE E UMA secoes e quatro NAO NASCIAM,
porque *secao so entra com gerador*. Duas delas eram estas. O exemplo que o
dono mandou trazia as duas com numero DIGITADO -- e numero digitado envelhece
calado, que e a lei que esta casa ja pagou quatro vezes.

Entao as duas nasceram pelo caminho contrario, e cada uma com a sua fonte:

* **Riscos** sai de `docs/RISCOS.md`, que **se edita** -- como o `STATUS.md`,
  porque probabilidade e impacto sao AVALIACAO, nao medida. O que nao se
  digita la e numero: a linha nomeia uma **sonda**, e o numero sai do codigo
  aqui, com a data em que foi medido.
* **Divida tecnica** sai de uma marca `// DIVIDA:` no PROPRIO FONTE Rust,
  varrida como o `conferidor.rs` varre os textos fora da fabrica. A lista de
  arquivos sai do disco, nunca de uma copia: lista digitada foi o que fez o
  rodape publicar 780 KiB onde a interface tinha 1.032.

# Por que a varredura e Python e nao um conferidor em Rust

O molde do `conferidor.rs` e o do MECANISMO -- varrer o fonte, contar, dizer
arquivo e linha. O molde do PORTAO nao da: um conferidor em Rust so responde
por `cargo run --example`, e no `portao-dos-geradores.py` isso e `nota-cargo`,
que o portao NAO roda (a worktree tem disco escasso). A secao ficaria com um
numero que nenhum portao confere -- exatamente o estado que o pedido 264
existe para acabar. Aqui a varredura e funcao pura do fonte versionado, e o
portao a confere byte a byte a cada rodada.

# O que ele recusa fazer, e por que

* **marca sem motivo** -- para a geracao nomeando arquivo e linha. `// DIVIDA:`
  sozinho e um lembrete, e lembrete nao e divida: ninguem sabe o que ficou
  faltando seis meses depois.
* **marca citando pedido que nao existe** -- para do mesmo jeito. Ponteiro
  para pedido inventado e pior que ponteiro nenhum, porque parece rastreavel.
* **risco sem fonte, sem data, ou com palavra fora do vocabulario** -- para
  nomeando a linha. Vocabulario aberto vira «meio-alto», e ai a coluna deixa
  de ordenar coisa nenhuma.
* **sonda que nao existe** -- para nomeando a chave. Chave morta e pior que
  chave faltando: parece medicao e nao mede nada.
"""

import datetime
import importlib.util
import pathlib
import re
import sys

AQUI = pathlib.Path(__file__).resolve().parent
RAIZ = AQUI.parent.parent
FONTE_DOS_RISCOS = RAIZ / "docs" / "RISCOS.md"
PADRAO = AQUI / "status-do-projeto.html"

# As duas secoes sao escritas DENTRO da pagina, entre estas marcas -- o mesmo
# desenho do bloco `catracas:` do `docs/QA-PDCA.md`. E' o que deixa este
# gerador rodar sozinho em segundos: a pagina inteira leva minutos porque
# varre `crates/` cinco vezes, e quem mexeu so no `RISCOS.md` nao precisa
# pagar isso. O `pagina-do-status-do-projeto.py` escreve as MESMAS marcas em
# volta das MESMAS secoes, entao rodar um depois do outro nao muda um byte.
MARCA_INICIO = "<!-- riscos:inicio -->"
MARCA_FIM = "<!-- riscos:fim -->"


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


PEDIDOS = importar(RAIZ / "docs" / "dossie" / "pagina-dos-pedidos.py", "rk_pedidos")
PMO = importar(RAIZ / "docs" / "pmo" / "pagina-do-status-do-projeto.py", "rk_pmo")
PROVAS = importar(RAIZ / "docs" / "dossie" / "pagina-dos-testes.py", "rk_provas")


# --------------------------------------------------------------- vocabulario
#
# Fechado de proposito. A coluna existe para ORDENAR o risco, e coluna que
# aceita qualquer palavra nao ordena nada -- na terceira rodada tem «alto»,
# «alta», «elevado» e «meio-alto», e ninguem consegue contar quantos sao os
# graves. `certa` nao e previsao: e' estado, o risco ja esta acontecendo.
PROBABILIDADES = ("certa", "alta", "média", "baixa")
IMPACTOS = ("alto", "médio", "baixo")
PESO_PROB = {p: i for i, p in enumerate(("baixa", "média", "alta", "certa"))}
PESO_IMP = {p: i for i, p in enumerate(("baixo", "médio", "alto"))}
DATA_ISO = re.compile(r"^\d{4}-\d{2}-\d{2}$")
COLUNAS = ("#", "risco", "prob.", "impacto", "dono", "avaliado em", "sonda", "fonte")


# -------------------------------------------------------------------- sondas
#
# Cada sonda devolve (valor, quando, ressalva). O valor sai SEMPRE de um leitor
# que ja existe -- nenhuma delas reconta o que outro gerador ja conta, pelo
# mesmo motivo que a setima pagina nao tem leitor proprio de quase nada:
# receita duplicada e receita que diverge.

def _sonda_gates(ctx):
    itens = ctx["itens"]
    return len(PMO.achar_gates(itens)), ctx["hoje"], ""


def _sonda_vermelhas(ctx):
    return len(PROVAS.guardas_vermelhas()), ctx["hoje"], ""


def _sonda_catracas(_ctx):
    """Catracas REPROVANDO, lidas do bloco gerado do `docs/QA-PDCA.md`.

    A tabela de la e' escrita pelo `docs/qa/medir.py`, que chama o conferidor
    de cada catraca. Reconta-la aqui seria uma segunda regua para o mesmo
    numero -- e duas reguas divergem na primeira que mudar.
    """
    p = RAIZ / "docs" / "QA-PDCA.md"
    if not p.exists():
        return None, "—", "falta o docs/QA-PDCA.md"
    texto = p.read_text(encoding="utf-8")
    ini = texto.find("<!-- catracas:inicio -->")
    fim = texto.find("<!-- catracas:fim -->")
    if ini < 0 or fim < 0:
        return None, "—", "o bloco `catracas:` nao esta no docs/QA-PDCA.md"
    bloco = texto[ini:fim]
    quando, do_mtime = PROVAS.quando_de(p)
    n = sum(1 for l in bloco.split("\n")
            if l.startswith("|") and "REPROVANDO" in l)
    return n, quando, "mtime" if do_mtime else ""


def _sonda_divida(ctx):
    return len(ctx["divida"]), ctx["hoje"], ""


# A lista das sondas sai DAQUI, e o `docs/RISCOS.md` so a cita pela chave.
# Quem escreve um risco novo com numero escolhe uma destas -- ou nenhuma, e ai
# a linha diz «—» em vez de fingir que mediu.
#
# O rotulo e TEXTO PURO de proposito: ele vai para dentro de uma celula que
# escapa o que recebe, e uma etiqueta com `<code>` ali sairia com as tags a
# mostra. Rotulo e' rotulo; quem quer marcacao usa o «de onde sai», que e'
# prosa da pagina.
SONDAS = {
    "gates": (_sonda_gates, "pedidos abertos parados com gente",
              "o <code>achar_gates()</code> de <code>docs/pmo/"
              "pagina-do-status-do-projeto.py</code> sobre o "
              "<code>docs/PENDENCIAS.md</code>"),
    "guardas-vermelhas": (_sonda_vermelhas, "guardas entregues vermelhas",
                          "o <code>guardas_vermelhas()</code> de "
                          "<code>docs/dossie/pagina-dos-testes.py</code>, que "
                          "varre <code>#[ignore = \"VERMELHA …\"]</code> no fonte"),
    "catracas-reprovando": (_sonda_catracas, "catracas reprovando",
                            "o bloco <code>catracas:</code> do "
                            "<code>docs/QA-PDCA.md</code>, escrito por "
                            "<code>docs/qa/medir.py</code>"),
    "divida": (_sonda_divida, "marcas // DIVIDA: no fonte",
               "a varredura desta mesma página (a seção seguinte)"),
}


# ------------------------------------------------------------- o RISCOS.md

def _celulas(linha):
    return [c.strip() for c in linha.strip().strip("|").split("|")]


def _num(apelido):
    """O numero do apelido (R7 -> 7), para R10 nao vir antes de R2."""
    d = re.sub(r"\D", "", apelido)
    return int(d) if d else 0


def ler_riscos(caminho=FONTE_DOS_RISCOS):
    """As linhas da tabela de `docs/RISCOS.md`, conferidas uma a uma.

    Ele PARA quando a linha esta malformada, em vez de pular: risco que some
    da pagina por causa de uma celula errada e a pior forma de esconder um
    risco -- ninguem procura o que nao sabe que faltou.
    """
    if not caminho.exists():
        raise SystemExit(
            f"falta {caminho.relative_to(RAIZ)} -- a secao de riscos sai DELE. "
            "Sem a fonte nao ha secao, e secao vazia mente mais que secao "
            "ausente.")
    linhas = caminho.read_text(encoding="utf-8").split("\n")
    dentro, riscos, vistos = False, [], set()
    for n, l in enumerate(linhas, 1):
        if not l.startswith("|"):
            dentro = False
            continue
        cels = _celulas(l)
        if tuple(cels) == COLUNAS:
            dentro = True
            continue
        if not dentro or all(re.fullmatch(r":?-{2,}:?", c) for c in cels):
            continue
        if len(cels) != len(COLUNAS):
            raise SystemExit(
                f"{caminho.name}:{n}: a linha tem {len(cels)} celulas e a "
                f"tabela tem {len(COLUNAS)} -- "
                f"{' | '.join(COLUNAS)}")
        r = dict(zip(COLUNAS, cels))
        onde = f"{caminho.name}:{n} ({r['#']})"
        if r["#"] in vistos:
            raise SystemExit(f"{onde}: o apelido se repete -- apelido de risco "
                             "nao se reaproveita, senao duas rodadas falam de "
                             "coisas diferentes com o mesmo nome.")
        vistos.add(r["#"])
        if r["prob."] not in PROBABILIDADES:
            raise SystemExit(f"{onde}: probabilidade {r['prob.']!r} fora do "
                             f"vocabulario {PROBABILIDADES}")
        if r["impacto"] not in IMPACTOS:
            raise SystemExit(f"{onde}: impacto {r['impacto']!r} fora do "
                             f"vocabulario {IMPACTOS}")
        if not DATA_ISO.fullmatch(r["avaliado em"]):
            raise SystemExit(f"{onde}: 'avaliado em' e {r['avaliado em']!r} -- "
                             "tem de ser AAAA-MM-DD. Avaliacao sem data nao "
                             "envelhece: ela so parece atual.")
        if not r["fonte"] or r["fonte"] == "—":
            raise SystemExit(f"{onde}: risco sem fonte nao entra. Risco "
                             "inventado numa rodada vira, tres rodadas depois, "
                             "uma preocupacao que ninguem confirma nem enterra.")
        if r["sonda"] not in ("—", "-", "") and r["sonda"] not in SONDAS:
            raise SystemExit(
                f"{onde}: a sonda {r['sonda']!r} nao existe. As que existem: "
                + ", ".join(sorted(SONDAS)) + ". Chave morta e pior que chave "
                "faltando -- parece medicao e nao mede nada.")
        r["linha"] = n
        riscos.append(r)
    if not riscos:
        raise SystemExit(
            f"{caminho.name}: nenhuma linha de risco foi lida -- o cabecalho "
            f"da tabela tem de ser exatamente: | {' | '.join(COLUNAS)} |")
    return riscos


# ------------------------------------------------------- a varredura do fonte

# A marca aceita os tres estilos de comentario do Rust, porque ela mora ao lado
# da frase que JA explicava a divida -- e essa frase tanto pode estar num
# `//!` de cabecalho de modulo quanto num `///` de item ou num `//` solto.
# Arrancar a explicacao do lugar dela para caber num estilo so seria trocar
# contexto por formato.
MARCA_DIVIDA = re.compile(r"^\s*//[/!]?\s*DIVIDA:\s*(.+?)\s*$")
PEDIDO_NA_MARCA = re.compile(r"^#(\d+)\s+(.*)$")
MOTIVO_MINIMO = 30


def varrer_divida(itens_do_pendencias=None):
    """Toda marca `// DIVIDA:` de `crates/`, com arquivo, linha e motivo.

    A lista de arquivos sai do DISCO (`crates/**/*.rs`), nunca de uma copia
    aqui dentro: uma lista digitada perde o arquivo que nascer amanha, e foi
    assim que o rodape do dossie publicou 780 KiB onde a interface tinha 1.032.
    """
    conhecidos = None
    if itens_do_pendencias is not None:
        conhecidos = {i["n"]: i["rotulo"] for i in itens_do_pendencias}
    achadas = []
    for rs in sorted((RAIZ / "crates").glob("*/**/*.rs")):
        try:
            texto = rs.read_text(encoding="utf-8")
        except UnicodeDecodeError:
            continue
        if "DIVIDA:" not in texto:
            continue
        rel = str(rs.relative_to(RAIZ))
        for n, l in enumerate(texto.split("\n"), 1):
            m = MARCA_DIVIDA.match(l)
            if not m:
                continue
            corpo = m.group(1)
            pedido, rotulo = None, None
            mp = PEDIDO_NA_MARCA.match(corpo)
            if mp:
                pedido, corpo = int(mp.group(1)), mp.group(2).strip()
                if conhecidos is not None and pedido not in conhecidos:
                    raise SystemExit(
                        f"{rel}:{n}: a marca cita o pedido #{pedido}, que nao "
                        "existe no docs/PENDENCIAS.md. Ponteiro para pedido "
                        "inventado e pior que ponteiro nenhum: parece "
                        "rastreavel.")
                rotulo = conhecidos.get(pedido) if conhecidos else None
            if len(corpo) < MOTIVO_MINIMO:
                raise SystemExit(
                    f"{rel}:{n}: a marca nao diz o que falta ({corpo!r}). "
                    "Marca sem motivo e lembrete, e lembrete nao e divida -- "
                    "seis meses depois ninguem sabe o que ficou faltando.")
            achadas.append({
                "arquivo": rel, "n": n, "pedido": pedido, "rotulo": rotulo,
                "motivo": corpo, "crate": rel.split("/")[1],
            })
    achadas.sort(key=lambda d: (d["arquivo"], d["n"]))
    return achadas


# ---------------------------------------------------------------- as secoes

# Markdown que o `docs/RISCOS.md` usa nas celulas, e so ele: negrito, italico
# e `codigo`. Ele existe porque a fonte e um .md de verdade -- lida no GitHub,
# lida no editor -- e nao um HTML disfarcado. O que NAO passa por aqui e o
# texto vindo do FONTE Rust (a coluna «o que falta» da divida): aquilo e dado
# copiado do codigo, e dado nao se estiliza -- quem le tem de ver o que esta
# escrito la, crase inclusive.
_MD = [
    (re.compile(r"\*\*(.+?)\*\*"), r"<b>\1</b>"),
    (re.compile(r"`([^`]+)`"), r"<code>\1</code>"),
    (re.compile(r"(?<![\w*])\*([^*]+)\*(?![\w*])"), r"<i>\1</i>"),
]


def marcacao(texto, esc):
    """O markdown da celula vira HTML -- e o resto e ESCAPADO antes."""
    saida = esc(texto)
    for rx, rep in _MD:
        saida = rx.sub(rep, saida)
    return saida


def _medir(chave, ctx):
    """(valor, quando, ressalva) da celula «medida» de uma linha de risco."""
    if chave not in SONDAS:
        return "—", "", ""
    valor, quando, ressalva = SONDAS[chave][0](ctx)
    if valor is None:
        # Sonda sem fonte aparece como NAO MEDIDA, com o motivo -- e a mesma
        # regra da bancada sem `resultados.json`: esconder o que nao mediu e
        # pior que mostrar o buraco, porque a pagina existe justamente para
        # dizer em que se pode confiar.
        return "NÃO MEDIDA", quando, ressalva
    return f"{valor}", quando, ressalva


def secao_riscos(P, ctx):
    """A secao dos riscos. `P` e o modulo da pagina -- os ajudantes de tela.

    Receber o modulo, em vez de copiar `esc`, `kpi` e `fonte` para ca, e a
    mesma regra dos leitores: receita duplicada e receita que diverge. Aqui
    ela vale duas vezes, porque a paleta e a marca mandam sobre a tela inteira
    e uma segunda copia de `kpi` envelheceria na primeira mudanca de estilo.
    """
    riscos = ctx["riscos"]
    esc, hoje = P.esc, ctx["hoje"]
    altos = [r for r in riscos if r["impacto"] == "alto"]
    certos = [r for r in riscos if r["prob."] == "certa"]
    com_dono = [r for r in riscos if r["dono"] == "dono"]
    medidos = [r for r in riscos if r["sonda"] in SONDAS]

    h = [P.h2("riscos", "Riscos"),
         '<p class="sub">A <b>nota é avaliação</b> — probabilidade e impacto '
         'saíram de uma leitura dos documentos, na data que cada linha diz. A '
         'coluna <b>medida</b> é número, e sai de uma sonda do gerador. Os '
         'dois tipos estão separados de propósito: quem discorda de uma '
         'avaliação muda a linha do <code>docs/RISCOS.md</code> com o motivo; '
         'quem discorda de um número roda a sonda.</p>',
         '<div class="kpis">']
    h.append(P.kpi(len(riscos), "riscos no catálogo", hoje))
    h.append(P.kpi(len(altos), "de impacto <b>alto</b>", hoje, classe="falta"))
    h.append(P.kpi(len(certos), "já acontecendo (<b>certa</b>)", hoje,
                   classe="parcial"))
    h.append(P.kpi(len(com_dono), "que dependem do <b>dono</b>", hoje,
                   classe="planejado"))
    h.append(P.kpi(len(medidos), "com sonda — o resto é avaliação pura", hoje))
    h.append("</div>")

    # A celula da medida NAO leva a classe `num`: ela e' `white-space:nowrap`,
    # e com o rotulo e a data dentro a coluna crescia ate ser cortada na
    # largura da tela -- defeito que a leitura do codigo nao mostra e que a
    # captura mostrou de primeira.
    h.append('<div class="rolo"><table><thead><tr><th>#</th>'
             '<th>risco, e a fonte de onde ele saiu</th>'
             '<th>prob.</th><th>impacto</th><th>dono</th><th>avaliado em</th>'
             '<th>medida</th></tr></thead><tbody>')
    ordem = sorted(riscos, key=lambda r: (-PESO_IMP[r["impacto"]],
                                          -PESO_PROB[r["prob."]], _num(r["#"])))
    for r in ordem:
        etiq_p = {"certa": "e-nao", "alta": "e-parcial",
                  "média": "e-plan", "baixa": "e-tem"}[r["prob."]]
        etiq_i = {"alto": "e-nao", "médio": "e-parcial", "baixo": "e-tem"}[r["impacto"]]
        medida, quando, ressalva = _medir(r["sonda"], ctx)
        cel = f'<b>{esc(medida)}</b>'
        if r["sonda"] in SONDAS:
            # A ressalva NAO substitui a data: ela a qualifica. `mtime` nao e
            # a hora da medicao, e a hora em que alguem gravou -- e um
            # `git checkout` move o mtime sem medir nada.
            marca = f" ({ressalva})" if ressalva else ""
            cel += (f'<br><span class="mtime">{esc(SONDAS[r["sonda"]][1])}'
                    f' · {esc(quando)}{esc(marca)}</span>')
        # A fonte vai DEBAIXO do risco, e nao numa coluna propria: em coluna
        # ela empurrava a tabela para fora da tela (um caminho de cognicao tem
        # 90 caracteres e nao quebra), e `.fonte` ja sabe quebrar palavra
        # longa. Isto so apareceu olhando a pagina no navegador.
        h.append(
            f'<tr><td class="mono">{esc(r["#"])}</td>'
            f'<td>{marcacao(r["risco"], esc)}'
            f'<p class="fonte"><b>Fonte:</b> {marcacao(r["fonte"], esc)}</p></td>'
            f'<td><span class="etiqueta {etiq_p}">{esc(r["prob."])}</span></td>'
            f'<td><span class="etiqueta {etiq_i}">{esc(r["impacto"])}</span></td>'
            f'<td class="mono">{esc(r["dono"])}</td>'
            f'<td class="q">{esc(r["avaliado em"])}</td>'
            f'<td>{cel}</td></tr>')
    h.append("</tbody></table></div>")

    h.append('<div class="nota a"><b>Risco não é pendência.</b> A pendência '
             'diz <i>o que ainda não foi feito</i>; o risco diz <i>o que '
             'acontece se continuar assim</i>. Quase toda linha acima aponta '
             'para um pedido do <code>docs/PENDENCIAS.md</code> — e há risco '
             'sem pedido nenhum, que é justamente o que esta seção existe '
             'para não perder.</div>')
    h.append(P.fonte(
        '<code>docs/RISCOS.md</code>, que <b>se edita</b> — a avaliação é '
        'humana e datada, como a do <code>docs/STATUS.md</code>. Os números da '
        'coluna <b>medida</b> não estão lá: cada linha nomeia uma <b>sonda</b>, '
        'e a sonda é resolvida aqui, contra ' + ", ".join(
            sorted(s[2] for s in SONDAS.values())) + '. Sonda que não existe '
        '<b>para</b> a página, nomeando a chave.'))
    return "\n".join(h)


def secao_divida(P, ctx):
    """A secao da divida tecnica, contada no fonte."""
    divida = ctx["divida"]
    esc = P.esc
    por_crate = {}
    for d in divida:
        por_crate[d["crate"]] = por_crate.get(d["crate"], 0) + 1
    arquivos = sorted({d["arquivo"] for d in divida})
    com_pedido = [d for d in divida if d["pedido"]]

    h = [P.h2("divida", "Dívida técnica, contada no fonte"),
         '<p class="sub">Cada linha abaixo é uma marca <code>// DIVIDA:</code> '
         'no código, <b>ao lado da frase que já explicava a dívida</b>. '
         'Nenhuma foi inventada aqui: a varredura achou o que estava escrito e '
         'a marca só o tornou contável. Dívida que só existe na cabeça de quem '
         'escreveu não se conta — e o que não se conta não desce.</p>',
         '<div class="kpis">']
    h.append(P.kpi(len(divida), "marcas <code>// DIVIDA:</code>", ctx["hoje"]))
    h.append(P.kpi(len(arquivos), "arquivos com dívida marcada", ctx["hoje"]))
    h.append(P.kpi(len(por_crate), "crates atingidos", ctx["hoje"]))
    h.append(P.kpi(len(com_pedido), "que citam um pedido do dono", ctx["hoje"],
                   classe="planejado"))
    h.append("</div>")

    if por_crate:
        h.append('<div class="barras">')
        maior = max(por_crate.values())
        for nome, n in sorted(por_crate.items(), key=lambda x: (-x[1], x[0])):
            h.append(P.barra(nome, [("cheia", n)], str(n), maior))
        h.append("</div>")

    h.append('<div class="rolo"><table><thead><tr><th>onde</th><th>pedido</th>'
             '<th>o que falta</th></tr></thead><tbody>')
    for d in divida:
        if d["pedido"]:
            classe = {"Feito": "e-tem", "Parcial": "e-parcial",
                      "Planejado": "e-plan"}.get(d["rotulo"], "e-nao")
            ped = (f'<span class="etiqueta {classe}">#{d["pedido"]} '
                   f'{esc(d["rotulo"] or "?")}</span>')
        else:
            ped = '<span class="mtime">sem pedido</span>'
        h.append(f'<tr><td class="mono">{esc(d["arquivo"])}:{d["n"]}</td>'
                 f'<td>{ped}</td><td>{esc(d["motivo"])}</td></tr>')
    h.append("</tbody></table></div>")

    # `P.ref(...)` e nao «§11» escrito aqui: esta secao entrou NO MEIO da
    # pagina e empurrou as catracas de 11 para 13 -- o numero que eu tivesse
    # digitado ja teria envelhecido nesta mesma rodada.
    h.append('<div class="nota v"><b>Este número não tem catraca, e isso é '
             'dito em vez de escondido.</b> Nada reprova quem acrescentar mais '
             'uma marca — ao contrário das catracas do ' + P.ref("catracas") +
             ', que só descem. '
             'Pôr uma aqui hoje puniria justamente quem <b>marca</b> a dívida '
             'que já existia, que é o contrário do que esta seção quer. A '
             'decisão é do papel G, e está no catálogo de riscos como '
             'R19.</div>')
    h.append(P.fonte(
        'a varredura de <code>// DIVIDA:</code> em '
        '<code>crates/**/*.rs</code> — a lista de arquivos sai do <b>disco</b>, '
        'não de uma cópia no script: lista digitada perde o arquivo que nascer '
        'amanhã. O estado de cada pedido citado vem do <code>ler()</code> de '
        '<code>docs/dossie/pagina-dos-pedidos.py</code>, e marca que cita '
        'pedido inexistente <b>para</b> o gerador. Feita por '
        '<code>docs/status/riscos.py</code>, no molde do '
        '<code>crates/phxsql-server/src/conferidor.rs</code>.'))
    return "\n".join(h)


def contexto(hoje=None):
    """O que as duas secoes precisam, medido uma vez so."""
    itens = PEDIDOS.ler()
    return {
        "hoje": hoje or datetime.date.today().isoformat(),
        "itens": itens,
        "riscos": ler_riscos(),
        "divida": varrer_divida(itens),
    }


def secoes(P, ctx):
    """As duas secoes, na ordem em que entram na pagina."""
    return secao_riscos(P, ctx), secao_divida(P, ctx)


def bloco(P, ctx):
    """O texto inteiro entre as marcas -- o mesmo que a pagina monta.

    Ele existe para que haja UMA montagem, e nao duas: a pagina junta as
    secoes com `\\n\\n` e poe as marcas em volta; este gerador repete
    exatamente isso ao escrever sozinho. Duas montagens divergiriam no
    primeiro espaco em branco, e o portao acusaria a pagina como VELHA sem
    nenhum numero ter mudado.
    """
    r, d = secoes(P, ctx)
    return MARCA_INICIO + "\n" + r + "\n\n" + d + "\n" + MARCA_FIM


def main():
    alvo = pathlib.Path(sys.argv[1]) if len(sys.argv) > 1 else PADRAO
    if not alvo.exists():
        raise SystemExit(
            f"{alvo} nao existe. Este gerador escreve as duas secoes DENTRO da "
            "setima pagina; a pagina inteira sai de `./status-html.sh`.")
    # Importado aqui, e nao no topo, para nao haver ciclo: a pagina importa
    # ESTE modulo. Daqui so saem os ajudantes de tela dela (`esc`, `kpi`,
    # `barra`, `fonte`, `h2`) e a numeracao das secoes.
    P = importar(AQUI / "pagina-do-status-do-projeto.py", "rk_pagina")
    ctx = contexto()
    texto = alvo.read_text(encoding="utf-8")
    i, f = texto.find(MARCA_INICIO), texto.find(MARCA_FIM)
    if i < 0 or f < 0:
        raise SystemExit(
            f"{alvo.name} nao tem as marcas {MARCA_INICIO}/{MARCA_FIM}. A "
            "pagina foi gerada por uma versao que ainda nao conhecia estas "
            "duas secoes -- rode `./status-html.sh` uma vez. Escrever sem a "
            "marca seria adivinhar onde a secao entra, e adivinhar e o que "
            "esta lei existe para nao fazer.")
    novo = texto[:i] + bloco(P, ctx) + texto[f + len(MARCA_FIM):]
    if novo == texto:
        print(f"as duas secoes ja estavam em dia em {alvo.relative_to(RAIZ)}")
    else:
        alvo.write_text(novo, encoding="utf-8")
        print(f"duas secoes escritas em {alvo.relative_to(RAIZ)}")
    print(f"  riscos: {len(ctx['riscos'])} no catalogo "
          f"({sum(1 for r in ctx['riscos'] if r['impacto'] == 'alto')} de "
          f"impacto alto), de docs/RISCOS.md")
    print(f"  divida: {len(ctx['divida'])} marcas `// DIVIDA:` em "
          f"{len({d['arquivo'] for d in ctx['divida']})} arquivos de crates/")
    # Gerador que faz menos do que o nome promete TEM de dizer que fez menos.
    nao_medidas = []
    for chave, (fn, rotulo, _de_onde) in sorted(SONDAS.items()):
        valor, _quando, porque = fn(ctx)
        if valor is None:
            nao_medidas.append((chave, rotulo, porque))
    if nao_medidas:
        print("  sonda(s) que NAO mediram -- a celula da pagina diz NAO MEDIDA:")
        for chave, rotulo, porque in nao_medidas:
            print(f"    - {chave} ({rotulo}): {porque}")
    # O outro sentido do laco, que e o que ninguem escreve: sonda declarada
    # que risco nenhum cita. Chave morta e pior que chave faltando -- ela
    # parece medicao, e so se descobre que nao media nada quando alguem a usa.
    usadas = {r["sonda"] for r in ctx["riscos"]}
    orfas = sorted(set(SONDAS) - usadas)
    if orfas:
        print("  sonda(s) que NENHUM risco cita (chave morta): "
              + ", ".join(orfas))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
