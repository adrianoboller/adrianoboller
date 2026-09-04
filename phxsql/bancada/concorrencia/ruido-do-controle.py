#!/usr/bin/env python3
"""Quanto varia o RUIDO DO CONTROLE (`ping`) nesta maquina, hoje? -- medido.

    python3 bancada/concorrencia/ruido-do-controle.py
    RODADAS=40 RODADA_S=1.5 python3 bancada/concorrencia/ruido-do-controle.py --json

Por que este medidor existe
----------------------------
O `quieta.Vigia` recusa publicar quando o `ping` -- o CONTROLE, que nao toma
a trava de dados -- andou mais que `tolerancia_controle` entre o comeco e o
fim de uma bateria. Esse numero vivia no `quieta.py` com o motivo em prosa
("e a dispersao que o ping mostra numa maquina parada") mas sem a corrida que
o sustenta: quantas baterias, que dispersao, medida quando. Isso e exatamente
o que esta casa chama de numero CITADO e nao MEDIDO -- a mesma doenca do
"120%" registrado no `docs/CONCORRENCIA.md`, que descreve uma maquina OCUPADA
(outras frentes compilando ao lado) e nao a dispersao natural de uma maquina
ociosa. Um numero medido em condicao suja nao serve de teto para condicao
limpa, e citar um sem o outro e o erro que este arquivo evita repetir.

O que ele mede, e por que MUITAS corridas e nao uma
-----------------------------------------------------
Nao mede UMA bateria: mede `RODADAS` (30 por padrao) baterias de controle de
nariz a nariz, 1 cliente, `ping` puro -- o mesmo caminho que o
`a-trava-serializa.py` usa como controle, reaproveitado por importacao direta
dele (nao reescrito: duas copias do mesmo medidor divergem sem ninguem notar).
Cada corrida carrega tambem, no MOMENTO em que rodou, a ocupacao e os
vizinhos rodaveis (`quieta.Amostra`) -- o que separa as corridas QUIETAS
(no maximo o proprio arnes) das SUJAS (outra frente do lado), em vez de
misturar as duas num unico numero que nao diz qual condicao gerou o qual.

O teto que sai daqui -- e a clausula que ele obedece
-----------------------------------------------------
So desce. A dispersao das corridas QUIETAS e a candidata a novo teto porque e
ela que descreve "a maquina parada" -- a mesma frase que ja estava no
comentario do `quieta.py`, agora com numero atras. Se essa dispersao for
MENOR que a `tolerancia_controle` em vigor, o numero antigo era generoso
demais (deixava passar bateria com mais ruido do que uma maquina realmente
parada produz) e este arquivo propoe o novo, com a margem de seguranca escrita
ao lado -- e quem aplica a proposta ao `quieta.py` decide por escrito, porque
mudar o default do Vigia mexe em TODOS os medidores desta pasta de uma vez.
Se a dispersao medida for MAIOR que a tolerancia em vigor, o numero em vigor
FICA: subir o teto e o que a clausula petrea proibe, mesmo com o motivo
escrito do lado.
"""
import importlib.util
import json
import os
import statistics
import sys
from pathlib import Path

AQUI = Path(__file__).resolve().parent
sys.path.insert(0, str(AQUI))
import quieta  # noqa: E402

# A porta e a pasta de trabalho tem de estar decididas ANTES de importar o
# a-trava-serializa.py, porque o cabecalho dele so as fixa com `setdefault`
# -- e "antes" aqui e o proprio motivo de este arquivo nao poder herdar a
# porta fixa 7497 do medidor antigo: dentro da FAIXA desta frente, livre.
os.environ.setdefault("PORTA", str(quieta.porta_livre()))
os.environ.setdefault("PHX_TRABALHO", f"/tmp/phx-ruido-{os.getpid()}")

# Reaproveita o `a-trava-serializa.py` por IMPORTACAO, nao por copia: o
# `rodada("sem-trava", ...)` de la e literalmente o controle que o Vigia usa
# em toda bateria desta pasta. Medir a dispersao de uma copia diferente
# mediria outro medidor, nao o que decide se um numero publica.
_t = AQUI / "a-trava-serializa.py"
spec = importlib.util.spec_from_file_location("trava_para_ruido", _t)
trava = importlib.util.module_from_spec(spec)
spec.loader.exec_module(trava)  # so define funcoes/constantes: principal()
pp = trava.pp                   # e guardado por `if __name__ == "__main__"`.

RODADAS = int(os.environ.get("RODADAS", "30"))
RODADA_S = float(os.environ.get("RODADA_S", "1.0"))


def estatisticas(xs):
    m = statistics.mean(xs)
    sd = statistics.pstdev(xs) if len(xs) > 1 else 0.0
    return {
        "n": len(xs),
        "media": m,
        "desvio": sd,
        "cv": (sd / m * 100) if m else 0.0,
        "min": min(xs),
        "max": max(xs),
        # "salto" e o mesmo calculo que o Vigia faz entre controle_antes e
        # controle_depois: (maior - menor) / referencia. Aqui a referencia e
        # a media da amostra inteira, para nao depender de qual corrida caiu
        # primeiro.
        "salto": ((max(xs) - min(xs)) / m * 100) if m else 0.0,
    }


