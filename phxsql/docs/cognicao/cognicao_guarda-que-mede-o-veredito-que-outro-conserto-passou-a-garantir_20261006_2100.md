# Guarda que mede o veredito que um conserto POSTERIOR passou a garantir não pega o defeito

**Estado:** PENDENTE

## 1. O que aconteceu

O provador (263) deu NAO PEGOU em `fechar-do-embutido-nao-sincroniza`
(`testes::fechar_sem_sincronizar_e_o_proximo_processo_abre`) e em
`elo-do-empilhar-pelo-disco`
(`servidor::testes_transacoes::integridade_na_transacao::o_elo_da_cascata_nao_desfaz_o_que_a_lista_escreveu_na_filha`).
Os dois passavam com o defeito reposto.

## 2. O que eu concluí primeiro, e estava errado

A hipótese natural era «o defeito sumiu do código» (aposentar a entrada). Estava
errada: o defeito **continua no código** e continua fazendo dano. O que mudou é
que um conserto posterior mascara o veredito final: o 563 faz o
`phx_base_abrir` reconstruir o índice marcado, e o 537 faz o COMMIT refazer o
elo. Aposentar a guarda teria deixado o defeito sem teste nenhum.

## 3. O que a medição disse

- Fechar reposto como só-o-`Drop`: byte 52 do `.ndx` = **1** no disco, e a sonda do
  processo seguinte vê `indice_precisa_reconstruir` = **false** e passa.
- Empilhar com `planejar_cascata_da_alteracao(.., None)`: depois do COMMIT o
  resultado sai certo; **dentro** da transação a filha lê `(10, 6, false)` onde
  devia ler `(11, 6, false)`.

## 4. A regra

Quando um defeito ganha um conserto posterior que cobre o veredito final, o
teste da guarda original tem de medir o **dano residual** (o estado em disco,
a leitura intermediária), não o resultado que o conserto novo já garante.

## 5. Como está guardado hoje

Os dois testes ganharam a asserção do dano residual (`testes.rs`, byte 52 = 0
depois do fechar; `servidor.rs`, leitura dentro da transação). Evidência de
RED/GREEN no relatório da frente e em `docs/TESTES.md` §9.10.
