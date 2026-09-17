#!/usr/bin/env python3
"""Duas perguntas que a bateria de 17/09/2026 deixou abertas, medidas aqui.

    python3 bancada/replicacao/custo-do-binario.py [linhas]

**1. `musl` contra `gnu`, na MESMA carga.** A bancada de conteiner compara
conteiner com processo e, para o trabalho ser igual, roda o processo com o
binario **musl** (`docker/provar.py :: subir_processo`). Esta certo para o que
ela quer provar -- so o transporte muda. O efeito colateral e' que o numero
dela (`a-processos`) nao e' comparavel ao do `medir.py`, que roda o binario
`gnu`: sao duas libc diferentes no caminho de alocacao. Quem lesse os dois
lado a lado concluiria coisa errada sobre o conteiner.

**2. Onde esta o `fsync`.** O pedido 193 registrou uma queda do
`master_linhas_s` e nomeou como candidato «o `fsync` que a onda 2 pos no
caminho de escrita» -- e registrou, com todas as letras, que **isso nao estava
medido**. A premissa do item vem antes do item: se a onda 2 pos um `fsync` por
ESCRITA, a conta de chamadas cresce com o numero de linhas; se ela pos um por
FECHO DE JANELA, nao cresce. Aqui a conta sai do nucleo, por `strace`, e nao
de leitura de codigo.

Sobe UM servidor por medida, nas portas 5894-5895, num diretorio proprio, e
mata pelo PID que guardou. Nada de `pkill`.
"""
import json
import os
import re
import shutil
import signal
import socket
import subprocess
import sys
import time

AQUI = os.path.dirname(os.path.abspath(__file__))
RAIZ = os.path.abspath(os.path.join(AQUI, "..", ".."))
GNU = os.path.join(RAIZ, "target", "release", "phxsqld")
MUSL = os.path.join(RAIZ, "target", "x86_64-unknown-linux-musl", "release",
                    "phxsqld")
TOKEN = "espelho"


def hash_da_senha(binario, senha):
    saida = subprocess.run([binario, "--senha"], input=senha + "\n",
                           capture_output=True, text=True).stdout
    return saida.split('": "')[1].split('"')[0]


def liga(porta, segundos=25):
    fim = time.monotonic() + segundos
    erro = None
    while time.monotonic() < fim:
        try:
            s = socket.create_connection(("127.0.0.1", porta), timeout=120)
            s.settimeout(120)
            f = s.makefile("rwb")

            def fala(p, _f=f):
                p.setdefault("token", TOKEN)
                _f.write((json.dumps(p) + "\n").encode())
                _f.flush()
                return json.loads(_f.readline().decode())

            r = fala({"op": "login", "usuario": "adm", "senha": "segredo1"})
            if not r.get("ok"):
                raise SystemExit(f"login: {r}")
            return fala, s, f
        except OSError as e:
            erro = e
            time.sleep(0.3)
    raise SystemExit(f"nao consegui falar com 127.0.0.1:{porta}: {erro}")


