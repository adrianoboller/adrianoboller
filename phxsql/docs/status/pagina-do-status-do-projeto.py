#!/usr/bin/env python3
"""A SETIMA PAGINA -- STATUS DO PROJETO do PhxSql.

    python3 docs/status/pagina-do-status-do-projeto.py [saida.html]
    ./status-html.sh                      # o comando, que e a porta desta pagina

Nao confundir com `docs/pmo/pagina-do-status-do-projeto.py`, que tem o MESMO
nome de arquivo e gera outra coisa (o painel PMO, tres vistas, molde do Phoenix
Cast). Esta aqui nasceu de um exemplo que o dono mandou de um projeto irmao
(P.O.S), e a pasta e outra de proposito: `docs/status/`.

# A lei que decide o que entra nesta pagina

O exemplo trazia **677 numeros no texto visivel, todos digitados a mao**.
Adota-lo como veio criaria 677 lugares onde um numero envelhece calado -- e
esta casa ja pagou isso quatro vezes (o selo da capa parado em 0.11.0 por
quatro lancamentos; 198 pedidos onde eram 203; 428 testes onde eram 451;
780 KiB de interface onde eram 1.032).

Entao a regra desta pagina e uma so, e ela e mais dura que «nao digite numero»:

    SECAO SO ENTRA COM GERADOR. Se um numero da secao nao sai de um
    gerador, a SECAO NAO NASCE -- vira pendencia nomeada, com o gerador
    que falta, e aparece na ultima secao em vez de sumir.

As que nao nasceram estao em `SECOES_SEM_GERADOR`, ao lado do gerador que
faltaria e de onde ele tiraria o numero. O `status-html.sh` as imprime, e NAO
como linha de exito: *gerador que faz menos do que o nome promete tem de dizer
que fez menos.*

# Ela nao tem leitor proprio de quase nada, e isso e a regra

*Receita duplicada e receita que diverge.* Duas contagens do `PENDENCIAS.md`
divergiriam na primeira mudanca de legenda -- e ja divergiram: o pedido 150
passou meses invisivel numa pagina com um simbolo que o leitor nao conhecia.
Entao este gerador IMPORTA os leitores que ja existem:

    pagina-dos-pedidos.py      os tres estados do PENDENCIAS.md
    pmo/rollup.py              o board por pilar do BACKLOG.md
    pmo/pagina-do-...          o lexico dos gates e o `texto_puro`
    pagina-dos-testes.py       as bancadas declaradas, guardas e catracas
    graficos-dos-testes.py     as barras com faixa min-max (regra do pedido 155)
    cobertura-por-area.py      os #[test] por area, contados no fonte
    tecnologias/extrair.py     as linhas por crate, classificadas
    numeros-do-projeto.py      a LISTA dos documentos contados

O que este arquivo le sozinho e so o que ninguem le ainda: as constantes
`MAGIC_*`/`VERSAO*` do fonte (o formato em disco), a `description` de cada
`Cargo.toml`, os pacotes de `pacotes/` e o `git log`.
"""

import datetime
import html
import importlib.util
import json
import pathlib
import re
import subprocess
import sys

RAIZ = pathlib.Path(__file__).resolve().parents[2]
# O contexto da secao de telemetria, guardado pelo `montar()` para o `main()`
# poder dizer que fez menos sem reler o arquivo (duas leituras podem discordar).
ctx_tlm_publico = None
PADRAO = RAIZ / "docs" / "status" / "status-do-projeto.html"
QUANTAS_FRENTES = 10


# ------------------------------------------------------------------ fontes

def importar(caminho: pathlib.Path, apelido: str):
    """Importa um gerador pelo CAMINHO: os nomes tem hifen e nao sao modulos."""
    if not caminho.exists():
        raise SystemExit(
            f"falta o modulo irmao {caminho.relative_to(RAIZ)} -- esta pagina "
            "nao reescreve o leitor dele. Sem ele nao ha numero, e numero que "
            "falta nao vira zero.")
    spec = importlib.util.spec_from_file_location(apelido, caminho)
    mod = importlib.util.module_from_spec(spec)
    sys.modules[apelido] = mod
    spec.loader.exec_module(mod)
    return mod


D = RAIZ / "docs" / "dossie"
PEDIDOS = importar(D / "pagina-dos-pedidos.py", "st_pedidos")
BOARD = importar(RAIZ / "docs" / "pmo" / "rollup.py", "st_rollup")
PMO = importar(RAIZ / "docs" / "pmo" / "pagina-do-status-do-projeto.py", "st_pmo")
PROVAS = importar(D / "pagina-dos-testes.py", "st_provas")
GRAF = importar(D / "graficos-dos-testes.py", "st_graf")
COBERTURA = importar(D / "cobertura-por-area.py", "st_cobertura")
TEC = importar(RAIZ / "docs" / "tecnologias" / "extrair.py", "st_tec")
NUMS = importar(D / "numeros-do-projeto.py", "st_nums")
RISCOS = importar(RAIZ / "docs" / "status" / "riscos.py", "st_riscos")
TELEMETRIA = importar(RAIZ / "docs" / "status" / "telemetria-medida.py",
                      "st_telemetria")
SERIE = importar(RAIZ / "docs" / "status" / "serie-historica.py", "st_serie")


# ------------------------------------------------------------------------
# A ORDEM das secoes, e o NUMERO de cada uma SAI DAQUI -- nunca do texto.
#
# Ate 16/09/2026 cada funcao trazia o proprio numero escrito no cabecalho.
# Ai duas secoes entraram NO MEIO (riscos e divida tecnica, pedido 264) e
# teriam envelhecido treze titulos de uma vez, mais as duas referencias em
# prosa. E' exatamente a lesao que a numeracao das figuras do dossie ja pagou,
# no dia em que duas figuras entraram no meio do documento e envelheceram
# dezesseis legendas.
#
# Quem acrescenta uma secao poe a chave na ordem certa desta lista e usa
# `H2[chave]`; quem cita uma secao em prosa usa `ref(chave)`. Nenhum dos dois
# escreve numero.
ORDEM = [
    "resumo", "descricao", "crates", "formato", "fluxo", "capacidades",
    "pedidos", "gates", "riscos", "divida", "testes", "guardas", "catracas",
    "bancadas", "desempenho", "replicacao", "telemetria", "idiomas",
    "dependencias", "documentacao", "pacotes", "board", "frentes", "serie",
    "nao_nasceram",
]
H2 = {chave: f'<h2 id="s{i}"><span class="n">{i:02d} ·</span>'
      for i, chave in enumerate(ORDEM, 1)}


def h2(chave, titulo):
    """O cabecalho de uma secao, com o numero saindo da ORDEM.

    Existe para quem monta uma secao FORA deste arquivo (o `riscos.py` do
    pedido 264): assim a secao de la nao precisa saber em que posicao caiu, e
    mover a chave na ORDEM continua sendo a unica coisa que renumera.
    """
    if chave not in H2:
        raise SystemExit(f"secao {chave!r} nao esta na ORDEM.")
    return H2[chave] + titulo + "</h2>"


def ref(chave):
    """«§NN» para citar uma secao no texto, sem digitar o numero dela."""
    if chave not in H2:
        raise SystemExit(f"secao {chave!r} nao esta na ORDEM -- referencia "
                         "para secao que nao existe e pior que referencia "
                         "nenhuma: ela parece verificavel.")
    return f"§{ORDEM.index(chave) + 1:02d}"


# ------------------------------------------------------------------------
# AS SECOES QUE NAO NASCERAM, e o gerador que falta a cada uma.
#
# Esta lista e o contrario de uma omissao: o exemplo do P.O.S tinha as quatro,
# com numero digitado em cada. Aqui elas aparecem COMO AUSENTES, nomeando o
# gerador que as faria nascer -- porque pagina que esconde o que nao mediu e a
# pior de todas, pelo mesmo motivo que bancada sem resultado aparece como NAO
# MEDIDA em vez de sumir da tabela.
SECOES_SEM_GERADOR = [
    # Riscos (§11 do exemplo) e Divida tecnica (§12) SAIRAM desta lista em
    # 16/09/2026, com o pedido 264: nasceram com gerador -- `docs/status/
    # riscos.py`, sobre um `docs/RISCOS.md` tabelado e sobre a marca
    # `// DIVIDA:` do proprio fonte Rust. Nao se apaga o registro de que elas
    # faltaram; o que muda e o lugar dele, que agora e o CHANGELOG e o pedido.
    #
    # Telemetria e logs (§15/§16) e Antes x Depois (§21) sairam na MESMA data,
    # com os pedidos 265 e 266 -- e nos dois a falta era ANTES do gerador: a
    # bancada de telemetria nao gravava `resultados.json` (entao nao havia data
    # de medicao para por ao lado do numero) e o `CAPABILITIES.json` e
    # sobrescrito (entao nao havia «antes»). Hoje sao `telemetria-medida.py`
    # sobre `bancada/telemetria/resultados.json` e `serie-historica.py` sobre
    # `docs/status/serie.jsonl`, versionado.
    #
    # A lista ficou VAZIA, e isso NAO e motivo para apagar a secao: ela diz
    # «nenhuma», que e' uma afirmacao conferivel, e volta a nomear a proxima
    # secao que alguem quiser sem gerador. Secao que some quando esta vazia
    # deixa de ser lida no dia em que voltar a ter conteudo.
]


# ------------------------------------------------------------------ ajudantes

def esc(t):
    return html.escape(str(t))


def milhar(n):
    return f"{n:,}".replace(",", ".")


def pct(n, total, casas=1):
    if not total:
        return "0,0"
    return f"{100.0 * n / total:.{casas}f}".replace(".", ",")


def texto_puro(h):
    return PMO.texto_puro(h)


def hoje_iso():
    return datetime.date.today().isoformat()


# ------------------------------------------------------- leitores proprios

MAGIA = re.compile(r'const (MAGIC[A-Z_0-9]*)\s*:\s*&\[u8;\s*\d+\]\s*=\s*b"([^"]*)"')
VERSAO_CONST = re.compile(r"const (VERSAO[A-Z_0-9]*)\s*:\s*u\d+\s*=\s*(\d+)")


def formato_em_disco():
    """Os arquivos do formato, lidos das constantes do FONTE.

    A lista nao se digita: uma tabela copiada aqui perderia o arquivo novo no
    dia em que ele nascesse, e e' exatamente esse o erro que a receita do KiB
    da interface ja custou. Quem manda e' o `const MAGIC_*` do Rust.
    """
    achados = []
    for rs in sorted((RAIZ / "crates").glob("*/src/**/*.rs")):
        try:
            texto = rs.read_text(encoding="utf-8")
        except UnicodeDecodeError:
            continue
        magias = MAGIA.findall(texto)
        if not magias:
            continue
        versoes = VERSAO_CONST.findall(texto)
        for nome, marca in magias:
            achados.append({
                "const": nome,
                "marca": marca.replace("\\0", "·"),
                "versoes": versoes,
                "fonte": str(rs.relative_to(RAIZ)),
            })
    achados.sort(key=lambda a: a["fonte"])
    return achados


def crates_medidos():
    """Cada crate com as linhas do `src/` e a `description` do Cargo.toml."""
    base = RAIZ / "crates"
    saida = []
    for d in sorted(base.iterdir()):
        if not (d / "Cargo.toml").exists():
            continue
        r = TEC.contar_crate(d)
        if r is None:
            continue
        m = re.search(r'^description\s*=\s*"([^"]*)"',
                      (d / "Cargo.toml").read_text(encoding="utf-8"), re.M)
        saida.append({
            "nome": d.name,
            "descricao": m.group(1) if m else "",
            "arquivos": r["arquivos_src"],
            "codigo": r["src"]["codigo"],
            "teste": r["src"]["teste"],
            "total": r["src"]["total"],
        })
    saida.sort(key=lambda c: c["total"], reverse=True)
    return saida


def pacotes():
    """Os zips de `pacotes/`, com tamanho e se o SHA256SUMS os cobre.

    E o nosso analogo da «ISO bootavel» do exemplo: o que se entrega para
    baixar. O numero e' o tamanho real do arquivo, nao um numero de catalogo.
    """
    d = RAIZ / "pacotes"
    if not d.exists():
        return [], None
    somas = {}
    p = d / "SHA256SUMS"
    if p.exists():
        for l in p.read_text(encoding="utf-8").splitlines():
            partes = l.split()
            if len(partes) == 2:
                somas[partes[1]] = partes[0]
    zips = []
    for z in sorted(d.glob("*.zip")):
        st = z.stat()
        zips.append({
            "nome": z.name,
            "mib": st.st_size / (1024 * 1024),
            "quando": datetime.datetime.fromtimestamp(st.st_mtime).strftime("%Y-%m-%d"),
            "somado": z.name in somas,
        })
    return zips, p if p.exists() else None


