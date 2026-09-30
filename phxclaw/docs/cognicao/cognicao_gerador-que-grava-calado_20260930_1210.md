# O certificador gravava a evidência calado — e gravava zero linhas

**Estado:** INFRUTÍFERO

**Causa:** o INSERT em `native_verification_runs` ia por `psql -c "..."`, e as aspas do JSON dos detalhes quebravam o comando; o código de retorno não era conferido. O relatório dizia «certificação gravada», o banco tinha 0 linhas.

**Prevenção:** SQL por stdin com `ON_ERROR_STOP=1`, e o certificador sai com código 2 e diz que falhou se a gravação falhar. Gerador que faz menos do que promete tem de dizer que fez menos.

## O que aconteceu

Primeira corrida de `tools/release_certification.py`: 13 portões rodados, 0 linhas de
evidência no banco.

## O que eu concluí primeiro, e estava errado

Que o `roda()` genérico bastava para a gravação — ele serve para portão, onde o rc vira
estado; para efeito colateral, rc ignorado é sucesso inventado.

## A regra

Todo efeito colateral de gerador confere o próprio resultado e o imprime.

## Como está guardado hoje

`tools/release_certification.py`, bloco de gravação, com a contagem de linhas impressa.
