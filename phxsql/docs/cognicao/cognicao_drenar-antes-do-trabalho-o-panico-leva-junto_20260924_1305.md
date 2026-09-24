# Drenar a lista antes do trabalho: o pânico leva junto o que ainda não foi feito

**Estado:** PENDENTE

## O que aconteceu

Pedido 451, A1 da revisão adversária do DBA
(`docs/propostas/parecer-dba-451-2026-09-24.md`). O `descarregar_sujas_com`
fazia `s.drain().collect()` das tabelas sujas **antes** de sincronizá-las, e só
no fim devolvia à lista as que tinham falhado. A ordem que o group commit
protege — a marca pendente só sai quando toda tabela teve `fsync` — valia
inteira no caminho de erro, porque o erro chega ao fim da função e devolve as
chaves.

O pânico não chega ao fim. Um pânico no meio do laço levava as chaves que ainda
não tinham sincronizado, e o reparo da trava de dados, que começa por este mesmo
fecho, achava as sujas vazias e drenava as marcas pendentes: o bilhete de um
commit cujo dado não passou por `fsync` saía do disco.

## O que eu concluí primeiro, e estava errado

Que chamar o fecho de sempre no passo 1 do reparo era seguro «porque ele já é
seguro contra erro de E/S: a chave volta para as sujas e a marca fica». Era
verdade sobre o **erro** e falso sobre o **pânico**: a garantia morava na
devolução do fim da função, e o pânico pula o fim. Escrevi o reparo em cima de
uma função cuja segurança eu tinha lido no comentário, e não no fluxo.

## O que a medição disse

`servidor::testes_do_panico_sob_a_trava::panico_no_fecho_da_janela_nao_apaga_a_marca_de_quem_nao_foi_ao_disco`,
com o gancho do fecho entrando em pânico ao chegar na tabela `b` e a `b`
continuando sem sincronizar. Com a lista drenada:

```text
a marca do commit SUMIU e a `b` nunca foi ao disco: o panico no meio do fecho
tirou a `b` das sujas, e o reparo drenou as marcas pendentes
  left: []
 right: ["transacao_1790254817841.tx"]
```

Com a cópia e a chave tirada só depois do `fsync` dela, a marca fica, a `b`
continua nas sujas, e o fecho seguinte, com a `b` sincronizando, leva a marca.

## A regra

Não tire item da estrutura compartilhada antes de terminar o trabalho sobre
ele: trabalhe sobre uma cópia e tire só o que terminou, porque a devolução do
fim da função não roda no pânico.

## Como está guardado hoje

- Guarda `fecho-drena-as-sujas-antes-do-fsync` no catálogo, que repõe o
  `drain` com a devolução das que falharam, a forma exata de antes.
- As três guardas antigas do fecho (`fecho-em-paralelo-engole-o-erro`,
  `fecho-em-paralelo-fio-que-nao-sobe`, `fecho-sem-suja-nao-drena-a-marca`)
  mantiveram o trecho, e foram reprovadas de novo contra o código novo.
- **O buraco:** não há varredura por `drain()` seguido de trabalho em outros
  lugares do servidor. O padrão só foi procurado neste fecho.
