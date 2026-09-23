"""A idade do binario, dita em voz alta antes de qualquer medicao.

Por que isto existe, e por que mora AQUI e nao dentro de cada bancada
---------------------------------------------------------------------
«Medidor com binario velho mede o passado» e petrea desta casa, e ela foi
paga tres vezes:

  1. Uma rodada inteira de ganhos (16,4 -> 7,5 us) ficou INVISIVEL na
     bancada de carga, porque `cargo build --release` nao recompila os
     `examples/` e ela chamava um. A conclusao «o esquema custa 2,2x»
     nasceu, com tabela e tudo, dessa diferenca.
  2. Em 23/09/2026 o papel F rodou a `prova-do-tunel.py` com um binario de
     antes do conserto dele e leu TRES `ERRO` como defeito do motor.
  3. Na mesma noite, o integrador rodou `cargo build --release --examples`
     antes da `sondar.py` -- e `--examples` constroi SO os examples, entao
     o `phxsqld` nao foi religado. A sonda devolveu o numero de antes do
     `DISTINCT` e do `UNION` entrarem, com a cara de numero fresco.

O terceiro caso e' o que obriga este arquivo a existir separado. O conserto
do caso 2 entrou SO na `prova-do-tunel.py`, e o caminho irmao -- a
`sondar.py`, que responde a mesma pergunta por outro caminho -- ficou sem
guarda. Guarda copiada e' guarda que diverge de si mesma; guarda importada,
nao. Quem escrever a proxima bancada importa daqui e herda a licao inteira.

O que ele NAO faz, de proposito
-------------------------------
Nao compila. Compilar dentro de uma bancada competiria com quem esta
compilando na mesma maquina -- e ja houve cinco frentes fazendo isso ao
mesmo tempo aqui. Ele DIZ a idade e nomeia o fonte mais novo; quem le
julga, em vez de acreditar.
"""
import sys
import time
from pathlib import Path


def conferir(binario, raiz, fatal=False):
    """Imprime a idade do `binario` contra os fontes de `raiz/crates`.

    Devolve `True` quando o binario esta na frente de todo fonte. Com
    `fatal=True`, um binario atrasado ENCERRA em vez de avisar -- que e' o
    que uma bancada quer quando o numero dela vai para uma pagina.
    """
    binario, raiz = Path(binario), Path(raiz)
    if not binario.exists():
        print(f"!! {binario} nao existe -- rode "
              f"`cargo build --release --bins` antes")
        sys.exit(1)
    bin_mt = binario.stat().st_mtime
    fontes = [f for f in (raiz / "crates").rglob("*.rs")
              if "/target/" not in str(f)]
    if not fontes:
        print("!! nenhum fonte .rs encontrado -- a raiz esta certa?")
        sys.exit(1)
    novo = max(fontes, key=lambda f: f.stat().st_mtime)
    atraso = novo.stat().st_mtime - bin_mt
    quando = time.strftime("%d/%m/%Y %H:%M", time.localtime(bin_mt))
    print(f"== binario: {quando} ({len(fontes)} fontes conferidos)")
    if atraso <= 0:
        return True
    print(f"!! {novo.relative_to(raiz)} e mais novo que o binario em "
          f"{int(atraso)}s -- esta corrida mede o PASSADO.")
    # `--bins` e nao `--examples`: foi essa troca que criou o caso 3.
    print("   Rode `cargo build --release --bins` e repita.")
    if fatal:
        sys.exit(1)
    return False
