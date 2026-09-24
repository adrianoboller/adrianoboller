#!/usr/bin/env python3
"""Roda TODAS as catracas do repositorio -- Python e Rust -- de um so lugar.

    python3 bancada/catracas/todas.py            roda e da o veredito
    python3 bancada/catracas/todas.py --lista     so lista, nao roda nada
    python3 bancada/catracas/todas.py --com-rust  tambem RODA os testes que
                                                   conferem as TETO_* do Rust
                                                   (caro -- ver secao de custo)

Por que ele existe (pedido 476)
--------------------------------
Ate aqui, cada regua em Python com `--catraca` (mapa da trava, mapa das
threads, debug-com-segredo, pkill-sem-pid, trecho-vivo) so era chamada de
dentro do item 0 da `bancada/bateria/prova-bateria.py` -- e algumas tambem do
`comunicacao.sh`, cada arquivo com a SUA PROPRIA lista escrita a mao. Um
commit passava por `trecho-vivo.py --catraca` (uma catraca so, pelo nome),
pela suite e pelo clippy, e subia outra catraca sem ninguem ver: foi o que
aconteceu em 24/09 com a `debug-com-segredo.py`, que so' a bateria completa
pegou, dias depois
(`docs/cognicao/cognicao_catraca-que-so-roda-dentro-da-bateria-nao-segura-a-integracao_20260924_0455.md`).

Este comando existe para ser o UM lugar que sabe quais catracas existem, e
tanto a bateria quanto o `comunicacao.sh` passam a chama-LO em vez de listar
cada regua -- mesmo motor, nunca duas listas (lei do dono, 23/09/2026:
"Funcoes e comandos nao podem ser repetidas ou duplicadas, devem vir do mesmo
motor").

Como ele acha as reguas em Python, sem lista digitada
------------------------------------------------------
Varre TODO `.py` do repositorio atras de uma linha que junte `sys.argv` (ou
`add_argument`, para quem vier a preferir argparse) com o texto `--catraca`
-- e' assim que um script DECLARA o proprio modo `--catraca`. Um script que
so' MENCIONA a flag para chamar outro (um `subprocess.run([sys.executable,
alvo, "--catraca"])`, como este proprio arquivo faz) carrega `sys.executable`
na mesma linha, nunca `sys.argv`, e por isso nao se acha -- sem precisar de
uma lista de exclusao. Hoje isso acha cinco: os dois mapas de
`bancada/concorrencia/` e as tres de `bancada/guardas/`.

Este crivo e' PROPOSITALMENTE diferente do que o `docs/qa/medir.py' usa
(scripts que respondem a `--numeros` imprimindo `catraca:nome=`). Os dois
hoje acham os MESMOS cinco arquivos, mas respondem perguntas diferentes: o
`medir.py` pergunta "quem se autodescreve para a tabela do QA-PDCA?"; este
pergunta "quem tem um modo `--catraca` proprio, que julga passa/nao passa?".
Reusar o crivo do `medir.py` faria um script com `--catraca` mas sem
`--numeros` (ou vice-versa) desaparecer de um dos dois sem motivo nenhum --
nao e' duplicacao, e' responder pergunta diferente com o texto do proprio
codigo, nao com uma lista.

Como ele trata as TETO_* do Rust
---------------------------------
As TETO_* ja sao conferidas por `cargo test` (o clippy/fmt/suite do
`CLAUDE.md`), entao este comando nao reimplementa o julgamento: ele LISTA
toda `const TETO\w*` achada em `crates/**/*.rs` (src E examples -- o pedido
pede "do fonte", nao so' `src/`) e diz, para cada uma, qual `#[test]` -- no
proprio arquivo ou nos `tests/*.rs` do mesmo crate -- CITA o nome dela no
corpo. Quando nenhum teste cita o nome, ele diz isso tambem: uma TETO sem
teste candidato pode ser catraca de verdade cujo teste mora em outro lugar
que este crivo textual nao alcanca, ou pode ser so' um limite de
funcionamento (`docs/CATRACAS.md` explica a diferenca) -- este comando nao
decide qual das duas, so' aponta.

**Ele NAO roda `cargo test` por padrao**, e a razao e' medida, nao chutada:
nesta arvore, em 24/09/2026, `cargo test -p phxsql-server --lib -- --list`
(que arrasta phxsql-core, phxsql-store e phxsql-sql, onde mora a maioria das
TETO_*) levou ~19 s FRIO -- e continuou perto disso mesmo sem nenhuma linha
mudar, porque o build de teste completo dos quatro crates e' o custo, nao a
execucao. Dezenove segundos e' de mil a duas mil vezes o custo das cinco
catracas em Python somadas (~0,03 s a ~2 s cada). Rodar isso em toda chamada
tornaria o comando caro demais para o uso que o motiva -- todo commit, pelo
integrador --, e o proprio pedido manda medir antes de decidir e NAO rodar a
suite inteira. Quem quiser rodar de verdade tem o `--com-rust`, que builda e
roda `cargo test` so' dos crates que tem alguma TETO_* (ainda caro, mas
opcional e medido na hora).

O veredito de cada regua em Python: o codigo de saida manda
--------------------------------------------------------------
Nenhuma prosa se le para decidir aprovado/reprovado -- e' a mesma lei que o
`--numeros` de cada regua ja segue ("Ele NAO le a prosa"). O codigo de saida
0 e' "segura"; != 0 e' "nao segura". A diferenca entre REPROVADA (a regua
julgou e reprovou de verdade) e QUEBRADA (o medidor nao deu veredito nenhum)
importa para quem le o relatorio, mesmo os dois reprovando a chamada:

* QUEBRADA por SILENCIO -- stdout e stderr vazios. Um medidor cujo trabalho
  inteiro e' relatar, e que nao relata nada, e' o defeito que este pedido
  nomeia por extenso: "medidor quebrado nao e' verde". Sem este crivo, um
  script que capturasse toda excecao e devolvesse `None` (que em Python vira
  saida 0) passaria como se tivesse aprovado.
* QUEBRADA por EXCECAO -- saida != 0 e o stderr tem um traceback do Python.
  O medidor nao terminou o proprio julgamento.
* QUEBRADA por PRAZO -- nao respondeu dentro do teto (padrao 120 s; nenhuma
  das cinco custa perto disso hoje).
* REPROVADA -- saida != 0, respondeu, sem traceback: o julgamento aconteceu
  e disse "nao segura". A prosa impressa pela propria regua (capturada e
  ecoada aqui) e' o unico lugar onde o motivo aparece, porque so' quem
  escreveu a regua sabe explicar o proprio numero.
"""
import re
import subprocess
import sys
import time
from pathlib import Path

