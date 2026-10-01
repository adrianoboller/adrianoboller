# Conta relativa a um contador do processo mede o vizinho que SOLTA — pedido 599 (b)

**Estado:** PENDENTE

Evidência candidata, para quem for promover:
`crates/phxsql-store/src/congelamento.rs::o_vizinho_que_solta_no_meio_nao_derruba_a_conta`;
guardas `contador-do-congelamento-relativo` e
`drop-do-congelamento-esquece-o-contador`, PROVADAS em 01/10/2026.

## O que aconteceu

`congelamento::testes::o_contador_volta_ao_que_era` caiu uma vez no
`./portoes.sh` e passou na corrida seguinte sem mudança. Ele lia
`antes = quantas()` e exigia `quantas() >= antes + 2` com duas tabelas
congeladas. `quantas()` é um `static` do processo, e os vizinhos do mesmo
módulo (`congelada_recusa_e_nomeia_a_tabela` e
`duas_reescritas_da_mesma_tabela_nao_comecam_juntas`) congelam e soltam em
paralelo.

## O que eu concluí primeiro, e estava errado

O próprio comentário do teste dizia «a conta é relativa de propósito: um zero
cravado mediria o vizinho». A conta relativa protege do vizinho que
**chega** e não protege do vizinho que **sai**. Se ele estava congelado na
leitura de `antes` e solta antes da conferência, o contador fica em
`antes + 1`. Era a lição do 261 com o sinal trocado: lá o vizinho que morre
comia o `+1`, aqui o vizinho que solta come.

## O que a medição disse

A montagem determinística segura um vizinho congelado, lê `antes`, congela as
duas e solta o vizinho no meio. Com a conta relativa reposta, cai **toda
vez**: a guarda está PROVADA, 1/1. E o achado que pagou o conserto: a conta
velha **não pegava** o defeito que dizia guardar. Com um `Drop` que tira do
registro e esquece o `fetch_sub`, o `conferir` continua devolvendo `Ok`, só
que pelo caminho do mutex, e o teste velho passava. O novo cai (guarda
`drop-do-congelamento-esquece-o-contador`, 2/2).

## A regra

**Sobre um contador do processo, cobre só o que o vizinho não estraga:** o
piso absoluto do que é seu e um invariante lido num retrato só. Aqui o
invariante é o contador igual ao tamanho do registro, lidos sob a mesma
trava, e nunca uma diferença entre duas leituras.

## Como está guardado hoje

`conferir_que_o_contador_volta(d, vizinho_age)` é o corpo dos dois testes. As
duas guardas de cima estão no catálogo. Não há outro uso de
`quantas() >= antes` na árvore. **Ainda há um buraco:** o comentário de
`servidor.rs` (~60491) já avisa que contar `congelamento::quantas()` ali
flocaria. Quem escrever o próximo teste sobre esse `static` precisa achar a
regra por conta própria.
