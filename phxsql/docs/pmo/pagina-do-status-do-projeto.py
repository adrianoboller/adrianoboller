#!/usr/bin/env python3
"""Gera o PAINEL PMO do PhxSql -- uma pagina, tres vistas.

    python3 docs/pmo/pagina-do-status-do-projeto.py [saida.html]

O dono pediu «um status assim» mostrando tres slides de outro projeto dele: um
fluxograma do trabalho, um organograma e um painel de contadores. Esta pagina e
o equivalente na nossa casa -- com a marca do PhxSql e, principalmente, com
TODO numero visivel saindo de um gerador. Numero digitado a mao envelhece
calado, e nesta casa ja envelheceu quatro vezes.

## De onde sai cada numero (nenhum se digita)

- **pedidos** (total/feitos/parciais/planejados): `docs/PENDENCIAS.md`, lido
  pelo modulo irmao `docs/dossie/pagina-dos-pedidos.py` -- a funcao `ler()` e o
  dicionario `ESTADOS`. Reescrever o parser aqui seria uma segunda receita do
  mesmo numero, e receita duplicada e receita que diverge.
- **board por pilar e escalao**: `docs/pmo/BACKLOG.md`, pelo `rollup.py`.
- **motor**: `CAPABILITIES.json`, cada numero com a data `medido_em` ao lado --
  a disciplina da pagina de testes: numero sem data e retrato que nunca
  existiu.
- **ultimas frentes**: `git log -n 8` (so leitura; este script nunca escreve no
  git).
- **gates externos**: os pedidos ainda abertos (◐/☐) cujo texto casa o
  `LEXICO_GATE` abaixo -- a pagina mostra a frase que casou, para ninguem ter
  de confiar no casador.
- **organograma**: os arquivos de `.claude/agents/` (frontmatter `name` e
  `description`), a tabela papel->agente do `README.md` da pasta e o escalao
  por papel do `docs/MODELOS.md`. O escalao aparece pelo NIVEL (forte/meio/
  leve) -- nome de modelo e proibido em artefato versionado. Papel sem escalao
  registrado aparece dizendo que nao tem, em vez de ganhar um inventado.

O fluxograma e conteudo FIXO de proposito: ele desenha o processo da casa, que
nao e medida. Vai em SVG a mao, como as figuras do dossie.

## Gerador que faz menos do que promete tem de dizer que fez menos

Fonte que falta e PARADA com o motivo, nunca um zero silencioso na pagina: foi
assim que um painel inteiro publicou numero velho anunciando sucesso. E o
arquivo de agente que nao achar caixa no organograma tambem PARA -- agente
invisivel e a mesma doenca do pedido que sumia da pagina por ter um simbolo
fora da legenda.

Rodar duas vezes sem mexer nas fontes da o MESMO arquivo, exceto o carimbo
«gerado em ... UTC» -- que e relogio de parede, e e o que o portao dos
geradores mascara no modo `sem-carimbo`.
"""

import datetime
import html
import importlib.util
import json
import math
import pathlib
import re
import subprocess
import sys

RAIZ = pathlib.Path(__file__).resolve().parents[2]      # docs/pmo -> docs -> phxsql
PADRAO = RAIZ / "docs" / "pmo" / "status-do-projeto.html"
CAPACIDADES = RAIZ / "CAPABILITIES.json"
MODELOS = RAIZ / "docs" / "MODELOS.md"

QUANTAS_FRENTES = 8

# As expressoes que marcam um item PARADO por gente, nao por engenharia. Lista
# EXPLICITA e nomeada: casador que "parece reconhecer tudo" vira um filtro que
# ninguem consegue conferir. A pagina mostra a frase que casou justamente para
# que o leitor julgue o casamento em vez de acreditar nele.
LEXICO_GATE = (
    "decisão do dono",
    "decisao do dono",
    "decisão fica com o dono",
    "decisao fica com o dono",
    "parado por decisão",
    "parado por decisao",
    "parada por decisão",
    "parada por decisao",
    "bloqueio externo",
    "só com o dono",
    "so com o dono",
    "o dono decide",
    "a escolha é do dono",
    "a escolha e do dono",
    "depende de você",
    "depende de voce",
)

# O processo da casa, caixa a caixa. E conteudo fixo -- processo nao e medida --
# e por isso mora aqui em cima, onde se le e se corrige, e nao espalhado pelo
# desenhista do SVG.
FLUXO = [
    ("pedido do dono", ["a fonte de tudo"]),
    ("pendência numerada", ["docs/PENDENCIAS.md · ☑️ ◐ ☐"]),
    ("board: pilar + escalão", ["docs/pmo/BACKLOG.md · rollup.py"]),
    ("medir a premissa", ["antes de implementar o item"]),
    ("frente / agente do papel", [".claude/agents/"]),
    ("prova real", ["FALHA com o defeito reposto,", "passa com o conserto"]),
    ("portões", ["fmt · clippy zero avisos · suíte verde"]),
    ("integração", ["o defeito que só aparece", "no encontro das frentes"]),
    ("geradores + portão dos geradores", ["re-rodar mudaria algum número visível?"]),
    ("commit · push · backup", ["só o integrador comita"]),
    ("páginas publicadas", ["dossiê · pedidos · testes · gráficos", "status · PMO"]),
]
# Onde o losango entra: depois de «medir a premissa», antes da frente.
FLUXO_DECISAO = 4
PERGUNTA_DECISAO = ("decisão do dono", "ou bloqueio externo?")
CAIXA_PARADO = ("PARADO", "entra na lista de gates —", "não some da página")

NIVEIS = ("forte", "meio", "leve")
ORDEM_PAPEL = ("A", "B", "C", "D", "E", "F", "G", "H", "I", "J", "SEC")


# ------------------------------------------------------------------ fontes

def importar(caminho: pathlib.Path, apelido: str):
    """Importa um modulo irmao cujo NOME TEM HIFEN (`import` nao o alcanca)."""
    if not caminho.exists():
        raise SystemExit(
            f"falta o modulo irmao {caminho.relative_to(RAIZ)} -- esta pagina "
            "nao reescreve o parser dele; sem ele nao ha numero, e numero que "
            "falta nao vira zero.")
    spec = importlib.util.spec_from_file_location(apelido, caminho)
    mod = importlib.util.module_from_spec(spec)
    sys.modules[apelido] = mod
    spec.loader.exec_module(mod)
    return mod


def ler_capacidades():
    if not CAPACIDADES.exists():
        raise SystemExit(
            "falta CAPABILITIES.json -- os numeros do motor sairiam em branco. "
            "Rode: flock /tmp/phx-cargo.lock python3 docs/dossie/numeros-do-projeto.py")
    c = json.loads(CAPACIDADES.read_text(encoding="utf-8"))
    faltam = [k for k in ("versao", "commit", "branch", "testes", "operacoes",
                          "linhas_rust", "linhas_doc", "crates",
                          "dependencias_externas", "idiomas", "medido_em")
              if k not in c]
    if faltam:
        raise SystemExit(
            "CAPABILITIES.json nao traz " + ", ".join(faltam)
            + " -- campo que falta nao vira zero na pagina.")
    return c


def ler_frentes():
    """As ultimas frentes, do proprio `git log`. So leitura."""
    try:
        r = subprocess.run(
            ["git", "-C", str(RAIZ), "log", "--no-color",
             "--format=%h\t%ad\t%s", "--date=format:%d/%m/%Y %H:%M",
             "-n", str(QUANTAS_FRENTES)],
            capture_output=True, text=True, check=False)
    except FileNotFoundError:
        raise SystemExit("nao achei o `git` no caminho -- as ultimas frentes "
                         "sairiam vazias, e lista vazia mente.")
    if r.returncode != 0:
        raise SystemExit("o `git log` falhou: " + (r.stderr or "").strip())
    frentes = []
    for l in r.stdout.split("\n"):
        if not l.strip():
            continue
        h, data, assunto = l.split("\t", 2)
        frentes.append({"hash": h, "data": data, "assunto": assunto})
    if not frentes:
        raise SystemExit("o `git log` nao devolveu commit nenhum.")
    return frentes


