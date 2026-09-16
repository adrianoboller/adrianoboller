#!/usr/bin/env python3
"""ANTES x DEPOIS -- a secao que o pedido 266 fez nascer, e a serie que a move.

    python3 docs/status/serie-historica.py [pagina.html]   # so esta secao
    python3 docs/status/serie-historica.py --semear        # a medicao de agora
    ./status-html.sh                                       # a pagina inteira

# O buraco que ele fecha

O `CAPABILITIES.json` e **sobrescrito** a cada medicao: ele guarda o AGORA e
mais nada. Sem o retrato de ontem gravado, «antes» sairia da memoria de quem
escreve -- que e exatamente o que a lei do gerador existe para substituir.

Entao nasce `docs/status/serie.jsonl`, **versionado**, uma linha por medicao.

# As tres regras da serie, e o erro que cada uma evita

1. **A data e a da MEDICAO, nunca a do commit.** O `medido_em` vem do proprio
   `CAPABILITIES.json`, que o `numeros-do-projeto.py` carimba na hora em que
   `cargo test` terminou. Juntar corridas de dias diferentes sem dizer quando e
   o erro que a pagina dos testes ja paga com a marca `(mtime)`.

2. **A linha entra pelo MESMO passo que grava o `CAPABILITIES.json`.** O
   `escrever_capacidades()` do `numeros-do-projeto.py` chama o `acrescentar()`
   daqui. Um segundo caminho que tambem soubesse montar a linha divergiria do
   primeiro na primeira mexida, e ai a serie contaria uma historia que o
   `CAPABILITIES.json` nao conta.

3. **Ninguem inventa o passado.** A serie nasce com UMA linha -- a medicao de
   hoje. Reconstrui-la do `git log` seria fabricar o retrato de ontem a partir
   do que estava no repositorio, e o `CAPABILITIES.json` de ontem nao foi
   guardado: o numero de testes de um commit antigo so se sabe rodando a suite
   naquele commit. Enquanto houver uma linha so, a secao **diz que nao consegue
   desenhar antes x depois** em vez de fingir que consegue -- secao que mente e
   pior que secao ausente.
"""

import importlib.util
import json
import pathlib
import sys

AQUI = pathlib.Path(__file__).resolve().parent
RAIZ = AQUI.parent.parent
PADRAO = AQUI / "status-do-projeto.html"
SERIE = AQUI / "serie.jsonl"

# A secao e escrita DENTRO da pagina, entre estas marcas -- o mesmo desenho do
# `riscos.py` (pedido 264) e do `telemetria-medida.py` (265).
MARCA_INICIO = "<!-- serie:inicio -->"
MARCA_FIM = "<!-- serie:fim -->"

# O que a serie guarda de cada medicao, e o rotulo de cada um.
#
# A lista e daqui porque e' a serie que decide o que ela guarda -- e nao a
# pagina, que so desenha. Campo novo no `CAPABILITIES.json` entra aqui de
# proposito, num passo pensado: serie que muda de colunas sozinha nao compara
# nada com as linhas de tras.
CAMPOS = [
    ("testes", "testes verdes", ""),
    ("operacoes", "operações no catálogo", ""),
    ("crates", "crates", ""),
    ("linhas_rust", "linhas de Rust", ""),
    ("linhas_doc", "linhas de documentação", ""),
    ("dependencias_externas", "dependências externas", ""),
    ("idiomas_fabrica", "textos pela fábrica de idiomas", ""),
    ("idiomas_fora", "textos ainda cravados no fonte", ""),
]


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


# --------------------------------------------------------------- a serie

def da_capacidade(cap: dict) -> dict:
    """A linha da serie, montada do dicionario do `CAPABILITIES.json`.

    UMA montagem so, e ela mora aqui: quem grava o `CAPABILITIES.json` chama
    esta funcao em vez de repetir o formato, para que os dois nunca contem
    historias diferentes.
    """
    idi = cap.get("idiomas") or {}
    linha = {
        "medido_em": cap.get("medido_em"),
        "versao": cap.get("versao"),
        "commit": (cap.get("commit") or "")[:12],
        "sujo": bool(cap.get("sujo")),
        "branch": cap.get("branch"),
    }
    for chave in ("testes", "operacoes", "crates", "linhas_rust", "linhas_doc",
                  "dependencias_externas"):
        linha[chave] = cap.get(chave)
    linha["idiomas_fabrica"] = idi.get("fabrica")
    linha["idiomas_fora"] = idi.get("fora")
    linha["idiomas_teto"] = idi.get("teto")
    return linha


