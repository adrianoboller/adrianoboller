#!/usr/bin/env python3
"""A sonda do `empilhar` rodada com o VIGIA na porta, e com o binario conferido.

    python3 bancada/concorrencia/reparticao-em-maquina-parada.py
    python3 bancada/concorrencia/reparticao-em-maquina-parada.py --mesmo-sujo

Por que este arquivo existe
---------------------------
O medidor `crates/phxsql-server/examples/reparticao-do-gatilho.rs` sabe medir
e nao sabe duas coisas que decidem se o numero dele vale:

1. **Se a maquina estava quieta.** Ele e um programa Rust em processo unico;
   nao le o `/proc/stat` nem sabe que uma compilacao comecou ao lado. O item
   164 ja recusou publicar duas vezes por isso, e a recusa foi a decisao
   certa. Quem sabe dizer «nao sei» e o `quieta.Vigia`, e ele mora em Python.
2. **Se o binario e o de agora.** `cargo build --release` NAO recompila os
   *examples*, e uma rodada inteira de ganhos ja ficou invisivel nesta casa
   porque a bancada chamava um binario de antes. Aqui a conferencia e do
   RELOGIO e nao da lembranca: o binario tem de ser mais novo que o fonte
   mais novo de `crates/`, senao a corrida nem comeca.

A segunda guarda e a que este arquivo acrescenta a bancada. Ela ja existia
como frase no `CLAUDE.md` -- «medidor com binario velho mede o passado» -- e
frase nao reprova corrida nenhuma.

O controle, e o que ele NAO e
------------------------------
O `Vigia` tem tres instrumentos, e o terceiro e a **curva de controle**: um
`ping`, que nao toma a trava de dados, medido no comeco e no fim. Se ele
desacelera entre as pontas, quem desacelerou foi a maquina.

A sonda do 164 sobe o servidor DENTRO do proprio processo, entao nao ha porta
para bater enquanto ela roda. O controle daqui e medido **antes e depois** da
sonda, com um `phxsqld` proprio que sobe, e medido, e morre -- ele nao fica de
pe durante a sonda, porque um vizinho ocioso ainda e um vizinho. Isso pega a
maquina que mudou de velocidade no periodo; NAO pega a que mudou so durante a
sonda, e para essa ficam as amostras de ocupacao, tiradas a cada dois
segundos com a sonda rodando.

A tolerancia de 15% do `Vigia` foi calibrada no `ping` pelo
`ruido-do-controle.py`, e e por isso que o controle daqui e `ping` tambem, e
nao um laco de CPU em Python: regua calibrada num instrumento nao vale noutro.
"""
import os
import subprocess
import sys
import threading
import time
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
import quieta  # noqa: E402

RAIZ = Path(__file__).resolve().parents[2]
SONDA = RAIZ / "target/release/examples/reparticao-do-gatilho"
FONTE = RAIZ / "crates/phxsql-server/examples/reparticao-do-gatilho.rs"
# O controle e curto de proposito: ele so precisa dizer se a maquina mudou de
# velocidade, e cada segundo dele e um segundo em que a maquina nao esta
# parada para a sonda.
os.environ.setdefault("SEGUNDOS", "4")


def carregar_o_arnes():
    """O `Servidor`, o `Cliente` e o `so_ping` do comboio, reaproveitados.

    Reaproveitados e nao copiados: harness copiado diverge do original no dia
    em que um dos dois for consertado, e ai duas bancadas medem coisas que
    parecem iguais.
    """
    import importlib.util
    caminho = Path(__file__).resolve().parent / "o-comboio-do-fecho.py"
    spec = importlib.util.spec_from_file_location("comboio", caminho)
    mod = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(mod)
    return mod


