#!/usr/bin/env python3
"""Regrava o painel da capa e o rodape do dossie com numeros MEDIDOS.

Existe pela mesma razao do `numeros-da-bancada.py`, e pelo mesmo historico: os
numeros do dossie ja sairam errados por serem DIGITADOS. Foram tres vezes --
arredondamento para cima, depois 276 testes quando eram 280, depois um rodape
inteiro parado numa versao anterior. Numero digitado envelhece calado.

O que este script conta, e a definicao de cada um -- que importa tanto quanto o
numero, porque «linhas de doc» sem dizer quais arquivos e um numero que nao se
confere:

    linhas de Rust    todo `.rs` de `crates/`, testes inclusive, `target/` fora
    testes            o que `cargo test --workspace` REPORTA como passado
    dependencias      pacotes do Cargo.lock menos os quatro crates daqui
    crates            pastas em `crates/`
    arquivos/tabela   quantos arquivos fisicos uma tabela tem, sem o espelho
    linhas de doc     ver DOCS abaixo
    interface         os arquivos de `ui/` que o `http.rs` embute com
                      `include_str!` -- a lista sai DELE, ver abaixo
    idiomas           o que o conferidor do servidor conta: textos de tela na
                      fabrica de idiomas, textos ainda cravados, e a catraca
    versao            o `version` do Cargo.toml do workspace

As duas ultimas seguem a receita publicada em `docs/dossie/LEIA-ME.md`, e nao a
que fosse mais comoda de escrever aqui: um numero de vitrine que ninguem
consegue reproduzir com a receita ao lado dele e um numero errado, ainda que a
conta esteja certa. Mexeu numa, mexa na outra.

    python3 docs/dossie/numeros-do-projeto.py            mede e grava
    python3 docs/dossie/numeros-do-projeto.py --so-medir mede e mostra

Rodar `cargo test` demora; `--sem-testes` reaproveita o numero que ja esta no
HTML em vez de medir de novo. Use so quando o que mudou nao foi codigo.
"""

import pathlib
import re
import subprocess
import sys

RAIZ = pathlib.Path(__file__).resolve().parents[2]
# Qual dossie reescrever. O nome mudou na 0.15.0 e pode mudar de novo:
# passar o caminho como primeiro argumento evita editar o script a cada vez.
def _alvo():
    for a in sys.argv[1:]:
        if a.endswith(".html"):
            # Resolvido: caminho relativo quebrava o `relative_to(RAIZ)`
            # da mensagem final, DEPOIS de ja ter gravado o arquivo.
            return pathlib.Path(a).resolve()
    return RAIZ / "docs" / "dossie" / "dossie-phxsql-0.18.html"


DOSSIE = _alvo()

ABRE = "<!-- projeto:inicio (gerado por docs/dossie/numeros-do-projeto.py) -->"
FECHA = "<!-- projeto:fim -->"
ABRE_RODAPE = "<!-- rodape:inicio (gerado por docs/dossie/numeros-do-projeto.py) -->"
FECHA_RODAPE = "<!-- rodape:fim -->"
# O selo da capa tambem: ele ficou quatro lancamentos dizendo 0.11.0, que e
# exatamente o erro que este script existe para nao deixar acontecer.
ABRE_SELO = "<!-- selo:inicio (gerado por docs/dossie/numeros-do-projeto.py) -->"
FECHA_SELO = "<!-- selo:fim -->"
# O painel dos idiomas. Ele existe porque «11% traduzida» e um numero de
# vitrine como qualquer outro -- e este muda a cada tela que alguem escreve.
ABRE_IDIOMAS = "<!-- idiomas:inicio (gerado por docs/dossie/numeros-do-projeto.py) -->"
FECHA_IDIOMAS = "<!-- idiomas:fim -->"

