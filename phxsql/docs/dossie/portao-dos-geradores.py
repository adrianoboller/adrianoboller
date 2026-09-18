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

* **`numeros-do-projeto.py`** e **`docs/qa/medir.py`** -- `nota-cargo`, acima.
* **`dossie_da_pasta.py`** e **`embutir-fontes.py`** -- nao escrevem numero
  nenhum (um so acha o arquivo, o outro embute fontes para o PDF).

## O pedido 360, e a varredura que ele exigiu

O `bancada/comparativo/resultados.json` e fonte de TRES renderizadores, e este
PLANO so cobria dois (`comparativo-no-dossie.py` e a pagina de status): o
`bancada/comparativo/documento.py`, que escreve o `docs/COMPARATIVO.md`, podia
divergir sem que nada acusasse -- o mesmo formato da lista usada como
inventario, e o agravante e que este PLANO existe justamente para SER o
inventario completo. A varredura por scripts que ESCREVEM derivado versionado
(regex sobre `write_text`/`open(..., "w")` cruzado com o alvo) achou mais
CINCO buracos do mesmo naipe, e cada um foi RODADO antes de entrar aqui --
nao basta ler o docstring, tem de rodar e comparar byte a byte:

* **`bancada/gestao/documento.py`** -> `docs/GESTAO.md` -- irmao gemeo do
  `comparativo/documento.py`, mesma disciplina ("a prosa mora aqui, a medicao
  mora no JSON"), mesma fonte-e-alvo versionados, zero relogio.
* **`bancada/acid/gerar-secoes.py`** -> `docs/ACID.md` -- mesma familia do
  `docs/tecnologias/extrair.py` (blocos `<!-- GERADO: chave -->` trocados a
  partir de `resultado.json`).
* **`bancada/utilizacao-padrao/gera-leia-me.py`** -> `LEIA-ME.md` da propria
  pasta e a secao 18 de `docs/DESEMPENHO.md` -- mesma familia de blocos
  GERADO, dois JSONs fixos como fonte.
* **`bancada/comparacao/grafico.py`** -> `comparacao-tres-motores.svg`/`.html`
  -- e a "receita real" que o `trio-de-motores.py` ja cita no proprio
  comentario (a figura que ele embute), mas nada no PLANO conferia SE ela
  estava fresca em relacao ao `um-milhao.json`; a guarda de mtime do trio nao
  sobrevive a um `git checkout`, entao sem esta entrada o buraco ficava aberto
  mesmo com o trio coberto. Por isso vem ANTES de `trio-de-motores.py` aqui.
* **`docs/geradores/direito-por-coluna.py`** -> `docs/SEGURANCA.md` -- mesma
  familia do `extrair.py`, le `direito_coluna.rs`. Rodar de verdade MUDOU o
  arquivo (137 -> 140 operacoes, 5 -> 6 que leem, 19 -> 20 que recusam): o
  documento publicado ja estava desatualizado no dia em que esta entrada
  nasceu, exatamente a doenca que o portao existe para acusar. Quem for
  fechar o portao inteiro precisa rodar
  `python3 docs/geradores/direito-por-coluna.py` e commitar o `SEGURANCA.md`
  antes -- senao esta linha nasce VERMELHA por um defeito real e anterior ao
  pedido 360, nao por culpa desta entrada.

Cinco ficaram de fora, por motivo NOMEADO em vez de esquecidos de novo:

* **`docs/fronteiras/mapa-do-servidor.py --escrever`** -- rodei e ele FALHOU:
  "a regiao 'arranque-e-identidade' ancora em 'novo', que nao existe mais no
  arquivo". Nao e o documento que esta velho, e o PROPRIO mapeador que precisa
  de conserto de codigo (reancorar a regiao) antes de poder entrar aqui --
  fora do alcance de um portao que so confere.
* **`bancada/graficos.py`** -- a pagina embute `maquina()`: `cpu`, `nproc`,
  `free`, `mysqld --version` e duas `SHOW VARIABLES` via `subprocess`, tudo
  perguntado ao vivo a cada corrida. Numa maquina sem MySQL(R) instalado (ou
  com outra versao) o portao acusaria VERMELHO por diferenca de AMBIENTE, nao
  de dado -- o mesmo risco que o modo `sem-carimbo` existe para evitar, e os
  `_CARIMBOS` de hoje nao cobrem string livre de versao de pacote.
* **`docs/pdf/gerar.py`** -- chama o Chromium de `/opt/pw-browsers` para
  imprimir o PDF (`timeout=180`). E dependencia de AMBIENTE externa e pesada,
  o mesmo motivo que ja tira o `olhar.mjs` do caminho automatico (`docs/pmo`):
  caminho absoluto do Playwright, frouxo fora do repositorio.
* **`bancada/durabilidade/gerar-matriz.py`** -> `matriz-gerada.md` -- e
  determinista e roda limpo, mas o proprio `gerar-secoes.py` explica que este
  arquivo e um INTERMEDIARIO que alguem COLA a mao em `docs/TRANSACOES.md`
  (o padrao que a casa esta deixando para tras, por causa exatamente desse
  passo manual). Conferir o intermediario sozinho nao fecha o buraco de
  verdade, que e a colagem; fica como duvida em aberto, nao como recusa.
* **`bancada/guardas/tabela-no-testes.py`** -> `docs/TESTES.md` -- a fonte nao
  e um arquivo fixo versionado: e um `--json` que aponta para a saida de
  `provar-guardas.py`, que chama `cargo test` uma vez por guarda do catalogo
  (~3.374 s medidos, mutacao mais compilacao). Mesma familia dos dois
  `nota-cargo` de cima, so que ainda sem essa marca escrita.

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
    # As tres entradas abaixo sao da mesma familia do extrair.py -- blocos
    # `<!-- GERADO: chave -->` trocados dentro de um docs/*.md a partir de
    # fonte versionada -- e entraram pela varredura do pedido 360, que achou
    # o PLANO cobrindo so dois dos tres leitores do resultados.json do
    # comparativo e foi conferir se havia mais buracos do mesmo formato.
    ("bancada/acid/gerar-secoes.py", ["docs/ACID.md"], "exato",
     "le bancada/acid/resultado.json; blocos GERADO, mesmo molde do extrair.py"),
    ("bancada/utilizacao-padrao/gera-leia-me.py",
     ["bancada/utilizacao-padrao/LEIA-ME.md", "docs/DESEMPENHO.md"], "exato",
     "le resultado.json e resultado-alfabetica.json da propria pasta; blocos GERADO"),
    ("docs/geradores/direito-por-coluna.py", ["docs/SEGURANCA.md"], "exato",
     "le CLASSES de crates/phxsql-server/src/direito_coluna.rs; bloco GERADO"),
    ("capturas-no-dossie.py", [DOSSIE], "exato",
     "embute os PNG ja reduzidos de capturas/ como data URI; deterministico"),
    ("tetos-da-trava.py", [DOSSIE], "exato",
     "le as corridas CERTO de bancada/concorrencia/; deterministico"),
    # O pedido 360: o resultados.json do comparativo alimenta TRES
    # renderizadores (este documento.py, a linha de baixo e a pagina de 21
    # secoes), e so os dois ultimos estavam cobertos. "exato" porque o unico
    # datetime do fonte e `d["quando"]` -- a data da MEDICAO, lida do proprio
    # JSON versionado, nao um relogio de parede -- conferido rodando duas
    # vezes seguidas e comparando byte a byte (saida identica). Entra ANTES
    # do comparativo-no-dossie.py porque os dois leem a mesma fonte e nao ha
    # nenhum gerador no PLANO que leia o COMPARATIVO.md por sua vez.
    ("bancada/comparativo/documento.py", ["docs/COMPARATIVO.md"], "exato",
     "le bancada/comparativo/resultados.json; funcao pura da fonte versionada"),
    # Irmao gemeo do de cima, achado na mesma varredura: mesma disciplina
    # ("a prosa mora aqui, a medicao mora no JSON"), mesmo fonte-e-alvo
    # versionados, zero relogio -- conferido do mesmo jeito.
    ("bancada/gestao/documento.py", ["docs/GESTAO.md"], "exato",
     "le bancada/gestao/resultados.json; funcao pura da fonte versionada"),
    ("comparativo-no-dossie.py",
     [DOSSIE, "docs/dossie/fig-fluxo-do-medidor.svg", "docs/dossie/fig-workflow-da-rodada.svg"], "exato",
     "le bancada/comparativo/ e cobertura-da-tela/; deterministico"),
    ("fluxo-do-motor.py",
     [DOSSIE, "docs/dossie/fig-fluxo-do-motor.svg", "docs/dossie/fig-workflow-do-motor.svg"], "exato",
     "le os portoes e passos do proprio fonte Rust; deterministico"),
    # Achado na varredura do pedido 360: e a "receita real" que o proprio
    # trio-de-motores.py cita no comentario dele (a figura que ele embute),
    # mas nada aqui conferia se ELA estava fresca em relacao ao
    # um-milhao.json -- a guarda de mtime do trio nao sobrevive a um
    # `git checkout`. Sem esta entrada, o buraco ficava aberto por baixo do
    # trio mesmo com o trio coberto. Vem ANTES dele por isso: o trio consome
    # o SVG que este escreve.
    ("bancada/comparacao/grafico.py",
     ["bancada/comparacao/comparacao-tres-motores.svg", "bancada/comparacao/comparacao-tres-motores.html"],
     "exato", "le bancada/comparacao/um-milhao.json; funcao pura da fonte versionada"),
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
    ("numeros-do-projeto.py", [DOSSIE, "CAPABILITIES.json"], "nota-cargo",
     "chama cargo test e cargo run --example; nao martelar o build nesta worktree"),
    # A tabela das catracas do QA. Entrou em 16/09/2026, e o buraco que ela
    # fecha foi medido no dia: o `docs/QA-PDCA.md` publicado trazia OITO
    # catracas quando o proprio gerador media VINTE (nove em Rust, onze em
    # `bancada/`), e um teto de idiomas em 1.050 quando ja era 1.049 -- um
    # derivado velho que nenhum portao acusava, porque o gerador dele nao
    # estava no PLANO. E o `numeros-do-projeto.py` que ja estava aqui mostra
    # o molde: `nota-cargo`, porque ele chama `cargo run --release --example`
    # seis vezes e martelar o build nesta worktree fura o piso de disco.
    ("docs/qa/medir.py --gravar", ["docs/QA-PDCA.md"], "nota-cargo",
     "chama cargo run --example seis vezes; tabela das catracas do QA-PDCA"),
    # Nao e' gerador do dossie -- e' o rollup do board de PMO (GOV-3). Entra
    # aqui pelo mesmo motivo do `docs/tecnologias/extrair.py`: e' um gerador
    # que reescreve um bloco marcado a partir de fonte versionada, e o mesmo
    # buraco vale -- rodada pode esquecer de roda-lo e o board fica com a
    # contagem de ontem. "sem-carimbo" porque o bloco carrega "gerado em
    # HH:MM UTC" (relogio de parede); os _CARIMBOS ja cobrem esse formato.
    ("docs/pmo/rollup.py", ["docs/pmo/BACKLOG.md"], "sem-carimbo",
     "conta aberto/entregue-fechado/parado por pilar do proprio BACKLOG.md; "
     "funcao pura da fonte versionada, so' o carimbo de hora e' relogio"),
    # O painel PMO (as tres vistas que o dono pediu no molde do Phoenix Cast).
    # Mesmo motivo do rollup: gerador de bloco/pagina a partir de fonte
    # versionada, e a rodada pode esquecer de roda-lo. "sem-carimbo" porque a
    # pagina carrega "gerado em HH:MM UTC" e o "contados hoje, DD/MM/AAAA" --
    # os dois ja cobertos pelos _CARIMBOS. O `git log` que ela embute e'
    # deterministico na arvore parada: commit novo MUDA a pagina, e ai ela
    # esta velha mesmo.
    ("docs/pmo/pagina-do-status-do-projeto.py", ["docs/pmo/status-do-projeto.html"],
     "sem-carimbo",
     "monta o painel PMO do PENDENCIAS.md, do BACKLOG.md, do CAPABILITIES.json, "
     "do git log e dos arquivos de .claude/agents/; so' o carimbo e' relogio"),
    # A SETIMA pagina -- o status do projeto, no molde que o dono mandou do
    # projeto irmao (P.O.S). Nome de arquivo IGUAL ao do painel PMO acima e
    # pasta diferente de proposito: `docs/status/`, e o comando e
    # `./status-html.sh`. Entra aqui pelo mesmo motivo dos dois de cima -- a
    # rodada pode esquecer de roda-lo e a pagina fica com o numero de ontem.
    # "sem-carimbo" porque o rodape carrega "gerado em DD/MM/AAAA HH:MM UTC" e
    # varias datas de medicao vem de `mtime` (o git nao preserva mtime); os
    # _CARIMBOS ja cobrem os dois formatos.
    # As duas secoes que o pedido 264 fez nascer na setima pagina: riscos (do
    # `docs/RISCOS.md`, que se edita) e divida tecnica (da marca `// DIVIDA:`
    # no fonte Rust). Ele escreve DENTRO da mesma pagina, entre as marcas
    # `riscos:inicio`/`riscos:fim` -- e por isso tem de vir ANTES do gerador
    # da pagina inteira aqui: os dois escrevem no mesmo alvo, e o portao
    # devolve os bytes depois de cada um. "sem-carimbo" porque a secao traz a
    # data de hoje em cada cartao e o `mtime` do `docs/QA-PDCA.md` na sonda
    # das catracas -- os dois ja cobertos pelos _CARIMBOS.
    ("docs/status/riscos.py", ["docs/status/status-do-projeto.html"],
     "sem-carimbo",
     "le docs/RISCOS.md e a marca // DIVIDA: de crates/**/*.rs; escreve as "
     "duas secoes entre as marcas da propria pagina"),
    # As duas secoes dos pedidos 265 e 266, pelo mesmo desenho do `riscos.py`:
    # cada gerador escreve DENTRO da setima pagina, entre as marcas dele. Os
    # tres tem de vir ANTES do gerador da pagina inteira -- eles escrevem no
    # mesmo alvo, e o portao devolve os bytes depois de cada um. "sem-carimbo"
    # porque a pagina toda carrega o "gerado em ... UTC" do rodape, e as datas
    # de medicao da telemetria sao ISO (ja cobertas pelos _CARIMBOS).
    ("docs/status/telemetria-medida.py", ["docs/status/status-do-projeto.html"],
     "sem-carimbo",
     "le bancada/telemetria/resultados.json e conta os portoes if !self.ligada() "
     "no telemetria.rs; bancada que nao rodou sai como NAO MEDIDA"),
    ("docs/status/serie-historica.py", ["docs/status/status-do-projeto.html"],
     "sem-carimbo",
     "le docs/status/serie.jsonl, versionado, uma linha por medicao; com menos "
     "de duas medicoes a secao diz que nao da para comparar"),
    ("docs/status/pagina-do-status-do-projeto.py",
     ["docs/status/status-do-projeto.html"], "sem-carimbo",
     "monta as secoes do CAPABILITIES.json, do PENDENCIAS.md, do BACKLOG.md, "
     "dos resultados.json das bancadas, das constantes do fonte Rust e do git "
     "log; secao sem gerador NAO nasce e sai nomeada na ULTIMA secao da pagina "
     "(o numero dela sai da ORDEM, entao nao se escreve aqui)"),
]