RAIZ = Path(__file__).resolve().parent.parent.parent
ESTE_ARQUIVO = Path(__file__).resolve()
TIMEOUT_PADRAO_S = 120

# Diretorios que nunca escondem uma regua e so' custariam tempo de varredura.
PASTAS_FORA = {".git", "target", "__pycache__", "node_modules"}


def _arquivo_interessa(caminho):
    return not (PASTAS_FORA & set(caminho.parts))


def descobrir_reguas_python():
    """Acha todo `.py` com um modo `--catraca` PROPRIO -- ver o docstring."""
    padrao = re.compile(
        r"(?:sys\.argv|add_argument)[^\n]*--catraca|--catraca[^\n]*(?:sys\.argv|add_argument)"
    )
    achadas = []
    for caminho in sorted(RAIZ.rglob("*.py")):
        if not _arquivo_interessa(caminho):
            continue
        if caminho == ESTE_ARQUIVO:
            continue  # o proprio comando nao se lista: ele nao tem --catraca
        try:
            texto = caminho.read_text(encoding="utf-8", errors="replace")
        except OSError:
            continue
        if padrao.search(texto):
            achadas.append(caminho)
    return achadas


# --------------------------------------------------------- as TETO_* do Rust

PADRAO_TETO = re.compile(r"\bconst\s+(TETO\w*)\s*:")