# Os arquivos fisicos de uma tabela, sem o espelho `.bkp` -- que e opcional e
# por isso nao entra na contagem que a capa mostra.
ARQUIVOS_POR_TABELA = ("reg", "ndx", "bin", "memo", "log", "trash", "reason")


def milhar(n: int) -> str:
    """Separador de milhar como se escreve em portugues: ponto."""
    return f"{n:,}".replace(",", ".")


def linhas_de_rust() -> int:
    total = 0
    for f in (RAIZ / "crates").rglob("*.rs"):
        if "target" in f.parts:
            continue
        total += len(f.read_text(encoding="utf-8", errors="replace").splitlines())
    return total


def quantos_crates() -> int:
    return sum(1 for d in (RAIZ / "crates").iterdir() if d.is_dir())


# A receita do LEIA-ME, na letra. Documento novo entra nos DOIS lugares.
DOCS_AVULSOS = (
    "README.md", "CHANGELOG.md", "MANUAL.txt",
    "bancada/LEIA-ME.md", "bancada/replicacao/LEIA-ME.md",
    "bancada/replicacao/docker/LEIA-ME.md",
    "bancada/carga/LEIA-ME.md",
    "marca/LEIA-ME.md", "docs/dossie/LEIA-ME.md",
)

# A interface sao os arquivos de `ui/` que o `http.rs` embute com
# `include_str!` -- e a lista sai DELE, e nao de uma copia aqui.
#
# Ela ja foi copiada aqui, com tres nomes, e envelheceu calada: o `http.rs`
# passou a embutir nove (o diagrama ER, a telemetria, a multitela e a
# integracao com a Claude entraram depois), e o rodape publicava 780 KiB
# quando a interface tinha 1.032. Lista de arquivos e numero como qualquer
# outro: ou sai de um gerador, ou esta errada e ninguem percebeu ainda.
HTTP_RS = "crates/phxsql-server/src/http.rs"


def arquivos_da_interface() -> tuple:
    fonte = (RAIZ / HTTP_RS).read_text(encoding="utf-8")
    # O que esta dentro de `#[cfg(test)]` NAO vai no binario -- e ali mora um
    # `include_str!` do CHANGELOG da grade, que um teste le para conferir a
    # versao. Conta-lo poria 25 KiB de markdown no numero da «interface».
    corte = fonte.find("#[cfg(test)]")
    if corte >= 0:
        fonte = fonte[:corte]
    nomes = re.findall(r'include_str!\("\.\./ui/([^"]+)"\)', fonte)
    if not nomes:
        sys.exit(f"nenhum include_str! de ui/ em {HTTP_RS} -- o padrao mudou?")
    # `dict.fromkeys` tira repetido preservando a ordem do arquivo, que e a
    # ordem em que a pagina os carrega.
    return tuple(f"crates/phxsql-server/ui/{n}" for n in dict.fromkeys(nomes))


def linhas_de_doc() -> int:
    arquivos = sorted((RAIZ / "docs").glob("*.md"))
    arquivos += [RAIZ / nome for nome in DOCS_AVULSOS]
    faltando = [f for f in arquivos if not f.exists()]
    if faltando:
        sys.exit(
            "a receita de linhas de doc aponta para arquivo que nao existe: "
            + ", ".join(str(f.relative_to(RAIZ)) for f in faltando)
        )
    return sum(
        len(f.read_text(encoding="utf-8", errors="replace").splitlines())
        for f in arquivos
    )


def dependencias_externas() -> int:
    """Pacotes do Cargo.lock menos os crates deste projeto.

    Tem de dar zero. Se der outra coisa, entrou dependencia -- e a capa vai
    dizer isso em vez de continuar publicando o zero por habito.
    """
    lock = (RAIZ / "Cargo.lock").read_text(encoding="utf-8")
    return lock.count("[[package]]") - quantos_crates()


