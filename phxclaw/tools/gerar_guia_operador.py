#!/usr/bin/env python3
"""Gera os trechos medidos do docs/GUIA_DO_OPERADOR.md -- nenhum comando, opcao, variavel de
ambiente, comando de segredo ou numero e digitado no guia.

O guia escreve a prosa; cada fato que existe no codigo entra por um marcador
<!-- gerado:TIPO:ARG:inicio --> ... <!-- gerado:TIPO:ARG:fim -->, e o ARG do marcador e o
que se pede. A lista do que o guia mostra mora nos marcadores do proprio guia, e nao numa
copia aqui: acrescentar um comando ao guia e escrever o marcador.

  cabecalho            <- data, versao e idade do binario (e o aviso se ele e mais velho
                          que o fonte)
  ajuda:CMD            <- `phxclaw ajuda CMD` (o texto que o binario imprime)
  precedencia          <- as linhas "//" do topo de `phxclaw config exemplo`
  chaves:P1,P2         <- `phxclaw config mostrar --json` numa pasta vazia: as chaves cuja
                          chave comeca com P1 ou P2 (variavel, tipo, padrao, natureza)
  segredos             <- o mesmo catalogo: toda chave segredo e o comando que a guarda
  ferramentas:N1,N2    <- `phxclaw ferramentas` (capacidade, padrao, descricao); a que nao
                          montou nesta maquina sai como NAO MONTADA, com o arquivo do fonte
  mcp-presets          <- os bracos de `fn preset` em crates/phxclaw-agent/src/mcp.rs
  http-config          <- os StatusCode das rotas de crates/phxclaw-agent/src/config.rs
  medicoes-ui          <- o ultimo docs/ui/fidelidade/<tipo>-*.json de cada tipo que o
                          `phxclaw ui` grava; tipo sem arquivo sai como NAO MEDIDA, com o
                          comando para rodar

E o gerador DIZ quando faz menos: binario mais velho que o fonte, ferramenta que nao montou,
medicao que nao existe e marcador de tipo desconhecido vao para uma secao de avisos na
saida, fora das linhas de exito -- e o marcador desconhecido reprova.

Uso: python3 tools/gerar_guia_operador.py [binario]   (padrao: target/debug/phxclaw)
"""

import datetime
import http
import json
import os
import re
import subprocess
import sys
import tempfile
from pathlib import Path

RAIZ = Path(__file__).resolve().parent.parent
DOC = RAIZ / "docs" / "GUIA_DO_OPERADOR.md"
FONTES = [
    RAIZ / "crates" / "phxclaw-agent" / "src",
    RAIZ / "apps" / "phxclaw" / "src",
    RAIZ / "crates" / "phxclaw-config-runtime" / "src",
]
MCP_RS = RAIZ / "crates" / "phxclaw-agent" / "src" / "mcp.rs"
CONFIG_RS = RAIZ / "crates" / "phxclaw-agent" / "src" / "config.rs"
MEDICAO_RS = RAIZ / "apps" / "phxclaw" / "src" / "medicao.rs"
PASTA_UI = RAIZ / "docs" / "ui" / "fidelidade"
MARCADOR = re.compile(
    r"<!-- gerado:(?P<nome>[a-z0-9_-]+(?::[a-z0-9_.,-]+)?):inicio -->.*?"
    r"<!-- gerado:(?P=nome):fim -->",
    re.S,
)

avisos = []


def ambiente_limpo():
    """Sem PHXCLAW_* do operador: o guia mostra o PADRAO do catalogo, nao a maquina de quem
    rodou o gerador."""
    return {k: v for k, v in os.environ.items() if not k.startswith("PHXCLAW_")}


def rodar(bin_, *args, cwd=None):
    r = subprocess.run(
        [str(bin_), *args], capture_output=True, text=True, env=ambiente_limpo(),
        timeout=120, cwd=cwd,
    )
    if r.returncode != 0:
        sys.exit(f"ERRO: {bin_} {' '.join(args)} saiu {r.returncode}: {r.stderr.strip()}")
    return r.stdout


def celula(v):
    if v is None:
        return "—"
    if isinstance(v, (bool, list, dict)):
        v = json.dumps(v, ensure_ascii=False)
    return str(v).replace("|", "\\|").replace("\n", " ")