def principal():
    if not pp.PHXSQLD.exists():
        print(f"falta {pp.PHXSQLD} -- rode "
              "`flock /tmp/phx-cargo.lock cargo build --release` antes")
        return 2

    vigia = quieta.Vigia().abrir()
    srv = pp.Phxsqld()
    try:
        print("=== ruido do controle (ping), hoje ===")
        print(f"    {RODADAS} corridas de {RODADA_S:.1f}s, 1 cliente, "
              f"nucleos={quieta.nucleos()}\n")

        corridas = []
        for i in range(RODADAS):
            ops, _cpu = trava.rodada("sem-trava", 1, RODADA_S, vigia)
            a = vigia.durante[-1]
            corridas.append({"i": i, "ops": ops, "ocupada": a.ocupada,
                              "vizinhos": a.vizinhos})
            print(f"   [{i + 1:2d}/{RODADAS}] {ops:8.0f} op/s   "
                  f"ocupada {a.ocupada:3.0f}%   vizinhos {a.vizinhos}")

        vigia.fechar()

        todas = [c["ops"] for c in corridas]
        quietas = [c["ops"] for c in corridas if c["vizinhos"] <= 1]
        sujas = [c["ops"] for c in corridas if c["vizinhos"] > 1]

        e_todas = estatisticas(todas)
        print(f"\n-- todas as {e_todas['n']} corridas")
        print(f"   media {e_todas['media']:8.0f} op/s   desvio {e_todas['desvio']:6.0f}   "
              f"CV {e_todas['cv']:5.1f}%   min {e_todas['min']:8.0f}   "
              f"max {e_todas['max']:8.0f}   salto {e_todas['salto']:5.1f}%")

        e_q = None
        if quietas:
            e_q = estatisticas(quietas)
            print(f"\n-- corridas QUIETAS (vizinhos<=1): {e_q['n']} de {RODADAS}")
            print(f"   media {e_q['media']:8.0f} op/s   desvio {e_q['desvio']:6.0f}   "
                  f"CV {e_q['cv']:5.1f}%   min {e_q['min']:8.0f}   "
                  f"max {e_q['max']:8.0f}   salto {e_q['salto']:5.1f}%")
        else:
            print(f"\n-- nenhuma das {RODADAS} corridas ficou QUIETA (vizinhos<=1): "
                  "a maquina nao parou hoje, e nao ha base limpa para propor teto.")

        e_s = None
        if sujas:
            e_s = estatisticas(sujas)
            print(f"\n-- corridas SUJAS (vizinhos>1): {e_s['n']} de {RODADAS}")
            print(f"   media {e_s['media']:8.0f} op/s   desvio {e_s['desvio']:6.0f}   "
                  f"CV {e_s['cv']:5.1f}%   min {e_s['min']:8.0f}   "
                  f"max {e_s['max']:8.0f}   salto {e_s['salto']:5.1f}%")

        teto_atual = vigia.tolerancia_controle * 100
        print(f"\n-- teto em vigor hoje no quieta.Vigia (tolerancia_controle): "
              f"{teto_atual:.0f}%")

        if e_q and e_q["n"] >= 5:
            # Margem: 1,5x o pior salto ja visto QUIETO, com piso de 3 desvios
            # padrao -- os dois sao escolhas de MARGEM de seguranca sobre um
            # numero medido, nao o teto em si. O maior dos dois manda.
            margem_salto = e_q["salto"] * 1.5
            margem_desvio = e_q["cv"] * 3
            proposto = max(margem_salto, margem_desvio)
            print(f"   proposta medida hoje: max(salto quieto x1,5 = "
                  f"{margem_salto:.1f}%, 3 desvios = {margem_desvio:.1f}%) "
                  f"= {proposto:.1f}%")
            if proposto < teto_atual:
                print(f"   MENOR que o em vigor ({teto_atual:.0f}%) -> a "
                      "clausula manda a catraca DESCER para este numero.")
            else:
                print(f"   NAO menor que o em vigor ({teto_atual:.0f}%) -> a "
                      "clausula so deixa descer; o teto atual FICA.")
        elif e_q:
            print(f"   so {e_q['n']} corrida(s) quieta(s): amostra pequena "
                  "demais para propor teto novo; o em vigor fica.")

        print()
        if "--json" in sys.argv:
            print(json.dumps({
                "corridas": corridas, "todas": e_todas,
                "quietas": e_q, "sujas": e_s,
                "teto_em_vigor_pct": teto_atual,
            }, indent=2))
        return 0
    finally:
        srv.parar()


if __name__ == "__main__":
    raise SystemExit(principal())