def descobrir_tetos_rust():
    """Toda `const TETO\\w*` em `crates/**/*.rs` -- src E examples.

    Retorna lista de (nome, arquivo_absoluto, linha). Nao distingue catraca
    de limite de funcionamento (essa distincao mora em `docs/CATRACAS.md` e
    exige julgamento humano do que a constante FAZ, nao so' do texto)."""
    achadas = []
    for arq in sorted(RAIZ.glob("crates/**/*.rs")):
        if not _arquivo_interessa(arq):
            continue
        texto = arq.read_text(encoding="utf-8", errors="replace")
        for n, linha in enumerate(texto.splitlines(), 1):
            m = PADRAO_TETO.search(linha)
            if m:
                achadas.append((m.group(1), arq, n))
    return achadas


def _testes_do_arquivo(texto):
    """Lista (nome_do_teste, corpo_com_chaves_balanceadas) de cada `#[test]`.

    O corpo sai balanceado -- e nao so' uma janela de linhas -- pelo mesmo
    motivo que o `docs/qa/medir.py` balanceia parenteses de `assert!`: uma
    janela fixa pega ou perde vizinho por acaso; contar chaves nao erra."""
    testes = []
    for m in re.finditer(r"#\[test\]", texto):
        pos = m.end()
        m2 = re.search(r"fn\s+(\w+)\s*\([^)]*\)[^{]*\{", texto[pos : pos + 400])
        if not m2:
            continue
        nome = m2.group(1)
        abre = pos + m2.end() - 1
        prof, j = 1, abre + 1
        while j < len(texto) and prof:
            if texto[j] == "{":
                prof += 1
            elif texto[j] == "}":
                prof -= 1
            j += 1
        testes.append((nome, texto[abre:j]))
    return testes


_CACHE_TESTES = {}


def _testes_cacheados(arquivo):
    if arquivo not in _CACHE_TESTES:
        texto = arquivo.read_text(encoding="utf-8", errors="replace")
        _CACHE_TESTES[arquivo] = _testes_do_arquivo(texto)
    return _CACHE_TESTES[arquivo]


def crate_de(arquivo):
    """`crates/phxsql-store/src/x.rs` -> `phxsql-store`."""
    partes = arquivo.relative_to(RAIZ).parts
    return partes[partes.index("crates") + 1]


def teste_que_confere(nome_const, arquivo):
    """Nomes `arquivo::teste` cujo corpo cita `nome_const` -- proprio arquivo
    e `tests/*.rs` do mesmo crate (onde mora o teste de integracao que usa a
    constante pelo caminho `crate::modulo::TETO_X`, como o da §5 do
    `docs/CATRACAS.md`)."""
    padrao = re.compile(r"\b" + re.escape(nome_const) + r"\b")
    achados = []
    for nome_teste, corpo in _testes_cacheados(arquivo):
        if padrao.search(corpo):
            achados.append(f"{arquivo.relative_to(RAIZ)}::{nome_teste}")
    tdir = RAIZ / "crates" / crate_de(arquivo) / "tests"
    if tdir.is_dir():
        for f in sorted(tdir.rglob("*.rs")):
            for nome_teste, corpo in _testes_cacheados(f):
                if padrao.search(corpo):
                    achados.append(f"{f.relative_to(RAIZ)}::{nome_teste}")
    return achados


# ------------------------------------------------------- rodar e julgar


def rodar_regua(caminho):
    """Roda uma regua em `--catraca` e devolve (estado, ok, tempo_s, saida).

    `estado` e' um rotulo curto para a linha do relatorio; `ok` e' o booleano
    que decide o codigo de saida deste comando; `saida` e' o texto (stdout +
    stderr) para ecoar quando nao segura -- so' quem escreveu a regua sabe
    explicar o proprio numero."""
    t0 = time.perf_counter()
    try:
        r = subprocess.run(
            [sys.executable, str(caminho), "--catraca"],
            capture_output=True,
            text=True,
            timeout=TIMEOUT_PADRAO_S,
        )
    except subprocess.TimeoutExpired:
        dt = time.perf_counter() - t0
        return (f"QUEBRADA (nao respondeu em {TIMEOUT_PADRAO_S}s)", False, dt, "")
    dt = time.perf_counter() - t0
    saida = (r.stdout or "") + (r.stderr or "")
    if not saida.strip():
        return ("QUEBRADA (nao imprime veredito)", False, dt, "")
    if r.returncode != 0 and "Traceback (most recent call last)" in (r.stderr or ""):
        ultima = [l for l in (r.stderr or "").splitlines() if l.strip()]
        motivo = ultima[-1] if ultima else "excecao sem mensagem"
        return (f"QUEBRADA (medidor caiu: {motivo})", False, dt, saida)
    if r.returncode != 0:
        return ("REPROVADA", False, dt, saida)
    return ("ok", True, dt, saida)