def kib_da_interface() -> int:
    """KiB de 1024 bytes, arredondado para BAIXO.

    Para baixo de proposito: arredondar um numero de vitrine para cima ja foi
    um dos erros que este script existe para nao repetir.
    """
    total = 0
    for nome in arquivos_da_interface():
        f = RAIZ / nome
        if not f.exists():
            sys.exit(f"o http.rs embute {nome}, que nao existe")
        total += f.stat().st_size
    return total // 1024


def versao() -> str:
    texto = (RAIZ / "Cargo.toml").read_text(encoding="utf-8")
    m = re.search(r'^version\s*=\s*"([^"]+)"', texto, re.M)
    if not m:
        sys.exit("nao achei `version` no Cargo.toml do workspace")
    return m.group(1)


def testes_que_passam() -> int:
    """O que o `cargo test` REPORTA -- nao o que se conta com grep no fonte.

    Contar `#[test]` no fonte da outro numero: teste dentro de `cfg(test)` que
    nao roda, teste ignorado, e macro que gera varios. O numero que vale e o
    que o corredor de testes diz ter passado.
    """
    r = subprocess.run(
        ["cargo", "test", "--workspace", "--offline"],
        cwd=RAIZ, capture_output=True, text=True,
    )
    if r.returncode != 0:
        sys.exit("cargo test falhou -- corrija antes de publicar numero")
    total = 0
    for linha in r.stdout.splitlines():
        m = re.match(r"test result: ok\. (\d+) passed", linha)
        if m:
            total += int(m.group(1))
    if total == 0:
        sys.exit("nenhum teste contado -- a saida do cargo mudou de forma?")
    return total


def idiomas() -> dict:
    """Quantos textos de tela passam pela fabrica de idiomas, e quantos nao.

    Quem conta e o CONFERIDOR do servidor, o mesmo que roda junto dos testes e
    que segura a catraca -- e nao um `grep` escrito aqui. Duas implementacoes
    da mesma contagem e o jeito classico de a vitrine e a catraca discordarem.
    """
    r = subprocess.run(
        ["cargo", "run", "--release", "--offline",
         "--example", "textos-fora-da-fabrica", "-p", "phxsql-server"],
        cwd=RAIZ, capture_output=True, text=True,
    )
    if r.returncode != 0:
        sys.exit("o conferidor de textos falhou:\n" + r.stderr[-2000:])
    n = {}
    for chave, rotulo in (("na fabrica", "fabrica"),
                          ("fora da fabrica", "fora"),
                          ("catraca (TETO)", "teto")):
        m = re.search(rf"{re.escape(chave)}\s*\.*\s*(\d+)", r.stdout)
        if not m:
            sys.exit(f"nao achei «{chave}» na saida do conferidor")
        n[rotulo] = int(m.group(1))
    n["total"] = n["fabrica"] + n["fora"]
    # Para BAIXO, como o KiB da interface: arredondar um numero de vitrine
    # para cima ja foi um dos erros que estes scripts existem para nao repetir.
    n["pct"] = int(100 * n["fabrica"] / n["total"]) if n["total"] else 0
    return n


def testes_do_html(html: str) -> int:
    m = re.search(r'<div class="v">([\d.]+)</div><div class="r">testes</div>', html)
    if not m:
        sys.exit("--sem-testes pediu o numero que esta no HTML, e ele nao esta la")
    return int(m.group(1).replace(".", ""))


def trocar(html: str, abre: str, fecha: str, novo: str, onde: str) -> str:
    i, j = html.find(abre), html.find(fecha)
    if i < 0 or j < 0:
        sys.exit(f"as marcas do {onde} nao estao no dossie: {abre}")
    return html[: i + len(abre)] + novo + html[j:]


