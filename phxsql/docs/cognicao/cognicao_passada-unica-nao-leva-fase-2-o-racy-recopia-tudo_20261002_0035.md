# A cópia em uma passada NÃO leva a fase 2: sem janela entre as fases, o «racily clean» recopia tudo

**Estado:** PENDENTE

## 1. O que aconteceu

Pedido 513, passo 2 (02/10/2026). O backup passou a rodar em duas fases —
`copiar_fase_1` sem trava e `acertar_fase_2` sem escritor
(`crates/phxsql-store/src/backup.rs`). Para a CLI e para os testes, que já
excluem o escritor por fora, mantive `executar` como «fase 1 + fase 2 em
seguida», pela lei de função não se duplicar: um motor só.

A prova por `strace` que já existia
(`crates/phxsql-store/tests/fsync-no-descritor-que-escreveu.rs::a_copia_sincroniza_no_descritor_que_a_escreveu`)
caiu no provador de guardas, na árvore **limpa**: **3 de 3** arquivos com
**2 aberturas com sucesso, esperava UMA**.

## 2. O que eu concluí primeiro, e estava errado

Que `executar` = fase 1 + fase 2 era o jeito honesto de ter um motor só, e que
a fase 2 sem janela custaria só o `stat` de cada arquivo (~2 µs). O custo que
não vi: a terceira rede de `precisa_recopiar` — o `mtime` a menos de 2 s do
`stat` da fase 1 recopia **sempre** — é correta no servidor e **vazia** na
passada única: todo arquivo recém-escrito do teste era «racy», e a fase 2
recopiava a árvore inteira, abrindo cada cópia uma segunda vez. O `fsync`
continuava no descritor que escreveu por último (a garantia do 552 não
quebrou), mas a prova conta aberturas, e contou duas.

## 3. O que a medição disse

- Suíte do armazém verde (`cargo test -p phxsql-store`, 288 + integrações) —
  **e mesmo assim** o provador acusou: a prova do `strace` só roda inteira na
  árvore do provador, e foi lá que apareceu.
- Com a fase 2 fora da passada única: 5/5 testes do `strace` verdes, e as 11
  guardas do backup provadas (9 PROVADAS, 1 REDUNDANTE declarada, 1 que
  ESTRAGOU por `seguem` errado e foi corrigida: a prova da manutenção cai
  pelo defeito reposto, não por estrago).
- Pelo soquete, o `inserir` durante a fase 1 do backup em arvore: **~1 ms**
  contra **1.500 ms** com a trava reposta na fase 1
  (`servidor::testes_do_retrato_do_backup::a_escrita_nao_espera_a_fase_1_e_o_retrato_e_o_do_fim`).
- Bancada `bancada/backup/retrato-com-escritor.py` (561 MiB, 20 tabelas, 3
  voltas): ver `bancada/backup/resultados.json`, rótulos `duas_passadas` e
  `passo_1`, com a data.

## 4. A regra

**Rede que existe para uma janela só se aplica onde há janela.** Antes de
reaproveitar um motor de duas fases numa passada única, pergunte o que cada
rede dele faz quando o intervalo entre as fases é zero — e rode a prova que
conta syscalls, porque a suíte funcional não vê uma abertura a mais.

## 5. Como está guardado hoje

- `executar`/`executar_zip` são só a fase 1, com o motivo escrito no doc de
  `executar` (`backup.rs`); o servidor é quem chama `acertar_fase_2`.
- A guarda `backup-fase-2-nao-acerta` prova a fase 2 pelo armazém;
  `backup-sem-fase-2` pelo servidor; a do `strace` segue cobrindo a passada
  única.
- O buraco que fica: o zip sem espaço para a árvore temporária cai na
  passada única sob a trava **dizendo** (`modo: retrato_inteiro`), e a
  bancada não mede esse caminho — ele é o passo 1 de sempre.