def raiz_do_repositorio() -> pathlib.Path:
    """Onde mora o `.claude/agents` -- pelo git, nao por um `..` chutado."""
    r = subprocess.run(["git", "-C", str(RAIZ), "rev-parse", "--show-toplevel"],
                       capture_output=True, text=True, check=False)
    if r.returncode != 0:
        raise SystemExit("nao consegui a raiz do repositorio pelo git: "
                         + (r.stderr or "").strip())
    return pathlib.Path(r.stdout.strip())


# ------------------------------------------------------------------ marcacao

def esc(t):
    return html.escape(str(t))


def marcar(t):
    """O pouco de Markdown das celulas, virando HTML. Escapa ANTES."""
    t = html.escape(t)
    t = re.sub(r"`([^`]+)`", r"<code>\1</code>", t)
    t = re.sub(r"\*\*([^*]+)\*\*", r"<strong>\1</strong>", t)
    t = re.sub(r"(?<!\*)\*([^*]+)\*(?!\*)", r"<em>\1</em>", t)
    return t


def texto_puro(h):
    """O texto de volta, sem as tags que o `marcar` pos. O lexico procura na
    frase que o leitor VE, nao na marcacao que ele nao ve."""
    return html.unescape(re.sub(r"<[^>]+>", " ", h)).replace("  ", " ").strip()


def num(v):
    if isinstance(v, bool):
        return "sim" if v else "não"
    if isinstance(v, int):
        return f"{v:,}".replace(",", ".")
    return str(v)


def plural(rotulo):
    """«Parcial» -> «parciais»; «Feito» -> «feitos». So para a LINHA DO SCRIPT --
    o rotulo da pagina sai do modulo dos pedidos, sem passar por aqui."""
    r = rotulo.lower()
    return r[:-1] + "is" if r.endswith("al") else r + "s"


def porcento(n, total):
    return f"{100.0 * n / total:.1f}".replace(".", ",") if total else "0,0"


def data_br(iso):
    m = re.match(r"(\d{4})-(\d{2})-(\d{2})", str(iso))
    return f"{m.group(3)}/{m.group(2)}/{m.group(1)}" if m else str(iso)


# ------------------------------------------------------------------ gates

def achar_gates(itens):
    """Os pedidos ainda abertos que o lexico marca como parados por gente.

    Coleta TODAS as frases que casaram em cada pedido (filter, nao find): a
    primeira frase da lista nao e mais verdadeira que a quinta, e mostrar so
    uma esconderia o motivo real no dia em que alguem acrescentar uma.
    """
    achados = []
    for it in itens:
        if it["classe"] == "feito":
            continue
        cru = texto_puro(it["pedido"]) + " — " + texto_puro(it["estado"])
        baixo = cru.lower()
        frases = [f for f in LEXICO_GATE if f.lower() in baixo]
        if not frases:
            continue
        p = baixo.find(frases[0].lower())
        ini, fim = max(0, p - 90), min(len(cru), p + len(frases[0]) + 130)
        trecho = ("… " if ini > 0 else "") + cru[ini:fim] + (" …" if fim < len(cru) else "")
        achados.append({
            "n": it["n"], "classe": it["classe"], "rotulo": it["rotulo"],
            "pedido": it["pedido"], "frases": frases, "trecho": trecho,
        })
    return achados


# ------------------------------------------------------------------ equipe

CELULAS = re.compile(r"^\|(.+)\|\s*$")
PAPEL_NA_CELULA = re.compile(r"^\*{0,2}([A-J])\*{0,2}\s*[—-]\s*(.+)$")
NIVEL_EM_NEGRITO = re.compile(r"\*\*(" + "|".join(NIVEIS) + r")\*\*")


def linhas_de_tabela(texto):
    """Cada linha de tabela Markdown como lista de celulas, sem separadores."""
    for l in texto.split("\n"):
        m = CELULAS.match(l.strip())
        if not m:
            continue
        cels = [c.strip() for c in m.group(1).split("|")]
        if all(re.fullmatch(r":?-{2,}:?", c) for c in cels):
            continue
        yield cels


def ler_escaloes():
    """agente -> (papel, nivel, celula do escalao) do docs/MODELOS.md."""
    if not MODELOS.exists():
        raise SystemExit(
            f"falta {MODELOS.relative_to(RAIZ)} -- sem ele o organograma teria "
            "de INVENTAR o escalao de cada papel, e escalao inventado e pior "
            "que escalao ausente.")
    fora = {}
    for cels in linhas_de_tabela(MODELOS.read_text(encoding="utf-8")):
        if len(cels) < 4:
            continue
        m = re.fullmatch(r"`([a-z0-9\-]+)`", cels[0])
        if not m:
            continue
        papel = cels[1].strip("* ")
        nivel = NIVEL_EM_NEGRITO.search(cels[2])
        if not nivel:
            continue
        fora[m.group(1)] = {
            "papel": papel,
            "nivel": nivel.group(1),
            "detalhe": marcar(cels[2]),
            "porque": marcar(cels[3]),
        }
    if not fora:
        raise SystemExit(
            "docs/MODELOS.md nao traz nenhuma linha `agente | papel | escalao` "
            "-- a tabela mudou de forma; conserte a leitura em vez de publicar "
            "um organograma sem escalao nenhum.")
    return fora


def ler_frontmatter(p: pathlib.Path):
    t = p.read_text(encoding="utf-8")
    if not t.startswith("---"):
        raise SystemExit(f"{p}: sem frontmatter `---` no topo.")
    bloco = t.split("---", 2)[1]
    campos = {}
    for l in bloco.split("\n"):
        m = re.match(r"^(name|description|tools):\s*(.+?)\s*$", l)
        if m:
            campos[m.group(1)] = m.group(2)
    for k in ("name", "description"):
        if k not in campos:
            raise SystemExit(f"{p}: frontmatter sem `{k}:`.")
    if campos["name"] != p.stem:
        raise SystemExit(
            f"{p}: o frontmatter diz name: {campos['name']!r} e o arquivo se "
            f"chama {p.stem!r} -- o organograma casa os dois pelo nome, e um "
            "casamento torto publica a descricao de outro agente.")
    return campos


def ler_equipe():
    """As caixas do organograma: papel, agente, escalao, o que responde.

    Tres fontes que se conferem: os ARQUIVOS de `.claude/agents/` (quem existe),
    a tabela do `README.md` da pasta (que papel cada um ocupa, e de onde vem o
    papel que nao virou agente) e o `docs/MODELOS.md` (o nivel). Agente sem
    caixa PARA o gerador -- agente invisivel e a mesma doenca do pedido que
    sumia da pagina por um simbolo fora da legenda.
    """
    pasta = raiz_do_repositorio() / ".claude" / "agents"
    if not pasta.is_dir():
        raise SystemExit(
            f"falta a pasta {pasta} -- o organograma sairia so com as caixas "
            "que nao tem agente, dizendo que a casa tem menos gente do que tem.")
    leia_me = pasta / "README.md"
    if not leia_me.exists():
        raise SystemExit(f"falta {leia_me} -- e dele que sai a tabela papel->agente.")

    fichas = {}
    for p in sorted(pasta.glob("*.md")):
        if p.name == "README.md":
            continue
        fichas[p.stem] = ler_frontmatter(p)

    escaloes = ler_escaloes()
    caixas, reivindicados = [], set()

    for cels in linhas_de_tabela(leia_me.read_text(encoding="utf-8")):
        if len(cels) < 3:
            continue
        nomes = [n for n in re.findall(r"`([^`]+)`", cels[1]) if n in fichas]
        m = PAPEL_NA_CELULA.match(cels[0])
        if not nomes and not m:
            continue                      # cabecalho, ou linha que nao e de papel
        origem = marcar(cels[1]) if not nomes else ""
        for nome in (nomes or [None]):
            if nome:
                reivindicados.add(nome)
            papel = (m.group(1) if m
                     else escaloes.get(nome, {}).get("papel", ""))
            if not papel:
                raise SystemExit(
                    f"{leia_me}: o agente {nome!r} aparece numa linha que nao "
                    "diz o papel, e o docs/MODELOS.md tambem nao o registra -- "
                    "caixa sem papel nao se adivinha.")
            caixas.append({
                "papel": papel,
                "titulo": (m.group(2) if m else papel),
                "agente": nome or "",
                "origem": origem,
                "porque": marcar(cels[2]),
                "descricao": esc(fichas[nome]["description"]) if nome else "",
                "ferramentas": esc(fichas[nome].get("tools", "")) if nome else "",
                "nivel": escaloes.get(nome, {}).get("nivel", "") if nome else "",
                "escalao": escaloes.get(nome, {}).get("detalhe", "") if nome else "",
            })

    orfaos = sorted(set(fichas) - reivindicados)
    if orfaos:
        raise SystemExit(
            "agente sem caixa no organograma: " + ", ".join(orfaos)
            + f" -- o {leia_me.name} nao os poe em nenhuma linha. Agente "
            "invisivel some da pagina em silencio; acrescente a linha.")

    sem_arquivo = sorted(set(escaloes) - set(fichas))

    def chave(c):
        p = c["papel"]
        base = ORDEM_PAPEL.index(p) if p in ORDEM_PAPEL else len(ORDEM_PAPEL)
        return (base, c["agente"] or "", c["titulo"])

    principais = sorted([c for c in caixas if not c["papel"].endswith("-sub")], key=chave)
    subs = sorted([c for c in caixas if c["papel"].endswith("-sub")], key=chave)
    return principais, subs, sem_arquivo


