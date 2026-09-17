#!/usr/bin/env python3
"""BULKINSERT: adiar o `.ndx` (carregar so o `.reg`/`.log` e reconstruir no
fim) compra quanto contra pagar os dois indices linha a linha -- na FORMA
exata do BULKINSERT: tabela reservada, VAZIA, N linhas de uma vez.

    flock /tmp/phx-cargo.lock cargo build --release --examples -p phxsql-store
    python3 bancada/carga/adiar-ndx/medir.py [--repeticoes N] [tamanhos...]

O medidor de verdade e `crates/phxsql-store/examples/bulkinsert-adiar-ndx.rs`
(instrumento, nao motor). Ele faz DUAS coisas por chamada: mede os dois
regimes E confere que os dois terminam no MESMO estado antes de apagar as
tabelas -- a bancada nao confere nada por conta propria, so recolhe as
`RESULTADO {json}` que ele imprime e junta a faixa min-max de VARIAS chamadas
de processo (a variancia de UM processo so nao e repeticao de verdade).

Por que este caso e so M=N, tabela vazia
-----------------------------------------
O pedido 114 (29/08/2026) ja mediu o caso GERAL -- carregar M linhas numa
tabela que ja tem N -- e fechou: o ganho cai para 1,22x quando M=N (dobra a
tabela) e vira prejuizo abaixo de M=~N/3, porque `reindexar` refaz a tabela
INTEIRA (N+M), nao so as linhas novas. Aquele medidor e
`--example adiar-vale-quando` e continua existindo; refazer aqui seria a
mesma bancada com nome diferente.

O que ESTE arquivo mede e o caso que o 114 nao cobria: BULKINSERT contra uma
tabela reservada e VAZIA (a carga que da nome ao recurso -- importar um
arquivo, migrar de outro banco, semear um ambiente). Ali N_antes=0 e M=N, e
o `reindexar` custa proporcional a M -- nao ha "tabela que ja tinha dado" para
inflar o refazer.
"""
import json
import os
import subprocess
import sys
import time
from pathlib import Path

AQUI = Path(__file__).resolve().parent
RAIZ = AQUI.parents[2]
sys.path.insert(0, str(RAIZ / "bancada" / "concorrencia"))
import quieta  # noqa: E402

BIN = RAIZ / "target" / "release" / "examples" / "bulkinsert-adiar-ndx"
FONTE_PROPRIA = RAIZ / "crates" / "phxsql-store" / "examples" / "bulkinsert-adiar-ndx.rs"


def binario_e_de_agora():
    """(vale, motivo) -- o relogio decide, nao a lembranca de ter compilado."""
    if not BIN.exists():
        return False, f"nao existe {BIN}"
    t_bin = BIN.stat().st_mtime
    mais_novo, quem = 0.0, None
    for p in list((RAIZ / "crates").rglob("*.rs")) + list((RAIZ / "crates").rglob("Cargo.toml")):
        t = p.stat().st_mtime
        if t > mais_novo:
            mais_novo, quem = t, p
    if mais_novo > t_bin:
        return False, (f"o binario e de {time.strftime('%H:%M:%S', time.localtime(t_bin))} e "
                        f"{quem.relative_to(RAIZ)} e de "
                        f"{time.strftime('%H:%M:%S', time.localtime(mais_novo))}")
    return True, (f"binario de {time.strftime('%H:%M:%S', time.localtime(t_bin))}, fonte mais "
                  f"novo ({quem.relative_to(RAIZ)}) de "
                  f"{time.strftime('%H:%M:%S', time.localtime(mais_novo))}")


def uma_chamada(n, repeticoes, vigia):
    """Uma chamada de processo faz `repeticoes` corridas dentro dele -- so a
    PRIMEIRA repeticao confere o estado (e mais caro); os demais so cronometram."""
    p = subprocess.run([str(BIN), str(n), str(repeticoes)],
                        capture_output=True, text=True)
    vigia.durante_a_rodada()
    if p.returncode != 0:
        raise SystemExit(
            f"bulkinsert-adiar-ndx {n} {repeticoes} saiu com rc={p.returncode}\n"
            f"--- stderr ---\n{p.stderr}\n--- stdout ---\n{p.stdout}")
    if "VERIFICACAO ok" not in p.stderr:
        raise SystemExit(
            f"a VERIFICACAO NAO apareceu para N={n} -- nao confio no resultado:\n{p.stderr}")
    linhas = [json.loads(l[len("RESULTADO "):]) for l in p.stdout.splitlines()
              if l.startswith("RESULTADO ")]
    if len(linhas) != repeticoes:
        raise SystemExit(f"esperava {repeticoes} linhas RESULTADO, vieram {len(linhas)}")
    return linhas, p.stderr