def o_binario_e_de_agora():
    """(vale, motivo). O relogio decide, nao a lembranca de ter compilado.

    Compara com o fonte mais novo de `crates/` inteiro, e nao so com o
    `examples/*.rs`: o que a sonda mede e o `servidor.rs`, e foi ele que mudou
    nas duas vezes em que esta armadilha mordeu.
    """
    if not SONDA.exists():
        return False, f"nao existe {SONDA}"
    binario = SONDA.stat().st_mtime
    mais_novo, quem = 0.0, None
    for p in (RAIZ / "crates").rglob("*.rs"):
        t = p.stat().st_mtime
        if t > mais_novo:
            mais_novo, quem = t, p
    for p in (RAIZ / "crates").rglob("Cargo.toml"):
        t = p.stat().st_mtime
        if t > mais_novo:
            mais_novo, quem = t, p
    if mais_novo > binario:
        return False, (
            f"o binario e de {time.strftime('%H:%M:%S', time.localtime(binario))}"
            f" e {quem.relative_to(RAIZ)} e de "
            f"{time.strftime('%H:%M:%S', time.localtime(mais_novo))}")
    return True, (
        f"binario de {time.strftime('%H:%M:%S', time.localtime(binario))}, "
        f"fonte mais novo ({quem.relative_to(RAIZ)}) de "
        f"{time.strftime('%H:%M:%S', time.localtime(mais_novo))}")


def controle(comboio, vigia):
    """Um `phxsqld` que sobe, e batido de `ping`, e morre na mesma funcao."""
    porta = quieta.porta_livre()
    srv = comboio.Servidor(porta, "por_lote")
    try:
        c = comboio.Cliente(porta)
        c.call({"op": "login", "usuario": "root", "senha": comboio.SENHA})
        return comboio.so_ping(porta, vigia)
    finally:
        srv.parar()


def rodar_a_sonda(vigia, argumentos):
    """A sonda como subprocesso, com amostras de ocupacao enquanto ela roda.

    `meus=1`: a sonda e um processo so, e sem descontar ela o proprio vigia
    acusaria a si mesmo -- instrumento que recusa sempre nao e mais util que
    instrumento que nunca recusa.
    """
    p = subprocess.Popen([str(SONDA)] + argumentos,
                         stdout=subprocess.PIPE, stderr=subprocess.STDOUT,
                         text=True)
    saida = []
    leitor = threading.Thread(target=lambda: saida.append(p.stdout.read()))
    leitor.start()
    while p.poll() is None:
        vigia.durante_a_rodada(meus=1)
        time.sleep(1.7)
    leitor.join()
    return p.returncode, (saida[0] if saida else "")


def principal():
    sujo_vale = "--mesmo-sujo" in sys.argv
    argumentos = [a for a in sys.argv[1:] if not a.startswith("--")]

    vale, motivo = o_binario_e_de_agora()
    print("=== a reparticao do `empilhar`, em maquina parada ===")
    print(f"    {quieta.nucleos()} nucleos | sonda: {SONDA.name}")
    print(f"    binario: {motivo}")
    if not vale:
        print("\n    O BINARIO E VELHO. Medidor com binario velho mede o "
              "passado.\n    Rode antes:\n"
              "      cargo build --release -p phxsql-server --examples --bins")
        return 2
    print()

    comboio = carregar_o_arnes()
    if not comboio.PHXSQLD.exists():
        print(f"falta {comboio.PHXSQLD} -- o controle `ping` precisa dele")
        return 2

    vigia = quieta.Vigia().abrir()
    vigia.controle_antes = controle(comboio, vigia)
    rc, saida = rodar_a_sonda(vigia, argumentos)
    vigia.controle_depois = controle(comboio, vigia)
    vigia.fechar()
    vigia.relatar()

    if rc != 0:
        print(f"a sonda saiu com rc={rc}; a saida esta abaixo por ser erro,\n"
              "nao numero:\n")
        print(saida)
        return rc

    if not vigia.publicavel() and not sujo_vale:
        print("Nenhum numero sai desta rodada -- e a saida da sonda NAO vai\n"
              "impressa, de proposito: numero sujo publicado com ressalva ao\n"
              "lado vira, tres documentos adiante, numero limpo.\n"
              "Rode de novo com a maquina parada, ou --mesmo-sujo para\n"
              "depurar o proprio arnes.")
        return 1
    if not vigia.publicavel():
        print(">>> NUMEROS SUJOS: a maquina nao estava parada. NAO CITAR. <<<\n")
    print(saida)
    return 0


if __name__ == "__main__":
    raise SystemExit(principal())