def main() -> None:
    so_medir = "--so-medir" in sys.argv
    html = DOSSIE.read_text(encoding="utf-8")

    n = {
        "rust": linhas_de_rust(),
        "crates": quantos_crates(),
        "doc": linhas_de_doc(),
        "kib": kib_da_interface(),
        "versao": versao(),
        "arquivos": len(ARQUIVOS_POR_TABELA),
        "deps": dependencias_externas(),
    }
    n["testes"] = testes_do_html(html) if "--sem-testes" in sys.argv else testes_que_passam()
    idi = idiomas()

    for chave, valor in n.items():
        print(f"  {chave:<9} {valor}")
    print(f"  idiomas   {idi['fabrica']} na fábrica, {idi['fora']} fora "
          f"({idi['pct']}%), catraca em {idi['teto']}")
    if so_medir:
        return

    painel = f"""
    <div><div class="v">{milhar(n['rust'])}</div><div class="r">linhas de Rust</div></div>
    <div><div class="v">{milhar(n['testes'])}</div><div class="r">testes</div></div>
    <div><div class="v">{n['deps']}</div><div class="r">dependências</div></div>
    <div><div class="v">{n['crates']}</div><div class="r">crates</div></div>
    <div><div class="v">{n['arquivos']}</div><div class="r">arquivos/tabela</div></div>
    <div><div class="v">{milhar(n['doc'])}</div><div class="r">linhas de doc</div></div>
  """
    rodape = f"""
  <p>PhxSql {n['versao']} · {milhar(n['rust'])} linhas de Rust em {n['crates']} crates,
  mais {n['kib']} KiB de interface · {milhar(n['testes'])} testes ·
  {'nenhuma dependência externa' if n['deps'] == 0 else str(n['deps']) + ' dependências externas'}.
  Especificação byte a byte em <code>docs/FORMATO.md</code>, cadastro e
  permissões em <code>docs/USUARIOS.md</code>, a replicação em
  <code>docs/REPLICACAO.md</code>, roteiro em <code>docs/PLANO.md</code>,
  o DbLink em <code>docs/DBLINK.md</code>, as junções em <code>docs/JUNCOES.md</code>,
  a revisão contra os motores maduros em <code>docs/COMPARACAO.md</code>,
  onde a escrita dói em <code>docs/DESEMPENHO.md</code>,
  e o que ainda falta em <code>docs/PENDENCIAS.md</code>.</p>
  """

    selo = f'\n  <div class="selo">Dossiê técnico · versão {n["versao"]}</div>\n  '

    painel_idiomas = f"""
    <div><div class="v">6</div><div class="r">idiomas na tabela</div></div>
    <div><div class="v">{milhar(idi['fabrica'])}</div><div class="r">textos na fábrica</div></div>
    <div><div class="v">{milhar(idi['fora'])}</div><div class="r">ainda cravados</div></div>
    <div><div class="v">{idi['pct']}%</div><div class="r">da interface</div></div>
    <div><div class="v">{milhar(idi['teto'])}</div><div class="r">catraca (o teto)</div></div>
  """

    html = trocar(html, ABRE, FECHA, painel, "painel da capa")
    html = trocar(html, ABRE_RODAPE, FECHA_RODAPE, rodape, "rodapé")
    html = trocar(html, ABRE_SELO, FECHA_SELO, selo, "selo da capa")
    html = trocar(html, ABRE_IDIOMAS, FECHA_IDIOMAS, painel_idiomas, "painel dos idiomas")

    # O <title> tambem: ele passou a versao inteira da 0.15 dizendo «0.15»
    # enquanto o selo, logo abaixo, dizia outra coisa. Numero visivel e numero
    # que sai de gerador -- e o titulo do documento e o mais visivel de todos.
    novo_titulo = f"<title>Dossiê PhxSql {n['versao']}</title>"
    if not re.search(r"<title>[^<]*</title>", html):
        sys.exit("o dossiê não tem <title> para atualizar")
    html = re.sub(r"<title>[^<]*</title>", novo_titulo, html, count=1)

    DOSSIE.write_text(html, encoding="utf-8")
    print(f"\ndossiê atualizado: {DOSSIE.relative_to(RAIZ)}")


if __name__ == "__main__":
    main()
