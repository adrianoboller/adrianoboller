#!/usr/bin/env python3
"""Quanto custa rodar a catraca de cada mapa -- com a CARGA anotada ao lado.

    python3 bancada/concorrencia/custo-das-catracas.py

Por que este medidor existe
---------------------------
A #252 (2) mandou o custo decidir onde cada catraca ia morar: barata na suite
do `cargo test`, cara no `numeros-do-projeto.py`. A primeira medicao saiu com
a maquina a load 4,9 e foi publicada **sem dizer isso** -- e meia hora depois
a mesma maquina estava a 12, com o provador de guardas e dois `cargo test` de
outras frentes ao lado. Numero de parede sem a carga escrita e teto superior
fingindo de custo, e a diferenca nao aparece no texto: os dois se leem igual.

O que ele mede, e a diferenca entre as duas colunas
---------------------------------------------------
PAREDE: quanto a corrida demorou nesta maquina neste minuto. E o que quem roda
vai sentir, e sobe com o vizinho compilando -- por isso so vale com a carga ao
lado, e mesmo assim como TETO SUPERIOR.

CPU (user+sys do processo filho): o TRABALHO da regua. Nao cresce porque o
vizinho compila, e por isso e o numero que se compara entre dias diferentes.

Publicar so a parede sob carga e palpite com aparencia de medida; publicar so
a cpu esconde o que quem roda vai sentir. Vao os dois, e vai a carga.

O que ele NAO faz: veredito. Quem aprova ou reprova e o `--catraca` de cada
mapa; aqui o codigo de saida do filho e' so mais uma coluna.
"""
import os, resource, subprocess, sys, time

def carga():
    return os.getloadavg()[0]

MAPAS = ("mapa-da-trava.py", "mapa-das-threads.py")
AQUI = os.path.dirname(os.path.abspath(__file__))

for medidor in MAPAS:
    print(f"--- {medidor}")
    for i in (1, 2, 3):
        a = resource.getrusage(resource.RUSAGE_CHILDREN)
        c0 = carga()
        t0 = time.monotonic()
        r = subprocess.run([sys.executable, os.path.join(AQUI, medidor), "--catraca"],
                           capture_output=True, text=True)
        parede = time.monotonic() - t0
        b = resource.getrusage(resource.RUSAGE_CHILDREN)
        cpu = (b.ru_utime - a.ru_utime) + (b.ru_stime - a.ru_stime)
        print(f"   corrida {i}: parede {parede:.3f} s | cpu {cpu:.3f} s | "
              f"load {c0:.2f} -> {carga():.2f} | saida {r.returncode}")
