#!/usr/bin/env python3
"""O PORTAO DOS GERADORES -- a catraca que amarra os geradores ao publicar.

O buraco que ele fecha: a pasta tem CATORZE geradores que escrevem TODO numero
visivel do dossie e das cinco paginas satelites, e a rodada pode ESQUECER de
roda-los. Quando isso acontece, o painel publicado fica com o numero de ontem
«anunciando sucesso pelo silencio» -- ninguem digitou errado, e mesmo assim a
vitrine mente. Ate agora nao havia um portao unico que reprovasse esse estado.

## O que ele faz (o EFEITO, nao a estrutura)

Para cada gerador, ele responde a uma pergunta so: **re-rodar mudaria algum
numero visivel?** O jeito robusto e rodar o gerador de verdade e comparar o
arquivo-alvo ANTES e DEPOIS. Se mudou, o derivado publicado estava VELHO ->
portao VERMELHO, nomeando qual gerador e qual arquivo. E o portao e
READ-ONLY: ele guarda os bytes de cada alvo antes de rodar e os DEVOLVE depois,
para nunca deixar a arvore meio-regerada. Detectar e papel dele; consertar e
rodar a receita de verdade (`docs/dossie/LEIA-ME.md`).

Duas doencas contam como VERMELHO, nao so a desatualizacao:

* **gerador que FALHA** (sai != 0) -- porque gerador que emite nada quando a
  fonte sumiu e a mesma doenca do conferidor que diz «limpo» sem ter conferido.
* **derivado que mudaria** ao re-rodar -- a desatualizacao propriamente dita.

## Os tres modos, e por que existem tres e nao um

Nem todo gerador da para conferir por byte cru, e fingir que da e o jeito
classico de um portao virar VERMELHO que ninguem acredita -- e portao em que
ninguem acredita nao segura nada. Cada gerador leva o crivo MAIS FORTE que ele
suporta, e o motivo fica escrito ao lado:

* **`exato`** -- o alvo e funcao pura das fontes versionadas. Comparacao byte a
  byte, zero mascara, zero risco de VERDE-falso. E o nucleo forte: a maioria
  dos numeros visiveis (pedidos, cobertura, bancada, tetos, comparativo, fluxo,
  numeracao das figuras, capturas) cai aqui.

* **`sem-carimbo`** -- o gerador embute um CARIMBO DE PROCEDENCIA que muda
  sozinho a cada corrida e nao e «numero medido»: o relogio de parede do
  «gerado em», a data de HOJE do «contados hoje», o `mtime` do arquivo lido
  (que o `git checkout` reescreve para a hora do checkout -- o git nao preserva
  mtime) e o commit curto do rodape. Comparar esses por byte da VERMELHO em toda
  arvore, sempre, sem nenhum defeito. Este modo apaga SO o carimbo dos dois
  lados e compara o resto -- pega mudanca de VALOR medido, ignora a data. O que
  ele NAO cobre, e esta escrito: uma data que envelheceu sozinha (mas essas
  datas vem de `mtime`/hoje/agora e nao se reproduzem entre checkouts de
  qualquer jeito).

* **`nota-cargo`** -- o `numeros-do-projeto.py` chama `cargo test` e
  `cargo run --example`, e esta worktree tem disco escasso: martelar o build
  aqui fura o piso. Ele NAO e rodado pelo portao; sai como NOTA, com o comando
  para roda-lo, e o resumo diz em alto e bom som que ele ficou por conferir --
  papel que nao esta cumprindo aparece como nao cumprindo, em vez de sumir do
  relatorio.

O `trio-de-motores.py` e `exato`, com um ajuste: a guarda propria dele compara
o `mtime` da figura com o da medicao, e essa comparacao nao sobrevive a um
`git checkout` (as duas saem com a hora do checkout, na ordem em que o git
gravou). O portao poe a figura como a MAIS NOVA antes de rodar -- reproduz a
pre-condicao que a receita de verdade garante rodando o `grafico.py` -- e entao
confere o BLOCO. A frescura figura-vs-medicao continua sendo guarda do proprio
trio, rodada na receita real; o portao nao a reproduz, e isso esta dito.

## O que fica de fora, e por que

* **`numeros-do-projeto.py`** -- `nota-cargo`, acima.
* **`dossie_da_pasta.py`** e **`embutir-fontes.py`** -- nao escrevem numero
  nenhum (um so acha o arquivo, o outro embute fontes para o PDF).

Uso:

    python3 docs/dossie/portao-dos-geradores.py            # confere e reprova
    python3 docs/dossie/portao-dos-geradores.py --lista    # so lista o plano
    python3 docs/dossie/portao-dos-geradores.py --so tetos # so os que casam «tetos»

Sai != 0 se algum gerador conferido esta VELHO ou FALHOU. O `--so` confere so
os geradores cujo nome contem o texto dado -- para re-conferir UM depois de o
consertar, sem esperar os catorze.
"""

