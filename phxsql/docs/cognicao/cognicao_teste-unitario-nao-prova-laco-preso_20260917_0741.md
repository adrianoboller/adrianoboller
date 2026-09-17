# Teste unitário não prova laço preso — dois laços provam

**17/09/2026, 07:41 UTC.** Frente do pedido 292, parte 2 (papel B).

## 1. O que aconteceu

No bidirecional, o evento do outro lado que viola um índice único **secundário**
(primária `porId`, secundário `porEmail`) era recusado por `Table::inserir`, e a
recusa subia pelo `?` de `aplicar_lote_bidi`
(`crates/phxsql-server/src/servidor.rs:4490`). Com isso o `desde = lote.ate`
(`servidor.rs:4563`) **nunca executava**: o mesmo lote voltava na rodada
seguinte, e na seguinte, para sempre. Não é uma linha perdida — é o par de
servidores parado, e parado em silêncio: o único sinal era uma linha de erro no
diário do processo, igual à de qualquer falha de rede.

O conserto é o mesmo desenho do `colisao_de_criacao` (pedido 229(a)) e do
`carimbos_do_futuro` (A9): a recusa vira **número** (`MapaDeToques::
recusas_por_unicidade`, publicado em `replicacao_estado.recusas_por_unicidade`)
e **grito** no log, e o laço segue.

## 2. O que eu concluí primeiro, e estava errado

**Duas coisas.**

*Primeira:* que o lugar do conserto era o `?` que o parecer nomeia —
`aplicar_lote_bidi`, onde o erro sobe. Errado: o **irmão** desta situação não é
quem tem o `?`, é quem já conta e grita a mesma família de estrago —
`aplicar_por_chave`, onde a colisão de `Sequence` é contada. Consertar no laço
teria criado um **segundo** padrão de contagem a dois níveis de distância do
primeiro, e teria perdido a chave canônica que a linha do log precisa nomear.
Quem chama as mesmas funções na mesma ordem é o vizinho de dentro, não o dono
do `?`.

*Segunda, e esta morreu medida:* concluí que engolir o erro deixaria
`evento_forcado` armado (o `forcar_proximo_evento` acontece **antes** da
escrita, e o `take()` só acontece na gravação do diário — `table.rs:3232` e
`3686`), e que o próximo evento entraria vestindo o carimbo do evento recusado.
Fui procurar o caminho: **todo** caminho de escrita do `aplicar_por_chave` arma
o carimbo na linha imediatamente anterior à escrita, e o handle da tabela morre
no fim do lote. O vazamento não existe, e um teste para ele **passaria com o
defeito reposto** — que é pior que teste faltando. Não entrou nada. Ficou o
motivo escrito aqui, para o próximo que olhar a mesma linha e desconfiar do
mesmo.

## 3. O que a medição disse

Com o defeito reposto (a recusa voltando a subir pelo `?`), no teste de soquete
com dois servidores:

* **20 repetições** da mesma linha `[SP000020] chave duplicada: indice unico
  porEmail ja tem essa chave` em **20,2 s** — uma por rodada do laço, e nenhuma
  delas dizendo que aquilo era um lote **repetido**;
* a posição consumida ficou em **0** (com o conserto: **1**, e **2** depois da
  linha seguinte);
* a linha seguinte do diário do parceiro — id 3, e-mail que não colide com nada
  — **nunca chegou**. Esse é o dano, e é o que o teste unitário não vê.

E o número que separa os dois testes: o unitário
(`chave_duplicada_no_unico_secundario_e_contada_e_o_laco_segue`) cai em
**0,10 s** com o defeito reposto, mas o que ele afirma é apenas «`Ok` em vez de
`Err`». Quem afirma a doença — *a próxima linha não chega* — precisa dos dois
laços rodando de verdade: **19,1 s verde / 26,0 s vermelho** no executor de
guardas.

Os testes do comportamento **velho** ficaram verdes com o defeito reposto
(2 de 3 unitários e o `seguem` da guarda). É o que prova que eles travam o
contrato antigo e não o conserto novo.

## 4. A regra

**Erro que o laço não sabe desfazer vira número e grito, nunca parada — e a
prova disso não é o veredito da função, é a LINHA SEGUINTE chegar.** Teste
unitário mede `Ok` contra `Err`; laço preso só aparece com os dois lados no ar.

## 5. Como está guardado hoje

* Unidade: `crates/phxsql-server/src/servidor.rs`, módulo
  `testes_da_recusa_por_unicidade` (3 testes, um deles o do comportamento
  velho, um deles o do erro que **continua** parando a rodada).
* Laço inteiro, pelo soquete:
  `crates/phxsql-server/tests/laco-do-unico-secundario.rs` (2 testes).
* Guarda de mutação: `laco-preso-no-unico-secundario` no
  `bancada/guardas/catalogo.py` — **PROVADA**, 1/1 caíram, `prazo: 120` com o
  motivo medido no comentário (a lição do `trava-atras-da-rede`: prazo menor
  que o vermelho mata a rodada antes de o teste conseguir reprovar).
* Documentação: `docs/REPLICACAO.md`, seção nova «O índice único SECUNDÁRIO».

**Onde o buraco ficou:** o irmão de forma — `alcancar_tabela` /
`aplicar_lote_da_replica`, a réplica de mão única — tem exatamente o mesmo
desenho (posição só anda depois do lote inteiro) e **continua parando de
propósito**, porque lá a garantia é de fidelidade por rowid e o motor imita a
thread SQL do MySQL(R). Não foi tocado, e o aviso de arranque já diz a
consequência (`servidor.rs:2422`: *«replica sem somente_leitura… os rowids
divergem e a replicacao para»*). Decidir se aquilo também vira número é outro
pedido, não este.
