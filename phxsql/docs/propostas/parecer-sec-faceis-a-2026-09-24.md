# Parecer SEC: lote «fáceis A» (369, 443, 529) — 24/09/2026

Revisor adversário de segurança, só leitura e provas de leitura. Conferido no
worktree da frente, sem commit, sobre o `49b5426`. Os testes das três passam
pelo `./cargo-da-frente.sh`. As provas estão no scratchpad da sessão, em
`sec-faceis-a/` (`sonda369.py`, `panico443.rs`). O revisor não grava arquivo
de achado; quem escreveu este documento foi o integrador, com o relato do
revisor.

**Veredito geral: LIBERA COM CONDIÇÃO.**

- As três correções consertam o que prometem.
- Nenhuma introduz defeito ativo novo no próprio escopo.
- As três redigem ANALISANDO: reserializam a árvore, não recortam texto.

A condição é abrir, como pedido, os residuais que já existiam antes, da mesma
família. Eles não bloqueiam a entrega das três.

## 369 — `baldes[].registros` na classe Estrutura: LIBERA

`peneirar_baldes` (`direito_coluna.rs`, perto de :645) tira `registros` só
quando a coluna que particiona está em `sem_ler`. Ela roda antes do atalho, e
o retorno cedo com `sem_ler` vazio mantém o comportamento velho byte a byte:
provado, o dono continua vendo os registros. O funil é único
(`aplicar_direito_por_coluna`, chamado em três lugares do `servidor.rs`),
então REST, HTTP e job herdam o conserto.

**Residual medido pelo protocolo.** O cenário: `cidade` negada à `ana`, sem a
marca `dado_pessoal`, e por isso o 358 não recusa a criação. A primeira letra
de cada linha continua saindo por **quatro portas** que o 369 não fecha:

- `paginacao.baldes[].existe` diz quais letras têm dado.
- `esquema.slots` com `primeiro_rowid` dá a conta exata do balde mais alto: 23003 slots → balde 24, «X», 3 linhas.
- `sistabelas.slots` é `PorColuna::Nenhum`, sem filtro nenhum.
- O `rowid` do `varrer` leva ao balde e daí à letra, exata por linha.

É o preço estrutural que o 358 declarou irremovível, e **não** foi
introduzido aqui. O gap real a nomear: a recusa do 358 dispara pela marca
`dado_pessoal`, e não pelo direito por coluna. Coluna negada por direito tem
proteção MENOR que coluna marcada numa tabela `PorLetra`/`PorPeríodo`.

E o `volumes[].periodo` (partição por período sobre coluna `Date` negada)
vaza mês e ano sem passar pelo `peneirar_baldes`, que só peneira `baldes`.

**Pedido proposto:** estender o crivo do direito por coluna a `baldes.existe`,
`slots` e `volumes.periodo`, ou alinhar a recusa do 358 ao direito de coluna.
Teste adverso: `ana`, com uma coluna `Date` negada e partição por período, lê
o rótulo do período em `esquema.volumes[]`.

## 443 — DbLink MySQL, `TETO_DE_COLUNAS` e `TETO_DO_REGISTRO`: LIBERA COM CONDIÇÃO

Os dois vetores citados fecham certo, com prova real nos dois sentidos:

- o `with_capacity(u64::MAX as usize)` de antes dá pânico de `capacity overflow` (replicado);
- os testes novos recusam com `LimiteExcedido`.

Ficam **dois residuais do mesmo módulo e da mesma ameaça** («quem está no meio
do fio em claro»), não tocados. Os dois foram replicados pelo revisor:

- **(a) `mysql.rs`, `texto_lenenc`/`le_lenenc`.** Um lenenc `0xFE` seguido de `u64::MAX`, no nome da coluna (`ler_coluna`) ou numa célula (`ler_linha`), faz `(*i+n).min(p.len())` embrulhar. O `&p[i..fim]` fica com `fim < i` e dá pânico em release: «slice index starts at 9 but ends at 8». O teto de colunas não protege isso, porque o estouro acontece DENTRO do quadro.
- **(b) `pg/mod.rs`, `ler_descricao`/`ler_linha`.** Os dois fazem `Vec::with_capacity(l.i16()? as usize)` com a contagem de campos vinda do par. Um `i16` negativo vira um `usize` gigante e dá pânico de `capacity overflow`. O dialeto Postgres não ganhou teto nenhum.

**Alcance.** Os dois pânicos são desenrolamento (matam a conexão), não aborto
do processo. A exceção: `dblink_sincronizar` e `dblink_ligar` chamam
`c.consultar` com a trava de dados na mão. O pânico desenrola pelo `Drop` da
`TravaMedida` até o `reparar_a_trava`, que repara quando está numa thread de
pedido e **aborta o processo** se a família for «servico».

Não é bloqueio: o defeito já existia e a criação do dblink é do
administrador.

**Pedido proposto:** teto no acumulado por lenenc no `mysql.rs` e teto na
contagem de campos no `pg/mod.rs`. É a mesma pergunta — «quanto este lado
reserva para quem não provou nada?» — e se resolve no mesmo lugar. Teste
adverso: um par falso que devolve nome de coluna com lenenc `u64::MAX`
(MySQL) e um `RowDescription` com `i16` negativo (PG).

## 529 — `por_login` linear, oráculo de tempo: LIBERA

`por_login` (`usuarios.rs`, perto de :1384) varre a lista inteira, sem sair
cedo. A prova por dentro é o contador `comparacoes_de_login_nesta_thread`,
compilado só em teste: dá o mesmo número de comparações para o primeiro, para
o último e para o ausente. O exemplo em release derrubou a diferença ao nível
do ruído.

**Residual honesto:** `u.login == login` é igualdade de `String`, que para no
comprimento e depois compara byte a byte. Um login mais longo, ou de prefixo
comum, ainda muda o custo de CADA comparação, na casa de sub-µs. Isso ficou
fora do escopo de propósito (o exemplo fixa a largura do login e diz por quê),
e é irrelevante diante do PBKDF2 e do ruído medido. Não é defeito: fica como
limitação documentada em `SEGURANCA.md` §26.7.

## Destino

As três correções entram. Os dois pedidos propostos (369-residual e
443-residual) passam pelo juiz PhxJev antes de entrar na conta, pela regra da
casa.
