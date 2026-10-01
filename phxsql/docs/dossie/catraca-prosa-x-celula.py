#!/usr/bin/env python3
"""A CATRACA PROSA x CELULA -- pedido 335, metade 2, via (B) do dono (01/10/2026).

O buraco que ela fecha: o parecer externo de 17/09/2026 achou SEIS pares de
frases no dossie publicado que davam dois estados para a mesma capacidade --
a FK «declarada e nunca imposta» numa pagina e «aplicada em todas as portas»
em outra; «o espaco volta com compactacao explicita» com o comando recusado;
«41% traduzida» com o painel gerado dizendo outro numero. A rodada de 01/10
consertou as seis NA FONTE, a mao. Sem catraca, a setima nasce na proxima
redacao e ninguem ve.

O dono escolheu (B): a prosa continua escrita a mao (voz autoral), e a
PUBLICACAO REPROVA quando a prosa contradiz a celula. Esta e a catraca.

## De onde sai a celula (o estado de cada capacidade)

Nao se digita quando ha medida -- o lexico so diz ONDE ler:

* `comparativo` -- `bancada/comparativo/resultados.json`, coluna `phxsql` da
  matriz medida contra o motor vivo (a mesma que a §33 do dossie publica).
  `tem` -> implementado, `nao` -> nao-existe; varios itens -> parcial se
  divergirem.
* `numero` -- um valor medido: `CAPABILITIES.json` (ex.: `idiomas.pct`) ou a
  contagem do `PENDENCIAS.md` pelo MESMO leitor da pagina dos pedidos
  (`pagina-dos-pedidos.py::ler`), para as duas nunca divergirem.
* `ancora` -- a excecao DECLARADA: estado escrito no lexico, e a catraca
  confere a PROVA (o teste que existe no fonte) e a AUSENCIA (o codigo que nao
  pode existir, ex.: um `fn compactar`). Prova sumida ou ausencia quebrada
  reprova: a celula envelheceu, e celula velha nao julga prosa.

## O lexico e EXPLICITO, e mora ao lado

`docs/dossie/contrato-das-capacidades.json`: por capacidade e POR ESTADO, as
frases-sinal que contradizem aquele estado. So a lista do estado ATUAL roda;
se a celula mudar para um estado sem lista, a catraca reprova dizendo isso --
lexico calado nao e lexico limpo (gerador que faz menos diz que fez menos).
Cada achado imprime a FRASE que casou e a CELULA que ela contradiz, para
quem le julgar em vez de acreditar.

## Onde roda

* `python3 docs/dossie/catraca-prosa-x-celula.py` -- sai != 0 listando
  `arquivo:linha`. `--catraca` faz o mesmo e e por ele que o
  `bancada/catracas/todas.py` a acha (varredura, sem lista).
* `portao-dos-geradores.py` a chama no fecho, junto da prova do leitor e do
  portao da versao: publicacao com contradicao e VERMELHO.
* `--autoteste` -- a prova real nos dois sentidos (ver `autoteste()`).
* `--lista` -- so imprime as celulas e de onde sairam.

## O que ela NAO pega (escrito para ninguem achar que pega)

Contradicao com redacao que nenhum sinal do lexico preve; capacidade que nao
esta no lexico; prosa em arquivo fora dos tres (`@dossie`, `README.md`,
`docs/FORMATO.md`). E o crivo e por FRASE: dois paragrafos que se
contradizem sem que nenhum dos dois contradiga a celula passam.
"""

import glob
import html
import importlib.util
import json
import pathlib
import re
import sys

AQUI = pathlib.Path(__file__).resolve().parent
RAIZ = AQUI.parent.parent  # docs/dossie -> docs -> phxsql
sys.path.insert(0, str(AQUI))
from dossie_da_pasta import achar_o_dossie  # noqa: E402

LEXICO = AQUI / "contrato-das-capacidades.json"
DOSSIE = "@dossie"

# O que o comparativo grava na coluna `phxsql` -> o estado da celula. Valor
# fora daqui REPROVA: a matriz aprendeu um estado que a catraca nao sabe ler.
DO_COMPARATIVO = {"tem": "implementado", "nao": "nao-existe"}