# ------------------------------------------------------------------ desenho

def rosca(itens, estados):
    """A rosca dos tres estados, em SVG a mao.

    A FORMA carrega o estado, nao so a cor -- cheia (feito), hachurada
    (parcial), so contorno (planejado) --, porque ler sem cor e o piso.
    """
    total = len(itens)
    cx = cy = 116
    r_ext, r_int = 96, 60
    partes, angulo = [], 0.0
    for simbolo, (classe, rotulo) in estados.items():
        n = sum(1 for i in itens if i["classe"] == classe)
        if not n:
            continue
        varredura = 360.0 * n / total
        a0, a1 = angulo, angulo + varredura
        angulo = a1
        preenche = {
            "feito": "var(--feito)",
            "parcial": "url(#hachura-parcial)",
            "planejado": "none",
        }[classe]
        traco = {"feito": "var(--papel)", "parcial": "var(--parcial)",
                 "planejado": "var(--planejado)"}[classe]
        largura = {"feito": 1.5, "parcial": 1.5, "planejado": 2}[classe]
        titulo = f"{rotulo}: {num(n)} de {num(total)} ({porcento(n, total)}%)"
        if n == total:
            # Setor de 360 graus nao existe em `path`: vira anel inteiro.
            d = (f'<circle cx="{cx}" cy="{cy}" r="{(r_ext + r_int) / 2}" '
                 f'fill="none" stroke="{preenche if preenche != "none" else traco}" '
                 f'stroke-width="{r_ext - r_int}"/>')
        else:
            d = (f'<path d="{setor(cx, cy, r_ext, r_int, a0, a1)}" '
                 f'fill="{preenche}" stroke="{traco}" stroke-width="{largura}"/>')
        partes.append(f"<g><title>{esc(titulo)}</title>{d}</g>")
    return f"""<svg viewBox="0 0 232 232" role="img" width="232" height="232"
     aria-label="Rosca dos {num(total)} pedidos por estado" class="rosca">
  <defs>
    <pattern id="hachura-parcial" width="7" height="7" patternUnits="userSpaceOnUse" patternTransform="rotate(45)">
      <rect width="7" height="7" fill="var(--papel-2)"/>
      <rect width="3.4" height="7" fill="var(--parcial)"/>
    </pattern>
  </defs>
  {''.join(partes)}
  <text x="{cx}" y="{cy - 4}" text-anchor="middle" class="rosca-n">{num(total)}</text>
  <text x="{cx}" y="{cy + 18}" text-anchor="middle" class="rosca-r">pedidos</text>
</svg>"""


def setor(cx, cy, r_ext, r_int, a0, a1):
    """Um setor de anel: 0 grau no topo, sentido horario."""
    def ponto(r, a):
        t = math.radians(a - 90)
        return cx + r * math.cos(t), cy + r * math.sin(t)
    grande = 1 if (a1 - a0) > 180 else 0
    x0, y0 = ponto(r_ext, a0)
    x1, y1 = ponto(r_ext, a1)
    x2, y2 = ponto(r_int, a1)
    x3, y3 = ponto(r_int, a0)
    return (f"M{x0:.2f} {y0:.2f} A{r_ext} {r_ext} 0 {grande} 1 {x1:.2f} {y1:.2f} "
            f"L{x2:.2f} {y2:.2f} A{r_int} {r_int} 0 {grande} 0 {x3:.2f} {y3:.2f} Z")


def fluxograma():
    """O processo da casa em SVG a mao: caixas, um losango e duas saidas."""
    # A meia largura do losango encolheu de 140 para 130 e o ramo andou para a
    # direita: na captura o rotulo «sim» encostava na caixa do PARADO, porque o
    # vao entre a ponta do losango e o ramo era do tamanho da palavra.
    larg, cx = 520, 180
    caixa_l, caixa_x = 280, 40
    meia_l = 130
    ramo_x, ramo_l = 346, 166
    y = 16
    partes, ligacoes = [], []
    pos = []
    for i, (titulo, sub) in enumerate(FLUXO):
        if i == FLUXO_DECISAO:
            # O losango: meia largura `meia_l`, altura 112.
            ly, meia_h = y, 56
            centro = ly + meia_h
            partes.append(
                f'<path d="M{cx} {ly} L{cx + meia_l} {centro} L{cx} {ly + 2 * meia_h} '
                f'L{cx - meia_l} {centro} Z" fill="var(--papel-2)" stroke="currentColor" '
                f'stroke-width="1.4"/>'
                f'<text x="{cx}" y="{centro - 4}" text-anchor="middle" class="fx-t">'
                f'{esc(PERGUNTA_DECISAO[0])}</text>'
                f'<text x="{cx}" y="{centro + 13}" text-anchor="middle" class="fx-t">'
                f'{esc(PERGUNTA_DECISAO[1])}</text>')
            # A saida «sim», para o ramo parado.
            ry = centro - 27
            partes.append(
                f'<rect x="{ramo_x}" y="{ry}" width="{ramo_l}" height="54" rx="6" '
                f'fill="none" stroke="var(--parado)" stroke-width="1.6" '
                f'stroke-dasharray="5 3"/>'
                f'<text x="{ramo_x + ramo_l / 2}" y="{ry + 20}" text-anchor="middle" '
                f'class="fx-t fx-parado">{esc(CAIXA_PARADO[0])}</text>'
                f'<text x="{ramo_x + ramo_l / 2}" y="{ry + 34}" text-anchor="middle" '
                f'class="fx-s">{esc(CAIXA_PARADO[1])}</text>'
                f'<text x="{ramo_x + ramo_l / 2}" y="{ry + 46}" text-anchor="middle" '
                f'class="fx-s">{esc(CAIXA_PARADO[2])}</text>')
            ligacoes.append(
                f'<line x1="{cx + meia_l}" y1="{centro}" x2="{ramo_x - 6}" y2="{centro}" '
                f'stroke="var(--parado)" stroke-width="1.4" marker-end="url(#seta-parado)"/>'
                f'<text x="{(cx + meia_l + ramo_x) / 2}" y="{centro - 7}" text-anchor="middle" '
                f'class="fx-r">sim</text>')
            y = ly + 2 * meia_h + 20
            pos.append(("losango", ly, 2 * meia_h))
            ligacoes.append(
                f'<text x="{cx + 9}" y="{y - 6}" class="fx-r">não</text>')
        # Cada linha de legenda paga 14 px de altura: a captura mostrou a
        # legenda de uma linha so ATRAVESSANDO a borda da caixa, e texto que
        # sai da caixa que o explica nao explica nada.
        alt = 44 if not sub else 34 + 14 * len(sub)
        linhas = [f'<text x="{cx}" y="{y + (20 if sub else 27)}" text-anchor="middle" '
                  f'class="fx-t">{esc(titulo)}</text>']
        for k, l in enumerate(sub):
            linhas.append(f'<text x="{cx}" y="{y + 36 + 13 * k}" text-anchor="middle" '
                          f'class="fx-s">{esc(l)}</text>')
        partes.append(
            f'<rect x="{caixa_x}" y="{y}" width="{caixa_l}" height="{alt}" rx="6" '
            f'fill="var(--papel-2)" stroke="currentColor" stroke-width="1.2"/>'
            + "".join(linhas))
        pos.append((titulo, y, alt))
        y += alt + 20
    # As setas entre caixas vizinhas, no vao de 20 px.
    for (_, y0, h0), (_, y1, _h) in zip(pos, pos[1:]):
        ligacoes.append(
            f'<line x1="{cx}" y1="{y0 + h0}" x2="{cx}" y2="{y1 - 5}" '
            f'stroke="currentColor" stroke-width="1.3" marker-end="url(#seta-fluxo)"/>')
    alto = y - 20 + 16
    return f"""<svg viewBox="0 0 {larg} {alto}" role="img" class="fluxo"
     aria-label="Fluxograma do trabalho: do pedido do dono às páginas publicadas, com o losango da decisão do dono ou bloqueio externo levando ao estado parado">
  <defs>
    <marker id="seta-fluxo" viewBox="0 0 10 10" refX="9" refY="5" markerWidth="6" markerHeight="6" orient="auto-start-reverse">
      <path d="M0 0 L10 5 L0 10 z" fill="currentColor"/>
    </marker>
    <marker id="seta-parado" viewBox="0 0 10 10" refX="9" refY="5" markerWidth="6" markerHeight="6" orient="auto-start-reverse">
      <path d="M0 0 L10 5 L0 10 z" fill="var(--parado)"/>
    </marker>
  </defs>
  {''.join(partes)}
  {''.join(ligacoes)}
</svg>"""