import difflib
import os
import re
import subprocess
import sys
import pathlib

AQUI = pathlib.Path(__file__).resolve().parent
RAIZ = AQUI.parent.parent  # docs/dossie -> docs -> phxsql
sys.path.insert(0, str(AQUI))
from dossie_da_pasta import achar_o_dossie  # noqa: E402

# --- Os carimbos de procedencia que o modo `sem-carimbo` apaga -------------
#
# So o que muda SOZINHO a cada corrida entra aqui. Cada padrao carrega o
# gerador que o produz, para que a lista nao vire um apaga-tudo silencioso.
_CARIMBOS = [
    # `mtime` lido de um resultado, no formato ISO -- pagina-dos-testes,
    # graficos-dos-testes, pagina-de-status («data do arquivo: ...»). O git
    # reescreve o mtime na hora do checkout, entao este numero nunca se
    # reproduz entre duas copias do repositorio.
    (re.compile(r"\d{4}-\d{2}-\d{2}[ T]\d{2}:\d{2}(?::\d{2})?"), "<DATA-HORA-ISO>"),
    # «gerado em DD/MM/YYYY HH:MM [UTC]» (relogio de parede) e «contados hoje,
    # DD/MM/YYYY» (data de hoje) -- todos os quatro geradores de pagina.
    (re.compile(r"\d{2}/\d{2}/\d{4}(?:\s+\d{2}:\d{2}(?::\d{2})?)?(?:\s*UTC)?"), "<DATA-BR>"),
    # o commit curto do rodape das perguntas: `(commit <code>abc1234</code>)`.
    # Cirurgico de proposito: NAO casa o build-id `(41e82efa97c8)` do
    # resultados.json que a pagina de testes mostra, que e VALOR medido.
    (re.compile(r"commit <code>[0-9a-f]{6,40}</code>"), "commit <code><HASH></code>"),
]


def sem_carimbo(texto: str) -> str:
    for rx, rep in _CARIMBOS:
        texto = rx.sub(rep, texto)
    return texto


# --- O plano: cada gerador, seus alvos, seu modo, e o porque ---------------
#
# `alvos` sao caminhos relativos a RAIZ; `DOSSIE` e resolvido em tempo de
# execucao por varredura (`achar_o_dossie`), porque o nome muda a cada refacao.
DOSSIE = "@dossie"

