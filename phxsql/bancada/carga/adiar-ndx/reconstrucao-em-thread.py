#!/usr/bin/env python3
"""Regime (c), pedido do dono em 17/09/2026: o BULKINSERT adia o `.ndx`,
SOLTA a reserva assim que o `.reg` esta gravado, e reconstroi o indice em
THREAD DE FUNDO -- com o servidor atendendo normalmente. A pergunta nao e se
a carga fica mais rapida (fica, por construcao: tirar 10,8 us/linha do
caminho critico). A pergunta e QUANTO A OPERACAO CONCORRENTE SENTE enquanto
o indice se reconstroi por tras.

    flock /tmp/phx-cargo.lock cargo build --release --examples -p phxsql-store --bins
    flock /tmp/phx-cargo.lock cargo build --release --bins -p phxsql-server
    python3 bancada/carga/adiar-ndx/reconstrucao-em-thread.py [n] [repeticoes]

Por que a RPC "reindexar" mede o regime de verdade, sem existir ainda
------------------------------------------------------------------------
O motor nao tem (ainda) indice suspenso nem reconstrucao de fato em thread --
essa e a divida citada em `crates/phxsql-server/src/carga.rs`. Mas a RPC
"reindexar" ja chama exatamente `Table::reindexar()` sob a MESMA trava global
(`self.dados`, um `RwLock`) que toda leitura e escrita do servidor usa
(`servidor.rs::travar_dados`/`travar_dados_para_ler`) -- e o servidor NAO
DISTINGUE "RPC de um cliente" de "chamada interna de uma thread de fundo": as
duas tomam a MESMA trava do MESMO jeito, pela mesma funcao. Chamar essa RPC
numa conexao separada enquanto outra conexao martela outra tabela reproduz
fielmente o que uma thread de fundo presa a essa trava causaria.

A tabela `carga` chega pronta para a RPC no estado que uma carga adiada
deixaria -- `.reg`/`.log` cheios, `.ndx` vazio -- via
`--example preparar-carga-adiada`, rodado ANTES de o servidor subir e por
isso FORA da janela medida (e o mesmo motivo de a bancada de rede separar
"preparo" de "carga" em outros arquivos desta pasta).
"""
import json
import os
import socket
import subprocess
import sys
import threading
import time
from pathlib import Path

AQUI = Path(__file__).resolve().parent
RAIZ = AQUI.parents[2]
sys.path.insert(0, str(RAIZ / "bancada" / "concorrencia"))
import quieta  # noqa: E402

PHXSQLD = RAIZ / "target" / "release" / "phxsqld"
PREPARADOR = RAIZ / "target" / "release" / "examples" / "preparar-carga-adiada"
BASE = Path("/tmp/phx-c-thread")
PORTA = 5912
TOKEN = "carga"

BASELINE_S = 3.0
DEPOIS_S = 3.0


def binario_e_de_agora(bin_path, rotulo):
    if not bin_path.exists():
        return False, f"nao existe {bin_path}"
    t_bin = bin_path.stat().st_mtime
    mais_novo, quem = 0.0, None
    for p in list((RAIZ / "crates").rglob("*.rs")) + list((RAIZ / "crates").rglob("Cargo.toml")):
        t = p.stat().st_mtime
        if t > mais_novo:
            mais_novo, quem = t, p
    if mais_novo > t_bin:
        return False, (f"{rotulo} e de {time.strftime('%H:%M:%S', time.localtime(t_bin))}, "
                        f"anterior a {quem.relative_to(RAIZ)} "
                        f"({time.strftime('%H:%M:%S', time.localtime(mais_novo))})")
    return True, f"{rotulo} de {time.strftime('%H:%M:%S', time.localtime(t_bin))}, e o mais novo"


def preparar_base(n):
    if BASE.exists():
        subprocess.run(["rm", "-rf", str(BASE)], check=True)
    (BASE / "base" / "loja").mkdir(parents=True)
    with open(BASE / "config.json", "w") as f:
        json.dump({"base": "base", "bind": f"127.0.0.1:{PORTA}", "token": TOKEN,
                   "web": {"ligado": False}}, f)
    r = subprocess.run([str(PREPARADOR), str(BASE / "base" / "loja"), str(n)],
                        capture_output=True, text=True)
    if r.returncode != 0:
        raise SystemExit(f"preparar-carga-adiada falhou:\n{r.stdout}\n{r.stderr}")
    print(f"    {r.stdout.strip()}")