# A figura cuja frescura o trio confere por mtime -- que o portao poe como a
# mais nova antes de rodar, porque o git checkout nao preserva mtime.
FIGURA_TRIO = RAIZ / "bancada" / "comparacao" / "comparacao-tres-motores.svg"
MEDICAO_TRIO = RAIZ / "bancada" / "comparacao" / "um-milhao.json"


def resolver(alvo: str, dossie: pathlib.Path) -> pathlib.Path:
    return dossie if alvo == DOSSIE else (RAIZ / alvo)


def partir(script: str):
    """(nome, argumentos) -- o PLANO pode trazer o comando, nao so o arquivo.

    O `docs/qa/medir.py` so escreve com `--gravar`; sem ele, imprime e nao
    toca em arquivo nenhum. Um PLANO que guardasse so o nome do arquivo faria
    o portao publicar uma receita que NAO regenera nada -- e receita que nao
    regenera e a mesma doenca do gerador chamado pela metade que esta casa ja
    pagou (o `pagina-dos-pedidos.py` imprimindo tres linhas de exito e
    pulando o painel do dossie)."""
    pedacos = script.split()
    return pedacos[0], pedacos[1:]


def caminho_do_gerador(script: str) -> pathlib.Path:
    """Onde o .py do gerador mora. Todo gerador desta lista sempre viveu em
    docs/dossie/ -- mas o extrair.py (pedido 156) mora em docs/tecnologias/,
    porque a pasta de tecnologias e dele, nao do dossie. Em vez de mudar a
    convencao para os catorze, o nome no PLANO vira caminho relativo a RAIZ
    quando tem "/"; sem "/" cai no comportamento antigo (AQUI/script)."""
    nome, _ = partir(script)
    return (RAIZ / nome) if "/" in nome else (AQUI / nome)


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
        rel = caminho_do_gerador(script).relative_to(RAIZ)
        _, args = partir(script)
        return "nota", [
            f"NAO conferido (chama cargo). Rode a mao antes de publicar:\n"
            f"      flock /tmp/phx-cargo.lock python3 {rel}"
            + ("".join(" " + a for a in args))
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
            [sys.executable, str(caminho_do_gerador(script))] + partir(script)[1],
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

    # A PROVA DA GUARDA, e nao so o resultado dela. O portao ja reprova um
    # derivado velho; o que ele nao via e' a guarda que parou de guardar. Em
    # 18/09/2026 o leitor do PENDENCIAS pulou onze pedidos em silencio e
    # imprimiu linha de exito -- o portao passou verde, porque o derivado
    # estava «em dia» com um leitor que contava menos.
    prova = RAIZ / "docs" / "dossie" / "prova-do-leitor-de-pedidos.py"
    r = subprocess.run([sys.executable, str(prova)],
                       capture_output=True, text=True, cwd=str(RAIZ))
    if r.returncode == 0:
        print("[  ok ] prova-do-leitor-de-pedidos.py")
    else:
        print("[VELHO] prova-do-leitor-de-pedidos.py")
        for linha in (r.stdout + r.stderr).strip().split("\n"):
            print("    " + linha)
        vermelhos.append("prova-do-leitor-de-pedidos.py")

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