def uma_carga(binario, porta, base, n, com_strace):
    """A MESMA carga do `medir.py`: lotes de 5.000, cinco colunas, memo."""
    shutil.rmtree(base, ignore_errors=True)
    os.makedirs(base, exist_ok=True)
    h = hash_da_senha(binario, "segredo1")
    cfg = {"base": "base", "bind": f"127.0.0.1:{porta}", "token": TOKEN,
           "web": {"ligado": False},
           "replicacao": {"papel": "source", "imagem_da_linha": True,
                          "id_servidor": "master"},
           "usuarios": [{"login": "adm", "nome": "Adriano", "id": 10,
                         "senha_hash": h,
                         "bases": {"*": {"ler": True, "inserir": True,
                                         "alterar": True, "excluir": True,
                                         "criar": True, "administrar": True,
                                         "diario": True, "verificar": True,
                                         "replicar": True}}}]}
    with open(os.path.join(base, "config.json"), "w") as g:
        json.dump(cfg, g, indent=2)
    log = open(os.path.join(base, "servidor.log"), "a")
    srv = subprocess.Popen([binario], cwd=base, stdout=log,
                           stderr=subprocess.STDOUT, stdin=subprocess.DEVNULL)
    time.sleep(2)
    fala, s, f = liga(porta)
    fala({"op": "criar_database", "database": "loja"})
    fala({"op": "criar_tabela", "database": "loja", "tabela": "clientes",
          "motivo_obrigatorio": False,
          "colunas": [{"nome": "id", "tipo": "Int4", "obrigatoria": True},
                      {"nome": "nome", "tipo": "Str(40)", "obrigatoria": True},
                      {"nome": "cidade", "tipo": "Str(30)"},
                      {"nome": "limite", "tipo": "Decimal(12,2)"},
                      {"nome": "ficha", "tipo": "Memo"}],
          "indices": [{"nome": "porId", "colunas": ["id"], "unico": True,
                       "primario": True}]})

    st, arquivo = None, os.path.join(base, "strace.txt")
    if com_strace:
        st = subprocess.Popen(
            ["strace", "-f", "-c", "-e",
             "trace=fsync,fdatasync,sync_file_range", "-p", str(srv.pid),
             "-o", arquivo])
        time.sleep(2)          # o strace tem de estar anexado ANTES da carga

    cid = ["Blumenau", "Joinville", "Itajai", "Curitiba", "Florianopolis"]
    t0 = time.perf_counter()
    i = 0
    while i < n:
        linhas = [{"id": k, "nome": f"Cliente {k:07d}", "cidade": cid[k % 5],
                   "limite": f"{k}.50",
                   "ficha": f"ficha do cliente {k}, com texto que mora no .memo"}
                  for k in range(i + 1, min(i + 5000, n) + 1)]
        r = fala({"op": "inserir_lote", "database": "loja",
                  "tabela": "clientes", "linhas": linhas})
        if not r.get("ok", True):
            raise SystemExit(f"carga: {r}")
        i += 5000
    carga_s = time.perf_counter() - t0

    contas, bruto = {}, ""
    if st is not None:
        st.send_signal(signal.SIGINT)
        st.wait(timeout=30)
        bruto = open(arquivo, errors="replace").read()
        for linha in bruto.splitlines():
            m = re.search(r"(\d+)\s+(fsync|fdatasync|sync_file_range)\s*$",
                          linha)
            if m:
                contas[m.group(2)] = int(m.group(1))

    # O cliente fecha primeiro: o TIME_WAIT tem de ficar do lado dele.
    f.close()
    s.close()
    srv.send_signal(signal.SIGTERM)
    try:
        srv.wait(timeout=20)
    except subprocess.TimeoutExpired:
        srv.kill()
    shutil.rmtree(base, ignore_errors=True)
    return {"linhas": n, "carga_s": round(carga_s, 3),
            "linhas_s": int(n / carga_s), "chamadas": contas,
            "total": sum(contas.values()),
            "por_linha": round(sum(contas.values()) / n, 6) if contas else None,
            "strace_bruto": bruto[-400:] if bruto else None}


def main():
    n = int(sys.argv[1]) if len(sys.argv) > 1 else 100_000
    if not os.path.exists(GNU):
        sys.exit(f"nao achei {GNU} -- `cargo build --release`")
    saida = {"linhas": n, "medido_em":
             time.strftime("%Y-%m-%d %H:%M", time.gmtime()) + " UTC"}
    base = f"/tmp/phx-custo-binario-{os.getpid()}"

    print(f"\n[1] a MESMA carga de {n:,} linhas, nos dois binarios")
    saida["gnu"] = uma_carga(GNU, 5894, base, n, com_strace=False)
    print(f"    gnu  {saida['gnu']['linhas_s']:,} linhas/s "
          f"({saida['gnu']['carga_s']} s)")
    if os.path.exists(MUSL):
        saida["musl"] = uma_carga(MUSL, 5895, base, n, com_strace=False)
        print(f"    musl {saida['musl']['linhas_s']:,} linhas/s "
              f"({saida['musl']['carga_s']} s)")
        saida["gnu_sobre_musl"] = round(
            saida["gnu"]["linhas_s"] / saida["musl"]["linhas_s"], 2)
        print(f"    gnu / musl = {saida['gnu_sobre_musl']}x")
    else:
        saida["musl"] = None
        print(f"    musl NAO MEDIDO -- nao achei {MUSL}. Rode "
              f"`cargo build --release --target x86_64-unknown-linux-musl "
              f"--bin phxsqld`")

    print(f"\n[2] quantos `fsync` custa a mesma carga (binario gnu)")
    saida["fsync"] = uma_carga(GNU, 5894, base, n, com_strace=True)
    fs = saida["fsync"]
    print(f"    {fs['total']} chamada(s) {fs['chamadas']} para {n:,} linhas "
          f"= {fs['por_linha']} por linha")
    print(f"    (com o strace anexado a carga rendeu "
          f"{fs['linhas_s']:,} linhas/s)")

    print("\nRESULTADO " + json.dumps(saida, ensure_ascii=False))
    alvo = os.path.join(AQUI, "custo-do-binario.json")
    with open(alvo, "w", encoding="utf-8") as g:
        json.dump(saida, g, ensure_ascii=False, indent=1)
    print(f"gravado em {alvo}")


if __name__ == "__main__":
    main()
