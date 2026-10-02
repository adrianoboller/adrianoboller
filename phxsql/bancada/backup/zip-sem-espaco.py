#!/usr/bin/env python3
"""O ZIP SEM ESPACO -- pedido 513, passo 2a, com ENOSPC DE VERDADE.

    cargo build --release -p phxsql-server --bin phxsqld
    python3 bancada/backup/zip-sem-espaco.py [--mb 64] [--binario ...]   # precisa de root

# O que prova, e por que aqui e nao em `cargo test`

O zip em duas passadas copia a arvore inteira para uma pasta temporaria no
DESTINO antes de comprimir. Se o destino nao tem espaco para ela, a guarda
(`copiar_fase_1_para_zip`, `livre < tamanho + 10%`) manda a copia para a
passada unica e DIZ (`modo: retrato_inteiro` + `motivo`). Um teste unitario
so passa `Some(0)` para o parametro; quem mede o `df` de verdade e quem
prova que o zip final CABE e restaura e' um sistema de arquivos pequeno de
verdade -- um `tmpfs` montado so' para a corrida, que devolve ENOSPC real.

Tres situacoes, no mesmo banco:

  1. destino em tmpfs MENOR que o banco (o zip, comprimido, cabe):
     esperado `retrato_inteiro` com `motivo`, zip integro, sem `.retrato.part`;
  2. destino em tmpfs MAIOR que o banco: esperado `duas_passadas` (o irmao:
     um portao que mandasse tudo para a passada unica passaria o item 1);
  3. o portao desligado nao existe aqui -- o vermelho dele e' o binario com a
     guarda removida (`--so-red` roda o item 1 e imprime o que o servidor
     respondeu; com a guarda reposta a copia morre com ENOSPC).

NUNCA usa pkill: o servidor morre pelo PID guardado.
"""
import argparse
import importlib.util
import json
import os
import shutil
import subprocess
import sys
import time

AQUI = os.path.dirname(os.path.abspath(__file__))
RAIZ = os.path.abspath(os.path.join(AQUI, "..", ".."))
RESULTADOS = os.path.join(AQUI, "resultados.json")

# O medidor irmao tem hifen no nome: carrega pelo caminho, sem copiar nada.
_spec = importlib.util.spec_from_file_location("retrato", os.path.join(AQUI, "retrato-com-escritor.py"))
R = importlib.util.module_from_spec(_spec)
_spec.loader.exec_module(R)


def montar_tmpfs(ponto, mb):
    os.makedirs(ponto, exist_ok=True)
    subprocess.run(["mount", "-t", "tmpfs", "-o", f"size={mb}m", "tmpfs", ponto], check=True)


def desmontar(ponto):
    subprocess.run(["umount", ponto], check=False)
    shutil.rmtree(ponto, ignore_errors=True)


def um_backup_zip(fio, destino):
    t0 = time.perf_counter()
    r = fio({"op": "backup", "destino": destino, "zip": True})
    ms = (time.perf_counter() - t0) * 1000
    return r, ms


