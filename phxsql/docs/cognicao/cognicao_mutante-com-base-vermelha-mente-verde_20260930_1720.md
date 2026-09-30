# Rodada de mutantes com a base vermelha: onze «vermelhos» que não provavam nada

**Estado:** INFRUTÍFERO
**Causa:** o roteiro de mutantes chamava `cargo test -p phxsql-core --lib tls aes` — dois filtros posicionais, que o `cargo` recusa com erro de uso. Todo mutante saía com código ≠ 0 e era contado como «vermelho».
**Prevenção:** o roteiro roda o MESMO comando sem mutante primeiro e para se a base não sair verde (`assert base.returncode==0`), imprimindo o `test result` da base. Hoje em `/tmp/claude-0/mut_aes.py`; a regra vale para todo roteiro de mutante.

## O que aconteceu

Pedido 572, T5 (AES-128-GCM). Onze defeitos repostos, onze «VERMELHO» na
primeira corrida. Parecia a melhor rodada do dia.

## O que eu concluí primeiro, e estava errado

Que os onze testes pegavam os onze defeitos. O relatório só olhava o código
de saída, e código de saída ≠ 0 tem duas causas: teste que falhou, ou comando
que nem rodou. A segunda produz exatamente a tabela que se quer ver.

## O que a medição disse

| corrida | base sem mutante | mutantes «vermelhos» | valem? |
|---|---|---|---|
| 1ª (`--lib tls aes`) | código 1: `Usage: cargo test [OPTIONS] [TESTNAME]` | 11/11 | não |
| 2ª (`--lib -- tls aes::`) | verde, 26 testes | 11/11 | sim |

Deu o mesmo resultado nas duas — e é por isso que a primeira é perigosa: o
erro não aparece no número, aparece só na base.

## A regra

Prova real nos dois sentidos tem um terceiro: **o controle positivo da própria
bateria** — o comando, sem defeito nenhum, tem de sair verde na mesma corrida.
Vermelho sem base verde ao lado não é prova.

## Como está guardado hoje

Só no roteiro desta rodada (`assert` da base). Não há catraca que obrigue os
outros roteiros de mutante a fazer o mesmo.