PLANO = [
    ("numeros-da-bancada.py", [DOSSIE, "docs/PENDENCIAS.md"], "exato",
     "le resultados.json das bancadas; funcao pura das fontes versionadas"),
    ("pagina-dos-pedidos.py", [DOSSIE, "docs/dossie/pedidos.html", "docs/PENDENCIAS.md"], "exato",
     "conta os tres estados do PENDENCIAS.md; deterministico"),
    ("cobertura-por-area.py", [DOSSIE, "docs/TESTES.md"], "exato",
     "conta #[test] por area no fonte; deterministico"),
    ("docs/tecnologias/extrair.py", ["docs/TECNOLOGIAS.md"], "exato",
     "regrava os 16 blocos GERADO (pedido 156); CAPABILITIES.json e "
     "versionado e o script nao tem relogio -- funcao pura das fontes"),
    ("capturas-no-dossie.py", [DOSSIE], "exato",
     "embute os PNG ja reduzidos de capturas/ como data URI; deterministico"),
    ("tetos-da-trava.py", [DOSSIE], "exato",
     "le as corridas CERTO de bancada/concorrencia/; deterministico"),
    ("comparativo-no-dossie.py",
     [DOSSIE, "docs/dossie/fig-fluxo-do-medidor.svg", "docs/dossie/fig-workflow-da-rodada.svg"], "exato",
     "le bancada/comparativo/ e cobertura-da-tela/; deterministico"),
    ("fluxo-do-motor.py",
     [DOSSIE, "docs/dossie/fig-fluxo-do-motor.svg", "docs/dossie/fig-workflow-do-motor.svg"], "exato",
     "le os portoes e passos do proprio fonte Rust; deterministico"),
    ("trio-de-motores.py", [DOSSIE], "exato-trio",
     "le bancada/comparacao/um-milhao.json; a guarda de mtime da figura e neutralizada"),
    ("perguntas-no-dossie.py", [DOSSIE], "sem-carimbo",
     "le docs/pdf/respostas/; rodape carrega relogio de parede e commit curto"),
    ("numerar-figuras.py", [DOSSIE], "exato",
     "renumera as legendas Figura N na ordem do documento; roda por ULTIMO"),
    ("pagina-de-status.py", ["docs/dossie/status.html"], "sem-carimbo",
     "le STATUS.md + CAPABILITIES.json; painel traz mtime, hoje e relogio"),
    ("pagina-dos-testes.py", ["docs/dossie/testes.html"], "sem-carimbo",
     "le CAPABILITIES.json + resultados.json; traz mtime e relogio"),
    ("graficos-dos-testes.py", ["docs/dossie/graficos.html"], "sem-carimbo",
     "le resultados.json das bancadas; traz mtime e relogio"),
    ("numeros-do-projeto.py", [DOSSIE, "docs/CAPABILITIES.json"], "nota-cargo",
     "chama cargo test e cargo run --example; nao martelar o build nesta worktree"),
]

# A figura cuja frescura o trio confere por mtime -- que o portao poe como a
# mais nova antes de rodar, porque o git checkout nao preserva mtime.
FIGURA_TRIO = RAIZ / "bancada" / "comparacao" / "comparacao-tres-motores.svg"
MEDICAO_TRIO = RAIZ / "bancada" / "comparacao" / "um-milhao.json"


def resolver(alvo: str, dossie: pathlib.Path) -> pathlib.Path:
    return dossie if alvo == DOSSIE else (RAIZ / alvo)


def caminho_do_gerador(script: str) -> pathlib.Path:
    """Onde o .py do gerador mora. Todo gerador desta lista sempre viveu em
    docs/dossie/ -- mas o extrair.py (pedido 156) mora em docs/tecnologias/,
    porque a pasta de tecnologias e dele, nao do dossie. Em vez de mudar a
    convencao para os catorze, o nome no PLANO vira caminho relativo a RAIZ
    quando tem "/"; sem "/" cai no comportamento antigo (AQUI/script)."""
    return (RAIZ / script) if "/" in script else (AQUI / script)


def primeiro_hunk(antes: str, depois: str, nome: str) -> str:
    """Um trecho curto do primeiro ponto que difere, para o relatorio."""
    a = antes.splitlines(keepends=True)
    b = depois.splitlines(keepends=True)
    linhas = []
    for ln in difflib.unified_diff(a, b, fromfile=f"{nome} (publicado)",
                                   tofile=f"{nome} (re-gerado)", n=0):
        linhas.append(ln.rstrip("\n"))
        if len(linhas) >= 8:
            linhas.append("      [...]")
            break
    return "\n".join("      " + x for x in linhas)