def imprimir_lista(reguas, tetos):
    print(f"=== {len(reguas)} regua(s) em Python com --catraca proprio ===")
    for r in reguas:
        print(f"  {r.relative_to(RAIZ)}")
    print(f"\n=== {len(tetos)} constante(s) TETO_* no fonte Rust ===")
    for nome, arq, linha in tetos:
        print(f"  {nome}  {arq.relative_to(RAIZ)}:{linha}")


def rodar_testes_rust(tetos):
    """`--com-rust`: builda e roda `cargo test --lib` dos crates com TETO_*.

    Nao filtra por teste (cargo so' aceita UM filtro de substring por
    chamada, e as TETO_* nao compartilham prefixo); roda a suite de lib
    inteira de cada crate envolvido, que e' o preco de pedir a conta na
    hora em vez de confiar no numero datado do docstring."""
    crates = sorted({crate_de(arq) for _, arq, _ in tetos})
    if not crates:
        return True
    cmd = ["cargo", "test"]
    for c in crates:
        cmd += ["-p", c]
    cmd += ["--lib"]
    print(f"\n=== --com-rust: {' '.join(cmd)} ===")
    t0 = time.perf_counter()
    r = subprocess.run(cmd, cwd=RAIZ)
    dt = time.perf_counter() - t0
    print(f"    ({dt:.1f}s, saida {r.returncode})")
    return r.returncode == 0


def main():
    args = sys.argv[1:]
    reguas = descobrir_reguas_python()
    tetos = descobrir_tetos_rust()

    if "--lista" in args:
        imprimir_lista(reguas, tetos)
        return 0

    print(f"=== catracas em Python ({len(reguas)} achadas por varredura de --catraca) ===")
    tudo_ok = True
    for caminho in reguas:
        estado, ok, dt, saida = rodar_regua(caminho)
        tudo_ok = tudo_ok and ok
        print(f"  {estado:<32} {caminho.relative_to(RAIZ)}  ({dt:.2f}s)")
        if not ok and saida.strip():
            for linha in saida.strip().splitlines()[-6:]:
                print(f"      {linha}")

    print(
        f"\n=== TETO_* do fonte Rust ({len(tetos)} achadas por varredura de "
        "'const TETO*:') ==="
    )
    print(
        "    NAO rodadas por padrao -- medido em 24/09/2026, "
        "`cargo test -p phxsql-server --lib -- --list` (arrasta core+store+sql) "
        "levou ~19s FRIO nesta arvore, caro demais para toda chamada. "
        "Use --com-rust para rodar de verdade."
    )
    sem_candidato = 0
    for nome, arq, linha in tetos:
        candidatos = teste_que_confere(nome, arq)
        if candidatos:
            rotulo = candidatos[0]
            if len(candidatos) > 1:
                rotulo += f"  (+{len(candidatos) - 1})"
        else:
            rotulo = "sem teste candidato achado"
            sem_candidato += 1
        print(f"  {nome:<30} {arq.relative_to(RAIZ)}:{linha:<6} {rotulo}")

    if "--com-rust" in args:
        if not rodar_testes_rust(tetos):
            tudo_ok = False

    print()
    print(
        f"TOTAL: {sum(1 for c in reguas if True)} regua(s) em Python "
        f"({'todas seguram' if tudo_ok else 'PELO MENOS UMA NAO SEGURA'}); "
        f"{len(tetos)} TETO_* listadas ({sem_candidato} sem teste candidato "
        "no crivo textual)"
    )
    return 0 if tudo_ok else 1


if __name__ == "__main__":
    sys.exit(main())