POR_EXTENSO = {
    "uma": 1, "duas": 2, "três": 3, "quatro": 4, "cinco": 5, "seis": 6,
    "sete": 7, "oito": 8, "nove": 9, "dez": 10, "onze": 11, "doze": 12,
    "treze": 13, "catorze": 14, "quatorze": 14, "quinze": 15,
    "dezesseis": 16, "dezessete": 17, "dezoito": 18, "dezenove": 19,
    "vinte": 20,
}


class Falha(Exception):
    """A catraca nao consegue julgar -- reprova, nunca passa calada."""


# ------------------------------------------------------------ o texto


_TOKEN_HTML = re.compile(
    r"<!--.*?-->|<script\b.*?</script>|<style\b.*?</style>|<[a-zA-Z/!][^>]*>|&#?\w+;",
    re.S | re.I)
_TOKEN_MD = re.compile(r"<!--.*?-->|<[a-zA-Z/][^>\n]*>", re.S)


def normalizar(texto, eh_html):
    """(texto_plano, linha_de_cada_caractere).

    Tira tag, comentario, script e style (no Markdown, so tag e comentario),
    decodifica entidade, apaga `*` e crase e colapsa espaco -- uma frase que
    atravessa `<code>` ou quebra de linha continua sendo UMA frase. Cada
    caractere de saida leva a linha do original, para o achado dizer
    `arquivo:linha` do fonte e nao do texto limpo."""
    rx = _TOKEN_HTML if eh_html else _TOKEN_MD
    cru, linhas = [], []
    linha, pos = 1, 0

    def por(trecho, ln):
        for ch in trecho:
            cru.append(ch)
            linhas.append(ln)

    for m in rx.finditer(texto):
        ini, fim = m.span()
        for ch in texto[pos:ini]:
            cru.append(ch)
            linhas.append(linha)
            if ch == "\n":
                linha += 1
        tok = m.group(0)
        if tok.startswith("&"):
            por(html.unescape(tok), linha)
        else:
            por(" ", linha)
        linha += tok.count("\n")
        pos = fim
    for ch in texto[pos:]:
        cru.append(ch)
        linhas.append(linha)
        if ch == "\n":
            linha += 1

    saida, saida_ln, ultimo_espaco = [], [], True
    for ch, ln in zip(cru, linhas):
        if ch in "*`":
            continue
        if ch.isspace():
            if ultimo_espaco:
                continue
            ch, ultimo_espaco = " ", True
        else:
            ultimo_espaco = False
        saida.append(ch)
        saida_ln.append(ln)
    return "".join(saida), saida_ln


# ------------------------------------------------------------ as celulas


def _importar_pedidos():
    caminho = AQUI / "pagina-dos-pedidos.py"
    spec = importlib.util.spec_from_file_location("phx_pedidos_335", caminho)
    mod = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(mod)
    return mod