def _curto(caminho: pathlib.Path) -> str:
    """O caminho relativo a RAIZ -- ou o absoluto, quando ele nao mora la.

    `relative_to` LEVANTA fora da raiz, e levantar de dentro da montagem de uma
    mensagem de erro troca o diagnostico por outro: a prova dos dois sentidos
    pegou isto passando um arquivo de `/tmp`, e o «linha 2 quebrada» virou um
    `ValueError` sobre subpath que nao dizia nada sobre a serie.
    """
    try:
        return str(caminho.relative_to(RAIZ))
    except ValueError:
        return str(caminho)


def ler(caminho: pathlib.Path = SERIE) -> list:
    """As medicoes, na ordem em que foram gravadas.

    Linha quebrada PARA a leitura nomeando o numero dela: uma serie que pula a
    linha corrompida em silencio publica um «antes» que nao e o antes.
    """
    if not caminho.exists():
        return []
    fora = []
    for n, linha in enumerate(caminho.read_text(encoding="utf-8").splitlines(), 1):
        linha = linha.strip()
        if not linha:
            continue
        try:
            fora.append(json.loads(linha))
        except json.JSONDecodeError as e:
            raise SystemExit(
                f"{_curto(caminho)}, linha {n}: {e}. A serie e' o "
                "unico registro do que ja foi medido -- pular a linha quebrada "
                "em silencio publicaria um «antes» que nao e' o antes.")
    return fora


def acrescentar(cap: dict, caminho: pathlib.Path = SERIE) -> bool:
    """Acrescenta a medicao do `cap` a serie. Devolve se acrescentou.

    Recusa a repetida -- mesma `medido_em` da ultima linha -- porque re-rodar
    o gerador sem re-medir nao e' uma medicao nova: seriam duas linhas dizendo
    que o mesmo instante aconteceu duas vezes, e a contagem de medicoes da
    pagina passaria a contar corridas do script em vez de medicoes.
    """
    linha = da_capacidade(cap)
    if not linha.get("medido_em"):
        raise SystemExit(
            "o CAPABILITIES.json nao traz `medido_em` -- a serie guarda a data "
            "da MEDICAO, e sem ela a linha nao entra. Data que falta nao vira "
            "a data de hoje.")
    ja = ler(caminho)
    if ja and ja[-1].get("medido_em") == linha["medido_em"]:
        return False
    caminho.parent.mkdir(parents=True, exist_ok=True)
    with caminho.open("a", encoding="utf-8") as f:
        f.write(json.dumps(linha, ensure_ascii=False, sort_keys=True) + "\n")
    return True


def contexto():
    return {"serie": ler()}


# ------------------------------------------------------------------- a secao

def _sem_antes(P, medicoes):
    """O que a secao diz quando ainda nao ha com que comparar.

    Nao e' uma desculpa: e' o resultado. Com uma medicao so, «antes x depois»
    nao existe, e desenhar uma variacao de 0% ao lado de cada numero daria a
    impressao de que o projeto nao mudou -- que e' uma afirmacao, e falsa.
    """
    quantas = len(medicoes)
    if quantas == 0:
        return ('<div class="ausente-bloco"><b>A série está vazia.</b> Nenhuma '
                'medição foi gravada ainda. A linha entra sozinha na próxima '
                'corrida de <code>docs/dossie/numeros-do-projeto.py</code>, '
                'que é quem grava o <code>CAPABILITIES.json</code>.</div>')
    return ('<div class="nota v"><b>Ainda não dá para desenhar «antes × '
            'depois», e esta caixa diz isso em vez de fingir.</b> A série tem '
            '<b>uma</b> medição — a de hoje. O «antes» só existe a partir da '
            'segunda, e reconstruí-lo do <code>git log</code> seria fabricar o '
            'retrato de ontem: o <code>CAPABILITIES.json</code> é sobrescrito, '
            'e o número de testes de um commit antigo só se sabe rodando a '
            'suíte naquele commit. A próxima corrida de '
            '<code>docs/dossie/numeros-do-projeto.py</code> acrescenta a '
            'segunda linha, e esta seção passa a comparar sozinha.</div>')