def conferir_um(script: str, alvos, modo: str, dossie: pathlib.Path):
    """Roda um gerador e devolve (estado, [motivos]).

    estado: 'verde' | 'vermelho' | 'nota'
    """
    if modo == "nota-cargo":
        return "nota", [
            f"NAO conferido (chama cargo). Rode a mao antes de publicar:\n"
            f"      flock /tmp/phx-cargo.lock python3 docs/dossie/{script}"
        ]

    caminhos = [resolver(a, dossie) for a in alvos]
    faltantes = [c for c in caminhos if not c.exists()]
    if faltantes:
        return "vermelho", [f"alvo nao existe: {c.relative_to(RAIZ)}" for c in faltantes]

    antes = {c: c.read_bytes() for c in caminhos}

    # O trio confere a frescura da figura por mtime, e mtime nao sobrevive a um
    # checkout. Pomos a figura como a mais nova para reproduzir a pre-condicao
    # que o grafico.py garante -- a frescura fica a cargo da guarda do trio, na
    # receita real.
    if modo == "exato-trio" and FIGURA_TRIO.exists() and MEDICAO_TRIO.exists():
        os.utime(FIGURA_TRIO, (MEDICAO_TRIO.stat().st_atime,
                               MEDICAO_TRIO.stat().st_mtime + 1))

    try:
        r = subprocess.run(
            [sys.executable, str(caminho_do_gerador(script))],
            cwd=RAIZ, capture_output=True, text=True,
        )
    except Exception as e:  # noqa: BLE001 -- o portao nunca cai; ele reprova
        for c, b in antes.items():
            c.write_bytes(b)
        return "vermelho", [f"o gerador nao rodou: {e}"]

    motivos = []
    if r.returncode != 0:
        cauda = (r.stderr or r.stdout or "").strip().splitlines()[-4:]
        motivos.append("FALHOU (saida != 0) -- gerador que nao emite quando a "
                       "fonte sumiu e VERMELHO:\n" + "\n".join("      " + l for l in cauda))

    # Comparar cada alvo, depois DEVOLVER os bytes originais (read-only).
    exato = modo in ("exato", "exato-trio")
    for c in caminhos:
        depois = c.read_bytes()
        if depois == antes[c]:
            continue
        if exato:
            a_txt = antes[c].decode("utf-8", "replace")
            d_txt = depois.decode("utf-8", "replace")
            motivos.append(
                f"VELHO: {c.relative_to(RAIZ)} -- re-rodar muda o arquivo:\n"
                + primeiro_hunk(a_txt, d_txt, c.name))
        else:  # sem-carimbo
            a_txt = sem_carimbo(antes[c].decode("utf-8", "replace"))
            d_txt = sem_carimbo(depois.decode("utf-8", "replace"))
            if a_txt != d_txt:
                motivos.append(
                    f"VELHO: {c.relative_to(RAIZ)} -- muda um VALOR medido "
                    f"(fora o carimbo de data/hora):\n"
                    + primeiro_hunk(a_txt, d_txt, c.name))

    # Restaurar SEMPRE -- o portao confere, nao conserta.
    for c, b in antes.items():
        c.write_bytes(b)

    if motivos:
        return "vermelho", motivos
    return "verde", []


def _filtro() -> str:
    if "--so" in sys.argv:
        i = sys.argv.index("--so")
        if i + 1 < len(sys.argv):
            return sys.argv[i + 1]
        raise SystemExit("--so precisa do texto do gerador, ex.: --so tetos")
    return ""


def main() -> int:
    dossie = achar_o_dossie()
    so = _filtro()
    plano = [p for p in PLANO if so in p[0]]
    if so and not plano:
        raise SystemExit(f"nenhum gerador casa «{so}» -- veja `--lista`")

    if "--lista" in sys.argv:
        print(f"dossie: {dossie.relative_to(RAIZ)}\n")
        for script, alvos, modo, porque in plano:
            als = ", ".join(a if a != DOSSIE else dossie.name for a in alvos)
            print(f"  [{modo:12}] {script:26} -> {als}\n"
                  f"                 {porque}")
        return 0

    print("PORTAO DOS GERADORES -- re-rodar mudaria algum numero visivel?")
    print(f"dossie: {dossie.relative_to(RAIZ)}"
          + (f"   (so «{so}»)" if so else "") + "\n")

    vermelhos, notas = [], []
    for script, alvos, modo, _porque in plano:
        estado, motivos = conferir_um(script, alvos, modo, dossie)
        marca = {"verde": "  ok ", "vermelho": "VELHO", "nota": " nota"}[estado]
        print(f"[{marca}] {script}")
        for m in motivos:
            print("    " + m)
        if estado == "vermelho":
            vermelhos.append(script)
        elif estado == "nota":
            notas.append(script)

    print()
    if notas:
        print(f"NOTA: {len(notas)} gerador(es) fora do portao (chamam cargo), "
              "a conferir a mao antes de publicar: " + ", ".join(notas))
    if vermelhos:
        print(f"VERMELHO: {len(vermelhos)} derivado(s) velho(s) ou com falha. "
              "Rode o gerador nomeado e commite o resultado.")
        print("  " + " ".join(vermelhos))
        return 1
    print("VERDE: nenhum derivado conferido esta velho.")
    return 0


if __name__ == "__main__":
    sys.exit(main())