def cabecalho(ctx):
    linhas = [
        f"Trechos marcados gerados em {ctx['hoje']} por `python3 tools/gerar_guia_operador.py`, "
        f"do binario `{ctx['versao']}` (compilado em {ctx['m_bin']:%Y-%m-%d %H:%M})."
    ]
    if ctx["velho"]:
        linhas.append(
            f"\n> **Binario mais velho que o fonte**: `{ctx['novo']}` e de "
            f"{ctx['m_fonte']:%Y-%m-%d %H:%M}. A ajuda e o catalogo abaixo sao os do binario, "
            "nao necessariamente os do fonte de hoje."
        )
    return "\n".join(linhas)


def ajuda(ctx, cmd):
    txt = rodar(ctx["bin"], "ajuda", cmd, cwd=ctx["vazia"]).rstrip()
    return f"Saida de `phxclaw ajuda {cmd}`:\n\n```text\n{txt}\n```"


def precedencia(ctx):
    ex = json.loads(rodar(ctx["bin"], "config", "exemplo", cwd=ctx["vazia"]))
    topo = ex.get("//", [])
    return "Cabecalho de `phxclaw config exemplo`:\n\n" + "\n".join(f"> {l}" for l in topo)


def chaves(ctx, prefixos):
    pre = prefixos.split(",")
    sel = [
        c for c in ctx["catalogo"]
        if any(c["chave"] == p or c["chave"].startswith(p + ".") for p in pre)
    ]
    if not sel:
        sys.exit(f"ERRO: nenhuma chave do catalogo comeca com {prefixos}")
    linhas = [
        f"De `phxclaw config mostrar --json` (catalogo do binario, pasta vazia): "
        f"{len(sel)} chave(s) em `{prefixos}`.",
        "",
        "| Chave | Variavel | Tipo | Padrao | Natureza | O que e |",
        "|---|---|---|---|---|---|",
    ]
    for c in sel:
        nat = celula(c["natureza"])
        if c["segredo"]:
            nat = "segredo: " + como_guardar(c)
        elif c["natureza"] == "ambiente":
            nat = f"so ambiente ({celula(c['motivo_so_ambiente'])})"
        tipo = c["tipo"]
        if c.get("opcoes"):
            tipo += ": " + " | ".join(map(str, c["opcoes"]))
        linhas.append(
            f"| `{c['chave']}` | `{c['variavel']}` | {celula(tipo)} | {celula(c['padrao'])} "
            f"| {nat} | {celula(c['descricao'])} |"
        )
    return "\n".join(linhas)


def como_guardar(c):
    """O comando vai em codigo; a explicacao de quem ainda nao tem comando, em texto."""
    com = celula(c["comando_do_segredo"])
    return f"`{com}`" if com.startswith(("phxclaw ", "PHXCLAW_")) else com


def segredos(ctx):
    sel = [c for c in ctx["catalogo"] if c["segredo"]]
    sem_comando = [c for c in sel if "phxclaw" not in (c["comando_do_segredo"] or "")]
    linhas = [
        f"De `phxclaw config mostrar --json`: **{len(sel)} segredos no catalogo**; "
        f"{len(sel) - len(sem_comando)} tem comando que os guarda no SecretBroker, "
        f"{len(sem_comando)} ainda nao.",
        "",
        "<details><summary>Os segredos, um por linha</summary>",
        "",
        "| Chave | Variavel | Como guardar |",
        "|---|---|---|",
    ]
    for c in sel:
        linhas.append(f"| `{c['chave']}` | `{c['variavel']}` | {como_guardar(c)} |")
    linhas += ["", "</details>"]
    if sem_comando:
        avisos.append(
            f"{len(sem_comando)} segredo(s) sem comando de broker: "
            + ", ".join(c["variavel"] for c in sem_comando)
        )
    return "\n".join(linhas)


def nomes_no_fonte():
    achados = {}
    pasta = RAIZ / "crates" / "phxclaw-agent" / "src"
    for arq in sorted(pasta.rglob("*.rs")):
        txt = arq.read_text(encoding="utf-8")
        for m in re.finditer(r'ToolSpec\s*\{\s*name:\s*"([a-z0-9_]+)"', txt):
            achados.setdefault(m.group(1), arq.relative_to(RAIZ).as_posix())
    return achados