def _delta(P, antes, depois):
    """A variacao de um campo, com o sinal -- ou «—» quando falta um dos dois.

    Subir nao e' sempre bom nem descer e' sempre ruim (textos cravados no fonte
    sobem quando a tela cresce), entao a etiqueta carrega o SINAL e a forma, e
    nao um juizo: quem le decide o que a seta quer dizer naquela linha.
    """
    if antes is None or depois is None:
        return '<span class="mtime">—</span>'
    d = depois - antes
    if d == 0:
        return '<span class="mtime">sem mudança</span>'
    sinal = "+" if d > 0 else "−"
    classe = "e-tem" if d > 0 else "e-nao"
    return ('<span class="etiqueta ' + classe + '">' + sinal
            + P.milhar(abs(d)) + "</span>")


def secao(P, ctx):
    medicoes = ctx["serie"]
    linhas = [P.h2("serie", "Antes × Depois — a série medida"),
              '<p class="sub">O <code>CAPABILITIES.json</code> é sobrescrito a '
              'cada medição e só guarda o <b>agora</b>. Esta seção sai de '
              '<code>docs/status/serie.jsonl</code>, versionado, com uma linha '
              'por medição e a data em que ela foi <b>medida</b> — nunca a do '
              'commit.</p>',
              '<div class="kpis">',
              P.kpi(P.milhar(len(medicoes)), "medições na série",
                    medicoes[-1]["medido_em"][:10] if medicoes else "—"),
              ]
    if len(medicoes) >= 2:
        a, b = medicoes[0], medicoes[-1]
        linhas.append(P.kpi(P.esc(a["medido_em"][:10]) + " → "
                            + P.esc(b["medido_em"][:10]),
                            "primeira e última medição", "", curto=True))
    linhas.append("</div>")

    if len(medicoes) < 2:
        linhas.append(_sem_antes(P, medicoes))
        if medicoes:
            a = medicoes[-1]
            linhas.append('<div class="rolo"><table><thead><tr><th>o que se '
                          'mede</th><th class="num">medido em '
                          + P.esc(a["medido_em"][:19])
                          + '</th></tr></thead><tbody>')
            for chave, rotulo, _u in CAMPOS:
                v = a.get(chave)
                linhas.append('<tr><td>' + rotulo + '</td><td class="num">'
                              + (P.milhar(v) if isinstance(v, int)
                                 else '<span class="mtime">—</span>')
                              + '</td></tr>')
            linhas.append("</tbody></table></div>")
    else:
        a, b = medicoes[0], medicoes[-1]
        linhas.append('<div class="rolo"><table><thead><tr><th>o que se mede'
                      '</th><th class="num">antes — '
                      + P.esc(a["medido_em"][:19])
                      + '</th><th class="num">depois — '
                      + P.esc(b["medido_em"][:19])
                      + '</th><th class="num">variação</th></tr></thead><tbody>')
        for chave, rotulo, _u in CAMPOS:
            va, vb = a.get(chave), b.get(chave)
            linhas.append(
                '<tr><td>' + rotulo + '</td>'
                '<td class="num">' + (P.milhar(va) if isinstance(va, int)
                                      else '<span class="mtime">—</span>')
                + '</td><td class="num">'
                + (P.milhar(vb) if isinstance(vb, int)
                   else '<span class="mtime">—</span>')
                + '</td><td class="num">' + _delta(P, va, vb) + '</td></tr>')
        linhas.append("</tbody></table></div>")

    if medicoes:
        linhas.append("<h3>As medições, uma por linha</h3>")
        linhas.append('<div class="rolo"><table><thead><tr>'
                      '<th>medida em</th><th>versão</th><th>commit</th>'
                      '<th class="num">testes</th><th class="num">linhas Rust'
                      '</th></tr></thead><tbody>')
        for m in medicoes[-10:]:
            sujo = " · árvore suja" if m.get("sujo") else ""
            linhas.append(
                '<tr><td class="mono">' + P.esc(str(m.get("medido_em"))[:19])
                + '</td><td>' + P.esc(str(m.get("versao", "—"))) + sujo
                + '</td><td class="mono">' + P.esc(str(m.get("commit", "—")))
                + '</td><td class="num">'
                + (P.milhar(m["testes"]) if isinstance(m.get("testes"), int)
                   else "—")
                + '</td><td class="num">'
                + (P.milhar(m["linhas_rust"])
                   if isinstance(m.get("linhas_rust"), int) else "—")
                + '</td></tr>')
        linhas.append("</tbody></table></div>")

    linhas.append(
        '<div class="nota"><b>A linha entra pelo mesmo passo que grava o '
        '<code>CAPABILITIES.json</code>.</b> O '
        '<code>escrever_capacidades()</code> de '
        '<code>docs/dossie/numeros-do-projeto.py</code> chama o '
        '<code>acrescentar()</code> daqui — não há um segundo caminho que '
        'possa divergir. E a série <b>recusa a repetida</b>: re-rodar o '
        'gerador sem re-medir não acrescenta linha, senão a contagem passaria '
        'a contar corridas do script em vez de medições.</div>')

    linhas.append(P.fonte(
        '<code>docs/status/serie.jsonl</code>, versionado, escrito por '
        '<code>acrescentar()</code> de '
        '<code>docs/status/serie-historica.py</code> no mesmo passo em que o '
        '<code>docs/dossie/numeros-do-projeto.py</code> grava o '
        '<code>CAPABILITIES.json</code>. A data de cada linha é a da '
        '<b>medição</b>, carimbada quando o <code>cargo test</code> terminou.'))
    return "\n".join(linhas)