def frentes():
    """As ultimas frentes, do proprio `git log`. So leitura."""
    r = subprocess.run(
        ["git", "-C", str(RAIZ), "log", "--no-color", "--no-merges",
         "--format=%h\t%ad\t%s", "--date=format:%Y-%m-%d %H:%M",
         "-n", str(QUANTAS_FRENTES)],
        capture_output=True, text=True, check=False)
    if r.returncode != 0:
        raise SystemExit("o `git log` falhou: " + (r.stderr or "").strip()
                         + " -- lista vazia mente, entao a pagina nao sai.")
    saida = []
    for l in r.stdout.split("\n"):
        if not l.strip():
            continue
        h, data, assunto = l.split("\t", 2)
        saida.append({"hash": h, "data": data, "assunto": assunto})
    if not saida:
        raise SystemExit("o `git log` nao devolveu commit nenhum.")
    return saida


def cognicoes():
    d = RAIZ / "docs" / "cognicao"
    return sorted(p.name for p in d.glob("cognicao_*.md")) if d.exists() else []


def paginas_geradas():
    """As paginas HTML que esta casa gera e publica, achadas por varredura.

    O nome do dossie muda a cada refacao, entao ele vem do `dossie_da_pasta`;
    as outras sao alvos declarados nos PLANOS dos geradores. Contar arquivo
    solto de `docs/` traria fluxograma de correio e relatorio de conteiner
    junto -- por isso a lista sai dos ALVOS do portao dos geradores.

    `PEDIDOS_FAIXAS` (pedido 403) nao e' um caminho -- e' um marcador que o
    proprio portao resolve por varredura, porque o CONJUNTO de paginas de
    pedidos muda de nome e de quantidade conforme o corte por tamanho se
    desloca. Sem tratar o marcador aqui, ele cairia no `else` como um
    caminho literal (`RAIZ / "@pedidos-faixas"`, que nao existe) e as
    paginas de pedidos sumiriam desta lista em silencio -- exatamente o
    "leitor que ninguem atualizou" que o pedido 403 pede para achar.
    """
    sys.path.insert(0, str(D))
    from dossie_da_pasta import achar_o_dossie  # noqa: PLC0415
    portao = importar(D / "portao-dos-geradores.py", "st_portao")
    dossie = achar_o_dossie()
    faixas_pedidos = getattr(portao, "PEDIDOS_FAIXAS", None)
    vistas = []
    for _script, alvos, _modo, _porque in portao.PLANO:
        for a in alvos:
            if faixas_pedidos is not None and a == faixas_pedidos:
                for p in sorted((RAIZ / "docs" / "dossie").glob("pedidos-*.html")):
                    if p not in vistas:
                        vistas.append(p)
                continue
            p = dossie if a == portao.DOSSIE else (RAIZ / a)
            if p.suffix == ".html" and p.exists() and p not in vistas:
                vistas.append(p)
    return sorted(vistas, key=lambda p: p.name), portao


# ------------------------------------------------------------------ marca

# A marca manda sobre qualquer paleta inventada (`marca/LEIA-ME.md`): Exo 2,
# fundo #010418, vermelhao #FF4D10 escurecendo para #C63C0A sobre papel. O
# exemplo do P.O.S vinha com --fire:#E24310 e IBM Plex Sans; aproveitamos a
# ESTRUTURA dele (barra de titulo, kpi, cartao, etiqueta, nota, barra-linha,
# tema por data-theme) e nada da paleta.
CABECA = """<meta charset="utf-8">
<title>Status do projeto — PhxSql</title>
<meta name="viewport" content="width=device-width, initial-scale=1">
<link rel="preconnect" href="https://fonts.googleapis.com">
<link rel="preconnect" href="https://fonts.gstatic.com" crossorigin>
<link rel="stylesheet" href="https://fonts.googleapis.com/css2?family=Exo+2:wght@400;500;600;700&family=Source+Serif+4:opsz,wght@8..60,400;8..60,600&family=IBM+Plex+Mono:wght@400;500;600&display=swap">
"""

