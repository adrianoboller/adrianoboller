#!/usr/bin/env python3
"""A catraca do `pkill` sem PID -- pedido 256.

    python3 bancada/guardas/pkill-sem-pid.py --catraca
    python3 bancada/guardas/pkill-sem-pid.py --numeros

# O defeito que motivou

`bancada/carga/bulkinsert.py` (e o irmao dela na mesma pasta,
`bancada/carga/medir.py`) chamavam `pkill -x phxsqld` para subir e para
derrubar o servidor proprio. `pkill` nunca aceita PID -- ele mata por NOME
ou por PADRAO em toda a maquina --, entao a chamada derrubava o `phxsqld` de
QUALQUER outra frente ou bancada viva ao lado. A regra da casa, que o
`zelador.sh` (que nem mata processo), o `prova-bateria.py` e o
`chutar-a-tomada.py` ja cumpriam, e subir o servidor com `Popen` direto (sem
`setsid`, que troca o PID debaixo do tapete) e derrubar so o PID guardado.

# Por que aqui, e nao em `bancada/guardas/catalogo.py`

O catalogo de guardas prova defeito reposto em codigo RUST: o executor
(`provar-guardas.py`) copia so `Cargo.toml`, `Cargo.lock`, `crates/`,
`exemplos/`, `docs/` e `testes-web/` para uma arvore a parte e roda
`cargo test` -- `bancada/` NAO esta nessa lista, entao uma entrada cujo
`arquivo` morasse la nunca rodaria (o executor nem a copiaria). O alcance do
catalogo e `crates/`; uma dívida de `bancada/` pede outro dono, e o dono
certo e uma catraca -- que e o que este arquivo e.

# O que ela conta, e o que ela NAO conta

CONTA: toda invocacao de verdade do comando `pkill` em `bancada/**/*.py` e
`bancada/**/*.sh` -- uma string entre aspas RETAS (`"pkill"`/`'pkill'`), do
jeito que um `subprocess.run([...])` python passa um argv, ou uma linha de
shell que COMECA (fora de comentario) com o comando.

NAO CONTA: a palavra `pkill` dentro de comentario ou docstring -- esta casa
sempre escreve isso entre CRASE (`` `pkill -f` ``), nunca entre aspas retas,
exatamente para que a leitura ("nunca pkill") nao se confunda com o uso. Um
arquivo pode e deve continuar dizendo "nunca `pkill`" no comentario acima do
`Popen` que prova a promessa.

# A catraca

So DESCE, e nasce hoje em 0: depois do conserto do pedido 256, a varredura
completa de `bancada/` nao acha mais nenhuma invocacao real. Um numero maior
reprova (alguem acrescentou `pkill`); um numero menor tambem reprova, porque
o piso ja e zero e nao ha como medir menos que isso -- o unico jeito de a
catraca aparecer "menor" seria um bug do proprio conferidor, que e
exatamente o que se quer pegar.
"""
import os
import sys

AQUI = os.path.dirname(os.path.abspath(__file__))
RAIZ = os.path.abspath(os.path.join(AQUI, "..", ".."))
BANCADA = os.path.join(RAIZ, "bancada")
EU = os.path.relpath(os.path.abspath(__file__), RAIZ)

TETO_PKILL_SEM_PID = 0


def achados():
    """Cada `arquivo:linha` com uma invocacao real de `pkill`.

    Exclui o proprio arquivo: o CRIVO precisa escrever `"pkill"` entre aspas
    retas para descrever e para implementar a busca, e sem a exclusao ele se
    acharia -- a mesma armadilha do `pgrep -f` que `esta-medindo.sh` documenta,
    so que aqui e uma string, nao uma linha de comando."""
    aqui = os.path.abspath(__file__)
    saida = []
    for dirpath, dirs, arquivos in os.walk(BANCADA):
        dirs[:] = [d for d in dirs if d != "__pycache__"]
        for nome in sorted(arquivos):
            if not (nome.endswith(".py") or nome.endswith(".sh")):
                continue
            caminho = os.path.join(dirpath, nome)
            if os.path.abspath(caminho) == aqui:
                continue
            try:
                with open(caminho, encoding="utf-8") as f:
                    linhas = f.readlines()
            except (OSError, UnicodeDecodeError):
                continue
            for n, linha in enumerate(linhas, 1):
                # invocacao python: pkill como argv de subprocess, entre
                # aspas RETAS -- o comentario desta casa usa crase, nunca isso
                if '"pkill"' in linha or "'pkill'" in linha:
                    saida.append((os.path.relpath(caminho, RAIZ), n, linha.strip()))
                    continue
                # invocacao de shell: comeca a linha (o comentario comeca
                # com `#`, nunca com o proprio comando)
                despida = linha.strip()
                if despida.startswith("pkill "):
                    saida.append((os.path.relpath(caminho, RAIZ), n, despida))
    return saida


def catraca():
    lista = achados()
    print("=== a catraca do `pkill` sem PID (pedido 256) ===")
    for arq, n, trecho in lista:
        print(f"   {arq}:{n}: {trecho[:90]}")
    medido = len(lista)
    if medido > TETO_PKILL_SEM_PID:
        print(f"\n   SUBIU  {medido} (teto {TETO_PKILL_SEM_PID})")
        print("   Reprovado: pkill mata por NOME na maquina inteira -- "
              "derruba so pelo PID guardado.")
        return 1
    if medido < TETO_PKILL_SEM_PID:
        print(f"\n   DESCEU -- BAIXE O TETO  {medido} (teto {TETO_PKILL_SEM_PID})")
        print("   Melhorou. Ponha o numero novo em TETO_PKILL_SEM_PID, no "
              "mesmo commit -- catraca frouxa nao segura nada.")
        return 1
    print(f"\n   ok  {medido} (teto {TETO_PKILL_SEM_PID})")
    return 0


def numeros():
    """Saida de maquina para o `docs/qa/medir.py` -- o inventario das catracas.

    Ele NAO le a prosa do `--catraca`, e nao le por decisao: `grep` em
    relatorio e resolver numero por comparacao de FRASE, e no dia em que
    alguem melhorar a redacao o inventario publica o numero de ontem sem
    dizer nada. A chave e estavel; o rotulo e livre.

    Este modo nasceu em 16/09/2026, com o buraco que ele fecha medido: o
    inventario gerado do `docs/QA-PDCA.md` varria `crates/*/examples/*.rs` e
    `crates/*/src/**`, e esta catraca -- que mora em `bancada/`, em Python --
    nao aparecia nele. O proprio `docs/CATRACAS.md` listava «sete catracas
    vivas que a tabela gerada nao conta» e esquecia esta: eram OITO."""
    print(f"catraca:nome=TETO_PKILL_SEM_PID;onde={EU};"
          f"valor={TETO_PKILL_SEM_PID};medido={len(achados())};tipo=teto;"
          "mede=invocacoes reais de `pkill` em bancada/**/*.py e *.sh")
    return 0


def principal():
    if "--catraca" in sys.argv:
        return catraca()
    if "--numeros" in sys.argv:
        return numeros()
    for arq, n, trecho in achados():
        print(f"{arq}:{n}: {trecho}")
    return 0


if __name__ == "__main__":
    sys.exit(principal())