def subir():
    log = open(BASE / "servidor.log", "a")
    proc = subprocess.Popen([str(PHXSQLD)], cwd=str(BASE), stdout=log,
                            stderr=subprocess.STDOUT, stdin=subprocess.DEVNULL)
    for _ in range(100):
        time.sleep(0.1)
        try:
            socket.create_connection(("127.0.0.1", PORTA), timeout=0.3).close()
            return proc
        except OSError:
            if proc.poll() is not None:
                raise SystemExit(f"servidor morreu ao subir -- veja {BASE}/servidor.log")
    proc.kill()
    raise SystemExit("servidor nao subiu")


def derrubar(proc):
    """So o PID que `subir` criou -- nunca `pkill` (regra da casa: zelador
    nem mata processo, e as bancadas irmas so matam o proprio PID)."""
    if proc.poll() is None:
        proc.terminate()
        try:
            proc.wait(timeout=10)
        except subprocess.TimeoutExpired:
            proc.kill()
            proc.wait(timeout=5)


class Cliente:
    def __init__(self):
        self.s = socket.create_connection(("127.0.0.1", PORTA))
        self.f = self.s.makefile("rwb")

    def fala(self, p):
        p.setdefault("token", TOKEN)
        self.f.write((json.dumps(p) + "\n").encode())
        self.f.flush()
        linha = self.f.readline()
        if not linha:
            raise SystemExit("conexao caiu no meio da medicao")
        return json.loads(linha.decode())

    def ok(self, p):
        r = self.fala(p)
        if not r.get("ok"):
            raise SystemExit(f"{p['op']}: {r.get('erro')}")
        return r["resultado"]

    def fechar(self):
        self.f.close()
        self.s.close()


class Operador(threading.Thread):
    """A operacao "a todo vapor": insere sequencialmente em `operacao`, uma
    linha por pedido, registrando (instante_do_ENVIO, latencia) de cada um --
    e a latencia isolada de um pedido e o que revela a pior parada."""

    def __init__(self):
        super().__init__()
        self.cliente = Cliente()
        self.parar = threading.Event()
        self.amostras = []  # [(t_envio_perf_counter, latencia_s), ...]
        self.proximo_id = 1
        self.erro = None

    def run(self):
        try:
            while not self.parar.is_set():
                t0 = time.perf_counter()
                self.cliente.ok({"op": "inserir", "database": "loja", "tabela": "operacao",
                                  "linha": {"id": self.proximo_id, "valor": self.proximo_id}})
                dt = time.perf_counter() - t0
                self.amostras.append((t0, dt))
                self.proximo_id += 1
        except Exception as e:  # noqa: BLE001
            self.erro = e
            self.parar.set()

    def parar_e_juntar(self):
        self.parar.set()
        self.join(timeout=15)
        self.cliente.fechar()


def op_por_s_na_janela(amostras, t_ini, t_fim):
    n = sum(1 for (t, _) in amostras if t_ini <= t < t_fim)
    dur = t_fim - t_ini
    return n / dur if dur > 0 else 0.0


def pior_pausa_na_janela(amostras, t_ini, t_fim):
    lat = [dt for (t, dt) in amostras if t_ini <= t < t_fim]
    return max(lat) if lat else 0.0


