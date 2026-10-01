# Carregar sob demanda mudou a ordem fábrica/erro e expôs uma corrida

**Estado:** PENDENTE (prova no `qualificacao/qualificar.mjs`, sonda de idioma da Configuração;
promove com o commit da SP000027)

## O que aconteceu

O lote 10 passou o phx-grid (564 KB) para carregamento sob demanda: no celular, o
DOMContentLoaded caiu de 4.432 para 1.297 ms. Com isso a tela Configuração passou a pedir
`/v1/config` antes de a fábrica de idiomas terminar de carregar; quando o pedido falhava, o aviso
de erro saía em português numa tela em inglês.

## O que eu concluí primeiro, e estava errado

Que a otimização era neutra para o resto da tela. Ela só mudou o tempo — e a correção do texto
dependia, sem ninguém ter escrito isso, de a fábrica chegar primeiro.

## O que a medição disse

O aviso agora é desenhado pela chave e redesenhado quando o idioma chega (`idiomas.aoTrocar`); a
sonda força o erro antes da fábrica e confere o idioma do texto.

## A regra

Mudar a ordem de carregamento é mudar comportamento: depois de qualquer «sob demanda», refaça as
checagens que dependem de algo ter chegado antes.