CSS = """<style>
/* A marca manda (marca/LEIA-ME.md): Exo 2, fundo #010418, vermelhao #FF4D10
   escurecendo para #C63C0A sobre papel -- e essa segunda adaptacao ja estava
   decidida por contraste. Tres tokens do tema claro seguem a MESMA regra, e o
   motivo foi medido nesta pagina: sobre o papel (#fbf9f7) eles passavam, mas
   quase todo texto desta pagina vive dentro de um cartao (#f3efec), e ali
   #7a6d66 dava 4,37:1 e #8a6a1f dava 4,41:1 -- abaixo do minimo. Escurecidos,
   dao 4,70:1 e 4,76:1 no cartao. Medir o par que o olho ve, e nao o par que o
   token sugere. */
:root{
  --papel:#fbf9f7; --papel-2:#f3efec; --papel-3:#e9e3de;
  --tinta:#1a1210; --tinta-2:#4a3f3a; --tinta-3:#756861;
  --linha:#ded6d0; --acento:#c63c0a; --marca:#010418;
  --feito:#2f7a3e; --parcial:#84651d; --planejado:#756861; --depois:#5b4a9e;
  --aberto:#1f5c93; --entregue:#2f7a3e; --parado:#b5257f;
  --incluir:#2f7a3e; --alterar:#84651d; --marcar:#b5257f;
  --excluir:#b3261e; --consultar:#1f5c93;
  --falta:#84651d;
  --c1:#c63c0a; --c2:#4a6fa5; --c3:#5c7a52; --c4:#0e7a85;
  --sombra:0 1px 2px rgba(26,18,16,.06),0 8px 24px rgba(26,18,16,.05);
  --escolhida:var(--tinta);
}
@media (prefers-color-scheme:dark){
  :root:not([data-theme="light"]){
    --papel:#010418; --papel-2:#0a1122; --papel-3:#131c31;
    --tinta:#dde2eb; --tinta-2:#a8b0c0; --tinta-3:#848da0;
    --linha:#1e2940; --acento:#ff8a1c; --marca:#010418;
    --feito:#5cbf74; --parcial:#d5a83c; --planejado:#8e9ab0; --depois:#b7a6ea;
    --aberto:#5fa6e8; --entregue:#5cbf74; --parado:#ff8fc7;
    --incluir:#5cbf74; --alterar:#d5a83c; --marcar:#ff8fc7;
    --excluir:#ff8a80; --consultar:#5fa6e8;
    --falta:#d5a83c;
    --c1:#ff8a1c; --c2:#6f9fe0; --c3:#7fb36e; --c4:#3fc8d4;
    --sombra:0 1px 2px rgba(0,0,0,.4),0 8px 24px rgba(0,0,0,.3);
    --escolhida:var(--tinta);
  }
}
:root[data-theme="dark"]{
  --papel:#010418; --papel-2:#0a1122; --papel-3:#131c31;
  --tinta:#dde2eb; --tinta-2:#a8b0c0; --tinta-3:#848da0;
  --linha:#1e2940; --acento:#ff8a1c; --marca:#010418;
  --feito:#5cbf74; --parcial:#d5a83c; --planejado:#8e9ab0; --depois:#b7a6ea;
  --aberto:#5fa6e8; --entregue:#5cbf74; --parado:#ff8fc7;
  --incluir:#5cbf74; --alterar:#d5a83c; --marcar:#ff8fc7;
  --excluir:#ff8a80; --consultar:#5fa6e8;
  --falta:#d5a83c;
  --c1:#ff8a1c; --c2:#6f9fe0; --c3:#7fb36e; --c4:#3fc8d4;
  --sombra:0 1px 2px rgba(0,0,0,.4),0 8px 24px rgba(0,0,0,.3);
  --escolhida:var(--tinta);
}
*{box-sizing:border-box}
html,body{margin:0;background:var(--papel);color:var(--tinta);
  font-family:"Source Serif 4",Georgia,"Times New Roman",serif;
  font-size:16px;line-height:1.55;-webkit-font-smoothing:antialiased}
h1,h2,h3,.rotulo,.etiqueta,.kpi b,.aba,.barra-linha .l{
  font-family:"Exo 2","Helvetica Neue",Arial,sans-serif}
code,.mono,.num{font-family:"IBM Plex Mono",ui-monospace,Menlo,monospace}
code{font-size:.86em;background:var(--papel-2);padding:1px 4px;border-radius:3px;
  color:var(--tinta-2);overflow-wrap:break-word}
.envelope{max-width:1100px;margin:0 auto;padding:0 18px 80px}

/* A barra de titulo e a do exemplo em ESTRUTURA; a cor e a da marca --
   fundo #010418 e a faixa do vermelhao embaixo. */
.barra-titulo{background:var(--marca);color:#dde2eb;border-bottom:5px solid #ff4d10;
  padding:26px 22px;border-radius:0 0 12px 12px;margin-bottom:4px}
.barra-titulo h1{margin:0 0 8px;font-size:clamp(21px,4.2vw,31px);font-weight:700;
  letter-spacing:-.015em;line-height:1.12;color:#fff;text-wrap:balance}
.barra-titulo h1 .x{color:#ff8a1c}
.barra-titulo .sub{color:#a8b0c0;font-size:13px;font-family:"IBM Plex Mono",monospace;
  overflow-wrap:break-word}
.barra-titulo .assinatura{color:#7c8598;font-size:12px;font-style:italic;margin-top:6px}

/* As ferramentas sao CONSULTA: contorno sempre, preenchimento so no hover --
   a convencao das cinco cores da acao desta casa. */
.ferramentas{position:sticky;top:0;z-index:20;display:flex;flex-wrap:wrap;gap:8px;
  align-items:center;background:var(--papel);border:1px solid var(--linha);
  border-radius:9px;padding:9px 11px;margin:14px 0 18px;box-shadow:var(--sombra)}
.ferramentas button,.ferramentas select{font-family:"Exo 2",sans-serif;font-size:12.5px;
  font-weight:600;color:var(--consultar);background:transparent;
  border:1px solid var(--consultar);border-radius:5px;padding:6px 13px;cursor:pointer;
  line-height:1.2}
.ferramentas button:hover,.ferramentas select:hover{background:var(--consultar);color:var(--papel)}
.ferramentas button[aria-pressed="true"]{background:var(--papel);border-color:var(--acento);
  color:var(--acento);box-shadow:inset 0 -2px 0 var(--acento)}
.ferramentas .sep{flex:1}
.ferramentas .r{font-family:"IBM Plex Mono",monospace;font-size:11px;
  letter-spacing:.1em;text-transform:uppercase;color:var(--tinta-3)}

/* O seletor pinta TITULO, nunca dado: rotulo se estiliza, dado nunca. */
h2{font-size:20px;font-weight:600;margin:46px 0 4px;padding-top:16px;
  border-top:1px solid var(--linha);letter-spacing:-.01em;color:var(--escolhida)}
h2 .n{font-family:"IBM Plex Mono",monospace;color:var(--acento);font-size:15px;
  margin-right:7px;font-variant-numeric:tabular-nums}
h3{font-size:15px;font-weight:600;margin:26px 0 8px;color:var(--escolhida)}
h2 + .sub,h3 + .sub{color:var(--tinta-3);font-size:13.5px;margin:0 0 16px;max-width:74ch}
p{margin:11px 0;max-width:76ch}
.fonte{font-family:"IBM Plex Mono",monospace;font-size:11.5px;color:var(--tinta-3);
  margin:10px 0 0;max-width:80ch;overflow-wrap:break-word}
.fonte b{color:var(--tinta-2);font-weight:500}

.kpis{display:grid;grid-template-columns:repeat(auto-fit,minmax(152px,1fr));gap:10px;margin:18px 0}
.kpi{background:var(--papel-2);border:1px solid var(--linha);border-left:3px solid var(--acento);
  border-radius:7px;padding:12px 14px;display:flex;flex-direction:column;gap:4px}
/* `>` de proposito: `.kpi b` casava tambem o <b> de dentro do ROTULO, e
   «bancadas NAO MEDIDAS» saiu com o rotulo do tamanho do numero. O CSS global
   morde todo componente novo, e isto so apareceu olhando a pagina. */
.kpi > b{font-size:27px;font-weight:700;line-height:1.05;color:var(--tinta);
  font-variant-numeric:tabular-nums;letter-spacing:-.01em;overflow-wrap:break-word}
.kpi > b.curto{font-size:18px}
.kpi span{font-size:12px;color:var(--tinta-2);line-height:1.35}
.kpi span b{font-size:inherit;font-weight:600;color:var(--tinta)}
.kpi .q{font-family:"IBM Plex Mono",monospace;font-size:10px;color:var(--tinta-3);margin-top:auto}
.kpi.feito > b{color:var(--feito)} .kpi.parcial > b{color:var(--parcial)}
.kpi.planejado > b{color:var(--planejado)} .kpi.falta > b{color:var(--falta)}
.kpi.depois > b{color:var(--depois)}

.cartao{background:var(--papel-2);border:1px solid var(--linha);border-radius:8px;
  padding:16px 18px;margin:16px 0;box-shadow:var(--sombra)}
.nota{border:1px solid var(--linha);border-left:3px solid var(--acento);background:var(--papel-2);
  padding:13px 17px;border-radius:0 6px 6px 0;margin:18px 0;font-size:14.5px;
  color:var(--tinta-2);max-width:76ch}
.nota b{color:var(--tinta)}
.nota.g{border-left-color:var(--feito)} .nota.a{border-left-color:var(--falta)}
.nota.v{border-left-color:var(--excluir)} .nota.c{border-left-color:var(--consultar)}

/* Etiqueta: contorno sempre. Fundo cheio so no hover, quando ha intencao. */
.etiqueta{display:inline-block;font-size:11px;font-weight:600;letter-spacing:.04em;
  padding:1px 8px;border-radius:11px;border:1px solid currentColor;white-space:nowrap;
  background:transparent}
.e-tem{color:var(--feito)} .e-nao{color:var(--excluir)} .e-parcial{color:var(--parcial)}
.e-plan{color:var(--consultar)} .e-parado{color:var(--parado)}

.rolo{overflow-x:auto;-webkit-overflow-scrolling:touch;margin:14px 0}
table{border-collapse:collapse;width:100%;min-width:520px;font-size:14px}
thead th{font-family:"IBM Plex Mono",monospace;font-weight:500;font-size:10px;
  letter-spacing:.12em;text-transform:uppercase;color:var(--tinta-3);text-align:left;
  padding:9px 11px 7px;border-bottom:1px solid var(--linha)}
tbody td{padding:9px 11px;border-bottom:1px solid var(--linha);vertical-align:top}
tbody tr:hover td{background:var(--papel-2)}
td.num,th.num{text-align:right;font-family:"IBM Plex Mono",monospace;
  font-variant-numeric:tabular-nums;white-space:nowrap}
td.mono{font-family:"IBM Plex Mono",monospace;font-size:12.5px}
td.q{font-family:"IBM Plex Mono",monospace;font-size:12px;color:var(--tinta-3);white-space:nowrap}
tr.ausente td{color:var(--falta)}
.mtime{color:var(--falta);font-size:10.5px}

/* A barra do exemplo, com a regra desta casa: o numero vem DEPOIS da barra,
   nunca por cima dela, e a forma carrega o estado (cheia, hachurada, so
   contorno) para se ler sem cor. */
.barras{margin:16px 0}
/* A coluna do numero e `auto`, nunca uma medida fixa: com `5.4em` o
   «244 · 91,4%» nao cabia em 62 px, vazava 21 px para fora da grade e punha
   3 px de ROLAGEM LATERAL na pagina inteira no telefone. Medido em 390 px com
   o navegador, nao lendo o CSS -- interface so se prova exercitando. */
.barra-linha{display:grid;grid-template-columns:minmax(96px,13em) minmax(0,1fr) auto;
  gap:10px;align-items:center;font-size:13.5px;margin:7px 0}
.barra-linha .l{font-weight:500;overflow-wrap:break-word}
.barra-linha .t{height:14px;border-radius:3px;background:var(--papel-3);
  display:flex;overflow:hidden}
.barra-linha .t i{display:block;height:100%}
.barra-linha .t .b-feito,.barra-linha .t .b-entregue{background:var(--feito)}
.barra-linha .t .b-parcial{background:repeating-linear-gradient(45deg,var(--parcial) 0 3px,transparent 3px 7px);
  border:1.4px solid var(--parcial)}
.barra-linha .t .b-planejado{background:transparent;border:1.4px solid var(--planejado)}
.barra-linha .t .b-depois{background:transparent;border:1.4px dashed var(--depois)}
.barra-linha .t .b-aberto{background:transparent;border:1.4px solid var(--aberto)}
.barra-linha .t .b-parado{background:repeating-linear-gradient(45deg,var(--parado) 0 3px,transparent 3px 7px);
  border:1.4px solid var(--parado)}
.barra-linha .t .b-cheia{background:var(--acento);opacity:.8}
.barra-linha .n{font-family:"IBM Plex Mono",monospace;font-size:12.5px;
  font-variant-numeric:tabular-nums;color:var(--tinta-2);text-align:right;white-space:nowrap}
.legenda{display:flex;flex-wrap:wrap;gap:10px 18px;margin:12px 0 0;font-size:12.5px;
  color:var(--tinta-2)}
.legenda span{display:inline-flex;align-items:center;gap:6px}
.legenda i{display:inline-block;width:20px;height:10px;border-radius:2px;
  border:1.4px solid transparent;flex:none}
.legenda .feito i{background:var(--feito)}
.legenda .parcial i{background:repeating-linear-gradient(45deg,var(--parcial) 0 3px,transparent 3px 7px);border-color:var(--parcial)}
.legenda .planejado i{border-color:var(--planejado)}
.legenda .depois i{border-color:var(--depois);border-style:dashed}
.legenda .aberto i{border-color:var(--aberto)}
.legenda .entregue i{background:var(--entregue)}
.legenda .parado i{background:repeating-linear-gradient(45deg,var(--parado) 0 3px,transparent 3px 7px);border-color:var(--parado)}

/* Os graficos vem do `graficos-dos-testes.py`, e trazem as classes dele --
   copiar o desenho aqui seria a receita duplicada que esta pagina evita. */
figure.g{margin:0 0 24px;padding:15px 17px;border:1px solid var(--linha);
  border-radius:7px;background:var(--papel-2)}
figure.g figcaption{margin-bottom:11px}
figure.g figcaption b{font-family:"Exo 2",sans-serif;font-size:14.5px}
figure.g .un{color:var(--tinta-3);font-size:12px;margin-left:6px}
figure.g figcaption .sub{color:var(--tinta-2);font-size:13px;margin-top:5px;max-width:70ch;line-height:1.45}
figure.g .dica{display:block;color:var(--tinta-3);font-size:11.5px;margin-top:6px;font-style:italic}
/* As duas variantes vem do `barras()` do graficos-dos-testes.py, que e quem
   desenha: a LARGA (viewBox 640) e a ESTREITA (360, rotulo em cima da
   barra). A 400px a larga escalava a ~330 e o texto de 12px virava ~6px --
   visto na captura. Uma so e visivel por vez, e o corte e o mesmo de la. */
figure.g svg{width:100%;height:auto;display:block}
figure.g svg.estreito{display:none}
@media (max-width:700px){figure.g svg.largo{display:none}figure.g svg.estreito{display:block}}
figure.g svg text{font-family:"IBM Plex Mono",monospace;font-size:12px;fill:var(--tinta-2)}
figure.g svg text.rot{fill:var(--tinta)}
figure.g svg text.val{fill:var(--tinta);font-weight:500}
figure.g svg text.vazio{fill:var(--falta);font-style:italic}
figure.g svg rect{opacity:.82}
figure.g svg rect.campeao{opacity:1;stroke:var(--tinta);stroke-width:1.5}
figure.g svg line.faixa{stroke:var(--tinta);stroke-width:1.4;opacity:.55}
.ausente-bloco{border-left:3px solid var(--falta);background:var(--papel-2);
  padding:12px 16px;border-radius:0 5px 5px 0;margin:0 0 20px;color:var(--falta);font-size:14px}

/* O fluxograma vem do `fluxo-do-motor.py` e traz o proprio fundo claro; a
   moldura branca e deliberada, para ele ler igual nos dois temas. */
.figura{margin:16px 0;border:1px solid var(--linha);border-radius:8px;overflow:hidden;
  background:#fbf9f7}
.figura svg{display:block;width:100%;height:auto}
figcaption.legenda-fig{margin:9px 0 0;color:var(--tinta-3);font-size:13px;max-width:74ch}

ul{margin:11px 0;padding-left:21px;max-width:76ch} li{margin:5px 0}
.faltantes{list-style:none;margin:16px 0;padding:0;display:grid;gap:12px}
.faltantes li{border:1px solid var(--linha);border-left:3px solid var(--falta);
  border-radius:0 6px 6px 0;padding:13px 16px;background:var(--papel-2)}
.faltantes .tt{font-family:"Exo 2",sans-serif;font-weight:600;font-size:15px;display:block}
.faltantes .g{font-family:"IBM Plex Mono",monospace;font-size:12px;color:var(--falta);
  display:block;margin:5px 0}
.faltantes .d{font-size:13.5px;color:var(--tinta-2);margin:5px 0 0}

.gates{list-style:none;margin:14px 0;padding:0;display:grid;gap:11px}
.gates li{border:1px solid var(--linha);border-left:3px solid var(--parado);
  border-radius:0 6px 6px 0;padding:12px 15px;background:var(--papel-2)}
.gates .cab{display:flex;flex-wrap:wrap;gap:9px;align-items:baseline}
.gates .n{font-family:"IBM Plex Mono",monospace;font-size:12px;color:var(--tinta-3)}
.gates .tt{font-weight:600;font-size:14.5px}
.gates .tr{margin:8px 0 0;font-size:13px;color:var(--tinta-2);
  border-left:2px solid var(--linha);padding-left:10px}

footer{margin-top:52px;padding-top:18px;border-top:1px solid var(--linha);
  color:var(--tinta-3);font-size:13px;max-width:78ch}
footer code{font-size:.9em}

@media (max-width:640px){
  .barra-linha{grid-template-columns:minmax(0,1fr) auto}
  .barra-linha .t{grid-column:1/3;order:3}
  .ferramentas{position:static}
  .kpi b{font-size:23px}
}
@media print{
  .ferramentas{display:none!important}
  html,body{background:#fff!important;color:#1a1210!important}
  .cartao,.kpi,table,.nota,figure.g,.figura,.gates li,.faltantes li{break-inside:avoid}
  h2{break-after:avoid}
  .barra-titulo{border-radius:0}
  .envelope{max-width:none;padding:0}
}
@media (prefers-reduced-motion:reduce){*{transition:none!important;animation:none!important}}
</style>
"""

# O seletor de cor do texto. Cada opcao traz um par (claro, escuro) medido
# contra o papel do tema -- os dois passam 4,5:1. A do exemplo tinha «Branco»
# valendo preto no tema claro, que e um rotulo que mente sobre o proprio
# efeito; aqui a opcao diz o que faz nos dois.
JS = """<script>
(function(){
  var CORES={
    claro:{padrao:"#1a1210",vermelhao:"#c63c0a",azul:"#1f5c93",verde:"#2f7a3e",ameixa:"#7b2d6b"},
    escuro:{padrao:"#dde2eb",vermelhao:"#ff8a1c",azul:"#7fb3f0",verde:"#5cbf74",ameixa:"#e79fd6"}
  };
  var raiz=document.documentElement;
  function temaAtual(){
    var t=raiz.getAttribute("data-theme");
    if(t)return t;
    return window.matchMedia&&window.matchMedia("(prefers-color-scheme:dark)").matches?"dark":"light";
  }
  function pintar(){
    var sel=document.getElementById("cor-do-texto");
    if(!sel)return;
    var paleta=temaAtual()==="dark"?CORES.escuro:CORES.claro;
    raiz.style.setProperty("--escolhida",paleta[sel.value]||paleta.padrao);
    try{localStorage.setItem("phxsql-status-cor",sel.value)}catch(e){}
  }
  function tema(t){
    raiz.setAttribute("data-theme",t);
    document.querySelectorAll("[data-tema]").forEach(function(b){
      b.setAttribute("aria-pressed",b.dataset.tema===t?"true":"false");
    });
    try{localStorage.setItem("phxsql-status-tema",t)}catch(e){}
    pintar();
  }
  document.querySelectorAll("[data-tema]").forEach(function(b){
    b.onclick=function(){tema(b.dataset.tema)};
  });
  var sel=document.getElementById("cor-do-texto");
  if(sel)sel.onchange=pintar;
  var imp=document.getElementById("imprimir");
  if(imp)imp.onclick=function(){window.print()};
  try{
    var t=localStorage.getItem("phxsql-status-tema");
    var c=localStorage.getItem("phxsql-status-cor");
    if(c&&sel)sel.value=c;
    if(t)tema(t);else pintar();
  }catch(e){pintar()}
})();
</script>
"""