def faixa(valores):
    return min(valores), max(valores), sorted(valores)[len(valores) // 2]


def principal():
    args = sys.argv[1:]
    repeticoes = 5
    if "--repeticoes" in args:
        i = args.index("--repeticoes")
        repeticoes = int(args[i + 1])
        del args[i:i + 2]
    tamanhos = [int(a) for a in args] or [10_000, 100_000, 1_000_000]

    vale, motivo = binario_e_de_agora()
    print("=== BULKINSERT: adiar o `.ndx` contra pagar linha a linha ===")
    print(f"    binario: {motivo}")
    if not vale:
        print("\n    O BINARIO E VELHO. Rode antes:\n"
              "      flock /tmp/phx-cargo.lock cargo build --release --examples "
              "-p phxsql-store\n")
        return 2

    if quieta_ja_medindo := subprocess.run([str(RAIZ / "bancada" / "esta-medindo.sh")],
                                            capture_output=True).returncode == 0:
        print("\n    HA MEDICAO EM CURSO nesta maquina -- espere ela terminar.")
        return 2

    resultado = {"gerado_em": time.strftime("%Y-%m-%dT%H:%M:%S%z"), "repeticoes": repeticoes,
                 "tamanhos": {}}

    vigia = quieta.Vigia().abrir()
    for n in tamanhos:
        print(f"\n--- N={n:,} linhas, {repeticoes} chamadas de processo "
              f"intercaladas ---".replace(",", "."))
        linhas, stderr = uma_chamada(n, repeticoes, vigia)
        for l in stderr.splitlines():
            print(f"    {l}")
        inline = [l["inline_total_s"] for l in linhas]
        carga = [l["adiado_carga_s"] for l in linhas]
        reidx = [l["adiado_reindexar_s"] for l in linhas]
        adiado = [l["adiado_total_s"] for l in linhas]
        mi = faixa(inline)
        ma = faixa(adiado)
        print(f"    inline:  min {mi[0]:.4f}s  mediana {mi[2]:.4f}s  max {mi[1]:.4f}s")
        print(f"    adiado:  min {ma[0]:.4f}s  mediana {ma[2]:.4f}s  max {ma[1]:.4f}s"
              f"   (carga {faixa(carga)[2]:.4f}s + reindexar {faixa(reidx)[2]:.4f}s na mediana)")
        cruzam = not (mi[1] < ma[0] or ma[1] < mi[0])
        print(f"    faixas se cruzam? {'SIM -- sem vencedor' if cruzam else 'nao'}")
        resultado["tamanhos"][str(n)] = {
            "inline_s": {"min": mi[0], "mediana": mi[2], "max": mi[1], "todos": inline},
            "adiado_s": {"min": ma[0], "mediana": ma[2], "max": ma[1], "todos": adiado},
            "adiado_carga_s_mediana": faixa(carga)[2],
            "adiado_reindexar_s_mediana": faixa(reidx)[2],
            "faixas_se_cruzam": cruzam,
        }
    vigia.fechar()
    vigia.relatar()

    if not vigia.publicavel():
        print("\nNENHUM NUMERO DESTA RODADA VALE PUBLICACAO -- a maquina nao estava "
              "quieta. Rode de novo.")
        return 1

    parcial = AQUI / "resultados.parcial.json"
    final = AQUI / "resultados.json"
    with open(parcial, "w") as f:
        json.dump(resultado, f, indent=2, ensure_ascii=False)
    os.replace(parcial, final)
    print(f"\nGravado em {final}")
    return 0


if __name__ == "__main__":
    raise SystemExit(principal())