def ferramentas(ctx, nomes):
    pedidos = nomes.split(",")
    por_nome = {t["nome"]: t for t in ctx["ferramentas"]}
    fonte = ctx.setdefault("fonte", nomes_no_fonte())
    linhas = ["| Ferramenta | Capacidade | Padrao | Descricao (a que o modelo le) |", "|---|---|---|---|"]
    for n in pedidos:
        t = por_nome.get(n)
        if t:
            padrao = "sim" if t["concedida"] else "**nao**"
            linhas.append(f"| `{n}` | `{t['capacidade']}` | {padrao} | {celula(t['descricao'])} |")
        elif n in fonte:
            linhas.append(
                f"| `{n}` | — | — | **NAO MONTADA nesta maquina** (depende de configuracao, chave, "
                f"indice ou feature; a condicao esta em `crates/phxclaw-agent/src/montagem.rs`); "
                f"definida em `{fonte[n]}` |"
            )
            avisos.append(f"ferramenta {n} nao montou nesta maquina; o guia diz isso")
        else:
            sys.exit(f"ERRO: ferramenta {n} nao existe nem no binario nem no fonte")
    return "\n".join(linhas)


def mcp_presets(ctx):
    txt = MCP_RS.read_text(encoding="utf-8")
    corpo = re.search(r"fn preset\(.*?\n\}\n", txt, re.S)
    if not corpo:
        sys.exit(f"ERRO: fn preset nao achada em {MCP_RS}")
    bracos = re.findall(r'"([a-z]+)"\s*=>\s*\(\s*"(https://[^"]+)",\s*(AuthDeclarada::Bearer|google)', corpo.group(0))
    if not bracos:
        sys.exit("ERRO: nenhum braco de preset reconhecido em mcp.rs")
    linhas = [
        f"Dos bracos de `fn preset` em `{MCP_RS.relative_to(RAIZ).as_posix()}`: "
        f"{len(bracos)} preset(s).",
        "",
        "| `preset` | URL | Credencial | Comando |",
        "|---|---|---|---|",
    ]
    for nome, url, auth in bracos:
        if auth == "AuthDeclarada::Bearer":
            linhas.append(f"| `{nome}` | `{url}` | Bearer fixo | `phxclaw mcp token {nome}` |")
        else:
            linhas.append(
                f"| `{nome}` | `{url}` | OAuth 2.0 + PKCE (Google; `cliente_id` do operador) "
                f"| `phxclaw mcp login {nome}` |"
            )
    return "\n".join(linhas)


def http_config(ctx):
    txt = CONFIG_RS.read_text(encoding="utf-8")
    ini = txt.find("// --- a rota")
    if ini < 0:
        sys.exit(f"ERRO: secao da rota nao achada em {CONFIG_RS}")
    rota = txt[ini:]
    vistos = []
    for m in re.finditer(r"StatusCode::([A-Z_]+)", rota):
        if m.group(1) not in vistos:
            vistos.append(m.group(1))
    linhas = [
        f"Codigos que as rotas de `{CONFIG_RS.relative_to(RAIZ).as_posix()}` devolvem, na ordem "
        "em que aparecem no fonte (alem do 200 e do 401 do Bearer):",
        "",
        "| Codigo | Nome |",
        "|---|---|",
    ]
    for nome in vistos:
        try:
            cod = http.HTTPStatus[nome].value
        except KeyError:
            cod = "?"
        linhas.append(f"| {cod} | `{nome}` |")
    return "\n".join(linhas)


def tipos_de_medicao_ui():
    """Os prefixos que o `phxclaw ui` grava, lidos do medicao.rs: o literal de
    `gravar_resultado(.., "X", ..)` e o `format!("X-{}", alvo)` com os alvos do USO."""
    txt = MEDICAO_RS.read_text(encoding="utf-8")
    tipos = re.findall(r'gravar_resultado\(&saida,\s*"([a-z]+)"', txt)
    alvos = re.search(r"--alvo ([a-z|]+)", txt)
    for base in re.findall(r'format!\("([a-z]+)-\{\}",\s*v\["alvo"\]', txt):
        if not alvos:
            sys.exit(f"ERRO: {base} grava por alvo e o USO de {MEDICAO_RS} nao diz os alvos")
        tipos += [f"{base}-{a}" for a in alvos.group(1).split("|")]
    return tipos