def uma_corrida(n, vigia):
    print(f"  preparando base com {n} linhas em `carga` (fora da janela medida)...")
    preparar_base(n)
    proc = subir()
    try:
        setup = Cliente()
        # `preparar-carga-adiada` ja criou o diretorio `base/loja` na marra
        # (fora do protocolo, de proposito -- e o unico jeito de deixar o
        # `.ndx` vazio sem o motor ter um modo de indice suspenso). O
        # servidor ve esse diretorio como database EXISTENTE, entao
        # `criar_database` aqui so seria repeticao -- e falharia dizendo
        # "ja existe".
        setup.ok({"op": "criar_tabela", "database": "loja", "tabela": "operacao",
                  "colunas": [{"nome": "id", "tipo": "Int8", "obrigatoria": True},
                              {"nome": "valor", "tipo": "Int8"}],
                  "indices": [{"nome": "porId", "colunas": ["id"], "unico": True,
                               "primario": True}]})

        op = Operador()
        op.start()
        time.sleep(BASELINE_S)
        t_baseline_fim = time.perf_counter()

        reindex = Cliente()
        t_ini_reindex = time.perf_counter()
        r = reindex.ok({"op": "reindexar", "database": "loja", "tabela": "carga"})
        t_fim_reindex = time.perf_counter()
        reindex.fechar()

        vigia.durante_a_rodada(meus=2)
        time.sleep(DEPOIS_S)
        t_depois_fim = time.perf_counter()

        op.parar_e_juntar()
        if op.erro:
            raise SystemExit(f"o operador quebrou: {op.erro}")
        if not op.amostras:
            raise SystemExit("o operador nao completou nenhum pedido -- corrida invalida")

        t0 = op.amostras[0][0]
        baseline_ops = op_por_s_na_janela(op.amostras, t0, t_baseline_fim)
        durante_ops = op_por_s_na_janela(op.amostras, t_ini_reindex, t_fim_reindex)
        depois_ops = op_por_s_na_janela(op.amostras, t_fim_reindex, t_depois_fim)
        pior_durante = pior_pausa_na_janela(op.amostras, t_ini_reindex - 0.5, t_fim_reindex + 0.5)
        pior_fora = pior_pausa_na_janela(op.amostras, t0, t_ini_reindex - 0.5)

        depois = setup.ok({"op": "verificar", "database": "loja", "tabela": "carga"})
        assert depois["registros"] == n, depois
        for nome_idx, qtd in depois["indices"].items():
            assert qtd == n, f"{nome_idx}: {qtd} de {n}"
        op_verif = setup.ok({"op": "verificar", "database": "loja", "tabela": "operacao"})
        assert op_verif["registros"] == len(op.amostras), \
            (op_verif["registros"], len(op.amostras))

        setup.fechar()
        return {
            "n": n,
            "reindexar_s": t_fim_reindex - t_ini_reindex,
            "reindexar_resultado": r,
            "baseline_op_s": baseline_ops,
            "durante_op_s": durante_ops,
            "depois_op_s": depois_ops,
            "pior_pausa_durante_s": pior_durante,
            "pior_pausa_fora_s": pior_fora,
            "total_ops_operador": len(op.amostras),
        }
    finally:
        derrubar(proc)


def principal():
    args = [a for a in sys.argv[1:] if not a.startswith("--")]
    n = int(args[0]) if len(args) > 0 else 200_000
    repeticoes = int(args[1]) if len(args) > 1 else 3

    ok1, m1 = binario_e_de_agora(PHXSQLD, "phxsqld")
    ok2, m2 = binario_e_de_agora(PREPARADOR, "preparar-carga-adiada")
    print("=== regime (c): reconstrucao do `.ndx` em thread, reserva solta ===")
    print(f"    {m1}")
    print(f"    {m2}")
    if not (ok1 and ok2):
        print("\n    BINARIO VELHO. Rode antes:\n"
              "      flock /tmp/phx-cargo.lock cargo build --release --examples "
              "-p phxsql-store --bins\n"
              "      flock /tmp/phx-cargo.lock cargo build --release --bins -p phxsql-server\n")
        return 2

    r = subprocess.run([str(RAIZ / "bancada" / "esta-medindo.sh")], capture_output=True)
    if r.returncode == 0:
        print("\n    HA MEDICAO EM CURSO nesta maquina -- espere.")
        return 2

    vigia = quieta.Vigia().abrir()
    corridas = []
    for i in range(repeticoes):
        print(f"\n--- corrida {i+1}/{repeticoes}, N={n:,} ---".replace(",", "."))
        c = uma_corrida(n, vigia)
        corridas.append(c)
        print(f"    reindexar levou {c['reindexar_s']*1000:.1f} ms")
        print(f"    op/s do operador -- antes: {c['baseline_op_s']:.0f}  "
              f"durante: {c['durante_op_s']:.0f}  depois: {c['depois_op_s']:.0f}")
        print(f"    PIOR PAUSA durante a reconstrucao: "
              f"{c['pior_pausa_durante_s']*1000:.1f} ms  "
              f"(fora dela, pior foi {c['pior_pausa_fora_s']*1000:.1f} ms)")
    vigia.fechar()
    vigia.relatar()

    if not vigia.publicavel():
        print("NENHUM NUMERO DESTA RODADA VALE PUBLICACAO -- a maquina nao estava quieta.")
        return 1

    resultado = {
        "gerado_em": time.strftime("%Y-%m-%dT%H:%M:%S%z"),
        "n": n, "repeticoes": repeticoes, "corridas": corridas,
    }
    parcial = AQUI / f"resultados-regime-c-{n}.parcial.json"
    final = AQUI / f"resultados-regime-c-{n}.json"
    with open(parcial, "w") as f:
        json.dump(resultado, f, indent=2, ensure_ascii=False)
    os.replace(parcial, final)
    print(f"\nGravado em {final}")
    return 0


if __name__ == "__main__":
    raise SystemExit(principal())