# ------------------------------------------------------------------ pedacos

def kpi(valor, rotulo, quando="", classe="", curto=False):
    c = f' class="kpi {classe}"' if classe else ' class="kpi"'
    q = f'<span class="q">{esc(quando)}</span>' if quando else ""
    b = ' class="curto"' if curto else ""
    return (f'<div{c}><b{b}>{esc(valor)}</b><span>{rotulo}</span>{q}</div>')


def barra(rotulo, partes, direita, total):
    """Uma linha de barra. `partes` = [(classe, quantidade), ...]."""
    p = []
    for classe, n in partes:
        if n and total:
            p.append(f'<i class="b-{classe}" style="width:{100.0 * n / total:.2f}%"></i>')
    return (f'<div class="barra-linha"><span class="l">{rotulo}</span>'
            f'<span class="t">{"".join(p)}</span>'
            f'<span class="n">{esc(direita)}</span></div>')


def fonte(texto):
    return f'<p class="fonte"><b>De onde sai:</b> {texto}</p>'


# ------------------------------------------------------------------ secoes

def secao_resumo(ctx):
    cap, quando_cap = ctx["cap"], ctx["quando_cap"]
    e = ctx["estados"]
    h = [H2["resumo"] + 'Resumo executivo</h2>',
         '<p class="sub">Cada cartao traz o numero e a <b>data em que ele foi '
         'medido</b>. Nenhum foi digitado.</p>', '<div class="kpis">']
    h.append(kpi(cap["versao"], "versão do motor", quando_cap, curto=True))
    h.append(kpi(milhar(cap["testes"]), "testes que passam", quando_cap))
    h.append(kpi(milhar(cap["operacoes"]), "operações do protocolo", quando_cap))
    h.append(kpi(milhar(cap["linhas_rust"]), "linhas de Rust", quando_cap))
    h.append(kpi(cap["crates"], "crates", quando_cap))
    h.append(kpi(cap["dependencias_externas"], "dependências externas",
                 quando_cap, classe="feito"))
    h.append(kpi(milhar(len(ctx["itens"])), "pedidos do dono", ctx["hoje"]))
    h.append(kpi(milhar(e["Feito"]), "pedidos feitos", ctx["hoje"], classe="feito"))
    h.append(kpi(milhar(e["Parcial"]), "pedidos parciais", ctx["hoje"], classe="parcial"))
    h.append(kpi(milhar(e["Planejado"]), "pedidos planejados", ctx["hoje"],
                 classe="planejado"))
    # «Depois da versão» (pedido 484, congelamento da 0.19) so' ganha cartao
    # quando existe algum -- zero hoje, e o cartao so' apareceria no dia em
    # que a outra frente marcar um pedido com `⏸` no PENDENCIAS.md.
    if e.get("Depois da versão"):
        h.append(kpi(milhar(e["Depois da versão"]), "pedidos depois da versão",
                     ctx["hoje"], classe="depois"))
    if ctx["n_guardas"] is not None:
        h.append(kpi(milhar(ctx["n_guardas"]), "guardas no catálogo", ctx["hoje"]))
    h.append(kpi(milhar(len(ctx["tetos"])), "catracas no fonte", ctx["hoje"]))
    h.append(kpi(f'{ctx["bancadas_medidas"]}/{ctx["bancadas_total"]}',
                 "bancadas com resultado", ctx["hoje"], curto=True))
    h.append(kpi(milhar(ctx["testes_area_total"]), "<code>#[test]</code> contados no fonte",
                 ctx["hoje"]))
    h.append("</div>")
    h.append(fonte(
        '<code>CAPABILITIES.json</code> (gravado por '
        '<code>docs/dossie/numeros-do-projeto.py</code>, que roda <code>cargo '
        'test</code>), <code>docs/PENDENCIAS.md</code> pelo <code>ler()</code> '
        'de <code>docs/dossie/pagina-dos-pedidos.py</code>, '
        '<code>bancada/guardas/catalogo.py</code>, as constantes '
        '<code>TETO_*</code> do fonte Rust e a tabela de bancadas de '
        '<code>docs/dossie/pagina-dos-testes.py</code>.'))
    return "\n".join(h)


def secao_descricao(ctx):
    cap = ctx["cap"]
    return f"""{H2['descricao']}O que o PhxSql é hoje</h2>
<p>Motor de dados em Rust no modelo de <b>arquivos separados</b> do HFSQL®, com
{esc(milhar(cap['linhas_rust']))} linhas de Rust em {esc(cap['crates'])} crates e
<b>{esc(cap['dependencias_externas'])}</b> dependências externas — só a
<code>std</code>. JSON, CRC-32, SHA-256, HMAC e PBKDF2 são escritos aqui e
conferidos contra vetor oficial. Fala um protocolo JSON de
{esc(milhar(cap['operacoes']))} operações na porta de dados, SQL pela camada
tradutora, ODBC 3.x por uma <code>cdylib</code> de ABI C, e serve o Centro de
Controle por HTTP.</p>
<p>O que ele <b>não</b> reivindica: <i>ACID compliant</i> seco. Há transação
desde o pedido 162 e a cascata do <code>ao_alterar</code> entra inteira no
conjunto de escrita, mas o isolamento entregue <b>sem pedir</b> é
<code>READ COMMITTED</code> — a leitura repetível existe pela trava e
<b>pedida</b>, e <code>SERIALIZABLE</code> não se reivindica sem prova. A folha
de marca diz <i>ACID compliant</i>; ela não é a fonte da verdade aqui.</p>
<div class="nota"><b>Esta página não é o dossiê nem o manual.</b> O dossiê
conta o projeto inteiro; o manual diz como operar; o
<code>docs/TECNOLOGIAS.md</code> inventaria o que se usou para fazer o produto
e para fazer o trabalho. Esta é a <b>sétima página</b>: o estado do projeto num
lugar só, e <b>só com o que sai de gerador</b>.</div>
{fonte('<code>CAPABILITIES.json</code>. As duas frases sobre ACID e marca saem '
       'do <code>CLAUDE.md</code> e de <code>docs/ACID.md</code> — são '
       'contrato, não medição, e por isso não trazem número novo.')}"""


def secao_crates(ctx):
    cs = ctx["crates"]
    total = sum(c["total"] for c in cs)
    cod = sum(c["codigo"] for c in cs)
    tes = sum(c["teste"] for c in cs)
    linhas = [H2["crates"] + 'Os crates, medidos</h2>',
              '<p class="sub">Linhas de <code>src/</code> classificadas linha a '
              'linha pelo extrator das tecnologias — código, teste, comentário '
              'e vazias são separados, não estimados.</p>',
              '<div class="rolo"><table><thead><tr><th>crate</th>'
              '<th class="num">arquivos</th><th class="num">código</th>'
              '<th class="num">teste</th><th class="num">total</th>'
              '<th>o que é</th></tr></thead><tbody>']
    for c in cs:
        linhas.append(
            f'<tr><td class="mono">{esc(c["nome"])}</td>'
            f'<td class="num">{milhar(c["arquivos"])}</td>'
            f'<td class="num">{milhar(c["codigo"])}</td>'
            f'<td class="num">{milhar(c["teste"])}</td>'
            f'<td class="num">{milhar(c["total"])}</td>'
            f'<td>{esc(c["descricao"])}</td></tr>')
    linhas.append(
        f'<tr><td class="mono"><b>total</b></td><td class="num"><b>'
        f'{milhar(sum(c["arquivos"] for c in cs))}</b></td>'
        f'<td class="num"><b>{milhar(cod)}</b></td>'
        f'<td class="num"><b>{milhar(tes)}</b></td>'
        f'<td class="num"><b>{milhar(total)}</b></td>'
        f'<td>proporção teste/código '
        f'<b>{esc(f"{tes / cod:.2f}".replace(".", ","))}×</b></td></tr>')
    linhas.append("</tbody></table></div>")
    linhas.append('<div class="barras">')
    maior = max(c["total"] for c in cs) or 1
    for c in cs:
        linhas.append(barra(f'<code>{esc(c["nome"])}</code>',
                            [("cheia", c["total"])], milhar(c["total"]), maior))
    linhas.append("</div>")
    linhas.append(
        '<div class="nota a"><b>Este total e o da ' + ref('resumo') + ' não são o mesmo número, '
        'e nenhum está errado.</b> Aqui são as linhas de <code>src/</code>, '
        'classificadas linha a linha; lá são <b>todos</b> os <code>.rs</code> '
        'de <code>crates/</code> — inclui <code>tests/</code> de integração e '
        'os <code>examples/</code> das bancadas. Duas receitas, dois nomes, e '
        'cada um dito onde aparece: número sem a receita ao lado é número que '
        'o próximo leitor conserta para o lado errado.</div>')
    linhas.append(fonte(
        'o <code>contar_crate()</code> de <code>docs/tecnologias/extrair.py</code> '
        '(o mesmo que escreve a tabela do <code>docs/TECNOLOGIAS.md</code>) e a '
        '<code>description</code> de cada <code>Cargo.toml</code>. A lista de '
        'crates sai da varredura de <code>crates/</code>: crate novo entra '
        'sozinho.'))
    return "\n".join(linhas)


def secao_formato(ctx):
    fs = ctx["formato"]
    linhas = [H2["formato"] + 'O formato em disco</h2>',
              '<p class="sub">Cada arquivo do formato se identifica por uma '
              'marca no cabeçalho, e a marca está no fonte — esta tabela sai '
              'dele, não de uma cópia aqui.</p>',
              '<div class="rolo"><table><thead><tr><th>constante</th>'
              '<th>marca</th><th>versões no mesmo arquivo</th><th>fonte</th>'
              '</tr></thead><tbody>']
    for f in fs:
        vs = ", ".join(f"<code>{esc(n)}</code>={esc(v)}" for n, v in f["versoes"]) or "—"
        linhas.append(
            f'<tr><td class="mono">{esc(f["const"])}</td>'
            f'<td class="mono">{esc(f["marca"])}</td><td>{vs}</td>'
            f'<td class="mono">{esc(f["fonte"])}</td></tr>')
    linhas.append("</tbody></table></div>")
    linhas.append(
        '<div class="nota"><b>A pétrea do formato:</b> a ordem de digitação é '
        'sagrada — o <code>.reg</code> nunca reaproveita slot excluído. E a '
        'regra primordial da integridade: <b>nunca se mata o pai que tem '
        'filhos</b>; <code>ao_excluir</code> aceita só <code>restringir</code>, '
        'e o par Cascata/Cascata não existe por consequência, não por uma '
        'segunda regra. A especificação byte a byte está em '
        '<code>docs/FORMATO.md</code>.</div>')
    linhas.append(fonte(
        'varredura de <code>crates/*/src/**/*.rs</code> atrás de '
        '<code>const MAGIC*: &amp;[u8; N] = b"…"</code> e dos '
        '<code>const VERSAO*</code> do mesmo arquivo. O <code>·</code> na '
        'marca é o byte <code>\\0</code> de preenchimento.'))
    return "\n".join(linhas)