# ------------------------------------------------------------------ a pagina

CABECA = """<meta charset="utf-8">
<title>Painel PMO PhxSql</title>
<meta name="viewport" content="width=device-width, initial-scale=1">
<link rel="preconnect" href="https://fonts.googleapis.com">
<link rel="preconnect" href="https://fonts.gstatic.com" crossorigin>
<link rel="stylesheet" href="https://fonts.googleapis.com/css2?family=Exo+2:wght@400;500;600;700&family=Source+Serif+4:opsz,wght@8..60,400;8..60,600&family=IBM+Plex+Mono:wght@400;500;600&display=swap">
"""

# Os tokens sao os da pagina de status (mesma casa, mesma marca); o que entra
# de novo aqui sao as tres cores de ESTADO do board e o azul da acao de
# consulta, que ja vale na interface web -- contorno sempre, preenchimento so
# no hover.
CSS = """<style>
:root{
  --papel:#fbf9f7; --papel-2:#f3efec; --papel-3:#e9e3de;
  --tinta:#1a1210; --tinta-2:#4a3f3a; --tinta-3:#6b5e57;
  --linha:#ded6d0; --acento:#c63c0a;
  --feito:#2f7a3e; --parcial:#7d5f18; --planejado:#6b5e57;
  --aberto:#1f5c93; --entregue:#2f7a3e; --parado:#b5257f;
  --consultar:#1f5c93;
  --sombra:0 1px 2px rgba(26,18,16,.06),0 8px 24px rgba(26,18,16,.05);
}
@media (prefers-color-scheme:dark){
  :root:not([data-theme="light"]){
    --papel:#010418; --papel-2:#0a1122; --papel-3:#131c31;
    --tinta:#dde2eb; --tinta-2:#a8b0c0; --tinta-3:#848da0;
    --linha:#1e2940; --acento:#ff8a1c;
    --feito:#5cbf74; --parcial:#d5a83c; --planejado:#8e9ab0;
    --aberto:#5fa6e8; --entregue:#5cbf74; --parado:#ff8fc7;
    --consultar:#5fa6e8;
    --sombra:0 1px 2px rgba(0,0,0,.4),0 8px 24px rgba(0,0,0,.3);
  }
}
:root[data-theme="dark"]{
  --papel:#010418; --papel-2:#0a1122; --papel-3:#131c31;
  --tinta:#dde2eb; --tinta-2:#a8b0c0; --tinta-3:#848da0;
  --linha:#1e2940; --acento:#ff8a1c;
  --feito:#5cbf74; --parcial:#d5a83c; --planejado:#8e9ab0;
  --aberto:#5fa6e8; --entregue:#5cbf74; --parado:#ff8fc7;
  --consultar:#5fa6e8;
  --sombra:0 1px 2px rgba(0,0,0,.4),0 8px 24px rgba(0,0,0,.3);
}
*{box-sizing:border-box}
body{margin:0;background:var(--papel);color:var(--tinta);
  font-family:"Source Serif 4",Georgia,"Times New Roman",serif;font-size:16px;line-height:1.55;
  -webkit-font-smoothing:antialiased}
h1,h2,h3,.rotulo,.pino,.aba,.chip,.cartao .nome{font-family:"Exo 2","Helvetica Neue",Arial,sans-serif}
code,.mono,.num{font-family:"IBM Plex Mono",ui-monospace,Menlo,monospace}
code{font-size:.86em;background:var(--papel-2);padding:1px 4px;border-radius:3px;color:var(--tinta-2)}
.envelope{max-width:1120px;margin:0 auto;padding:0 16px 80px}
header{padding:40px 0 20px;border-bottom:1px solid var(--linha)}
.tarja{display:flex;flex-wrap:wrap;align-items:baseline;gap:10px;margin-bottom:12px}
.rotulo{font-size:10.5px;letter-spacing:.18em;text-transform:uppercase;color:var(--acento);font-weight:600}
/* A versao e o commit sao DADO: mono, e sem transformacao nenhuma -- rotulo se
   estiliza, dado nunca. */
.versao{font-family:"IBM Plex Mono",monospace;font-size:11.5px;color:var(--tinta-3)}
h1{font-size:clamp(26px,4.6vw,42px);font-weight:700;line-height:1.08;margin:0 0 14px;letter-spacing:-.015em;text-wrap:balance}
h1 .x{color:var(--acento)}
.chamada{max-width:68ch;color:var(--tinta-2);font-size:16.5px;margin:0}
h2{font-size:21px;font-weight:600;margin:44px 0 6px;letter-spacing:-.01em}
h2:first-child{margin-top:30px}
h2 + .sub,h3 + .sub{color:var(--tinta-3);font-size:14.5px;margin:0 0 18px;max-width:70ch}
h3{font-size:15px;font-weight:600;margin:26px 0 10px}

/* As abas sao CONSULTA: contorno sempre, preenchimento so no hover -- a
   convencao das cores da acao da casa. A ativa se marca por peso e barra, nao
   por fundo cheio, que e o que o hover reserva para a intencao. */
.abas{display:flex;flex-wrap:wrap;gap:8px;margin:20px 0 4px}
.aba{background:transparent;border:1px solid var(--consultar);color:var(--consultar);
  font-size:12.5px;font-weight:600;letter-spacing:.02em;padding:7px 16px;border-radius:5px;
  cursor:pointer;line-height:1.2}
.aba:hover{background:var(--consultar);color:var(--papel)}
.aba[aria-selected="true"]{background:var(--papel);border-color:var(--acento);color:var(--acento);
  box-shadow:inset 0 -2px 0 var(--acento)}
.aba.tema{border-color:var(--linha);color:var(--tinta-3);margin-left:auto}
.aba.tema:hover{background:var(--papel-3);color:var(--tinta)}
body.com-abas .vista{display:none}
body.com-abas .vista.ativa{display:block}

.placar{display:grid;grid-template-columns:repeat(auto-fit,minmax(158px,1fr));gap:10px;margin:18px 0 0}
.placar .c{border:1px solid var(--linha);border-radius:6px;padding:13px 15px;background:var(--papel-2);
  display:flex;flex-direction:column;gap:5px}
.placar .v{font-family:"Exo 2",sans-serif;font-size:30px;font-weight:700;line-height:1.05;
  font-variant-numeric:tabular-nums;letter-spacing:-.01em;overflow-wrap:break-word}
.placar .v.curta{font-size:19px}
.placar .c.larga{grid-column:span 2}
/* Dado comprido (branch, commit) em mono e quebrando no hifen -- na captura o
   nome da branch partia no meio da palavra, em Exo 2, parecendo outro nome. */
.placar .dado .v{font-family:"IBM Plex Mono",monospace;font-size:16px;overflow-wrap:break-word;word-break:normal}
.placar .r{font-family:"IBM Plex Mono",monospace;font-size:10px;letter-spacing:.12em;
  text-transform:uppercase;color:var(--tinta-3);line-height:1.35}
.placar .q{font-size:10.5px;color:var(--tinta-3);margin-top:auto;font-family:"IBM Plex Mono",monospace}
.placar .feito .v{color:var(--feito)} .placar .parcial .v{color:var(--parcial)}
.placar .planejado .v{color:var(--planejado)} .placar .parado .v{color:var(--parado)}
.placar .aberto .v{color:var(--aberto)} .placar .entregue .v{color:var(--entregue)}

.duo{display:grid;grid-template-columns:minmax(232px,auto) minmax(240px,1fr);gap:22px;align-items:center;margin:18px 0 0}
.rosca{display:block;max-width:100%;height:auto}
.rosca-n{font-family:"Exo 2",sans-serif;font-size:40px;font-weight:700;fill:var(--tinta);font-variant-numeric:tabular-nums}
.rosca-r{font-family:"IBM Plex Mono",monospace;font-size:11px;fill:var(--tinta-3);letter-spacing:.1em}
.fatias{list-style:none;margin:0;padding:0;display:grid;gap:10px;max-width:440px}
.fatias li{display:grid;grid-template-columns:26px 1fr auto;gap:11px;align-items:center;font-size:14.5px}
.fatias i{display:inline-block;width:26px;height:13px;border-radius:2px;border:1.5px solid transparent}
.fatias .feito i{background:var(--feito)}
.fatias .parcial i{background:repeating-linear-gradient(45deg,var(--parcial) 0 3px,transparent 3px 7px);border-color:var(--parcial)}
.fatias .planejado i{background:transparent;border-color:var(--planejado)}
.fatias .n{font-family:"IBM Plex Mono",monospace;font-size:13px;font-variant-numeric:tabular-nums;color:var(--tinta-2)}
.fatias .s{color:var(--tinta-3)}

/* As barras do board. A forma carrega o estado: aberto so contorno, entregue
   cheia, parado hachurado -- le-se sem cor. O numero vem DEPOIS da barra,
   nunca por cima dela. */
.pilares{list-style:none;margin:16px 0 0;padding:0;display:grid;gap:11px}
.pilares li{display:grid;grid-template-columns:8.6em 1fr 7.6em;gap:12px;align-items:center}
.pilares .l{font-family:"Exo 2",sans-serif;font-size:14px;font-weight:500}
.pilares .t{height:16px;background:var(--papel-3);border-radius:3px;display:flex;overflow:hidden}
.pilares .t span{display:block;height:100%}
.pilares .t .b-aberto{border:1.6px solid var(--aberto);background:transparent}
.pilares .t .b-entregue{background:var(--entregue)}
.pilares .t .b-parado{background:repeating-linear-gradient(45deg,var(--parado) 0 3px,transparent 3px 7px);border:1.6px solid var(--parado)}
.pilares .n{font-family:"IBM Plex Mono",monospace;font-size:12.5px;font-variant-numeric:tabular-nums;
  color:var(--tinta-2);text-align:right;white-space:nowrap}
.pilares .n b{font-weight:600;color:var(--tinta)}
.legenda{display:flex;flex-wrap:wrap;gap:12px 20px;margin:14px 0 0;font-size:13px;color:var(--tinta-2)}
.legenda span{display:inline-flex;align-items:center;gap:7px}
.legenda i{display:inline-block;width:22px;height:11px;border-radius:2px;border:1.5px solid transparent;flex:none}
.legenda .aberto i{border-color:var(--aberto)}
.legenda .entregue i{background:var(--entregue)}
.legenda .parado i{background:repeating-linear-gradient(45deg,var(--parado) 0 3px,transparent 3px 7px);border-color:var(--parado)}
.legenda .feito i{background:var(--feito)}
.legenda .parcial i{background:repeating-linear-gradient(45deg,var(--parcial) 0 3px,transparent 3px 7px);border-color:var(--parcial)}
.legenda .planejado i{border-color:var(--planejado)}

.chips{display:flex;flex-wrap:wrap;gap:6px;margin:12px 0 0}
.chip{font-family:"IBM Plex Mono",monospace;font-size:11.5px;border:1px solid var(--linha);
  border-radius:4px;padding:3px 8px;color:var(--tinta-2);background:var(--papel-2)}
.chip.forte{border-color:var(--acento);color:var(--acento)}
.chip.meio{border-color:var(--aberto);color:var(--aberto)}
.chip.leve{border-color:var(--tinta-3);color:var(--tinta-3)}
.chip.frase{border-color:var(--parado);color:var(--parado)}

.regra{border-left:3px solid var(--acento);background:var(--papel-2);padding:15px 18px;
  border-radius:0 5px 5px 0;margin:26px 0;font-size:15px;color:var(--tinta-2);max-width:72ch}
.regra .t{display:block;font-family:"Exo 2",sans-serif;font-weight:700;color:var(--tinta);
  font-size:15px;margin-bottom:6px;letter-spacing:.01em}

.gates{list-style:none;margin:16px 0 0;padding:0;display:grid;gap:12px}
.gates li{border:1px solid var(--linha);border-left:3px solid var(--parado);border-radius:0 6px 6px 0;
  padding:13px 16px;background:var(--papel-2)}
.gates .cab{display:flex;flex-wrap:wrap;gap:10px;align-items:baseline}
.gates .n{font-family:"IBM Plex Mono",monospace;font-size:12.5px;color:var(--tinta-3)}
.gates .tt{font-weight:600;font-size:15px}
.gates .tr{margin:9px 0 0;font-size:13.5px;color:var(--tinta-2);border-left:2px solid var(--linha);padding-left:11px}
.vazio{border:1px dashed var(--linha);border-radius:6px;padding:14px 16px;color:var(--tinta-2);
  background:var(--papel-2);font-size:14.5px}

.pino{display:inline-flex;align-items:center;gap:6px;font-size:11.5px;font-weight:600;white-space:nowrap}
.pino::before{content:"";width:9px;height:9px;border-radius:50%;border:1.5px solid currentColor;flex:none}
.pino.feito{color:var(--feito)}.pino.feito::before{background:currentColor}
.pino.parcial{color:var(--parcial)}.pino.parcial::before{background:linear-gradient(90deg,currentColor 50%,transparent 50%)}
.pino.planejado{color:var(--planejado)}

.rolo{overflow-x:auto;-webkit-overflow-scrolling:touch;margin-top:14px}
table{border-collapse:collapse;width:100%;min-width:560px}
thead th{font-family:"IBM Plex Mono",monospace;font-weight:500;font-size:10px;letter-spacing:.12em;
  text-transform:uppercase;color:var(--tinta-3);text-align:left;padding:10px 12px 8px;border-bottom:1px solid var(--linha)}
tbody td{padding:11px 12px;border-bottom:1px solid var(--linha);vertical-align:top;font-size:14.5px}
tbody tr:hover td{background:var(--papel-2)}
td.h,td.d{font-family:"IBM Plex Mono",monospace;font-size:12.5px;color:var(--tinta-3);white-space:nowrap}
td.h{color:var(--acento)}

figure{margin:18px 0 0}
.fluxo{display:block;width:100%;max-width:540px;height:auto;margin:0 auto;color:var(--tinta-2)}
.fx-t{font-family:"Exo 2",sans-serif;font-size:13px;font-weight:600;fill:var(--tinta)}
.fx-s{font-family:"IBM Plex Mono",monospace;font-size:10px;fill:var(--tinta-3)}
.fx-r{font-family:"IBM Plex Mono",monospace;font-size:11px;fill:var(--tinta-3)}
.fx-parado{fill:var(--parado)}
figcaption{margin-top:12px;color:var(--tinta-3);font-size:13.5px;max-width:70ch;margin-left:auto;margin-right:auto}

.topo{display:grid;justify-items:center;gap:0;margin:18px 0 0}
.cartao{border:1px solid var(--linha);border-radius:7px;background:var(--papel-2);padding:13px 16px;
  box-shadow:var(--sombra);max-width:100%}
.cartao .nome{font-size:15.5px;font-weight:600;display:block}
.cartao .papel{font-family:"IBM Plex Mono",monospace;font-size:10.5px;letter-spacing:.12em;
  text-transform:uppercase;color:var(--tinta-3);display:block;margin-bottom:4px}
.cartao .dsc{font-size:13.5px;color:var(--tinta-2);margin:8px 0 0}
.cartao .fr{font-family:"IBM Plex Mono",monospace;font-size:11px;color:var(--tinta-3);margin:8px 0 0}
.cartao.dono{border-color:var(--acento);text-align:center}
.cartao.dono .nome{font-size:19px}
.haste{width:2px;height:20px;background:var(--linha)}
.trilho{height:2px;background:var(--linha);width:min(100%,900px);margin:0 auto}
.grade{display:grid;grid-template-columns:repeat(auto-fit,minmax(252px,1fr));gap:12px;margin:18px 0 0}
.grade .cartao{align-self:start}
.conferencia{margin:18px 0 0;font-family:"IBM Plex Mono",monospace;font-size:11.5px;color:var(--tinta-3)}

footer{margin-top:52px;padding-top:20px;border-top:1px solid var(--linha);color:var(--tinta-3);
  font-size:13.5px;max-width:74ch}
@media (max-width:700px){
  .duo{grid-template-columns:1fr}
  .placar .c.larga{grid-column:span 1}
  .pilares li{grid-template-columns:1fr auto}
  .pilares .t{grid-column:1/3}
  .abas .tema{margin-left:0}
}
@media (prefers-reduced-motion:reduce){*{transition:none!important;animation:none!important}}
</style>
"""