def medicoes_ui(ctx):
    tabela = ["| Medicao | Arquivo | Medida em | Comando gravado |", "|---|---|---|---|"]
    detalhes = []
    for tipo in tipos_de_medicao_ui():
        arqs = sorted(PASTA_UI.glob(f"{tipo}-2*.json"))
        sub = tipo.split("-")[0]
        if not arqs:
            tabela.append(f"| `{tipo}` | — | **NAO MEDIDA** | rode `phxclaw ui {sub}` |")
            avisos.append(f"medicao {tipo} sem arquivo em {PASTA_UI.relative_to(RAIZ)}: NAO MEDIDA")
            continue
        a = arqs[-1]
        d = json.loads(a.read_text(encoding="utf-8"))
        # a data e a que o resultado declara, nunca o mtime do arquivo
        data = d.get("data") or "**sem data no resultado**"
        tabela.append(
            f"| `{tipo}` | `{a.relative_to(RAIZ).as_posix()}` | {data} | `{celula(d.get('comando'))}` |"
        )
        for braco in ("so_ocr", "com_modelo"):
            ag = (d.get(braco) or {}).get("agregado")
            if not ag:
                continue
            detalhes += [
                "",
                f"`{a.name}`, braco `{braco}` (mediana e faixa min–max sobre as telas, medido em {data}):",
                "",
                "| Metrica | Mediana | Min | Max | N |",
                "|---|---|---|---|---|",
            ]
            for k in sorted(ag):
                v = ag[k]
                if isinstance(v, dict) and "mediana" in v:
                    detalhes.append(f"| `{k}` | {v['mediana']} | {v['min']} | {v['max']} | {v['n']} |")
            tot = ag.get("totais")
            if tot:
                detalhes += ["", "Totais: " + ", ".join(f"{k} {v}" for k, v in tot.items()) + "."]
    return "\n".join(tabela + detalhes)


GERADORES = {
    "cabecalho": lambda ctx, _a: cabecalho(ctx),
    "ajuda": ajuda,
    "precedencia": lambda ctx, _a: precedencia(ctx),
    "chaves": chaves,
    "segredos": lambda ctx, _a: segredos(ctx),
    "ferramentas": ferramentas,
    "mcp-presets": lambda ctx, _a: mcp_presets(ctx),
    "http-config": lambda ctx, _a: http_config(ctx),
    "medicoes-ui": lambda ctx, _a: medicoes_ui(ctx),
}


def main():
    bin_ = Path(sys.argv[1]) if len(sys.argv) > 1 else RAIZ / "target" / "debug" / "phxclaw"
    if not bin_.is_file():
        sys.exit(f"ERRO: {bin_} nao existe; rode `cargo build -p phxclaw` antes")
    bin_ = bin_.resolve()
    m_bin = datetime.datetime.fromtimestamp(bin_.stat().st_mtime)
    novo = max((p for d in FONTES for p in d.rglob("*.rs")), key=lambda p: p.stat().st_mtime)
    m_fonte = datetime.datetime.fromtimestamp(novo.stat().st_mtime)

    with tempfile.TemporaryDirectory(prefix="guia-operador-") as vazia:
        ctx = {
            "bin": bin_,
            "vazia": vazia,
            "hoje": datetime.date.today().isoformat(),
            "m_bin": m_bin,
            "m_fonte": m_fonte,
            "novo": novo.relative_to(RAIZ).as_posix(),
            "velho": m_bin < m_fonte,
        }
        ctx["versao"] = rodar(bin_, "version", cwd=vazia).strip()
        vista = json.loads(rodar(bin_, "config", "mostrar", "--json", "--pasta", vazia, cwd=vazia))
        ctx["catalogo"] = vista["chaves"]
        ctx["ferramentas"] = json.loads(rodar(bin_, "ferramentas", cwd=vazia))["ferramentas"]
        if ctx["velho"]:
            avisos.append(f"binario de {m_bin:%Y-%m-%d %H:%M} mais velho que {ctx['novo']} "
                          f"({m_fonte:%Y-%m-%d %H:%M})")

        texto = DOC.read_text(encoding="utf-8")
        feitos = []

        def trocar(m):
            nome = m.group("nome")
            tipo, _, arg = nome.partition(":")
            if tipo not in GERADORES:
                sys.exit(f"ERRO: marcador de tipo desconhecido: gerado:{nome}")
            corpo = GERADORES[tipo](ctx, arg)
            feitos.append(nome)
            return f"<!-- gerado:{nome}:inicio -->\n{corpo.rstrip()}\n<!-- gerado:{nome}:fim -->"

        texto = MARCADOR.sub(trocar, texto)
        DOC.write_text(texto, encoding="utf-8")

    print(f"{DOC.relative_to(RAIZ)}: {len(feitos)} trecho(s) gerado(s)")
    if avisos:
        print("\nFEZ MENOS DO QUE O NOME PROMETE:")
        for a in avisos:
            print(f"  - {a}")


if __name__ == "__main__":
    main()