def secao_fluxo(ctx):
    svg = ctx["svg_motor"]
    if not svg:
        return (H2["fluxo"] + 'O caminho de um pedido</h2>'
                '<div class="ausente-bloco">A figura <code>'
                'docs/dossie/fig-fluxo-do-motor.svg</code> não existe — rode '
                '<code>python3 docs/dossie/fluxo-do-motor.py</code>.</div>')
    return f"""{H2['fluxo']}O caminho de um pedido</h2>
<p class="sub">Os portões do <code>despachar</code> e o caminho de gravação,
desenhados <b>a partir do próprio fonte Rust</b> — portão que entrar no código
entra na figura.</p>
<figure class="figura">{svg}</figure>
<figcaption class="legenda-fig">O portão de permissão é <b>um só</b>, e o campo
que ele lê é o furo: três operações escondem tabela dele — <code>juntar</code>,
<code>unir</code> e <code>pivotar</code> —, e as três pagam conferência
própria.</figcaption>
{fonte('<code>docs/dossie/fluxo-do-motor.py</code>, que lê os portões e os '
       'passos do fonte e escreve o SVG à mão. A figura entra embutida: a '
       'página é um arquivo só.')}"""


def secao_capacidades(ctx):
    d = ctx["comparativo"]
    if not d or "__erro__" in d:
        return (H2["capacidades"] + 'O que o motor faz — medido'
                '</h2><div class="ausente-bloco">'
                '<code>bancada/comparativo/resultados.json</code> não existe — '
                'rode <code>python3 bancada/comparativo/medir.py</code>.</div>')
    motores = d.get("motores_vivos", {})
    linhas = [H2["capacidades"] + 'O que o motor faz — medido</h2>',
              '<p class="sub">A mesma pergunta feita a quatro motores '
              '<b>vivos na mesma máquina</b>, com sonda de efeito — nunca '
              '«aceitou» — e um controle positivo que todos têm de recusar.</p>',
              '<div class="kpis">',
              kpi(d.get("capacidades", "—"), "capacidades perguntadas",
                  str(d.get("quando", ""))[:10]),
              kpi(d.get("faltam_no_phxsql", "—"), "faltam no PhxSql",
                  str(d.get("quando", ""))[:10], classe="falta"),
              "</div>",
              '<div class="rolo"><table><thead><tr><th>capacidade</th>'
              '<th>PhxSql</th><th>a prova que rodou</th></tr></thead><tbody>']
    for l in d.get("linhas", []):
        estado, prova = (l.get("phxsql") or ["—", ""])[:2]
        classe = {"tem": "e-tem", "nao": "e-nao"}.get(estado, "e-parcial")
        rot = {"tem": "tem", "nao": "não tem"}.get(estado, estado)
        titulo = re.sub(r"`([^`]+)`", r"<code>\1</code>", esc(l.get("titulo", "")))
        titulo = titulo.replace("&#x27;", "'")
        linhas.append(
            f'<tr><td>{titulo}</td>'
            f'<td><span class="etiqueta {classe}">{esc(rot)}</span></td>'
            f'<td>{esc(prova)}</td></tr>')
    linhas.append("</tbody></table></div>")
    if motores:
        linhas.append('<p class="fonte"><b>Os motores vivos nesta corrida:</b> '
                      + " · ".join(f"{esc(k)} {esc(v)}" for k, v in motores.items())
                      + "</p>")
    linhas.append(fonte(
        '<code>bancada/comparativo/resultados.json</code>, medido em '
        f'<b>{esc(str(d.get("quando", ""))[:19])}</b>. O texto de cada linha é '
        'o que a sonda observou, não um veredito escrito à mão.'))
    return "\n".join(linhas)


def secao_pedidos(ctx):
    itens, e = ctx["itens"], ctx["estados"]
    # O denominador das barras EXCLUI "depois da versão" (pedido 484, formula
    # do dono no congelamento da 0.19: falta = (parcial+planejado) /
    # (feito+parcial+planejado)) -- contar os `⏸` aqui subestimaria o quanto
    # falta de verdade PARA ESTA VERSAO. Hoje `depois` e' sempre 0, entao
    # `total` continua igual a `len(itens)`.
    depois = e.get("Depois da versão", 0)
    total = e["Feito"] + e["Parcial"] + e["Planejado"]
    barras_estado = [
        barra("Feito", [("feito", e["Feito"])],
              f'{milhar(e["Feito"])} · {pct(e["Feito"], total)}%', total),
        barra("Parcial", [("parcial", e["Parcial"])],
              f'{milhar(e["Parcial"])} · {pct(e["Parcial"], total)}%', total),
        barra("Planejado", [("planejado", e["Planejado"])],
              f'{milhar(e["Planejado"])} · {pct(e["Planejado"], total)}%', total),
    ]
    legenda_spans = ['<span class="feito"><i></i>feito</span>',
                     '<span class="parcial"><i></i>parcial</span>',
                     '<span class="planejado"><i></i>planejado</span>']
    if depois:
        # Barra e legenda propria so' quando existe pelo menos um -- o
        # numero NUNCA some (fica no cartao do resumo mesmo com zero), mas
        # esta barra especifica so' nasce quando ha algo para desenhar. Sem
        # `total` proprio de proposito: ela nao e' fatia do que falta, e'
        # visivel ao lado.
        barras_estado.append(
            barra("Depois da versão", [("depois", depois)],
                  milhar(depois), depois))
        legenda_spans.append('<span class="depois"><i></i>depois da versão</span>')
    linhas = [H2["pedidos"] + 'Os pedidos do dono</h2>',
              '<p class="sub">Um por linha do <code>docs/PENDENCIAS.md</code>, '
              'lido pelo mesmo <code>ler()</code> da página dos pedidos — duas '
              'contagens divergiriam na primeira mudança de legenda. O '
              'denominador das barras é <b>feito + parcial + planejado</b> — '
              'um pedido «depois da versão» não pesa a favor nem contra o '
              'que falta agora.</p>',
              '<div class="barras">', *barras_estado, "</div>",
              '<div class="legenda">' + "".join(legenda_spans) + '</div>']
    abertos = [i for i in itens
              if i["classe"] != "feito" and i["classe"] != "depois"]
    abertos.sort(key=lambda i: i["n"], reverse=True)
    linhas.append("<h3>Os dez pedidos abertos mais recentes</h3>")
    linhas.append('<div class="rolo"><table><thead><tr><th class="num">#</th>'
                  '<th>estado</th><th>o que você pediu</th></tr></thead><tbody>')
    for i in abertos[:10]:
        classe = "e-parcial" if i["classe"] == "parcial" else "e-plan"
        linhas.append(
            f'<tr><td class="num">{i["n"]}</td>'
            f'<td><span class="etiqueta {classe}">{esc(i["rotulo"])}</span></td>'
            f'<td>{i["pedido"]}</td></tr>')
    linhas.append("</tbody></table></div>")
    linhas.append(fonte(
        '<code>docs/PENDENCIAS.md</code>, pelo <code>ler()</code> de '
        '<code>docs/dossie/pagina-dos-pedidos.py</code>. A lista inteira, com o '
        'estado de cada um, está em <code>docs/dossie/pedidos-*.html</code> — '
        'páginas por faixa de número, cortadas pelo <b>tamanho medido</b> a '
        'cada corrida (pedido 403; a página antiga, única, tinha passado de '
        '1,3 MB). O número de páginas não é fixo. '
        'Pedido com estado fora da legenda <b>para</b> o leitor com o número da '
        'linha — o 150 passou meses invisível por causa disso.'))
    return "\n".join(linhas)


def secao_gates(ctx):
    gates = ctx["gates"]
    linhas = [H2["gates"] + 'O que está travado com você</h2>',
              '<p class="sub">Pedido aberto cujo texto casa um <b>léxico '
              'explícito</b> do gerador. A frase que casou aparece: quem lê '
              'julga o casamento em vez de acreditar nele.</p>']
    if not gates:
        linhas.append('<div class="nota g">Nenhum pedido aberto casa o léxico '
                      'de travamento nesta leitura.</div>')
    else:
        linhas.append('<ul class="gates">')
        for g in gates[:12]:
            frases = " · ".join(f"«{esc(f)}»" for f in g["frases"])
            linhas.append(
                f'<li><div class="cab"><span class="n">#{g["n"]}</span>'
                f'<span class="etiqueta e-parado">{esc(g["rotulo"])}</span>'
                f'<span class="tt">{g["pedido"]}</span></div>'
                f'<p class="tr">{esc(g["trecho"])}</p>'
                f'<p class="fonte"><b>Casou:</b> {frases}</p></li>')
        linhas.append("</ul>")
        if len(gates) > 12:
            linhas.append(f'<p class="fonte">São {milhar(len(gates))} no total; '
                          f'os doze de número mais alto aparecem acima.</p>')
    linhas.append(fonte(
        'o <code>achar_gates()</code> de '
        '<code>docs/pmo/pagina-do-status-do-projeto.py</code> — o mesmo léxico '
        'do painel PMO, importado e não copiado.'))
    return "\n".join(linhas)


def secao_testes(ctx):
    cap, por_area = ctx["cap"], ctx["por_area"]
    total = ctx["testes_area_total"]
    linhas = [H2["testes"] + 'Testes e cobertura por área</h2>',
              '<p class="sub">Dois números diferentes de propósito: o de cima é '
              'o que a suíte <b>executou</b>; o de baixo é quantos '
              '<code>#[test]</code> existem no fonte, por área.</p>',
              '<div class="kpis">',
              kpi(milhar(cap["testes"]), "testes que passaram na suíte",
                  ctx["quando_cap"], classe="feito"),
              kpi(milhar(total), "<code>#[test]</code> no fonte", ctx["hoje"]),
              kpi(milhar(len(por_area)), "áreas com teste", ctx["hoje"]),
              "</div>",
              '<div class="barras">']
    maior = max(por_area.values()) if por_area else 1
    for area, n in por_area.most_common():
        linhas.append(barra(esc(area), [("cheia", n)],
                            f"{milhar(n)} · {pct(n, total)}%", maior))
    linhas.append("</div>")
    linhas.append(fonte(
        'o <code>medir()</code> de <code>docs/dossie/cobertura-por-area.py</code>, '
        'que conta <code>#[test]</code> em <code>crates/*/src</code> e '
        '<code>crates/*/tests</code>; e o campo <code>testes</code> do '
        '<code>CAPABILITIES.json</code>, que sai de um <code>cargo test</code> '
        'de verdade.'))
    return "\n".join(linhas)


def secao_guardas(ctx):
    linhas = [H2["guardas"] + 'O catálogo de guardas</h2>',
              '<p class="sub">Cada guarda é um defeito que esta casa já pagou, '
              'escrito de um jeito que a máquina consegue <b>repor</b> — e o '
              'teste nomeado tem de cair quando ele volta. Prova real nos dois '
              'sentidos.</p>', '<div class="kpis">']
    if ctx["n_guardas"] is None:
        linhas.append('<div class="ausente-bloco">o catálogo não pôde ser lido: '
                      + esc(ctx["erro_guardas"]) + "</div>")
    else:
        linhas.append(kpi(milhar(ctx["n_guardas"]), "guardas no catálogo", ctx["hoje"]))
    corrida = ctx["corrida_guardas"]
    if corrida and "guardas" in corrida:
        vereditos = {}
        for g in corrida["guardas"]:
            v = g.get("veredito", "?")
            vereditos[v] = vereditos.get(v, 0) + 1
        q = str(corrida.get("quando", ""))[:16]
        for v in sorted(vereditos):
            classe = {"PROVADA": "feito", "REDUNDANTE": "parcial"}.get(v, "falta")
            linhas.append(kpi(milhar(vereditos[v]), esc(v.lower()), q, classe=classe))
    linhas.append(kpi(milhar(len(ctx["vermelhas"])), "guardas VERMELHAS no fonte",
                      ctx["hoje"], classe="falta"))
    linhas.append("</div>")
    if ctx["vermelhas"]:
        linhas.append('<div class="rolo"><table><thead><tr><th>teste</th>'
                      '<th>o que a marca diz</th><th>fonte</th></tr></thead><tbody>')
        for nome, marca, onde in ctx["vermelhas"]:
            linhas.append(f'<tr><td class="mono">{esc(nome)}</td>'
                          f'<td>{esc(marca)}</td>'
                          f'<td class="mono">{esc(onde)}</td></tr>')
        linhas.append("</tbody></table></div>")
    linhas.append(
        '<div class="nota a"><b>Guarda quebrada não é guarda reprovada — é '
        'pior.</b> Quebrada não roda, e o catálogo continua a contando como '
        'cobertura. O contador acima é o que o catálogo <b>tem</b>; o que ele '
        '<b>provou</b> sai da última corrida do provador, que leva cerca de uma '
        'hora porque repõe o defeito e roda <code>cargo test</code> para cada '
        'entrada.</div>')
    linhas.append(fonte(
        'o <code>guardas()</code> de <code>docs/dossie/pagina-dos-testes.py</code>, '
        'que <b>importa</b> <code>bancada/guardas/catalogo.py</code> em vez de '
        'contar <code>"id":</code> por regex — contar texto mediria o arquivo, '
        'importar mede a lista que a bateria percorre; '
        '<code>bancada/guardas/ultima-corrida.json</code>; e a varredura de '
        '<code>#[ignore = "VERMELHA …"]</code> no fonte.'))
    return "\n".join(linhas)