JS = """<script>
(function(){
  var raiz=document.documentElement;
  try{
    var g=localStorage.getItem("phx-pmo-tema");
    if(g&&!raiz.hasAttribute("data-theme"))raiz.setAttribute("data-theme",g);
  }catch(e){}
  function temaAtual(){
    var t=raiz.getAttribute("data-theme");
    if(t)return t;
    return (window.matchMedia&&window.matchMedia("(prefers-color-scheme: dark)").matches)?"dark":"light";
  }
  var bt=document.getElementById("bt-tema");
  if(bt)bt.addEventListener("click",function(){
    var novo=temaAtual()==="dark"?"light":"dark";
    raiz.setAttribute("data-theme",novo);
    try{localStorage.setItem("phx-pmo-tema",novo);}catch(e){}
  });
  var abas=Array.prototype.slice.call(document.querySelectorAll(".aba[data-vista]"));
  var vistas=Array.prototype.slice.call(document.querySelectorAll(".vista"));
  if(!abas.length||!vistas.length)return;
  document.body.classList.add("com-abas");
  function mostrar(id){
    vistas.forEach(function(v){v.classList.toggle("ativa",v.id===id);});
    abas.forEach(function(a){a.setAttribute("aria-selected",String(a.dataset.vista===id));});
    try{history.replaceState(null,"","#"+id);}catch(e){}
  }
  abas.forEach(function(a){a.addEventListener("click",function(){mostrar(a.dataset.vista);});});
  var inicial=(location.hash||"").replace("#","");
  mostrar(vistas.some(function(v){return v.id===inicial;})?inicial:vistas[0].id);
})();
</script>
"""