def celula_de(cap):
    """(estado_ou_numero, de_onde_saiu) -- levanta Falha se nao der para ler."""
    c = cap["celula"]
    tipo = c["tipo"]
    if tipo == "comparativo":
        arq = RAIZ / "bancada" / "comparativo" / "resultados.json"
        d = json.loads(arq.read_text(encoding="utf-8"))
        por_chave = {l["chave"]: l for l in d["linhas"]}
        estados, partes = [], []
        for item in c["itens"]:
            if item not in por_chave:
                raise Falha(f"o comparativo nao tem o item «{item}» -- o lexico "
                            "aponta para uma linha que a matriz nao mede mais")
            bruto = por_chave[item]["phxsql"][0]
            if bruto not in DO_COMPARATIVO:
                raise Falha(f"o comparativo diz «{bruto}» em «{item}», estado que "
                            "esta catraca nao sabe ler")
            estados.append(DO_COMPARATIVO[bruto])
            partes.append(f"{item}={bruto}")
        estado = estados[0] if len(set(estados)) == 1 else "parcial"
        return estado, (f"comparativo medido em {d.get('quando', '?')}: "
                        + ", ".join(partes))
    if tipo == "numero":
        if c["fonte"] == "docs/PENDENCIAS.md":
            mod = _importar_pedidos()
            contas = mod.contas_de(mod.ler())
            valor = contas
            for k in c["caminho"]:
                valor = valor[k]
            return int(valor), f"PENDENCIAS.md, contado pelo leitor da pagina dos pedidos ({'.'.join(c['caminho'])})"
        arq = RAIZ / c["fonte"]
        d = json.loads(arq.read_text(encoding="utf-8"))
        valor = d
        for k in c["caminho"]:
            valor = valor[k]
        return int(valor), (f"{c['fonte']} {'.'.join(c['caminho'])}, medido em "
                            f"{d.get('medido_em', '?')}")
    if tipo == "ancora":
        for p in c.get("prova", []):
            alvo = RAIZ / p["arquivo"]
            if not alvo.exists() or p["contem"] not in alvo.read_text(encoding="utf-8"):
                raise Falha(f"a prova da celula sumiu: «{p['contem']}» nao esta em "
                            f"{p['arquivo']} -- a celula «{c['estado']}» envelheceu")
        for a in c.get("ausencia", []):
            rx = re.compile(a["rx"])
            for f in sorted(glob.glob(str(RAIZ / a["glob"]), recursive=True)):
                for n, ln in enumerate(pathlib.Path(f).read_text(
                        encoding="utf-8", errors="replace").splitlines(), 1):
                    if rx.search(ln):
                        rel = pathlib.Path(f).relative_to(RAIZ)
                        raise Falha(f"a ausencia que sustenta «{c['estado']}» quebrou: "
                                    f"{rel}:{n} «{ln.strip()[:80]}» -- {a['porque']}")
        provas = len(c.get("prova", [])) + len(c.get("ausencia", []))
        return c["estado"], (f"ancora declarada no lexico ({c['motivo']}); "
                             f"{provas} prova(s) conferida(s) no fonte agora")
    raise Falha(f"tipo de celula desconhecido: {tipo}")


# ------------------------------------------------------------ o julgamento


def _contexto(texto, ini, fim, folga=70):
    a, b = max(0, ini - folga), min(len(texto), fim + folga)
    return (("…" if a else "") + texto[a:ini] + "[[" + texto[ini:fim] + "]]"
            + texto[fim:b] + ("…" if b < len(texto) else ""))


def julgar(lexico, textos, celulas):
    """Varre os textos e devolve (achados, aceitos, problemas).

    `textos`: {rotulo: texto_cru}; `celulas`: {chave: (valor, origem)}. Separado
    da leitura de disco para o autoteste poder repor uma contradicao antiga
    num texto em memoria, sem tocar no arquivo versionado."""
    achados, aceitos, problemas = [], [], []
    planos = {}
    for rot, cru in textos.items():
        planos[rot] = normalizar(cru, rot.endswith(".html"))

    for cap in lexico["capacidades"]:
        chave = cap["chave"]
        if chave not in celulas:
            continue  # celula que falhou ja virou problema
        valor, origem = celulas[chave]
        eh_numero = cap["celula"]["tipo"] == "numero"
        lista_de = "numero" if eh_numero else valor
        sinais = cap["contradiz"].get(lista_de)
        if not sinais:
            problemas.append(
                f"[{chave}] a celula esta em «{valor}» e o lexico nao tem frase "
                f"nenhuma para esse estado -- escreva-as em {LEXICO.name}; "
                "sem elas a catraca nao confere esta capacidade")
            continue

        # as excecoes historicas desta capacidade, ja normalizadas
        hist = []
        for h in cap.get("historico", []):
            trecho, _ = normalizar(h["trecho"], False)
            hist.append((h, trecho.lower(), []))

        for rot, (plano, lns) in planos.items():
            baixo = plano.lower()
            for s in sinais:
                for m in re.finditer(s["rx"], plano, re.I):
                    if eh_numero:
                        bruto = m.group("n").lower()
                        n = POR_EXTENSO.get(bruto)
                        n = int(bruto) if n is None else n
                        if n == valor:
                            continue
                    achado = {
                        "arquivo": rot, "linha": lns[m.start()], "chave": chave,
                        "celula": valor, "origem": origem, "porque": s["porque"],
                        "frase": _contexto(plano, m.start(), m.end()),
                    }
                    coberto = None
                    for h, trecho, usos in hist:
                        if h["arquivo"] not in (rot, DOSSIE if rot.startswith("dossie-") else None):
                            continue
                        i = baixo.find(trecho)
                        while i >= 0:
                            if i <= m.start() and m.end() <= i + len(trecho):
                                coberto = h
                                usos.append(1)
                                break
                            i = baixo.find(trecho, i + 1)
                        if coberto:
                            break
                    if coberto:
                        achado["motivo_aceito"] = coberto["motivo"]
                        aceitos.append(achado)
                    else:
                        achados.append(achado)
        for h, trecho, usos in hist:
            if not usos:
                problemas.append(
                    f"[{chave}] excecao historica que nao casa mais nada em "
                    f"{h['arquivo']}: «{h['trecho']}» -- tire-a do lexico; "
                    "excecao orfa e porta aberta para a proxima frase")
    return achados, aceitos, problemas


