# Congelar a filha não basta: a varredura da chave congela a MÃE também

**Estado:** FRUTÍFERO
**Evidência:** `crates/phxsql-server/src/servidor.rs::a_janela_da_varredura_solta_a_vizinha_e_segura_filha_e_mae`

## O que aconteceu

Pedido 422, a terceira irmã: tirar da trava global a varredura
`conferir_chave` do `declarar_fk`, que é O(linhas da filha) com uma busca na
mãe por linha. O molde já existia (`acrescentar_coluna`): travar, congelar,
soltar, trabalhar, retravar, gravar.

## O que eu concluí primeiro, e estava errado

Que bastava congelar a **tabela do pedido**, a filha, como o molde congela a
tabela que reescreve. O raciocínio era «a varredura lê a filha, então é a
filha que não pode mudar».

Errado pela metade que ninguém vê: a varredura também lê a **mãe**, e a
chave ainda não está gravada no esquema. Enquanto a varredura corre, o
`excluir` da mãe não sabe que existe filha apontando para ela — e apaga o pai.
A chave nasce «conferida» sobre uma órfã: o «nunca» da regra primordial,
quebrado pelo próprio conserto de desempenho.

## O que a medição disse

Com um gancho de teste que roda **na janela**, na mesma thread do
`declarar_fk` (a trava está solta, então ele consegue pedir):

| defeito reposto | resultado |
|---|---|
| nenhum | vizinha grava; filha e pai recusam `EmMigracao` — 3/3 verdes |
| sem congelar a mãe | o `excluir` do pai referenciado **passa** na janela; a varredura, que roda depois do gancho, acha a órfã e recusa a chave — vermelho. Numa corrida real o pai sairia **depois** de a varredura passar pela filha, e ninguém recusaria |
| sem congelar a filha | «a filha GRAVOU no meio da varredura» — vermelho |
| varredura com a trava na mão | vermelho |
| sem perguntar à transação viva | a declaração congela a filha de uma transação aberta — vermelho |

E um segundo erro, este de prova: a primeira versão conferia
`congelamento::quantas() == 0` no fim. Deu **2** — o contador é do
**processo**, e os testes vizinhos congelam em paralelo. Trocado por prova
local: a própria tabela volta a gravar.

## A regra

Quando uma operação solta a trava para ler duas tabelas, congele **toda
tabela cuja mudança invalidaria o que ela concluiu** — não só a que o pedido
nomeia.

## Como está guardado hoje

O teste acima, com os três defeitos repostos medidos em 29/09/2026. O
buraco que fica, nomeado no pedido 422: as regravações **condicionais** do
esquema (bloco que não cabe; `marcar_lgpd` pré-v6) continuam sob a trava.