def cartao_placar(valor, rotulo, quando, classe="", curta=False):
    return (f'<div class="c {classe}"><div class="v{" curta" if curta else ""}">{valor}</div>'
            f'<div class="r">{rotulo}</div><div class="q">{quando}</div></div>')


def vista_painel(itens, estados, hoje, board, cap, gates, frentes):
    total = len(itens)
    contas = {c: sum(1 for i in itens if i["classe"] == c)
              for _, (c, _r) in estados.items()}
    fonte_ped = f"contados hoje, {hoje} · PENDENCIAS.md"
    fonte_brd = f"contados hoje, {hoje} · pmo/BACKLOG.md"
    q_cap = f"medido em {data_br(cap['medido_em'])} · CAPABILITIES.json"

    cartoes = [cartao_placar(num(total), "pedidos do dono, ao todo", fonte_ped)]
    for _simbolo, (classe, rotulo) in estados.items():
        n = contas[classe]
        cartoes.append(cartao_placar(
            num(n), f"{rotulo.lower()} · {porcento(n, total)}% do total",
            fonte_ped, classe))

    estados_board = board["estados"]
    cartoes_board = [
        cartao_placar(num(board["total"]), "itens no board", fonte_brd),
        cartao_placar(num(estados_board["aberto"]), "abertos", fonte_brd, "aberto"),
        cartao_placar(num(estados_board["entregue-fechado"]), "entregues / fechados",
                      fonte_brd, "entregue"),
        cartao_placar(num(estados_board["parado"]), "parados", fonte_brd, "parado"),
        cartao_placar(num(len(board["forte_abertos"])),
                      "itens de escalão forte abertos", fonte_brd),
    ]

    fatias = []
    for _simbolo, (classe, rotulo) in estados.items():
        n = contas[classe]
        forma = {"feito": "barra cheia", "parcial": "hachurada",
                 "planejado": "só contorno"}[classe]
        fatias.append(
            f'<li class="{classe}"><i></i><span>{esc(rotulo)} '
            f'<span class="s">— {esc(forma)}</span></span>'
            f'<span class="n">{num(n)} · {porcento(n, total)}%</span></li>')

    barras = []
    maior = max(p["total"] for p in board["pilares"])
    for p in board["pilares"]:
        larg = 100.0 * p["total"] / maior
        pedacos = []
        for chave, css in (("aberto", "b-aberto"), ("entregue-fechado", "b-entregue"),
                           ("parado", "b-parado")):
            n = p[chave]
            if not n:
                continue
            pedacos.append(
                f'<span class="{css}" style="width:{100.0 * n / p["total"]:.4f}%" '
                f'title="{esc(chave)}: {n}"></span>')
        barras.append(
            f'<li><span class="l">{esc(p["nome"])}</span>'
            f'<span class="t" style="width:{larg:.4f}%">{"".join(pedacos)}</span>'
            f'<span class="n">{p["aberto"]} · {p["entregue-fechado"]} · {p["parado"]}'
            f' &nbsp;<b>{p["total"]}</b></span></li>')

    fortes = "".join(f'<span class="chip">{esc(i)}</span>'
                     for i in board["forte_abertos"])

    if gates:
        lista = []
        for g in gates:
            frases = "".join(f'<span class="chip frase">«{esc(f)}»</span>' for f in g["frases"])
            lista.append(
                f'<li><div class="cab"><span class="n">#{g["n"]}</span>'
                f'<span class="tt">{g["pedido"]}</span>'
                f'<span class="pino {g["classe"]}">{esc(g["rotulo"])}</span></div>'
                f'<div class="chips">{frases}</div>'
                f'<p class="tr">{esc(g["trecho"])}</p></li>')
        bloco_gates = f'<ul class="gates">{"".join(lista)}</ul>'
    else:
        bloco_gates = (
            '<div class="vazio">Nenhum gate externo casou o léxico — e a lista '
            'fica aqui dizendo isso, em vez de sumir da página.</div>')
    lexico = "".join(f'<span class="chip">«{esc(f)}»</span>' for f in LEXICO_GATE)

    linhas_frentes = "".join(
        f'<tr><td class="h">{esc(f["hash"])}</td><td class="d">{esc(f["data"])}</td>'
        f'<td>{esc(f["assunto"])}</td></tr>' for f in frentes)

    idi = cap["idiomas"]
    motor = [
        cartao_placar(esc(cap["versao"]), "versão do motor", q_cap, curta=True),
        cartao_placar(num(cap["testes"]), "testes na suíte inteira", q_cap),
        cartao_placar(num(cap["operacoes"]), "operações do protocolo", q_cap),
        cartao_placar(num(cap["linhas_rust"]), "linhas de Rust", q_cap),
        cartao_placar(num(cap["linhas_doc"]), "linhas de documentação", q_cap),
        cartao_placar(num(cap["crates"]), "crates no workspace", q_cap),
        cartao_placar(num(cap["dependencias_externas"]), "dependências externas", q_cap),
        cartao_placar(f'{num(idi["pct"])}%',
                      f'textos de tela na fábrica de idiomas · {num(idi["fabrica"])} '
                      f'de {num(idi["total"])}, teto {num(idi["teto"])}', q_cap),
        cartao_placar(esc(cap["commit"][:7]), "commit medido (7 primeiros de 40)",
                      q_cap, "dado", curta=True),
        cartao_placar(esc(cap["branch"]), "branch", q_cap, "dado larga", curta=True),
        cartao_placar(num(bool(cap.get("sujo"))), "árvore suja ao medir",
                      q_cap, curta=True),
    ]

    return f"""<section id="painel" class="vista ativa" role="tabpanel" aria-label="Painel">
<h2>Os pedidos do dono, nos três estados</h2>
<p class="sub">Contados no <code>docs/PENDENCIAS.md</code> pelo mesmo leitor da página dos
pedidos — uma receita só para o mesmo número, porque receita duplicada é receita que diverge.</p>
<div class="placar">{"".join(cartoes)}</div>

<div class="duo">
  {rosca(itens, estados)}
  <ul class="fatias">{"".join(fatias)}</ul>
</div>

<div class="regra">
  <span class="t">iniciado ≠ concluído — parcial não vira feito</span>
  Um pedido só muda de <em>parcial</em> para <em>feito</em> com prova real nos dois
  sentidos (o teste falha com o defeito reposto e passa com o conserto) e com os
  portões verdes. O que está pela metade aparece pela metade, com a metade que
  falta nomeada na própria linha.
</div>

<h2>O board, por pilar</h2>
<p class="sub">Do <code>docs/pmo/BACKLOG.md</code>, pelo <code>rollup.py</code>. A largura da
barra é o tamanho do pilar; a forma dentro dela é o estado — e ela se lê sem cor.</p>
<ul class="pilares">{"".join(barras)}</ul>
<div class="legenda">
  <span class="aberto"><i></i>aberto — só contorno</span>
  <span class="entregue"><i></i>entregue / fechado — cheia</span>
  <span class="parado"><i></i>parado — hachurada</span>
</div>
<h3>Itens de escalão forte ainda abertos</h3>
<div class="chips">{fortes}</div>
<div class="placar">{"".join(cartoes_board)}</div>

<h2>Gates externos pendentes</h2>
<p class="sub">Pedidos ainda abertos cujo texto casa o léxico do gerador. A frase que
casou aparece ao lado — quem lê julga o casamento em vez de acreditar nele.</p>
{bloco_gates}
<h3>O léxico, por extenso</h3>
<div class="chips">{lexico}</div>

<h2>Últimas frentes</h2>
<p class="sub">Os {num(len(frentes))} commits mais recentes, do próprio <code>git log</code>.
O assunto vai como foi escrito — dado não se estiliza.</p>
<div class="rolo">
  <table>
    <thead><tr><th>commit</th><th>quando</th><th>assunto</th></tr></thead>
    <tbody>{linhas_frentes}</tbody>
  </table>
</div>

<h2>O motor, medido</h2>
<p class="sub">Tudo do <code>CAPABILITIES.json</code>, e cada cartão traz a data da medição:
número sem data é retrato que nunca existiu.</p>
<div class="placar">{"".join(motor)}</div>
</section>"""