def ler_celulas(lexico):
    celulas, problemas = {}, []
    for cap in lexico["capacidades"]:
        try:
            celulas[cap["chave"]] = celula_de(cap)
        except Falha as e:
            problemas.append(f"[{cap['chave']}] {e}")
    return celulas, problemas


def ler_textos(lexico):
    textos = {}
    for a in lexico["arquivos"]:
        caminho = achar_o_dossie() if a == DOSSIE else RAIZ / a
        rot = caminho.name if a == DOSSIE else a
        textos[rot] = caminho.read_text(encoding="utf-8")
    return textos


def relatorio(achados, aceitos, problemas, celulas):
    for a in aceitos:
        print(f"  aceito  {a['arquivo']}:{a['linha']}  [{a['chave']}] historico: "
              f"{a['motivo_aceito']}")
    for a in achados:
        print(f"CONTRADIZ {a['arquivo']}:{a['linha']}  [{a['chave']}]")
        print(f"    frase : «{a['frase']}»")
        print(f"    celula: {a['celula']}  ({a['origem']})")
        print(f"    porque: {a['porque']}")
    for p in problemas:
        print(f"SEM JULGAMENTO {p}")
    print()
    if achados or problemas:
        print(f"VERMELHO: {len(achados)} contradicao(oes) prosa x celula, "
              f"{len(problemas)} celula(s)/lexico sem julgamento; "
              f"{len(celulas)} capacidade(s) conferida(s).")
        return 1
    print(f"VERDE: {len(celulas)} capacidade(s) conferida(s), nenhuma frase "
          f"contradiz a celula ({len(aceitos)} citacao(oes) historica(s) aceita(s)).")
    return 0


# ------------------------------------------------------------ prova real


# As frases ANTIGAS, copiadas do commit 2c77e6fe (o que as tirou), uma por
# contradicao que o lexico cobre. Repor cada uma tem de DERRUBAR a catraca.
FRASES_ANTIGAS = [
    ("integridade_referencial_imposta",
     "<p>não descuido: ela é declarada e nunca imposta na gravação</p>"),
    ("sql_alem_do_select_simples",
     "<p>A operação <code>sql</code> traduz um <code>SELECT</code> simples para as</p>"),
    ("traducao_da_interface_pct",
     '<span class="t">4 — A interface está 41% traduzida</span>'),
    ("compactacao_do_reg",
     "<p>na ordem em que foram digitados. O espaço volta com compactação explícita — e a</p>"),
    ("pedidos_parciais",
     "<p>Das que faltam, as quatro parciais são as que pesam.</p>"),
]


