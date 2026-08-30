# Document cache_paginas in MANUAL and PENDENCIAS
# 29/08 00:35

import pathlib
p = pathlib.Path("MANUAL.txt")
s = p.read_text()
alvo = '''    CPU. O "cpu_percentual" nao e uma cota do sistema operacional'''
novo = '''    CACHE DE PAGINAS. O "cache_paginas" diz quantas paginas do .ndx cada
    tabela aberta guarda em RAM. Cada pagina tem 4 KiB, entao 2.048 dao 8 MiB
    por tabela aberta -- e o servidor abre e fecha a tabela a cada operacao,
    entao o teto vale enquanto a operacao dura.

    E o segundo campo que mais muda a velocidade da gravacao, depois da
    durabilidade. Toda insercao DESCE a arvore do indice -- raiz, no interno,
    folha --, e sem cache isso e uma leitura de pagina inteira mais um CRC-32
    de pagina inteira em cada nivel. A raiz e a MESMA pagina em todas as
    insercoes da carga. Medido, com dois indices:

        sem cache ................... 22.516 linhas/s   44,4 us por linha
        com 2.048 paginas ........... 53.988 linhas/s   18,5 us por linha  2,40x

    2.048 e o joelho da curva: dobrar para 4.096 compra 0,8 us por linha e
    custa mais 8 MiB por tabela. A varredura inteira esta em
    docs/DESEMPENHO.md.

    O cache e de LEITURA. Toda gravacao atravessa para o arquivo na hora --
    segurar pagina suja em RAM daria mais e trocaria uma garantia por
    velocidade sem avisar: hoje so uma queda da MAQUINA atrasa o .ndx em
    relacao ao .reg, e nao uma queda do processo.

    CPU. O "cpu_percentual" nao e uma cota do sistema operacional'''
assert s.count(alvo) == 1
s = s.replace(alvo, novo, 1)
p.write_text(s)
print("ok")