def vista_fluxo(estados):
    legenda = []
    formas = {"feito": "barra cheia", "parcial": "hachurada", "planejado": "só contorno"}
    for simbolo, (classe, rotulo) in estados.items():
        legenda.append(
            f'<span class="{classe}"><i></i>{esc(simbolo)} {esc(rotulo)} '
            f'— {esc(formas[classe])}</span>')
    return f"""<section id="fluxo" class="vista" role="tabpanel" aria-label="Fluxo">
<h2>Como um pedido vira página publicada</h2>
<p class="sub">O processo da casa, do pedido do dono ao que se publica. Este quadro é
conteúdo fixo de propósito — processo não é medida —, e o único desvio dele é o losango:
o que depende de decisão sua ou de bloqueio externo fica <strong>parado</strong> e vai
para a lista de gates, em vez de sumir.</p>
<figure>
  {fluxograma()}
  <figcaption>O caminho de um pedido. As caixas são passos; o losango é a única
  bifurcação, e a saída «sim» não volta para o fluxo: ela vira item da lista de
  gates externos do painel.</figcaption>
</figure>
<div class="legenda">{"".join(legenda)}</div>
<div class="regra">
  <span class="t">parcial não vira feito</span>
  Só vira feito com <strong>prova real</strong> — o teste falha com o defeito reposto e
  passa com o conserto — e com os <strong>portões verdes</strong>: <code>fmt</code>,
  <code>clippy</code> com zero avisos e a suíte inteira. Teste que passa por engano é
  pior que teste que falta.
</div>
</section>"""


def caixa_equipe(c):
    partes = [f'<span class="papel">{esc(c["papel"])}</span>']
    nome = c["agente"] if c["agente"] else c["titulo"]
    partes.append(f'<span class="nome">{esc(nome)}</span>')
    pinos = []
    if c["agente"]:
        pinos.append(f'<span class="chip">{esc(c["titulo"])}</span>')
    if c["nivel"]:
        pinos.append(f'<span class="chip {c["nivel"]}">escalão {esc(c["nivel"])}</span>')
    elif c["agente"]:
        pinos.append('<span class="chip">escalão não registrado por papel</span>')
    if pinos:
        partes.append(f'<div class="chips">{"".join(pinos)}</div>')
    if c["descricao"]:
        partes.append(f'<p class="dsc">{c["descricao"]}</p>')
    else:
        partes.append(f'<p class="dsc">{c["origem"] or c["porque"]}</p>')
    if c["escalao"]:
        partes.append(f'<p class="fr">escalão: {c["escalao"]}</p>')
    if c["ferramentas"]:
        partes.append(f'<p class="fr">ferramentas: {c["ferramentas"]}</p>')
    return f'<div class="cartao">{"".join(partes)}</div>'