def autoteste():
    lexico = json.loads(LEXICO.read_text(encoding="utf-8"))
    celulas, problemas = ler_celulas(lexico)
    textos = ler_textos(lexico)
    falhas = 0

    # GREEN: os arquivos de hoje passam.
    ach, _, prob = julgar(lexico, textos, celulas)
    ok = not ach and not prob and not problemas
    print(f"[{'ok' if ok else 'FALHOU'}] GREEN: os tres arquivos de hoje passam "
          f"({len(ach)} achado(s), {len(prob) + len(problemas)} problema(s))")
    falhas += not ok

    # RED: cada frase antiga reposta no meio do dossie derruba a catraca, na
    # linha certa e na capacidade certa.
    rot = next(r for r in textos if r.endswith(".html"))
    linhas = textos[rot].split("\n")
    meio = len(linhas) // 2
    for chave, frase in FRASES_ANTIGAS:
        mut = dict(textos)
        mut[rot] = "\n".join(linhas[:meio] + [frase] + linhas[meio:])
        ach, _, _ = julgar(lexico, mut, celulas)
        pegou = [a for a in ach if a["chave"] == chave and a["linha"] == meio + 1]
        ok = bool(pegou)
        print(f"[{'ok' if ok else 'FALHOU'}] RED: «{frase[:60]}…» reposta em "
              f"{rot}:{meio + 1} -> "
              + (f"CONTRADIZ [{chave}] celula={pegou[0]['celula']}" if ok
                 else "a catraca NAO caiu"))
        falhas += not ok

    # RED pelo outro lado: a CELULA muda e a prosa de hoje passa a mentir. Se
    # a compactacao virasse «implementado», o «foi recusado» de hoje tem de cair.
    cel = dict(celulas)
    cel["compactacao_do_reg"] = ("implementado", "celula forjada pelo autoteste")
    ach, _, _ = julgar(lexico, textos, cel)
    pegou = [a for a in ach if a["chave"] == "compactacao_do_reg"]
    ok = bool(pegou)
    print(f"[{'ok' if ok else 'FALHOU'}] RED: celula da compactacao forjada em "
          f"«implementado» -> {len(pegou)} frase(s) de hoje passam a contradizer"
          + (f" (ex.: {pegou[0]['arquivo']}:{pegou[0]['linha']})" if ok else ""))
    falhas += not ok

    # RED do lexico calado: estado sem lista reprova, nao passa.
    cel = dict(celulas)
    cel["le_as_proprias_escritas"] = ("parcial", "celula forjada pelo autoteste")
    _, _, prob = julgar(lexico, textos, cel)
    ok = any("le_as_proprias_escritas" in p for p in prob)
    print(f"[{'ok' if ok else 'FALHOU'}] RED: celula num estado sem frases no "
          "lexico -> " + ("SEM JULGAMENTO (reprova)" if ok else "passou calada"))
    falhas += not ok

    # RED da excecao historica: tirada, a citacao velha volta a contradizer.
    lex2 = json.loads(LEXICO.read_text(encoding="utf-8"))
    for cap in lex2["capacidades"]:
        cap.pop("historico", None)
    ach, _, _ = julgar(lex2, textos, celulas)
    ok = any(a["chave"] == "integridade_referencial_imposta" for a in ach)
    print(f"[{'ok' if ok else 'FALHOU'}] RED: sem a excecao historica, a citacao "
          "«a chave estrangeira é catálogo» volta a reprovar")
    falhas += not ok

    print()
    print("AUTOTESTE " + ("VERDE" if not falhas else f"VERMELHO ({falhas} falha(s))"))
    return 1 if falhas else 0


def main():
    args = sys.argv[1:]
    if "--autoteste" in args:
        return autoteste()
    lexico = json.loads(LEXICO.read_text(encoding="utf-8"))
    celulas, problemas = ler_celulas(lexico)
    if "--lista" in args:
        for cap in lexico["capacidades"]:
            v = celulas.get(cap["chave"])
            print(f"  {cap['chave']:34} {v[0] if v else 'SEM CELULA'}"
                  f"  ({v[1] if v else ''})")
        for p in problemas:
            print(f"SEM JULGAMENTO {p}")
        return 1 if problemas else 0
    # `--catraca` (o modo que o bancada/catracas/todas.py acha por varredura)
    # e o julgamento padrao: mesmo motor, nunca um segundo caminho.
    if "--catraca" in sys.argv[1:]:
        print("modo --catraca (bancada/catracas/todas.py)")
    print("CATRACA PROSA x CELULA -- a prosa contradiz o estado medido?")
    textos = ler_textos(lexico)
    print("arquivos: " + ", ".join(textos) + "\n")
    achados, aceitos, prob = julgar(lexico, textos, celulas)
    return relatorio(achados, aceitos, problemas + prob, celulas)


if __name__ == "__main__":
    sys.exit(main())