def sobras(destino):
    try:
        return sorted(n for n in os.listdir(destino) if n.endswith(".retrato.part"))
    except OSError:
        return []


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--mb", type=int, default=64)
    ap.add_argument("--binario", default=os.path.join(RAIZ, "target", "release", "phxsqld"))
    ap.add_argument("--rotulo", default="zip_sem_espaco")
    ap.add_argument("--so-red", action="store_true",
                    help="so' o item 1, sem exigir o veredito (para o binario com o defeito reposto)")
    a = ap.parse_args()
    if os.geteuid() != 0:
        raise SystemExit("precisa de root para montar o tmpfs; sem ele nao ha ENOSPC de verdade a medir")
    base = os.path.join("/tmp", f"phx-zip-513-{os.getpid()}")
    pequeno = os.path.join(base, "pequeno")
    grande = os.path.join(base, "grande")
    shutil.rmtree(base, ignore_errors=True)
    os.makedirs(base)
    p = R.subir(a.binario, os.path.join(base, "srv"))
    montados = []
    try:
        fio = R.Fio()
        linhas = R.criar_banco(fio, 6, a.mb)
        tamanho = R.bytes_da_raiz(os.path.join(base, "srv"))
        # menor que o banco (nao cabe a arvore) e maior que o zip comprimido
        tmpfs_pequeno = max(4, a.mb // 4)
        montar_tmpfs(pequeno, tmpfs_pequeno)
        montados.append(pequeno)
        montar_tmpfs(grande, int(a.mb * 2.5))
        montados.append(grande)
        print(f"banco {tamanho / 1048576:.0f} MiB; tmpfs pequeno {tmpfs_pequeno} MiB, grande {int(a.mb * 2.5)} MiB")
        res = {}

        r1, ms1 = um_backup_zip(fio, pequeno)
        res["pequeno"] = {"ok": r1.get("ok"), "ms": round(ms1, 1)}
        print("item 1 (tmpfs pequeno):", json.dumps(r1)[:600])
        if a.so_red:
            return
        f = []
        c1 = r1.get("resultado", {})
        if not r1.get("ok"):
            f.append(f"o backup em disco pequeno FALHOU em vez de cair na passada unica: {r1}")
        else:
            if c1.get("modo") != "retrato_inteiro":
                f.append(f"modo esperado retrato_inteiro, veio {c1.get('modo')}")
            if "sem espaco" not in c1.get("motivo", ""):
                f.append(f"o motivo nao diz 'sem espaco': {c1.get('motivo')!r}")
            if sobras(pequeno):
                f.append(f"sobrou arvore temporaria: {sobras(pequeno)}")
            zip_ = c1.get("arquivo", "")
            if not os.path.isfile(zip_):
                f.append(f"o zip nao existe: {zip_}")
            else:
                res["pequeno"]["zip_bytes"] = os.path.getsize(zip_)
                # `conferir_backup` so' le arvore; o zip se confere restaurando
                # (a restauracao confere o SHA-256 do manifesto de dentro).
                rr = fio({"op": "restaurar_backup", "origem": zip_, "de": R.DB, "database": "voltou"})
                if not rr.get("ok"):
                    f.append(f"restaurar_backup do zip: {rr}")
                else:
                    for t in linhas:
                        v = fio({"op": "varrer", "database": "voltou", "tabela": t, "max": 1})
                        o = fio({"op": "varrer", "database": R.DB, "tabela": t, "max": 1})
                        if not v.get("ok"):
                            f.append(f"varrer voltou.{t}: {v}")
                        elif v["resultado"].get("registros") != o["resultado"].get("registros"):
                            f.append(f"{t}: restaurado {v['resultado'].get('registros')} x "
                                     f"original {o['resultado'].get('registros')}")
        res["pequeno"]["modo"] = c1.get("modo")
        res["pequeno"]["motivo"] = c1.get("motivo")
        res["pequeno"]["livre_citado"] = c1.get("motivo")

        r2, ms2 = um_backup_zip(fio, grande)
        c2 = r2.get("resultado", {})
        res["grande"] = {"ok": r2.get("ok"), "ms": round(ms2, 1), "modo": c2.get("modo")}
        print("item 2 (tmpfs grande):", json.dumps(r2)[:400])
        if c2.get("modo") != "duas_passadas":
            f.append(f"com espaco o modo devia ser duas_passadas, veio {c2.get('modo')}")
        if "motivo" in c2:
            f.append("com espaco o servidor deu 'motivo' de falta de espaco")
        if sobras(grande):
            f.append(f"sobrou arvore temporaria no grande: {sobras(grande)}")

        retrato = {
            "quando": time.strftime("%Y-%m-%d %H:%M:%S"),
            "binario": a.binario,
            "mb_pedidos": a.mb,
            "bytes_em_disco": tamanho,
            "tmpfs_pequeno_mib": tmpfs_pequeno,
            "tmpfs_grande_mib": int(a.mb * 2.5),
            "itens": res,
            "falhas": f,
            "passou": not f,
        }
        R.gravar(a.rotulo, retrato)
        print("PASSOU" if not f else "FALHOU:\n  " + "\n  ".join(f))
        if f:
            sys.exit(1)
    finally:
        R.matar(p)
        for m in montados:
            desmontar(m)
        shutil.rmtree(base, ignore_errors=True)


if __name__ == "__main__":
    main()