def secao_catracas(ctx):
    tetos = ctx["tetos"]
    linhas = [H2["catracas"] + 'As catracas</h2>',
              '<p class="sub">Cada <code>TETO_*</code> é um número que <b>só '
              'desce</b>. Catraca frouxa não segura nada — e ela nunca sobe, '
              'nem quando a régua muda: régua que passa a medir mais '
              '<b>aposenta</b> a antiga e faz nascer outra, no número medido do '
              'dia.</p>',
              '<div class="rolo"><table><thead><tr><th>catraca</th>'
              '<th class="num">teto</th><th>fonte</th></tr></thead><tbody>']
    for nome, valor, onde in tetos:
        linhas.append(f'<tr><td class="mono">{esc(nome)}</td>'
                      f'<td class="num">{milhar(valor)}</td>'
                      f'<td class="mono">{esc(onde)}</td></tr>')
    linhas.append("</tbody></table></div>")
    linhas.append(fonte(
        'o <code>catracas()</code> de <code>docs/dossie/pagina-dos-testes.py</code>, '
        'que varre <code>const TETO*</code> em <code>crates/*/src/**/*.rs</code>. '
        'A lista não se digita: catraca nova que ninguém lembrasse de '
        'acrescentar sumiria da página, e a página passaria a dizer que há '
        'menos trava do que há. O catálogo com o defeito de cada uma está em '
        '<code>docs/CATRACAS.md</code>.'))
    return "\n".join(linhas)


def secao_bancadas(ctx):
    linhas = [H2["bancadas"] + 'As bancadas, com a data de '
              'cada medição</h2>',
              '<p class="sub">Os <code>resultados.json</code> são de corridas de '
              '<b>dias diferentes</b>: juntá-los sem dizer quando publicaria um '
              'retrato que nunca existiu. Bancada sem resultado aparece como '
              '<b>não medida</b>, com o comando para rodar — nunca some da '
              'tabela.</p>',
              '<div class="kpis">',
              kpi(f'{ctx["bancadas_medidas"]}/{ctx["bancadas_total"]}',
                  "bancadas com resultado", ctx["hoje"], curto=True),
              kpi(ctx["bancadas_total"] - ctx["bancadas_medidas"],
                  "bancadas <b>não medidas</b>", ctx["hoje"], classe="falta"),
              "</div>",
              '<div class="rolo"><table><thead><tr><th>bancada e o que ela '
              'prova</th><th>medida em</th><th>o que saiu</th></tr></thead><tbody>']
    for b in PROVAS.BANCADAS:
        linhas.append(PROVAS.linha_bancada(b))
    linhas.append("</tbody></table></div>")
    linhas.append(
        '<div class="nota"><b>Bancada compara trabalho igual, não só pergunta '
        'igual.</b> Os dois erros já cometidos aqui saíram do mesmo lugar e '
        'apontaram para lados opostos: um <code>WHERE id IN (…)</code> contra '
        'vinte mil buscas separadas (41× a favor do outro motor), e um '
        '<code>COUNT(*)+SUM</code> sobre 1.250.000 linhas contra a leitura de '
        '20.000 (5× a favor do nosso). Nenhum dos dois aparecia no número.</div>')
    linhas.append(fonte(
        'a tabela <code>BANCADAS</code> e o <code>linha_bancada()</code> de '
        '<code>docs/dossie/pagina-dos-testes.py</code>, importados. A data sai '
        'do próprio resultado quando ele a traz; quando sai do '
        '<code>mtime</code>, a linha <b>diz</b> <span class="mtime">(mtime)'
        '</span> — <code>mtime</code> é a hora em que alguém gravou, não a hora '
        'em que se mediu.'))
    return "\n".join(linhas)


def secao_desempenho(ctx):
    blocos, quando = ctx["trio"]
    linhas = [H2["desempenho"] + 'Desempenho — os quatro '
              'motores a um milhão</h2>',
              '<p class="sub">Os quatro na <b>mesma rodada</b>, intercalados. '
              'Cada barra traz a faixa min–max, e o vencedor só é contornado '
              'quando as faixas <b>não se cruzam</b> — esta casa já declarou '
              'vencedor dentro do ruído uma vez.</p>']
    linhas.extend(blocos)
    linhas.append(fonte(
        'o <code>g_tres_motores()</code> de '
        '<code>docs/dossie/graficos-dos-testes.py</code>, que lê '
        '<code>bancada/comparacao/um-milhao.json</code> — medido em '
        f'<b>{esc(quando[0])}</b>. O SVG é escrito à mão, sem biblioteca de '
        'gráfico, e a regra da faixa é a do pedido 155.'))
    return "\n".join(linhas)


def secao_replicacao(ctx):
    d = ctx["replicacao"]
    if not d or "__erro__" in d:
        return (H2["replicacao"] + 'Replicação</h2>'
                '<div class="ausente-bloco">'
                '<code>bancada/replicacao/resultados.json</code> não existe — '
                'rode a bancada da replicação.</div>')
    q = str(d.get("quando", ""))[:10]
    cascata = d.get("cascata", {})
    cl = ctx["cluster"] or {}
    linhas = [H2["replicacao"] + 'Replicação, medida com '
              'quatro servidores</h2>',
              '<p class="sub">Um dos dois pilares que a folha de marca promete '
              '— e este <b>virou verdade</b>: quatro processos '
              '<code>phxsqld</code>, retrato SHA-256 de cada linha conferido no '
              'fim.</p>', '<div class="kpis">',
              kpi(milhar(d.get("master_linhas_s", 0)), "linhas/s no master", q),
              kpi(milhar(d.get("replica_eventos_s", 0)), "eventos/s na réplica", q),
              kpi(str(d.get("alcance_s", "—")).replace(".", ",") + " s",
                  "para a réplica alcançar", q, curto=True),
              kpi(milhar(d.get("linhas", 0)), "linhas replicadas", q),
              kpi("sim" if d.get("iguais_no_fim") else "NÃO",
                  "os dois lados iguais no fim", q,
                  classe="feito" if d.get("iguais_no_fim") else "falta", curto=True),
              ]
    if cascata:
        linhas.append(kpi(f'{milhar(cascata.get("um_salto_ms", 0))} → '
                          f'{milhar(cascata.get("dois_saltos_ms", 0))} ms',
                          "um salto → dois saltos", q, curto=True))
    if cl:
        linhas.append(kpi(str(cl.get("promocao_s", "—")).replace(".", ",") + " s",
                          "promoção automática no cluster",
                          ctx["quando_cluster"], curto=True))
    linhas.append("</div>")
    if d.get("topologia"):
        linhas.append(f'<p class="fonte"><b>Topologia da corrida:</b> '
                      f'<code>{esc(d["topologia"])}</code> — '
                      f'{esc(d.get("maquina", ""))}</p>')
    if d.get("atraso_ms"):
        linhas.append('<div class="rolo"><table><thead><tr><th>o que se fez no '
                      'master</th><th class="num">atraso até a réplica (ms)</th>'
                      '</tr></thead><tbody>')
        for k, v in d["atraso_ms"].items():
            linhas.append(f'<tr><td>{esc(k)}</td>'
                          f'<td class="num">{milhar(v)}</td></tr>')
        linhas.append("</tbody></table></div>")
    if d.get("atraso_ms_inclui"):
        linhas.append(f'<div class="nota a"><b>O que este atraso inclui:</b> '
                      f'{esc(d["atraso_ms_inclui"])}</div>')
    linhas.append(fonte(
        '<code>bancada/replicacao/resultados.json</code> e '
        '<code>bancada/cluster/resultados.json</code>, cada um com a data da '
        'própria corrida. O desenho está em <code>docs/REPLICACAO.md</code>.'))
    return "\n".join(linhas)


def secao_idiomas(ctx):
    idi = ctx["cap"].get("idiomas") or {}
    linhas = [H2["idiomas"] + 'A fábrica de idiomas</h2>',
              '<p class="sub">Texto de tela entra pela fábrica — é pétreo. A '
              'máquina existe desde a 0.17.0; o que faltava era o laço que '
              '<b>conta</b>, e ele é uma catraca que só desce.</p>',
              '<div class="kpis">',
              kpi(milhar(idi.get("fabrica", 0)), "textos pela fábrica",
                  ctx["quando_cap"], classe="feito"),
              kpi(milhar(idi.get("fora", 0)), "ainda cravados no fonte",
                  ctx["quando_cap"], classe="falta"),
              kpi(milhar(idi.get("teto", 0)), "o teto da catraca", ctx["quando_cap"]),
              kpi(f'{idi.get("pct", 0)}%', "da tela pela fábrica",
                  ctx["quando_cap"], curto=True),
              "</div>",
              '<div class="barras">',
              barra("pela fábrica", [("feito", idi.get("fabrica", 0))],
                    milhar(idi.get("fabrica", 0)), idi.get("total", 1)),
              barra("cravado no fonte", [("parcial", idi.get("fora", 0))],
                    milhar(idi.get("fora", 0)), idi.get("total", 1)),
              "</div>",
              '<div class="nota"><b>Três armadilhas já pagas:</b> rótulo se '
              'traduz, dado nunca (o conferidor apaga tudo o que a página '
              '<b>interpola</b> antes de varrer); texto se resolve por '
              '<b>chave</b>, nunca por comparação da frase; e chave morta é pior '
              'que chave faltando, porque o tradutor a vê, traduz nos seis '
              'idiomas e nada muda na tela.</div>']
    linhas.append(fonte(
        'o campo <code>idiomas</code> do <code>CAPABILITIES.json</code>, que '
        'sai do conferidor <code>textos-fora-da-fabrica</code> rodado por '
        '<code>docs/dossie/numeros-do-projeto.py</code>. O procedimento de '
        'acrescentar um texto está em <code>docs/MENSAGENS.md</code>.'))
    return "\n".join(linhas)


def secao_dependencias(ctx):
    cap = ctx["cap"]
    return f"""{H2['dependencias']}Zero dependências externas</h2>
<p class="sub">Não é ascetismo: é o que fez a compilação cruzada para Windows
funcionar de primeira e o que permite <code>cargo build --offline</code>.</p>
<div class="kpis">
{kpi(cap['dependencias_externas'], 'pacotes de terceiros no <code>Cargo.lock</code>',
     ctx['quando_cap'], classe='feito')}
{kpi(cap['crates'], 'crates, todos deste projeto', ctx['quando_cap'])}
</div>
<p>JSON, CRC-32, SHA-256, HMAC e PBKDF2 são escritos aqui e conferidos contra
<b>vetor oficial</b> — FIPS 180-4, RFC 4231 e os vetores de PBKDF2. O método é
sempre o mesmo: ler a norma, entender, reescrever, provar contra vetor. Não é
refatoração de implementação alheia, e a diferença não aparece no código
pronto: aparece na pergunta «onde esta lógica <b>diverge</b> da de origem, e
qual restrição nossa causou a divergência?».</p>
<div class="nota c"><b>Onde a convergência dos motores maduros bate na
pétrea.</b> PostgreSQL®, MariaDB® e MySQL® cifram a conexão; por «três motores
convergindo é aceite automático» seríamos obrigados a fazer igual. Mas cifrar
pede biblioteca, e zero dependências é pétrea: o <b>comportamento</b> entra
como meta, o <b>meio</b> não passa sem o dono. O choque aparece — não se aceita
calado nem se ignora calado.</div>
{fonte('o <code>dependencias_externas()</code> de '
       '<code>docs/dossie/numeros-do-projeto.py</code>, que conta '
       '<code>[[package]]</code> no <code>Cargo.lock</code> e subtrai os crates '
       'deste projeto. Se der outra coisa que zero, a página diz isso em vez de '
       'publicar o zero por hábito.')}"""