def bloco(P, ctx):
    """O texto inteiro entre as marcas -- o mesmo que a pagina monta."""
    return MARCA_INICIO + "\n" + secao(P, ctx) + "\n" + MARCA_FIM


def semear():
    """Acrescenta a medicao que JA esta no `CAPABILITIES.json`, pelo mesmo
    `acrescentar()`.

    Existe para a PRIMEIRA linha, e para a rodada em que alguem mediu antes de
    este gerador existir. Nao e' um segundo caminho: e' o mesmo
    `da_capacidade()` recebendo o dicionario lido do arquivo em vez do
    dicionario recem-montado. O que ele NAO faz e' inventar: a data que entra e
    o `medido_em` que o arquivo carrega, e a linha repetida e' recusada.
    """
    alvo = RAIZ / "CAPABILITIES.json"
    if not alvo.exists():
        raise SystemExit(
            "CAPABILITIES.json nao existe -- nao ha medicao para semear. "
            "Rode: flock /tmp/phx-cargo.lock python3 "
            "docs/dossie/numeros-do-projeto.py")
    cap = json.loads(alvo.read_text(encoding="utf-8"))
    if acrescentar(cap):
        print(f"semeada a medicao de {cap.get('medido_em')} em "
              f"{_curto(SERIE)}")
    else:
        print(f"a medicao de {cap.get('medido_em')} JA e a ultima linha de "
              f"{_curto(SERIE)} -- nada a fazer. Re-rodar o gerador "
              "sem re-medir nao e uma medicao nova.")
    return 0


def main():
    if "--semear" in sys.argv:
        return semear()
    alvo = pathlib.Path(sys.argv[1]) if len(sys.argv) > 1 else PADRAO
    if not alvo.exists():
        raise SystemExit(
            f"{alvo} nao existe. Este gerador escreve a secao DENTRO da setima "
            "pagina; a pagina inteira sai de `./status-html.sh`.")
    # Importado aqui, e nao no topo, para nao haver ciclo: a pagina importa
    # ESTE modulo.
    P = importar(AQUI / "pagina-do-status-do-projeto.py", "sh_pagina")
    ctx = contexto()
    texto = alvo.read_text(encoding="utf-8")
    i, f = texto.find(MARCA_INICIO), texto.find(MARCA_FIM)
    if i < 0 or f < 0:
        raise SystemExit(
            f"{alvo.name} nao tem as marcas {MARCA_INICIO}/{MARCA_FIM}. A "
            "pagina foi gerada por uma versao que ainda nao conhecia esta "
            "secao -- rode `./status-html.sh` uma vez.")
    novo = texto[:i] + bloco(P, ctx) + texto[f + len(MARCA_FIM):]
    if novo == texto:
        print(f"a secao ja estava em dia em {_curto(alvo)}")
    else:
        alvo.write_text(novo, encoding="utf-8")
        print(f"secao escrita em {_curto(alvo)}")

    # Gerador que faz menos do que o nome promete TEM de dizer que fez menos.
    medicoes = ctx["serie"]
    print(f"  {len(medicoes)} medicao(oes) em {_curto(SERIE)}")
    if len(medicoes) < 2:
        print("  NAO da para desenhar antes x depois com menos de duas "
              "medicoes -- a secao diz isso, e nao inventa o passado do "
              "git log. A proxima corrida de docs/dossie/numeros-do-projeto.py "
              "acrescenta a segunda linha.")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