def vista_equipe(principais, subs, sem_arquivo, n_agentes):
    orquestrador = [c for c in principais if c["papel"] == "A"]
    demais = [c for c in principais if c["papel"] != "A"]
    grade = "".join(caixa_equipe(c) for c in demais)
    # Os subagentes saem em FILA PROPRIA, e nao dentro da caixa do J: empilhados
    # ali dentro, os tres esticavam a linha inteira da grade e deixavam um vazio
    # do tamanho de tres caixas ao lado -- a captura mostrou isso, o codigo nao.
    fila_subs = ""
    if subs:
        dono = ", ".join(sorted({s["papel"].split("-")[0] for s in subs}))
        fila_subs = (
            f'<h3>Subagentes do papel {esc(dono)}</h3>'
            f'<p class="sub">Profundidade de domínio entra como subagente, não como '
            f'papel novo — o dono de cada domínio já existe.</p>'
            f'<div class="grade">{"".join(caixa_equipe(s) for s in subs)}</div>')
    topo = "".join(caixa_equipe(c) for c in orquestrador)
    com_nivel = sum(1 for c in principais + subs if c["nivel"])
    conferencia = (
        f'{num(n_agentes)} arquivos em <code>.claude/agents/</code>, todos com caixa · '
        f'{num(len(principais) + len(subs))} caixas no organograma · '
        f'{num(com_nivel)} com escalão registrado em <code>docs/MODELOS.md</code>'
        + (f' · {num(len(sem_arquivo))} registro(s) do MODELOS.md sem arquivo de '
           f'agente: {esc(", ".join(sem_arquivo))}' if sem_arquivo else
           ' · nenhum registro do MODELOS.md sem arquivo de agente'))
    return f"""<section id="equipe" class="vista" role="tabpanel" aria-label="Equipe">
<h2>Quem responde por quê</h2>
<p class="sub">Os papéis pétreos, com o agente que os torna invocáveis. O escalão aparece
pelo <strong>nível</strong> — nome de modelo é proibido em artefato versionado —, e o papel
que não tem nível registrado diz isso em vez de ganhar um inventado. O que cada um
responde sai do próprio arquivo do agente.</p>
<div class="topo">
  <div class="cartao dono">
    <span class="papel">dono / PO</span>
    <span class="nome">Adriano Boller</span>
    <p class="dsc">Pede, decide e destrava. O que depende dele fica parado e aparece
    na lista de gates externos do painel.</p>
  </div>
  <div class="haste"></div>
  {topo}
  <div class="haste"></div>
</div>
<div class="trilho"></div>
<div class="grade">{grade}</div>
{fila_subs}
<p class="conferencia">{conferencia}</p>
</section>"""


def pagina(cap, itens, estados, hoje, board, gates, frentes,
           principais, subs, sem_arquivo, n_agentes, agora):
    return (CABECA + CSS + f"""<div class="envelope">
<header>
  <div class="tarja">
    <span class="rotulo">Painel PMO</span>
    <span class="versao">PhxSql {esc(cap["versao"])} · {esc(cap["branch"])}</span>
  </div>
  <h1>O que está <span class="x">feito</span>, o que está pela metade,<br>
  e o que ainda é só plano</h1>
  <p class="chamada">Três vistas da mesma rodada: o <strong>painel</strong> com os
  contadores, o <strong>fluxo</strong> que um pedido percorre e a <strong>equipe</strong>
  que responde por cada pedaço. Nenhum número desta página se digita — todos saem das
  fontes nomeadas embaixo de cada cartão, e a página inteira se refaz rodando um
  script. Número digitado à mão envelhece calado.</p>
  <nav class="abas" role="tablist" aria-label="Vistas">
    <button class="aba" type="button" data-vista="painel" role="tab" aria-controls="painel" aria-selected="true">Painel</button>
    <button class="aba" type="button" data-vista="fluxo" role="tab" aria-controls="fluxo" aria-selected="false">Fluxo</button>
    <button class="aba" type="button" data-vista="equipe" role="tab" aria-controls="equipe" aria-selected="false">Equipe</button>
    <button class="aba tema" id="bt-tema" type="button">tema claro / escuro</button>
  </nav>
</header>
"""
            + vista_painel(itens, estados, hoje, board, cap, gates, frentes)
            + vista_fluxo(estados)
            + vista_equipe(principais, subs, sem_arquivo, n_agentes)
            + f"""<footer>
  Gerado por <code>docs/pmo/pagina-do-status-do-projeto.py</code> em {esc(agora)} UTC,
  de <code>docs/PENDENCIAS.md</code>, <code>docs/pmo/BACKLOG.md</code>,
  <code>CAPABILITIES.json</code>, <code>git log</code>, <code>.claude/agents/</code> e
  <code>docs/MODELOS.md</code>. Esta página <strong>não se edita</strong>: mexeu numa
  fonte, rode o gerador. O portão dos geradores confere, em modo
  <code>sem-carimbo</code>, se re-rodar mudaria algum número visível.
</footer>
</div>
""" + JS)


# ------------------------------------------------------------------ main

def main():
    args = [a for a in sys.argv[1:] if not a.startswith("--")]
    saida = pathlib.Path(args[0]).resolve() if args else PADRAO

    pedidos_py = importar(RAIZ / "docs" / "dossie" / "pagina-dos-pedidos.py", "phx_pedidos")
    rollup_py = importar(RAIZ / "docs" / "pmo" / "rollup.py", "phx_rollup")

    itens = pedidos_py.ler()
    estados = pedidos_py.ESTADOS
    contas = {c: sum(1 for i in itens if i["classe"] == c) for _, (c, _r) in estados.items()}

    do_board = rollup_py.ler()
    pilares = []
    for it in do_board:
        if it["pilar"] not in [p["nome"] for p in pilares]:
            pilares.append({"nome": it["pilar"], "total": 0,
                            "aberto": 0, "entregue-fechado": 0, "parado": 0})
        p = [x for x in pilares if x["nome"] == it["pilar"]][0]
        p[it["estado"]] += 1
        p["total"] += 1
    board = {
        "total": len(do_board),
        "pilares": pilares,
        "estados": {e: sum(1 for i in do_board if i["estado"] == e)
                    for e in rollup_py.ESTADOS},
        "forte_abertos": sorted(i["id"] for i in do_board
                                if i["escalao"] == "forte" and i["estado"] == "aberto"),
    }

    cap = ler_capacidades()
    frentes = ler_frentes()
    gates = achar_gates(itens)
    principais, subs, sem_arquivo = ler_equipe()
    n_agentes = len(principais) + len(subs) - sum(
        1 for c in principais + subs if not c["agente"])

    agora_utc = datetime.datetime.now(datetime.timezone.utc)
    agora = agora_utc.strftime("%d/%m/%Y %H:%M")
    hoje = agora_utc.strftime("%d/%m/%Y")

    saida.write_text(
        pagina(cap, itens, estados, hoje, board, gates, frentes,
               principais, subs, sem_arquivo, n_agentes, agora),
        encoding="utf-8")

    print(f"pedidos (docs/PENDENCIAS.md): {num(len(itens))} — "
          + ", ".join(f"{num(contas[c])} {plural(r)}" for _, (c, r) in estados.items()))
    print(f"board (docs/pmo/BACKLOG.md): {num(board['total'])} itens em "
          f"{len(pilares)} pilares — "
          + ", ".join(f"{num(board['estados'][e])} {e}" for e in rollup_py.ESTADOS)
          + f"; {num(len(board['forte_abertos']))} em forte abertos")
    print(f"motor (CAPABILITIES.json): {cap['versao']}, {num(cap['testes'])} testes, "
          f"{num(cap['operacoes'])} operacoes, {num(cap['linhas_rust'])} linhas de Rust, "
          f"{num(cap['dependencias_externas'])} dependencias — medido em "
          f"{data_br(cap['medido_em'])}")
    print(f"gates (PENDENCIAS.md x lexico de {len(LEXICO_GATE)} expressoes): "
          f"{num(len(gates))} pedido(s) — "
          + (", ".join(f"#{g['n']}" for g in gates) if gates
             else "nenhum casou (a lista diz isso na pagina)"))
    print(f"frentes (git log -n {QUANTAS_FRENTES}): {num(len(frentes))} commits, "
          f"de {frentes[0]['hash']} a {frentes[-1]['hash']}")
    print(f"equipe (.claude/agents + README.md + docs/MODELOS.md): {num(n_agentes)} "
          f"agentes, {num(len(principais) + len(subs))} caixas, "
          f"{num(sum(1 for c in principais + subs if c['nivel']))} com escalao registrado"
          + (f"; {len(sem_arquivo)} registro(s) sem arquivo: " + ", ".join(sem_arquivo)
             if sem_arquivo else ""))
    print(f"pagina gravada: {saida}")


if __name__ == "__main__":
    main()