def secao_documentacao(ctx):
    linhas = [H2["documentacao"] + 'Documentação</h2>',
              '<p class="sub">Esta seção <b>nasceu</b> porque o gerador existe: '
              'a lista dos documentos contados sai do código, não de uma cópia '
              'aqui.</p>', '<div class="kpis">',
              kpi(milhar(len(ctx["docs"])), "documentos contados", ctx["hoje"]),
              kpi(milhar(ctx["linhas_doc"]), "linhas de documentação", ctx["hoje"]),
              kpi(milhar(len(ctx["cognicoes"])), "arquivos de cognição", ctx["hoje"]),
              kpi(milhar(len(ctx["paginas"])), "páginas geradas e publicadas",
                  ctx["hoje"]),
              kpi(milhar(ctx["n_geradores"]), "geradores no portão", ctx["hoje"]),
              "</div>",
              "<h3>As páginas que esta casa gera</h3>",
              '<div class="rolo"><table><thead><tr><th>página</th>'
              '<th class="num">KiB</th><th>quem a escreve</th></tr></thead><tbody>']
    for p, quem in ctx["paginas_com_dono"]:
        # O tamanho DESTA pagina nao entra: escreve-lo muda o tamanho dela, e o
        # numero seguinte ja e outro. Nao e escrupulo teorico -- o portao dos
        # geradores pegou isto em cheio (103 -> 104 KiB a cada corrida), e um
        # numero que nunca chega a ponto fixo deixaria o portao VERMELHO para
        # sempre, que e o jeito de um portao virar ruido que ninguem le.
        if p == ctx["saida"]:
            tam = '<span class="mtime">— (esta página)</span>'
        else:
            tam = milhar(p.stat().st_size // 1024)
        linhas.append(
            f'<tr><td class="mono">{esc(str(p.relative_to(RAIZ)))}</td>'
            f'<td class="num">{tam}</td>'
            f'<td class="mono">{esc(quem)}</td></tr>')
    linhas.append("</tbody></table></div>")
    linhas.append(
        '<p class="fonte">O tamanho <b>desta</b> página não aparece de '
        'propósito: escrevê-lo muda o tamanho dela, e o número seguinte já '
        'seria outro — número que não chega a ponto fixo não é número medido, '
        'é número que persegue a própria cauda.</p>')
    linhas.append("<h3>Os dez maiores documentos</h3>")
    linhas.append('<div class="barras">')
    maiores = sorted(ctx["docs_linhas"], key=lambda x: x[1], reverse=True)[:10]
    maior = maiores[0][1] if maiores else 1
    for nome, n in maiores:
        linhas.append(barra(f'<code>{esc(nome)}</code>', [("cheia", n)],
                            milhar(n), maior))
    linhas.append("</div>")
    linhas.append(fonte(
        'o <code>arquivos_de_doc()</code> e o <code>linhas_de_doc()</code> de '
        '<code>docs/dossie/numeros-do-projeto.py</code> — a mesma receita que '
        'alimenta o <code>CAPABILITIES.json</code>, e por isso os dois números '
        'não podem divergir. As páginas saem dos <b>alvos</b> do '
        '<code>PLANO</code> de <code>docs/dossie/portao-dos-geradores.py</code>: '
        'página nova entra na conta sozinha no dia em que o gerador dela entrar '
        'no portão.'))
    return "\n".join(linhas)


def secao_pacotes(ctx):
    zips, somas = ctx["pacotes"]
    linhas = [H2["pacotes"] + 'O que se baixa</h2>',
              '<p class="sub">O exemplo do P.O.S tinha «ISO bootável»; o nosso '
              'equivalente é o <b>pacote de fontes e binários</b>, montado por '
              'script e <b>nunca à mão</b> — pacote feito à mão é pacote que '
              'ninguém consegue refazer igual.</p>']
    if not zips:
        linhas.append('<div class="ausente-bloco">Não há zip em '
                      '<code>pacotes/</code> — rode <code>./empacotar.sh</code>.'
                      '</div>')
    else:
        linhas.append('<div class="rolo"><table><thead><tr><th>pacote</th>'
                      '<th class="num">MiB</th><th>gerado em</th>'
                      '<th>no <code>SHA256SUMS</code>?</th></tr></thead><tbody>')
        for z in zips:
            marca = ('<span class="etiqueta e-tem">sim</span>' if z["somado"]
                     else '<span class="etiqueta e-nao">NÃO</span>')
            mib = f"{z['mib']:.1f}".replace(".", ",")
            linhas.append(
                f'<tr><td class="mono">{esc(z["nome"])}</td>'
                f'<td class="num">{esc(mib)}</td>'
                f'<td class="q">{esc(z["quando"])}</td><td>{marca}</td></tr>')
        linhas.append("</tbody></table></div>")
    linhas.append(
        '<div class="nota"><b>Cada zip traz o próprio conferidor.</b> '
        '<code>./phxsql conferir-pacote</code> viaja dentro do pacote e '
        'responde ÍNTEGRO ou o que difere, falta <b>e veio a mais</b> — o '
        'arquivo a mais é o que a conferência de hash comum não vê. O segundo '
        'caminho, que não depende de rodar nada do zip, é '
        '<code>sha256sum -c MANIFESTO.sha256</code>.</div>')
    linhas.append(fonte(
        'varredura de <code>pacotes/*.zip</code> — tamanho e <code>mtime</code> '
        'reais do arquivo — cruzada com <code>pacotes/SHA256SUMS</code>. O '
        'tamanho é o do arquivo, não um número de catálogo; a data é a de '
        'gravação, e por isso ela diz <code>mtime</code> e não «medido».'))
    return "\n".join(linhas)


def secao_board(ctx):
    board = ctx["board"]
    por_pilar = {}
    for b in board:
        d = por_pilar.setdefault(b["pilar"], {"aberto": 0, "entregue-fechado": 0,
                                              "parado": 0})
        d[b["estado"]] = d.get(b["estado"], 0) + 1
    linhas = [H2["board"] + 'O board, por pilar</h2>',
              '<p class="sub">A forma carrega o estado — aberto só contorno, '
              'entregue cheia, parado hachurado —, então a barra se lê sem '
              'cor.</p>', '<div class="barras">']
    maior = max((sum(v.values()) for v in por_pilar.values()), default=1)
    for pilar in sorted(por_pilar):
        v = por_pilar[pilar]
        t = sum(v.values())
        linhas.append(barra(
            esc(pilar),
            [("entregue", v.get("entregue-fechado", 0)),
             ("parado", v.get("parado", 0)), ("aberto", v.get("aberto", 0))],
            f'{milhar(t)}', maior))
    linhas.append("</div>")
    linhas.append('<div class="legenda"><span class="entregue"><i></i>entregue '
                  'ou fechado</span><span class="parado"><i></i>parado</span>'
                  '<span class="aberto"><i></i>aberto</span></div>')
    fortes = [b for b in board if b["escalao"] == "forte" and b["estado"] == "aberto"]
    linhas.append('<div class="kpis">')
    linhas.append(kpi(milhar(len(board)), "itens no board", ctx["hoje"]))
    for est, rot, classe in (("aberto", "abertos", ""),
                             ("entregue-fechado", "entregues ou fechados", "feito"),
                             ("parado", "parados", "falta")):
        n = sum(1 for b in board if b["estado"] == est)
        linhas.append(kpi(milhar(n), rot, ctx["hoje"], classe=classe))
    linhas.append(kpi(milhar(len(fortes)), "abertos de escalão <b>forte</b>",
                      ctx["hoje"], classe="falta"))
    linhas.append("</div>")
    linhas.append(fonte(
        'o <code>ler()</code> de <code>docs/pmo/rollup.py</code>, sobre '
        '<code>docs/pmo/BACKLOG.md</code>. O estado é a <b>primeira palavra</b> '
        'da última célula; palavra fora do léxico <b>para</b> o leitor '
        'nomeando a linha. O escalão aparece pelo <b>nível</b>, nunca por nome '
        'de modelo.'))
    return "\n".join(linhas)


def secao_frentes(ctx):
    linhas = [H2["frentes"] + 'As últimas frentes</h2>',
              '<p class="sub">Do próprio <code>git log</code>, só leitura. O '
              'commit conta a decisão e o motivo, não a lista de arquivos.</p>',
              '<div class="rolo"><table><thead><tr><th>commit</th><th>quando</th>'
              '<th>o que decidiu</th></tr></thead><tbody>']
    for f in ctx["frentes"]:
        linhas.append(f'<tr><td class="mono">{esc(f["hash"])}</td>'
                      f'<td class="q">{esc(f["data"])}</td>'
                      f'<td>{esc(f["assunto"])}</td></tr>')
    linhas.append("</tbody></table></div>")
    linhas.append(fonte(
        f'<code>git log --no-merges -n {QUANTAS_FRENTES}</code> na raiz do '
        'repositório. Commit novo muda esta seção — e aí ela estava velha '
        'mesmo, que é o que o portão dos geradores existe para pegar.'))
    return "\n".join(linhas)


def secao_nao_nasceram(ctx):
    # A QUANTIDADE tambem sai da lista. Ate 16/09/2026 este paragrafo dizia
    # «Quatro secoes do exemplo nao entraram» com o numero escrito -- e nesse
    # mesmo dia as quatro viraram duas (pedido 264) e depois ZERO (265 e 266),
    # com a frase intacta. Numero digitado envelhece calado, inclusive dentro
    # da secao que existe para dizer o que falta.
    quantas = len(SECOES_SEM_GERADOR)
    if quantas:
        sub = (f'{quantas} seç{"ões" if quantas > 1 else "ão"} do exemplo não '
               f'entr{"aram" if quantas > 1 else "ou"}, e a razão é a mesma '
               'para todas: <b>o número não sai de gerador nenhum</b>. Elas '
               'aparecem aqui com o gerador que falta — página que esconde o '
               'que não mediu é a pior de todas.')
    else:
        sub = ('<b>Nenhuma</b> — todas as seções desta página nascem de um '
               'gerador. Esta seção continua existindo, e continua sendo a '
               'mais importante: ela é o lugar onde a próxima seção sem '
               'gerador vai aparecer em vez de entrar com número digitado.')
    linhas = [H2["nao_nasceram"] + 'O que <b>não</b> nasceu '
              'nesta página</h2>',
              f'<p class="sub">{sub}</p>']
    if quantas:
        linhas.append('<ul class="faltantes">')
        for s in SECOES_SEM_GERADOR:
            linhas.append(
                f'<li><span class="tt">{esc(s["titulo"])} '
                f'<span class="etiqueta e-nao">não nasceu</span></span>'
                f'<span class="g">falta: {s["gerador"]}</span>'
                f'<p class="d">Tiraria o número {s["de_onde"]}</p>'
                f'<p class="fonte">{esc(s["exemplo"])} · pedido '
                f'<b>#{s["pedido"]}</b> no <code>docs/PENDENCIAS.md</code>'
                '</p></li>')
        linhas.append("</ul>")
    else:
        linhas.append(
            '<div class="nota g"><b>As quatro que faltavam nasceram em '
            '16/09/2026, e nenhuma nasceu pelo atalho.</b> Riscos e Dívida '
            'técnica (pedido 264) exigiram um <code>docs/RISCOS.md</code> '
            'tabelado e uma marca <code>// DIVIDA:</code> no fonte; Telemetria '
            'e logs (265) exigiu que a bancada passasse a gravar '
            '<code>resultados.json</code>, porque sem ele não havia a data em '
            'que o número foi medido; Antes × Depois (266) exigiu uma série '
            'versionada, porque o <code>CAPABILITIES.json</code> é sobrescrito '
            'e só guarda o agora. Nos quatro casos <b>a fonte veio antes do '
            'gerador</b> — é isso que a lei cobra, e é por isso que ela demora '
            'mais que digitar o número.</div>')
    linhas.append(
        '<div class="nota v"><b>Por que isto é a seção mais importante da '
        'página.</b> O exemplo trazia <b>677 números no texto visível, todos '
        'digitados à mão</b>. Adotá-lo como veio criaria 677 lugares onde um '
        'número envelhece calado — e esta casa já pagou isso quatro vezes: o '
        'selo da capa parado numa versão por quatro lançamentos, 198 pedidos '
        'onde eram 203, 428 testes onde eram 451, 780 KiB de interface onde '
        'eram 1.032. A quarta ensina o alcance da lei: <b>gerador certo chamado '
        'pela metade entrega número velho anunciando sucesso.</b></div>')
    return "\n".join(linhas)


# ------------------------------------------------------------------ montagem

def montar(saida=PADRAO):
    hoje = hoje_iso()
    cap, (quando_cap, cap_mtime) = PROVAS.capabilities()
    for k in ("versao", "testes", "operacoes", "crates", "linhas_rust",
              "dependencias_externas", "idiomas", "medido_em", "commit", "branch"):
        if k not in cap:
            raise SystemExit(
                f"CAPABILITIES.json nao traz `{k}` -- campo que falta nao vira "
                "zero na pagina. Rode: flock /tmp/phx-cargo.lock python3 "
                "docs/dossie/numeros-do-projeto.py")

    itens = PEDIDOS.ler()
    estados = {"Feito": 0, "Parcial": 0, "Planejado": 0}
    for i in itens:
        estados[i["rotulo"]] = estados.get(i["rotulo"], 0) + 1
    gates = PMO.achar_gates(itens)
    gates.sort(key=lambda g: g["n"], reverse=True)
    board = BOARD.ler()

    n_guardas, erro_guardas = PROVAS.guardas()
    tetos = PROVAS.catracas()
    vermelhas = PROVAS.guardas_vermelhas()
    corrida, _ = PROVAS.ler_json("bancada/guardas/ultima-corrida.json")

    por_area, _sem_teste = COBERTURA.medir()
    comparativo, _ = PROVAS.ler_json("bancada/comparativo/resultados.json")
    replicacao, _ = PROVAS.ler_json("bancada/replicacao/resultados.json")
    cluster, p_cluster = PROVAS.ler_json("bancada/cluster/resultados.json")
    quando_cluster = PROVAS.quando_de(p_cluster, cluster)[0] if cluster else "—"

    medidas = sum(1 for b in PROVAS.BANCADAS if (RAIZ / b["json"]).exists())

    svg_motor = ""
    fig = D / "fig-fluxo-do-motor.svg"
    if fig.exists():
        svg_motor = fig.read_text(encoding="utf-8")
        svg_motor = re.sub(r"^<\?xml[^>]*\?>\s*", "", svg_motor).strip()
        # A versao SOLTA da figura traz um `<style>svg{...}</style>` proprio,
        # pensado para arquivo isolado. Embutido, `<style>` de SVG inline e
        # CSS da PAGINA INTEIRA: a regra `svg{background:#fbf9f7}` pintou de
        # claro os quatro graficos de barras da secao 15 no tema escuro, com
        # texto claro em cima (1,24:1, medido). Achado exercitando a pagina;
        # lendo o codigo nao aparece. O seletor e' preso a ESTA figura antes
        # de embutir -- a moldura clara do fluxograma continua deliberada.
        svg_motor = svg_motor.replace("<style>svg{", "<style>.figura>svg{", 1)

    paginas, portao = paginas_geradas()
    faixas_pedidos = getattr(portao, "PEDIDOS_FAIXAS", None)
    dono_da_pagina = {}
    dono_das_faixas_pedidos = None
    for script, alvos, _modo, _porque in portao.PLANO:
        for a in alvos:
            # O marcador PEDIDOS_FAIXAS nao e' uma chave de pagina -- e' o
            # dono de TODO o conjunto `pedidos-*.html` (pedido 403).
            # Registrar como chave literal deixaria cada faixa cair no
            # fallback do DOSSIE, atribuindo o script ERRADO.
            if faixas_pedidos is not None and a == faixas_pedidos:
                dono_das_faixas_pedidos = script
                continue
            dono_da_pagina.setdefault(a, script)
    com_dono = []
    pasta_pedidos = RAIZ / "docs" / "dossie"
    for p in paginas:
        rel = str(p.relative_to(RAIZ))
        if (dono_das_faixas_pedidos and p.parent == pasta_pedidos
                and p.name.startswith("pedidos-")):
            quem = dono_das_faixas_pedidos
        else:
            quem = dono_da_pagina.get(rel) or dono_da_pagina.get(portao.DOSSIE, "—")
        if "/" not in quem:
            quem = f"docs/dossie/{quem}"
        com_dono.append((p, quem))

    docs = NUMS.arquivos_de_doc()
    docs_linhas = [(d.name, len(d.read_text(encoding="utf-8", errors="replace")
                                .splitlines())) for d in docs]

    ctx = {
        "hoje": hoje,
        "saida": pathlib.Path(saida).resolve(),
        "cap": cap,
        "quando_cap": quando_cap + (" (mtime)" if cap_mtime else ""),
        "itens": itens,
        "estados": estados,
        "gates": gates,
        "board": board,
        "n_guardas": n_guardas,
        "erro_guardas": erro_guardas,
        "tetos": tetos,
        "vermelhas": vermelhas,
        "corrida_guardas": corrida,
        "por_area": por_area,
        "testes_area_total": sum(por_area.values()),
        "comparativo": comparativo,
        "replicacao": replicacao,
        "cluster": cluster,
        "quando_cluster": quando_cluster,
        "bancadas_total": len(PROVAS.BANCADAS),
        "bancadas_medidas": medidas,
        "crates": crates_medidos(),
        "formato": formato_em_disco(),
        "svg_motor": svg_motor,
        "trio": GRAF.g_tres_motores(),
        "pacotes": pacotes(),
        "frentes": frentes(),
        "docs": docs,
        "docs_linhas": docs_linhas,
        "linhas_doc": sum(n for _, n in docs_linhas),
        "cognicoes": cognicoes(),
        "paginas": paginas,
        "paginas_com_dono": com_dono,
        "n_geradores": len(portao.PLANO),
    }

    # As duas secoes do pedido 264 vem do `riscos.py`, e vem CERCADAS pelas
    # marcas dele: e' o que deixa aquele gerador reescrever so este pedaco em
    # segundos, sem re-rodar esta pagina inteira (minutos, porque ela varre
    # `crates/` cinco vezes). A montagem e a MESMA dos dois lados -- o
    # `RISCOS.bloco()` repete este `\n\n` -- para que rodar um depois do outro
    # nao mude um byte, e o portao nao acuse VELHO sem numero nenhum ter
    # mudado.
    ctx_risco = RISCOS.contexto(ctx["hoje"])
    r_riscos, r_divida = RISCOS.secoes(sys.modules[__name__], ctx_risco)

    # O mesmo desenho para as duas secoes dos pedidos 265 e 266: cada uma vem
    # do seu gerador, cercada pelas marcas dele. Os tres geradores repetem aqui
    # exatamente a montagem que fazem sozinhos -- se divergissem num espaco em
    # branco, o portao acusaria a pagina como VELHA sem numero nenhum mudar.
    ctx_tlm = TELEMETRIA.contexto()
    ctx_tlm["hoje"] = ctx["hoje"]
    # O `main()` precisa saber se a bancada rodou para dizer que fez menos --
    # e quem ja leu o arquivo foi este passo. Reler la seria uma segunda
    # leitura que pode discordar desta.
    global ctx_tlm_publico
    ctx_tlm_publico = ctx_tlm
    ctx_serie = SERIE.contexto()

    secoes = [
        secao_resumo(ctx), secao_descricao(ctx), secao_crates(ctx),
        secao_formato(ctx), secao_fluxo(ctx), secao_capacidades(ctx),
        secao_pedidos(ctx), secao_gates(ctx),
        RISCOS.MARCA_INICIO + "\n" + r_riscos, r_divida + "\n" + RISCOS.MARCA_FIM,
        secao_testes(ctx),
        secao_guardas(ctx), secao_catracas(ctx), secao_bancadas(ctx),
        secao_desempenho(ctx), secao_replicacao(ctx),
        TELEMETRIA.bloco(sys.modules[__name__], ctx_tlm),
        secao_idiomas(ctx),
        secao_dependencias(ctx), secao_documentacao(ctx), secao_pacotes(ctx),
        secao_board(ctx), secao_frentes(ctx),
        SERIE.bloco(sys.modules[__name__], ctx_serie),
        secao_nao_nasceram(ctx),
    ]
    if len(secoes) != len(ORDEM):
        raise SystemExit(
            f"{len(secoes)} secoes montadas para {len(ORDEM)} chaves na ORDEM "
            "-- a numeracao sai da ORDEM, entao uma secao que nasce sem chave "
            "(ou uma chave sem secao) desloca todas as outras em silencio.")

    agora = datetime.datetime.now(datetime.timezone.utc).strftime("%d/%m/%Y %H:%M")
    cabecalho = f"""<header class="barra-titulo">
  <h1>STATUS DO PROJETO — <span class="x">PhxSql</span></h1>
  <div class="sub">versão {esc(cap['versao'])} · commit {esc(cap['commit'][:8])}
  {' · árvore suja' if cap.get('sujo') else ''} · branch {esc(cap['branch'])} ·
  motor medido em {esc(cap['medido_em'])}</div>
  <div class="assinatura">Built to store. Engineered to scale.</div>
</header>"""

    ferramentas = """<div class="ferramentas no-print">
  <span class="r">tema</span>
  <button data-tema="light" aria-pressed="false">claro</button>
  <button data-tema="dark" aria-pressed="false">escuro</button>
  <span class="r">cor do título</span>
  <select id="cor-do-texto" aria-label="cor dos títulos">
    <option value="padrao">padrão</option>
    <option value="vermelhao">vermelhão</option>
    <option value="azul">azul</option>
    <option value="verde">verde</option>
    <option value="ameixa">ameixa</option>
  </select>
  <span class="sep"></span>
  <button id="imprimir">imprimir / PDF</button>
</div>"""

    rodape = f"""<footer>
  Gerado por <code>docs/status/pagina-do-status-do-projeto.py</code> em {agora} UTC,
  pelo comando <code>./status-html.sh</code>. <b>Nenhum número desta página foi
  digitado</b>: cada um sai de um gerador, e cada seção diz de qual. Onde o
  gerador não existe, a seção <b>não nasceu</b> — está nomeada na {ref('nao_nasceram')}, com o
  gerador que falta. Esta página <b>não se edita</b>: mexeu numa fonte, rode o
  gerador. Como rodar e de onde sai cada número:
  <code>docs/status/LEIA-ME.md</code>.
</footer>"""

    corpo = "\n\n".join(secoes)
    return (CABECA + CSS + cabecalho + '<div class="envelope">\n'
            + ferramentas + "\n\n" + corpo + "\n" + rodape + "\n</div>\n" + JS)


def main():
    saida = pathlib.Path(sys.argv[1]) if len(sys.argv) > 1 else PADRAO
    html_saida = montar(saida)
    saida.parent.mkdir(parents=True, exist_ok=True)
    saida.write_text(html_saida, encoding="utf-8")
    kib = len(html_saida.encode("utf-8")) / 1024
    print(f"pagina gravada: {saida.relative_to(RAIZ)}  ({kib:.0f} KiB)")
    # Gerador que faz menos do que o nome promete TEM de dizer que fez menos:
    # o `pagina-dos-pedidos.py` gravava tres coisas, imprimia tres linhas de
    # exito e pulava o painel do dossie -- e tres paineis ficaram atrasados sem
    # um digito digitado.
    if SECOES_SEM_GERADOR:
        print(f"  {len(SECOES_SEM_GERADOR)} secao(oes) NAO nasceram por falta "
              f"de gerador ({ref('nao_nasceram')} da pagina):")
        for s in SECOES_SEM_GERADOR:
            falta = re.sub(r"\s+([,.;])", r"\1", texto_puro(s["gerador"]))
            print(f"    - {s['titulo']}: falta {falta} (pedido #{s['pedido']})")
    else:
        print(f"  nenhuma secao ficou sem gerador ({ref('nao_nasceram')} da "
              "pagina diz «nenhuma», e continua existindo para a proxima)")
    # As secoes que vem de gerador IRMAO dizem, elas mesmas, o que nao mediram:
    # a pagina inteira nao pode engolir isso, senao quem roda `./status-html.sh`
    # nunca ouve falar da bancada que nao rodou.
    dados = ctx_tlm_publico.get("dados") if ctx_tlm_publico else None
    if dados is None or "__erro__" in (dados or {}):
        print("  a bancada de telemetria NAO rodou: a secao saiu como NAO "
              "MEDIDA, com o comando (pedido #265)")
    if len(SERIE.ler()) < 2:
        print("  a serie tem menos de duas medicoes: a secao «Antes x Depois» "
              "diz que nao da para comparar, e nao inventa o passado")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
