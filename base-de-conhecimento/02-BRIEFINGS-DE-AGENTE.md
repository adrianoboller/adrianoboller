# Briefings de agente

O ativo mais reaproveitavel desta base. Cada briefing leva a
REGRA junto do pedido -- e por isso que o agente nao repete o erro
que o projeto ja pagou. Copie o formato, troque o assunto.

53 despachos.

---

## 1. Analisar insert do InnoDB e Aria  ·  29/08 04:39

```
Você é o analista de fontes de motores concorrentes do projeto **PhxSql**. Trabalhe e escreva **em português**, com identificadores e nomes de arquivo **sem acento** (o texto corrido pode ter acento).

# Sua missão, e só ela

Ler os fontes do **MySQL(R) InnoDB** e do **MariaDB(R) (InnoDB e Aria)** e responder **uma** pergunta com evidência de código: **o que o caminho de INSERÇÃO deles faz que o nosso não faz, e por que eles são ~2,3× mais rápidos que nós inserindo.**

Você NÃO escreve código do PhxSql. Você produz um documento. Não edite nada em `/home/user/adrianoboller/phxsql/` exceto o arquivo de saída indicado no fim.

# Os fontes, já baixados

- **MySQL(R) 8.0**: `/tmp/claude-0/-home-user-adrianoboller/34595649-0af6-575a-8f79-80dbe8cb7a5d/scratchpad/fontes/mysql/`
  - `storage/innobase/` (InnoDB inteiro), `sql/`
  - Comece por `storage/innobase/row/row0ins.cc` (3.755 linhas), `btr/btr0cur.cc`, `buf/buf0buf.cc`, `log/log0*.cc`, `ibuf/ibuf0ibuf.cc`, `page/page0*.cc`, `mtr/mtr0mtr.cc`
- **MariaDB(R) 11.4**: `.../fontes/mariadb/`
  - `storage/innobase/`, **`storage/maria/`** (Aria) e `storage/myisam/`
  - **Aria interessa muito**: é o motor de arquivos separados (`.MAD`/`.MAI`), que é o modelo do PhxSql. Veja `storage/maria/ma_write.c`, `ma_blockrec.c`, `ma_pagecache.c`, `ma_check.c`

Use `git -C <dir> sparse-checkout add <caminho>` se precisar de mais alguma pasta (o clone é `--filter=blob:none --sparse`).

# O nosso motor, para comparar

Código em `/home/user/adrianoboller/phxsql/crates/`. Leia, nesta ordem:

1. `docs/DESEMPENHO.md` — **leia inteiro antes de tudo**. Ele traz cada medição já feita e, mais importante, **quatro diagnósticos plausíveis que a medição derrubou**. Não repita nenhum deles.
2. `docs/FORMATO.md` — o formato em disco, byte a byte.
3. `crates/phxsql-store/src/table.rs` (`inserir`), `reg.rs`, `ndx.rs`, `log.rs`.

## O que já está medido do nosso lado (não remeça, use)

- Inserção com 2 índices: **16,4 µs por linha**, repartidos em `.reg`+`.log` 4,9 (30%), 1º índice 5,5 (33%), conferir chave única 0,9 (6%), 2º índice 5,2 (32%).
- Por linha o `.ndx` toca 11,42 páginas servidas do cache, 0,00 lidas do arquivo, **2,06 gravadas**. Só leitura de arquivo e gravação pagam CRC: **4,8 µs, 29% do total**.
- CRC-32 de uma página de 4 KiB: 2,34 µs. Copiar uma página: 0,07 µs. Um `lseek`: 0,10 µs.
- Bancada de 10 milhões: **PhxSql 265,2 s contra MySQL(R) 113,5 s (2,34×)**. Nós ganhamos em varrer (2,22 contra 23,75), atualizar (1,26 contra 6,40) e buscar (1,21 contra 2,62); perdemos em inserir e excluir.
- A nossa taxa **cai com o tamanho** (54.180 → 37.712 linhas/s de 1M a 10M); a do MySQL(R) é plana (90.621 → 88.109). **Num processo só a nossa não cai** (16,0 µs com 200 mil, 16,4 com 6 milhões) — a bancada carrega em lotes de 50.000 abrindo e fechando a tabela, e sobraram ~6,6 µs por linha sem explicação depois que consertei a abertura.

## As regras do projeto que limitam o que pode ser copiado

- **Zero dependências externas**: só a `std` do Rust. Nada de crate nova.
- **A ordem de digitação é sagrada**: o `.reg` grava na ordem em que a linha foi digitada e **nunca reaproveita slot excluído**. Endereçamento é O(1): `offset = data_offset + (rowid−1) × slot_size`. Qualquer ideia que quebre isso é inaceitável — diga isso explicitamente se encontrar uma.
- Não temos transação, nem MVCC, nem rollback. Não afirme que temos.
- Ao citar outros bancos, use sempre **MySQL(R)**, **MariaDB(R)**, **InnoDB**, **Aria**.

# O que eu quero que você descubra, com citação de arquivo:linha

1. **O caminho de uma inserção no InnoDB, do `handler::write_row` até a página gravada.** Quantas páginas ele toca? Quantas ele *escreve*? Onde ele adia trabalho?
2. **Checksum**: o InnoDB confere/gera checksum de página em qual momento e com qual algoritmo? Ele faz isso **por inserção** ou só quando a página deixa o buffer pool? (Nós pagamos 4,8 µs de CRC por linha — 29%. Se eles pagam só no flush, esta é a diferença mais importante do documento.) Procure `buf_flush_*`, `buf_calc_page_*`, `innodb_checksum_algorithm`, `crc32`.
3. **Buffer pool contra o nosso cache de páginas**: eles seguram página **suja** em RAM e escrevem depois; nós escrevemos através (write-through) e nunca seguramos suja. Quanto isso vale, e o que se perde em garantia? Ache onde o InnoDB decide o *flush*.
4. **Change buffer / insert buffer** (`ibuf0ibuf.cc`): o InnoDB adia a manutenção de **índice secundário não único**. Como exatamente? O que ele grava no lugar? Como e quando ele funde? **Isto pode ser o análogo certo do que a gente tentou e mediu como prejuízo** — leia `docs/DESEMPENHO.md` §4.4 antes de escrever sobre isso.
5. **Aria (`storage/maria/`)**, que é o modelo mais próximo do nosso: como ele escreve uma linha e mantém o índice? `ma_pagecache.c` tem página suja? Ele tem checksum por página? O `.MAI` é B+tree — ele faz *bulk insert* / `ma_disable_indexes`?
6. **Agrupamento de escrita**: `mtr_t` (mini-transaction), redo log, `log_buffer`, group commit. **Cuidado**: já está medido e escrito no nosso `DESEMPENHO.md` §3 que WAL/group commit atacam o gargalo do InnoDB (o `fsync`) e **não** o nosso — o nosso é 95% CPU com 0,0 MiB lidos. Só mencione se achar algo que valha para um motor que já é *append-only* e já sincroniza uma vez por carga.
7. **O que explica a taxa deles ser PLANA com o tamanho** e a nossa cair. É buffer pool? É a altura da árvore? É o change buffer? Ache a evidência.

# Método, que é a regra mais forte do projeto

**Diagnóstico plausível não é diagnóstico medido.** Este projeto já errou quatro vezes por escrever a explicação bonita antes de medir — está tudo em `docs/DESEMPENHO.md`. Então:

- Toda afirmação sobre o código deles precisa de **arquivo:linha**.
- Toda afirmação sobre custo precisa dizer **se é medida ou inferida**. Se for inferida, escreva «inferido, não medido» na própria frase.
- Se uma ideia deles for inaplicável às nossas regras, **diga por quê** em vez de a omitir.
- Ordene o que achar por **ganho estimado ÷ custo de implementar**, e diga qual você atacaria primeiro.

# A saída

Escreva **um único arquivo**: `/home/user/adrianoboller/phxsql/docs/CONCORRENTES.md`

Estrutura sugerida:
1. O que foi lido (versões, caminhos, commits)
2. O caminho de inserção do InnoDB, passo a passo, com citações
3. O caminho do Aria, e por que ele é o parente próximo
4. **Tabela comparativa: nós × InnoDB × Aria**, linha por decisão de projeto (checksum, cache, índice secundário, ordem física, durabilidade)
5. **O que estamos errando**, em ordem de valor, cada item com: o que eles fazem, arquivo:linha, o que custaria em nós, o que quebraria das nossas regras
6. **O que NÃO devemos copiar**, e por quê (inclua o que bate na ordem de digitação e na regra zero-dependência)
7. As perguntas que a leitura levantou e só a medição responde

Não use `**ACID compliant**` nem «replicação nativa» descrevendo o PhxSql: nenhum dos dois é verdade hoje.

Ao terminar, responda com um resumo de no máximo 25 linhas: os três achados de maior valor, cada um com arquivo:linha e o ganho estimado, e o que você atacaria primeiro.
```

---

## 2. Gaps do motor e proteção de dados  ·  29/08 04:40

```
Você é o **agente do motor** do projeto **PhxSql**, um motor de dados em Rust sem nenhuma dependência externa. Você tem um worktree git só seu — trabalhe nele.

**Leia `/home/user/adrianoboller/CLAUDE.md` inteiro antes de qualquer coisa.** Ele traz as regras que não se quebram e as lições que já custaram caro. Depois leia `phxsql/docs/PENDENCIAS.md` (os 129 pedidos e o estado de cada um) e `phxsql/docs/DESEMPENHO.md`.

# Como você trabalha

Você deve **lançar 5 subagentes** (ferramenta Agent, `subagent_type: general-purpose`), um por item abaixo, e integrar o trabalho deles. Lance os 5 na mesma mensagem para rodarem em paralelo. Cada subagente recebe **um item e um conjunto de arquivos disjunto** — passe isso explicitamente para eles, porque edições concorrentes no mesmo arquivo se destroem.

## Os 5 itens (todos de `docs/PENDENCIAS.md`)

1. **#101 — cifrar `.log`, `.trash` e `.reason`.** O bloqueio registrado é «não temos cifra de bloco». Resolva escrevendo **ChaCha20 + Poly1305 (RFC 8439)** em `crates/phxsql-core/src/cifra.rs`, **conferido contra os vetores oficiais do RFC** — esta é regra dura do projeto: criptografia se confere contra vetor oficial, nada de «parece certo». Depois ligue nos três arquivos, com a chave derivada por PBKDF2 (já existe em `phxsql-core`). **Arquivos: `crates/phxsql-core/src/cifra.rs` (novo) e o `lib.rs` do core.** Não mexa em `log.rs`/`lixeira.rs` ainda — entregue a primitiva e o desenho da integração.

2. **#101 — compactar arquivo append-only.** O bloqueio registrado é «compactar append-only exige rotacionar e reescrever». Meça o problema antes de resolver: **quanto os três arquivos ocupam de verdade** numa tabela de 1 milhão de linhas, e quanto uma compactação por volume fechado economizaria. Se o ganho não pagar, **diga isso com o número** — é o resultado certo tantas vezes quanto o outro. **Arquivos: um `examples/` novo em `phxsql-store` e um relatório.**

3. **#125 — marcar coluna como dado pessoal (LGPD/GDPR).** Uma marca por coluna no esquema, mais uma operação de protocolo que audita onde elas estão. O cadastro de colunas já tem `caption`, `descricao` e `mascara`: é mais um campo. **Atenção: isto muda o formato do esquema (PSCH)** — leia como `softdeleted` e `rownum` entraram, atualize `docs/FORMATO.md` no mesmo commit, e garanta que um esquema escrito antes desta versão continua abrindo. **Arquivos: `crates/phxsql-core/src/schema.rs`, `crates/phxsql-core/src/types.rs`, e um teste novo.** Não mexa na interface web.

4. **#86 — cliente PostgreSQL(R) para o DbLink.** Os tijolos já existem: SCRAM-SHA-256 usa SHA-256, HMAC e PBKDF2, que o projeto já escreveu. Faça o handshake e o protocolo de fio mínimo (mensagem de startup, autenticação SCRAM, `Query` simples, `RowDescription`/`DataRow`). **Arquivos: `crates/phxsql-server/src/pg/` (novo diretório).** Conferir o SCRAM contra o vetor do RFC 7677.

5. **#6 — servidor MCP.** O protocolo do PhxSql já é JSON por linha; falta a tradução de vocabulário para MCP. Desenhe e implemente o esqueleto: `initialize`, `tools/list`, `tools/call`, mapeando as operações que já existem. **Arquivos: `crates/phxsql-server/src/mcp.rs` (novo).**

# Regras que valem para você e para todos os seus subagentes

- **Zero dependências externas.** Só a `std`. Se algo parecer exigir uma crate, **não acrescente** — escreva aqui ou diga que não dá.
- Código, comentários e documentação **em português**; identificadores e comentários **sem acento**.
- Comentário explica **por que**, não o que.
- **Medir a premissa do item vem antes de implementar o item.** Este projeto já derrubou quatro diagnósticos plausíveis por medi-los — estão em `docs/DESEMPENHO.md`. Se a premissa de um item não se sustentar, **o resultado certo é o número e a recusa**, não a implementação.
- **Guarda nova entra pedida, não imposta**: nada pode quebrar cliente ou arquivo escrito antes desta versão. O teste que mais importa numa mudança dessas é o do comportamento **velho**.
- Antes de commitar: `cargo fmt --all`, `cargo clippy --workspace --all-targets` (**tem de dar zero avisos**) e `cargo test --workspace`.
- Mexeu no formato em disco? Atualize `docs/FORMATO.md` no mesmo commit.
- **NÃO toque** em `CHANGELOG.md`, `docs/PENDENCIAS.md` nem em `docs/dossie/` — eu consolido isso. Se seus subagentes tocarem, vai dar conflito.
- Mensagem de commit conta a **decisão e o motivo**, não a lista de arquivos. Sem identificador de modelo em nada que vá para o repositório.
- Commite no seu worktree, **não empurre e não abra PR**.

Cada subagente deve terminar rodando `cargo clippy` e `cargo test` e reportar o resultado real — se falhar, dizer que falhou, com a saída.

Ao terminar, responda com no máximo 30 linhas: item por item, o que ficou pronto, o que foi medido e recusado (com o número), o que ficou faltando e por quê, e o resultado de `cargo test --workspace` e `cargo clippy`.
```

---

## 3. Gaps de serviço e interface  ·  29/08 04:41

```
Você é o **agente de serviço e interface** do projeto **PhxSql**, um motor de dados em Rust sem nenhuma dependência externa, com uma interface web embutida. Você tem um worktree git só seu — trabalhe nele.

**Leia `/home/user/adrianoboller/CLAUDE.md` inteiro antes de qualquer coisa.** Ele traz as regras que não se quebram e as lições que já custaram caro — em especial as três sobre interface, que valem muito para você. Depois leia `phxsql/docs/PENDENCIAS.md` (os 129 pedidos e o estado de cada um).

# Como você trabalha

Você deve **lançar 5 subagentes** (ferramenta Agent, `subagent_type: general-purpose`), um por item abaixo, e integrar o trabalho deles. Lance os 5 na mesma mensagem para rodarem em paralelo.

**O risco número um do seu grupo:** `crates/phxsql-server/ui/index.html` tem mais de sete mil linhas e **quatro dos seus itens querem tocá-lo**. Edições concorrentes nele se destroem. Então: mande cada subagente entregar o pedaço de HTML/CSS/JS **num arquivo de fragmento separado** (por exemplo `/tmp/.../fragmentos/<item>.html`) com a instrução exata de onde encaixar, e **faça você mesmo, em série, a costura no `index.html`**. O mesmo vale para `crates/phxsql-server/src/servidor.rs`, que também é enorme e compartilhado.

## Os 5 itens (todos de `docs/PENDENCIAS.md`)

1. **#51 — jobs de execução.** É o mais barato dos três («triggers, stored procedures, jobs»), porque **o agendador do backup já é o desenho**: veja `subir_backup_agendado` e `hora_de_rodar` em `crates/phxsql-server/src/servidor.rs` e `config.rs`. Um job é um nome, uma agenda e uma operação do protocolo já existente, com registro do que rodou e do que falhou. **Arquivos: `crates/phxsql-server/src/jobs.rs` (novo) + `config.rs` + fragmento de tela.**

2. **#40 — parar e subir o serviço pela interface, trocando a porta.** O impedimento registrado é «o `accept` bloqueia; exige mexer no laço». Resolva de verdade: um `accept` com tempo limite ou um soquete de despertar, para o laço poder ser mandado parar. **Cuidado**: derrubar o serviço pela tela é a operação mais fácil de transformar em tiro no pé — pense em quem fica sem resposta e em como a pessoa volta se errar a porta nova. **Arquivos: o laço de `accept` em `servidor.rs` (entregue como patch mínimo e bem delimitado) + fragmento de tela.**

3. **#127 — diagrama ER e editor de modelo.** As chaves estrangeiras **já estão declaradas e já vêm no `esquema`** — falta o desenho. É SVG, que é do que o dossiê inteiro é feito. Comece só pelo **diagrama** (o editor visual é outra rodada, e diga isso). **Arquivos: fragmento de tela + um módulo JS.** Regras de estilo em `phxsql/marca/LEIA-ME.md`.

4. **#125 — a tela que audita dado pessoal (LGPD/GDPR).** Outro agente está fazendo a **marca no esquema**; você faz **só a tela** que lista onde as marcas estão, por base e por tabela. Assuma que o esquema devolve, por coluna, um campo booleano `pessoal`. **Arquivos: fragmento de tela.**

5. **#83 e #7 — o começo da camada SQL.** Os dois estão parciais/planejados esperando por ela, e `docs/SQL.md` **já traz o desenho** — leia antes de escrever qualquer linha. Entregue o **analisador léxico e sintático de um `SELECT` simples** (`SELECT colunas FROM tabela [WHERE coluna op valor] [ORDER BY] [LIMIT]`), traduzindo para as operações que já existem no protocolo. Sem planejador, sem junção, sem subconsulta. **Arquivos: `crates/phxsql-sql/` (crate nova no workspace).** `docs/SQL.md` diz o que **não** tem substrato — respeite.

# Regras que valem para você e para todos os seus subagentes

- **Zero dependências externas.** Só a `std`. Nada de crate de fora, nada de CDN na interface.
- Código, comentários e documentação **em português**; identificadores e comentários **sem acento** (o texto que aparece na tela pode ter).
- **Cores da ação, na tela**: verde inclui, amarelo altera, rosa marca, vermelho exclui de vez, azul consulta — **sempre contorno, nunca fundo cheio**; o preenchimento só no `hover`.
- **O CSS global morde todo componente novo.** `input{width:100%}` e `label{text-transform:uppercase}` já produziram um rádio do tamanho da célula e «BLUMENAU» no lugar de «Blumenau» — que é uma **mentira sobre o dado**. Nenhum dos dois aparece lendo o código.
- **Interface só se prova exercitando.** Componente novo se **abre no navegador e se olha**. O Chromium está em `/opt/pw-browsers/chromium` e o Playwright em `/opt/node22/lib/node_modules/playwright` (importe pelo caminho absoluto: `import { chromium } from '/opt/node22/lib/node_modules/playwright/index.mjs'`). Suba o servidor, abra a tela, tire a captura, **olhe**. Um item de tela que não foi aberto no navegador não está pronto.
- **Coluna/peça nova no fim de uma lista quebra quem filtra pela primeira**: procure quem usa `find(...)` onde devia usar `filter(...)`.
- Antes de commitar: `cargo fmt --all`, `cargo clippy --workspace --all-targets` (**tem de dar zero avisos**) e `cargo test --workspace`.
- **NÃO toque** em `CHANGELOG.md`, `docs/PENDENCIAS.md` nem em `docs/dossie/` — eu consolido isso.
- Mensagem de commit conta a **decisão e o motivo**, não a lista de arquivos. Sem identificador de modelo em nada que vá para o repositório.
- Commite no seu worktree, **não empurre e não abra PR**.

Ao terminar, responda com no máximo 30 linhas: item por item, o que ficou pronto, **quais telas você realmente abriu no navegador e o que viu**, o que ficou faltando e por quê, e o resultado de `cargo test --workspace` e `cargo clippy`.
```

---

## 4. Analisar o insert do Cassandra  ·  29/08 06:24

```
Você é o analista de fontes do projeto **PhxSql**. Escreva **em português**, com identificadores e nomes de arquivo **sem acento** (o texto corrido pode ter).

# A missão

Ler os fontes do **Apache Cassandra 5.0** e responder duas perguntas com evidência de `arquivo:linha`:

1. **O que o caminho de escrita dele faz para ser tão rápido**, e quanto disso é aplicável a um motor de arquivos separados com ordem de digitação sagrada.
2. **Como funciona o quórum de escrita**: o cliente pede confirmação de N réplicas antes do OK. Quero o mecanismo exato — quem conta, quem espera, o que acontece com as réplicas que não responderam, e o que o cliente pode concluir do OK.

**Você não escreve código do PhxSql.** Produz um documento.

# Os fontes, já baixados

`/tmp/claude-0/-home-user-adrianoboller/34595649-0af6-575a-8f79-80dbe8cb7a5d/scratchpad/fontes/cassandra/`

- `src/java/org/apache/cassandra/db/` — comece por `Keyspace.java` (`apply`), `Mutation.java`, `Memtable*`, e **`db/commitlog/`** (`CommitLog.java`, `AbstractCommitLogService.java`, `BatchCommitLogService.java`, `PeriodicCommitLogService.java`)
- `src/java/org/apache/cassandra/service/` — **`StorageProxy.java`** é onde o quórum mora; veja também `AbstractWriteResponseHandler.java` e as classes de `ConsistencyLevel`
- `src/java/org/apache/cassandra/io/sstable/` — para onde a memtable descarrega

Use `git -C <dir> sparse-checkout add <caminho>` se precisar de mais alguma pasta.

# O nosso motor, e o que já foi decidido

Código em `/home/user/adrianoboller/phxsql/crates/`. **Leia antes de escrever qualquer linha:**

1. `docs/DESEMPENHO.md` **inteiro**, e em especial:
   - **§5, «Por que LSM não cabe dentro do motor atual»** — já há uma decisão registrada contra LSM. Sua tarefa **não** é repeti-la nem contradizê-la de leve: é conferi-la contra o código real do Cassandra e dizer se ela continua de pé, onde erra, e o que ela deixou passar.
   - **§3** — WAL e group commit já foram medidos contra o nosso gargalo e recusados, porque miram o `fsync` do InnoDB e o nosso é 95% CPU.
   - **§4.8** — o achado mais recente: depois do write-back, o custo dominante virou a **codificação da linha**, não a árvore. Duas colunas (`Decimal(15,2)` e `Date`) fazem a inserção passar de 7,50 para 16,61 µs.
2. `docs/CONCORRENTES.md` — a análise do InnoDB e do Aria, já feita. **Não repita o que está lá**; referencie.
3. `docs/REPLICACAO.md` e `docs/CLUSTER.md` — o que temos hoje de replicação, que é assíncrona e puxada pela réplica.
4. `crates/phxsql-store/src/table.rs` (`inserir`), `reg.rs`, `ndx.rs`, `log.rs`.

## Números nossos, medidos (use, não remeça)

- Inserção com dois índices, esquema simples: **7,5 µs/linha**; com o esquema da bancada (mais `Decimal` e `Date`): **16,61 µs**.
- Bancada de 10 milhões: PhxSql **261,8 s** contra MySQL(R) **111,7 s**. Nós somos **95% CPU** e escrevemos **2,4 GiB** contra **32,0 GiB** do MySQL(R).
- Replicação: master 34.048 linhas/s, réplica 17.450 eventos/s, três em paralelo.

## As regras que limitam o que pode ser copiado

- **Zero dependências externas.** Só a `std` do Rust.
- **A ordem de digitação é sagrada**: o `.reg` grava na ordem em que a linha foi digitada, endereçamento O(1) por rowid, e **nunca reaproveita slot excluído**. Qualquer ideia que quebre isso é inaceitável — diga explicitamente quando encontrar uma.
- Não temos transação, nem MVCC, nem rollback. Não afirme que temos.
- Cite sempre **Cassandra**, **MySQL(R)**, **MariaDB(R)**, **InnoDB**, **Aria**.

# O que eu quero saber, em ordem de valor

1. **O commit log é o segredo, ou é a memtable?** `CommitLog.add` e o `AbstractCommitLogService`: qual é a política de `fsync` (`periodic` contra `batch`), e o que o cliente perde em cada uma. Compare com a nossa janela de durabilidade e com o `BULKINSERT`.
2. **O que exatamente a escrita NÃO faz** que a nossa faz. (Nós fazemos: codificar a linha, gravar o slot com CRC, manter B+tree por índice, gravar o evento no diário.) Aponte cada um no código deles.
3. **O quórum**, em detalhe — é o item 2 da missão e vale um capítulo próprio. Quero:
   - onde o coordenador decide quantas confirmações esperar (`ConsistencyLevel`, `blockFor`);
   - o que acontece com as réplicas lentas depois do OK (**hinted handoff**);
   - o que o OK **garante** e o que ele **não** garante — em particular, se `QUORUM` significa que o dado está durável em N discos ou só recebido em N processos;
   - o custo: latência contra segurança.
4. **O que disso cabe no PhxSql**, e o que não cabe. Seja duro aqui: a nossa replicação é **puxada pela réplica**, assíncrona, e existe por causa de firewall (o master abre uma porta e não alcança ninguém). Um quórum de escrita exige o master **empurrar** e **esperar** — é outro desenho. Diga o que ele custaria e o que ele daria.

# Método, que é a regra mais forte do projeto

**Diagnóstico plausível não é diagnóstico medido.** Este projeto já derrubou **seis** diagnósticos plausíveis por medi-los; estão em `docs/DESEMPENHO.md`. Então:

- Toda afirmação sobre o código deles precisa de **arquivo:linha**.
- Toda afirmação de custo diz **se é medida ou inferida**; se inferida, escreva «inferido, não medido» na frase.
- **Toda proposta que você fizer precisa vir com a prova que a confirmaria** — qual medição, em que exemplo, com que número esperado. O Adriano pediu isto explicitamente: *«todo teste deve ter uma prova real e a documentação do aprendizado»*.
- Se uma ideia não couber nas nossas regras, **diga por quê** em vez de omitir.

# A saída

**Um arquivo**: `/home/user/adrianoboller/phxsql/docs/CASSANDRA.md`

1. O que foi lido (versão, commit, caminhos)
2. O caminho de uma escrita, passo a passo, com citações
3. **O commit log e a memtable**: o que é o ganho de verdade
4. **O quórum de escrita**, capítulo próprio: mecanismo, garantias, o que o OK não diz
5. **A §5 do nosso `DESEMPENHO.md` continua de pé?** Confirme, corrija ou complete — com o código na mão
6. **O que cabe aqui**, em ordem de valor ÷ custo, cada item com a **prova que o confirmaria**
7. **O que não cabe**, e por quê

Não escreva que o PhxSql é *ACID compliant* nem que tem *replicação nativa*: nenhum dos dois é verdade hoje.

Ao terminar, responda em no máximo 25 linhas: o mecanismo real do ganho de escrita, o resumo do quórum, e as duas coisas que você levaria para o PhxSql — cada uma com a medição que a provaria.
```

---

## 5. Ligar protocolo, consoles e catalogo  ·  29/08 06:40

```
Você é o **agente de protocolo e consoles** do projeto **PhxSql**, motor de dados em Rust sem nenhuma dependência externa. Você tem um worktree git só seu — trabalhe nele, commite nele, **não empurre e não abra PR**.

**Leia `/home/user/adrianoboller/CLAUDE.md` inteiro antes de qualquer coisa**, depois `phxsql/docs/PENDENCIAS.md`, `phxsql/docs/SQL.md` e `phxsql/docs/MCP.md`.

# A sua fronteira de arquivos (importante: outro agente trabalha em paralelo)

**Seus:** `crates/phxsql-server/src/servidor.rs`, `crates/phxsql-server/src/mcp.rs`, `crates/phxsql-server/src/main.rs`/`lib.rs`, `crates/phxsql-sql/`, crates novas que você criar, `docs/SQL.md`, `docs/MCP.md`, `MANUAL.txt`.
**Proibidos (são do outro agente):** `crates/phxsql-store/src/log.rs`, `lixeira.rs`, `motivo.rs`, `crates/phxsql-core/src/cifra.rs`, `crates/phxsql-server/src/dblink/`, `crates/phxsql-server/src/pg/`, `crates/phxsql-server/src/config.rs`, `docs/FORMATO.md`.
**Proibidos para os dois:** `CHANGELOG.md`, `docs/PENDENCIAS.md`, `docs/dossie/` — eu consolido.

Se um item seu parecer exigir um arquivo proibido, escreva o que precisaria mudar num comentário/relatório em vez de mudar.

# Os 5 itens

1. **O catálogo de operações auto-descrito — a peça que serve tudo.** Hoje o `despachar` conhece ~40 operações e nada as descreve por dados. Crie um módulo (`catalogo.rs` no servidor) onde cada operação declara: nome, o que faz (uma frase), parâmetros (nome, tipo, obrigatório, o que significa), permissão exigida (`Atividade`), e um exemplo de pedido JSON. **A regra que motiva: número digitado à mão envelhece calado — e ajuda escrita à mão também.** Um teste tem de conferir que **toda** operação do `despachar` está no catálogo e vice-versa, para operação nova não nascer sem descrição (procure o `match` do despachar e derive a lista dele, ou vice-versa). Exponha como op `catalogo` no protocolo (quem tem acesso vê só o que pode).

2. **#83 — ligar a crate `phxsql-sql` ao servidor.** Op `sql`: recebe `{"op":"sql","database":"X","texto":"SELECT ..."}`, traduz com a crate que já existe (léxico+sintaxe+tradutor prontos, 44 testes) e executa **passando pelo `despachar`** — o portão de permissão continua sendo UM; a op `sql` não pode virar porta dos fundos que pula o direito por tabela (lembre: `juntar`/`unir` já foram esse furo uma vez). Erros de sintaxe viram mensagem que aponta a posição.

3. **#6 — o transporte MCP.** `mcp.rs` já tem `initialize`, `tools/list` e `tools/call`. Falta quem leia de stdin/escreva em stdout (JSON-RPC por linha). Faça `phxsqld --mcp` (ou subcomando). E **troque o `tools/list` escrito à mão pelo catálogo do item 1** — hoje são 9 ferramentas digitadas, e é exatamente a duplicação que o catálogo elimina.

4. **`phxsqlcmd` — o console interativo (pedido novo do Adriano).** Crate nova `crates/phxsql-cmd`: um prompt que fala o protocolo JSON com um servidor (host, porta, usuário — o desafio-resposta de autenticação já existe no cliente da replicação, `crates/phxsql-server/src/replica.rs`, use o mesmo caminho). Comandos da linha viram pedidos; `/help` lista **do catálogo** (op `catalogo` pela rede); `/help <comando>` detalha parâmetros e exemplo. Sem `readline` de fora — leitura de linha da `std` basta (sem histórico/setas nesta rodada, e diga isso no `--help`). Saída tabular legível para listas. Prove com teste por soquete contra um servidor de verdade.

5. **A meia-verdade do #127 — `criar_tabela` não declara chave estrangeira.** O `esquema` reporta FKs e o formato as suporta, mas nenhuma operação do protocolo as cria (só via API Rust). Acrescente o campo no `criar_tabela` (e confira se `duplicar_tabela` preserva). Teste que cria com FK pelo protocolo e a vê voltar no `esquema`.

# Regras (valem como leis)

- **Zero dependências externas.** Só a `std`.
- Código, comentários, documentação e commits **em português**; identificadores e comentários **sem acento**.
- Comentário explica **por que**, não o que.
- **Prova real nos dois sentidos**: para cada defeito que um teste novo protege, mostre no relatório que o teste **falha** com o defeito reposto e passa com a correção. Teste que passa por engano é pior que teste que falta — já aconteceu aqui com `socket.makefile()`.
- **Guarda nova entra pedida, não imposta**: nada quebra cliente ou arquivo antigo. O teste que mais importa é o do comportamento velho.
- **Documente o aprendizado**: o que você descobriu fazendo (armadilhas, decisões, o que recusou e por quê) vai numa seção nova de `docs/SQL.md`/`docs/MCP.md` ou num `docs/CONSOLE.md` novo — não só no relatório.
- Antes de cada commit: `cargo fmt --all`, `cargo clippy --workspace --all-targets` (**zero avisos**), `cargo test --workspace`.
- Um commit por item, mensagem contando decisão e motivo. Sem identificador de modelo em nada.

Ao terminar: relatório de no máximo 30 linhas — por item: o que ficou pronto, a prova real (o teste que falha sem e passa com), o que ficou de fora e por quê, e os números finais de `cargo test` e `clippy`.
```

---

## 6. Cifra integrada e dialeto PostgreSQL  ·  29/08 06:41

```
Você é o **agente do motor e da periferia** do projeto **PhxSql**, motor de dados em Rust sem nenhuma dependência externa. Você tem um worktree git só seu — trabalhe nele, commite nele, **não empurre e não abra PR**.

**Leia `/home/user/adrianoboller/CLAUDE.md` inteiro antes de qualquer coisa**, depois `phxsql/docs/PENDENCIAS.md`, `phxsql/docs/SEGURANCA.md` (§8 traz o desenho da integração da cifra, já decidido), `phxsql/docs/FORMATO.md` e `phxsql/docs/DBLINK.md`.

# A sua fronteira de arquivos (importante: outro agente trabalha em paralelo)

**Seus:** `crates/phxsql-store/src/log.rs`, `lixeira.rs`, `motivo.rs`, `volume.rs` (se precisar), `crates/phxsql-core/src/cifra.rs`, `crates/phxsql-server/src/config.rs`, `crates/phxsql-server/src/dblink/`, `crates/phxsql-server/src/pg/`, `docs/FORMATO.md`, `docs/SEGURANCA.md`, `docs/DBLINK.md`, testes novos seus.
**Proibidos (são do outro agente):** `crates/phxsql-server/src/servidor.rs`, `mcp.rs`, `main.rs`, `crates/phxsql-sql/`, `docs/SQL.md`, `docs/MCP.md`.
**Proibidos para os dois:** `CHANGELOG.md`, `docs/PENDENCIAS.md`, `docs/dossie/`.

Se um item exigir um arquivo proibido (ex.: expor um campo novo numa operação do servidor), escreva a mudança necessária no relatório em vez de fazê-la.

# Os 3 itens

1. **#101 — ligar a ChaCha20-Poly1305 ao `.log`, `.trash` e `.reason`.** A primitiva existe (`cifra.rs`, RFC 8439, todos os vetores passando) e o desenho está em `docs/SEGURANCA.md` §8 — siga-o; se discordar dele em algo, escreva por quê antes de divergir. Pontos que são lei:
   - **Pedida, não imposta**: cifra liga por configuração (`config.json`), e o padrão é DESLIGADA. Arquivo escrito antes continua abrindo. O teste que mais importa é o do comportamento velho.
   - **Nonce nunca repete em append-only** — o tipo `Sequencia` da primitiva existe para isso; prove com teste.
   - Chave derivada por PBKDF2 do material que a configuração aponta, **nunca** senha em claro em arquivo ou log.
   - Um arquivo cifrado aberto sem chave (ou com chave errada) tem de dar **erro claro**, não lixo nem pânico.
   - A replicação lê o `.log` — decida e documente o que acontece com um source cifrado (a réplica recebe decifrado pela sessão autenticada? o `posicao`/`replicar` continuam funcionando?). Teste isso.
   - Mexeu no formato? `docs/FORMATO.md` **no mesmo commit**.

2. **#101 (parte 2) — o `bytes_por_arquivo` do diário.** A compactação foi medida e recusada porque **nenhum volume fecha**: os três arquivos cortam a 1 GiB e o `.log` só fecharia o primeiro volume em ~24 milhões de eventos. A decisão registrada como faltante é tornar o corte do diário configurável. Faça: campo de configuração para o tamanho de volume do `.log`/`.trash`/`.reason` (com o padrão atual inalterado — comportamento velho intacto), e **meça de novo** a economia de compactar volume fechado com um corte pequeno numa tabela de 1 milhão (o medidor `--example quanto-ocupa` já existe; estenda-o). Se a conta continuar não pagando, **o número e a recusa são o resultado certo**.

3. **#86 — o dialeto PostgreSQL(R) no DbLink.** O cliente existe (`src/pg/`, SCRAM conferido contra o RFC 7677) e `Motor::conecta()` está `false` de propósito porque as operações do DbLink montam SQL de MySQL(R). Escreva a montagem de SQL por dialeto (aspas de identificador, `LIMIT`/`OFFSET`, tipos no `CREATE`, booleano, data), ligue `conecta()` para PostgreSQL(R) e prove. **Não há servidor PostgreSQL(R) instalado nesta máquina** — a prova real é por soquete contra um servidor falso seu que fala o protocolo de fio (como `bancada/carga/bulkinsert.py` faz com o nosso), conferindo byte a byte o que o cliente manda, mais os vetores do RFC. Diga explicitamente no relatório que a prova contra um PostgreSQL(R) de verdade fica pendente e o que ela exigiria.

# Regras (valem como leis)

- **Zero dependências externas.** Só a `std`.
- Código, comentários, documentação e commits **em português**; identificadores e comentários **sem acento**.
- Comentário explica **por que**, não o que.
- **Criptografia se confere contra vetor oficial** — nada de «parece certo».
- **Senha nunca em texto puro** — nem em arquivo, nem em log, nem em resposta.
- **Prova real nos dois sentidos**: para cada defeito que um teste novo protege, mostre que o teste **falha** com o defeito reposto e passa com a correção.
- **Medir a premissa vem antes de implementar o item** — este projeto já derrubou seis diagnósticos plausíveis; se a premissa cair, o número e a recusa são o resultado certo.
- **Documente o aprendizado** em `docs/SEGURANCA.md`/`docs/DBLINK.md`, não só no relatório.
- Antes de cada commit: `cargo fmt --all`, `cargo clippy --workspace --all-targets` (**zero avisos**), `cargo test --workspace`.
- Um commit por item, mensagem contando decisão e motivo. Sem identificador de modelo em nada.

Ao terminar: relatório de no máximo 30 linhas — por item: o que ficou pronto, a prova real, o que foi medido e recusado (com o número), o que ficou de fora e por quê, e os números finais de `cargo test` e `clippy`.
```

---

## 7. Triggers e stored procedures  ·  29/08 12:12

```
Você é um agente do time PhxSql trabalhando num worktree isolado do repositório adrianoboller/adrianoboller. O projeto é o motor de dados em Rust em `phxsql/`. Leia o CLAUDE.md da raiz INTEIRO antes de começar — ele vale inteiro. Sua missão fecha os pedidos 49/50 do projeto, agora com a decisão que faltava tomada pelo Adriano: **a linguagem dos triggers e das stored procedures é a do MySQL(R)/MariaDB(R)** (sintaxe similar, não idêntica — o que não couber, recusa com erro claro dizendo o que não é suportado).

## Missão

1. **TRIGGERS estilo MySQL(R)**: `CREATE TRIGGER nome BEFORE|AFTER INSERT|UPDATE|DELETE ON tabela FOR EACH ROW <corpo>`, `DROP TRIGGER`, listagem. O corpo suporta um subconjunto útil e honesto: `SET NEW.col = expr`, `IF/ELSEIF/ELSE/END IF`, `SIGNAL SQLSTATE '45000' SET MESSAGE_TEXT = '...'` (a recusa que cancela a escrita), e DML simples (INSERT de auditoria numa outra tabela). `NEW.*` legível/gravável em BEFORE INSERT/UPDATE; `OLD.*` legível em UPDATE/DELETE.
2. **STORED PROCEDURES estilo MySQL(R)**: `CREATE PROCEDURE nome(IN p1 TIPO, OUT p2 TIPO) <corpo>`, `DROP PROCEDURE`, `CALL nome(args)`. Corpo: `DECLARE`, `SET`, `IF`, `WHILE/END WHILE`, `SIGNAL`, e DML/SELECT usando o motor existente. OUT devolve no resultado do CALL.
3. **Um interpretador só** para os dois corpos — é por isso que os dois pedidos estão na mesma missão. Zero dependências: parser e avaliador escritos à mão, só `std`.

## Onde mexer

- O SQL existente: leia `phxsql/docs/SQL.md` e o módulo de SQL do servidor (op `sql` no `servidor.rs` — procure onde SELECT é tratado) antes de desenhar. O CREATE/DROP/CALL entram pela op `sql`.
- Persistência: arquivo JSON por database (ex.: `gatilhos.json` e `procedimentos.json` no diretório do database) — NÃO invente formato binário novo. Arquivo novo no layout em disco = atualizar `phxsql/docs/FORMATO.md` no mesmo commit. Arquivo ausente = zero triggers, comportamento velho intacto.
- Ponto de disparo dos triggers: dentro do caminho de escrita do servidor (ops `inserir`, `atualizar`, `excluir` e o lote). **O caminho SEM trigger tem de custar zero** — a lição do Profiler no CLAUDE.md: o portão (há trigger para esta tabela+evento?) vem ANTES de qualquer trabalho, sem parse, sem clone, sem lock extra. BEFORE pode alterar NEW e pode recusar via SIGNAL (a escrita não acontece e o erro chega ao cliente com a MESSAGE_TEXT); AFTER roda depois e falha de AFTER não desfaz a escrita (documente essa semântica com honestidade — não há transação).
- `catalogo.rs`: toda operação nova do `despachar` precisa de entrada no catálogo — há teste que trava. Se você só estender a op `sql`, atualize a descrição dela.
- Permissão: criar/excluir trigger e procedure exige o direito de administrar ou de criar na tabela — decida, aplique e documente. `CALL` roda com o poder de quem chama. Cuidado: se criar op nova sem campo `"tabela"`, o portão por tabela não a cobre sozinho — conferência própria dentro da op (lição do juntar/unir no CLAUDE.md).

## Regras da casa que mais mordem aqui

- Zero dependências externas, só `std`. Código, comentários e commits em português; identificadores sem acento. Comentário explica por quê, não o quê.
- **Toda bateria de testes tem prova real nos dois sentidos**: pelo menos um teste novo tem de FALHAR com o defeito reposto (faça a prova, registre no relatório) e passar com o conserto. Aprendizado (frutífero ou infrutífero) vai para o doc da área — crie `phxsql/docs/TRIGGERS.md` contando a linguagem, o subconjunto suportado, o que foi recusado e por quê, e os aprendizados.
- Prova por soquete: teste python no estilo de `phxsql/bancada/carga/bulkinsert.py` e `phxsql/bancada/dblink/prova-sincronia.py` (leia-os), erga um `phxsqld` seu nas portas **5301 (dados) / 5701 (web)** com config própria (referência: `phxsql/exemplos/Config_exemplo_01.json`). Roteiro mínimo: trigger BEFORE INSERT que normaliza um campo; trigger que SIGNAL recusa e a linha NÃO entra (confira contagem); trigger AFTER INSERT de auditoria gravando noutra tabela; procedure com IN/OUT e WHILE somando; DROP dos dois; e **o teste do comportamento velho**: tabela sem trigger grava exatamente como antes. NUNCA mate um phxsqld que não seja o seu (há um demo no ar em 5199/5599 — não toque; mate só pelo PID que você criou).
- Meça o custo do caminho sem trigger antes/depois (o exemplo `--example onde-doi` ou o de carga em `phxsql-store` podem servir; lembre `cargo build --release --examples -p phxsql-store` antes de medir — binário velho mede o passado). Se houver regressão mensurável no caminho sem trigger, conserte antes de entregar.

## Portões antes do commit final

`cargo fmt --all` && `cargo clippy --workspace --all-targets` (ZERO avisos) && `cargo test --workspace` (tudo verde), e a prova por soquete passando.

## O que NÃO fazer

Não atualize `PENDENCIAS.md`, `CHANGELOG.md` nem `docs/dossie/` (a integração faz). Não abra PR, não publique artifact, não faça push, não mexa na UI além do estritamente necessário. Não inclua identificador de modelo em nada commitado.

## Entrega

UM commit final no seu worktree (mensagem em português contando decisão e motivo). Relatório final: o que fez, o que ficou de fora e por quê, as provas com números (inclusive qual teste falhou com o defeito reposto), aprendizados, arquivos tocados, e o caminho do worktree.
```

---

## 8. Gestão de jobs com e-mail  ·  29/08 12:13

```
Você é um agente do time PhxSql trabalhando num worktree isolado do repositório adrianoboller/adrianoboller. O projeto é o motor de dados em Rust em `phxsql/`. Leia o CLAUDE.md da raiz INTEIRO antes de começar — ele vale inteiro.

## Missão

**Gestão de jobs**: o Adriano quer ver o que está rodando ou parado, e ser avisado **por e-mail** quando um job parar ou falhar.

1. **Estado por job**: nunca-rodou / agendado / rodando / ok / falhou / desligado, com última execução (quando, duração, resultado ou erro) e próxima prevista. Descubra o que já existe: há `job_salvar`, `job_rodar`, agenda `Cada{minutos}`/`Diaria`, `jobs.json` e `jobs.log` — leia o módulo de jobs do `phxsql-server` inteiro antes de desenhar.
2. **Aviso por e-mail**: quando um job FALHA, e quando um job estava agendado e NÃO rodou na janela esperada (parado — ex.: servidor esteve fora do ar, ou job desligado por erro). O cliente SMTP já existe (o aviso de disco apertado usa) — reuse-o, não escreva outro. Configuração: para quem mandar e se está ligado, no bloco de e-mail que já existe no config; **sem config de e-mail, nada muda** (guarda nova entra pedida, não imposta — o teste que mais importa é o do comportamento velho).
3. **Operações**: `job_listar` (ou amplie a existente) devolvendo o estado completo; entrada no `catalogo.rs` (há teste que trava operação sem descrição).
4. **Tela Jobs** (a barra de ferramentas já tem o botão): lista com estado em `pino` colorido, última/próxima execução, botões rodar-agora / ligar / desligar / excluir seguindo as CORES DA AÇÃO do CLAUDE.md (verde inclui, amarelo altera, vermelho exclui, azul consulta — sempre contorno, nunca fundo cheio), e a configuração do aviso por e-mail. Cuidado com o CSS global que morde (`label{text-transform:uppercase}`, `input{width:100%}`) — reuse classes existentes (`form-dbl`, `linha-chk`, `pino`, `aviso`) em vez de inventar.

## Regras da casa que mais mordem aqui

- Zero dependências externas, só `std`. Código/comentários/commits em português, identificadores sem acento. Comentário explica por quê.
- **Toda bateria de testes tem prova real nos dois sentidos**: pelo menos um teste tem de FALHAR com o defeito reposto (prove e registre). Aprendizado vai para doc da área (amplie o doc que cobre jobs, ou crie `phxsql/docs/JOBS.md`).
- **Prova por soquete + SMTP falso**: script python que ergue um SMTP falso local (socket puro, capture o RCPT e o corpo) e um `phxsqld` seu nas portas **5303 (dados) / 5703 (web)** com config própria (referência: `phxsql/exemplos/Config_exemplo_01.json`). Roteiro mínimo: job que roda ok (estado vira ok); job cujo pedido falha de propósito (estado falhou + E-MAIL CAPTURADO com o motivo); job atrasado além da janela (e-mail de parado); sem bloco de e-mail no config, mesmos eventos e NENHUM e-mail. NUNCA mate um phxsqld que não seja o seu (há um demo no ar em 5199/5599 com jobs reais rodando — não toque; mate só pelo PID que você criou).
- **Interface só se prova exercitando**: exercite a tela Jobs com Playwright (chromium em `/opt/pw-browsers/chromium`, import de `/opt/node22/lib/node_modules/playwright/index.mjs`) contra o seu servidor, com screenshots. Estilo de script: veja os `.mjs` em `/tmp/claude-0/-home-user-adrianoboller/34595649-0af6-575a-8f79-80dbe8cb7a5d/scratchpad/` (ex.: `ver-dblink.mjs`). Olhe as screenshots de verdade e conserte o que estiver feio ou errado.
- E-mail: assunto/corpo em português; NUNCA vaze senha ou hash em mensagem nenhuma.

## Portões antes do commit final

`cargo fmt --all` && `cargo clippy --workspace --all-targets` (ZERO avisos) && `cargo test --workspace`, prova por soquete+SMTP passando, tela exercitada.

## O que NÃO fazer

Não atualize `PENDENCIAS.md`, `CHANGELOG.md` nem `docs/dossie/`. Não abra PR, não publique artifact, não faça push. Não inclua identificador de modelo em nada commitado.

## Entrega

UM commit final no worktree (mensagem em português: decisão e motivo). Relatório: o que fez, o que provou (números, e qual teste falhou com o defeito reposto), aprendizados, arquivos tocados, caminho do worktree.
```

---

## 9. Driver ODBC para terceiros  ·  29/08 12:13

```
Você é um agente do time PhxSql trabalhando num worktree isolado do repositório adrianoboller/adrianoboller. O projeto é o motor de dados em Rust em `phxsql/`. Leia o CLAUDE.md da raiz INTEIRO antes de começar — ele vale inteiro. Sua missão fecha o pedido 7 (drivers ODBC e OLE DB para terceiros), com escopo honesto.

## Missão

1. **Crate nova `phxsql-odbc`** no workspace: uma `cdylib` com ABI C implementando o núcleo do ODBC 3.x, **zero dependências, só `std`** (uma cdylib de ABI C se escreve com `std` pura — `#[no_mangle] extern "system"`). Funções mínimas para um consumidor real funcionar: `SQLAllocHandle`, `SQLFreeHandle`, `SQLSetEnvAttr`, `SQLDriverConnect` (e `SQLConnect`), `SQLDisconnect`, `SQLExecDirect`, `SQLNumResultCols`, `SQLDescribeCol`, `SQLBindCol`, `SQLFetch`, `SQLGetData`, `SQLRowCount`, `SQLGetDiagRec`, `SQLFreeStmt`. Devoluções e structs conforme a especificação ODBC (os tipos são C: ponteiros, SQLSMALLINT, SQLRETURN...). Strings: comece por ANSI (`...A` sem sufixo); wide (`W`) só se couber sem inchar — se ficar de fora, diga no doc.
2. **Transporte**: o driver fala o protocolo JSON da porta de dados do PhxSql (linha JSON por pedido — leia como `phxsql/bancada/dblink/prova-sincronia.py` conversa: `{"op": ..., "token": ...}` + `login`). Connection string DSN-less: `Driver=PhxSql;Server=host;Port=5000;Token=...;UID=...;PWD=...;Database=...`. SELECT vai pela op `sql`; o resultado JSON vira colunas/linhas ODBC com tipos honestos (INT→SQL_INTEGER, DECIMAL→SQL_DECIMAL com escala, texto→SQL_VARCHAR, data/hora→tipos de data). **Senha nunca em log nem em mensagem de diagnóstico.**
3. **OLE DB: NÃO implementar** — é COM, e um provider nativo não cabe com honestidade nesta rodada. Documente em `phxsql/docs/ODBC.md` que consumidores OLE DB usam o provider padrão da Microsoft para ODBC (MSDASQL) sobre este driver — é a ponte canônica — e o que um provider nativo exigiria.
4. **Windows**: confira que a crate COMPILA para o alvo Windows que o projeto já usa na compilação cruzada (veja `phxsql/empacotar.sh` para descobrir o alvo; a compilação cruzada zero-deps já funcionou de primeira no projeto). Não precisa rodar no Windows — compilar limpo já é o critério. Se o `empacotar.sh` aceitar a `.so`/`.dll` sem retrabalho, inclua; senão, deixe dito no doc.

## Prova real (o critério da entrega)

Nada de «parece certo»: erga um `phxsqld` seu nas portas **5305 (dados) / 5705 (web)** com config própria (referência: `phxsql/exemplos/Config_exemplo_01.json`), crie tabela com INT, VARCHAR, DECIMAL e DATE, insira linhas conhecidas, e prove o driver **pela ABI**: um harness que faz `dlopen` da `.so` e chama a sequência real `SQLAllocHandle(ENV)→SQLSetEnvAttr(ODBC3)→SQLAllocHandle(DBC)→SQLDriverConnect→SQLAllocHandle(STMT)→SQLExecDirect(SELECT)→SQLNumResultCols/SQLDescribeCol→SQLFetch+SQLGetData até SQL_NO_DATA→SQLGetDiagRec num erro proposital` — em C compilado com o gcc da máquina, ou em python `ctypes` (também é prova de ABI). Compare os valores devolvidos com os inseridos, DECIMAL com as casas certas. Se conseguir instalar unixODBC (`isql`) no ambiente, melhor ainda — registre o resultado; se a rede não deixar, o harness de `dlopen` é a prova. Pelo menos um teste tem de FALHAR com um defeito reposto (ex.: truncamento de buffer em `SQLGetData` sem `SQL_SUCCESS_WITH_INFO`) e passar com o conserto — registre qual. NUNCA mate um phxsqld que não seja o seu (há um demo no ar em 5199/5599 — não toque).

## Regras da casa

Zero dependências (só `std`); código/comentários/commits em português, identificadores sem acento; comentário explica por quê. `phxsql/docs/ODBC.md` novo: como instalar/registrar no Windows (odbcinst) e no unixODBC, a connection string, o subconjunto suportado e o que ficou de fora com o motivo, a ponte MSDASQL, e os aprendizados da prova (frutíferos ou não). Antes do commit: `cargo fmt --all` && `cargo clippy --workspace --all-targets` (ZERO avisos) && `cargo test --workspace`.

## O que NÃO fazer

Não atualize `PENDENCIAS.md`, `CHANGELOG.md` nem `docs/dossie/`. Não abra PR, não publique artifact, não faça push. Não inclua identificador de modelo em nada commitado.

## Entrega

UM commit final no worktree. Relatório: o que fez, o subconjunto ODBC coberto, a prova com números (inclusive o teste que falhou com o defeito reposto), o resultado da compilação para Windows, aprendizados, arquivos tocados, caminho do worktree.
```

---

## 10. Editor ER com arrastar  ·  29/08 12:14

```
Você é um agente do time PhxSql trabalhando num worktree isolado do repositório adrianoboller/adrianoboller. O projeto é o motor de dados em Rust em `phxsql/`. Leia o CLAUDE.md da raiz INTEIRO antes de começar — ele vale inteiro. Sua missão fecha a segunda metade do pedido 127: o **editor visual do modelo**.

## Missão

O diagrama ER já existe e é leitura (`phxsql/crates/phxsql-server/ui/diagrama-er.js`, botão «Diagrama ER» na barra). O Adriano quer o passo seguinte: **desenvolvimento visual das tabelas, campos e relacionamentos, com arrastar-e-soltar, incluindo as conexões DbLink**.

1. **Arrastar tabelas** pelo diagrama (pointer events à mão — `pointerdown/move/up` com captura), posições lembradas (localStorage por database serve; documente que é por navegador).
2. **Criar tabela nova** a partir do diagrama (botão verde de incluir): abre um cartão para nome + campos (nome, tipo, obrigatório, chave primária) e chama a op real `criar_tabela` — descubra o contrato dela lendo `servidor.rs`/`catalogo.rs` e a tela «Nova tabela» existente; reuse o que der.
3. **Editar campos**: o que o servidor permitir de verdade. Se não existir op de alterar estrutura, o editor é honesto: mostra o que não dá para alterar e por quê, em vez de fingir — NÃO invente op nova de alterar estrutura nesta rodada sem necessidade; se criar alguma op, entrada no `catalogo.rs` (teste trava) e portão de permissão correto (op sem campo `"tabela"` precisa de conferência própria — CLAUDE.md).
4. **Relacionamento arrastando**: puxar de um campo de uma tabela até o campo de outra cria a chave estrangeira DECLARADA (o motor declara e não impõe — há teste que trava isso; o editor tem de dizer essa verdade na tela, não escondê-la).
5. **DbLink no diagrama**: as ligações DbLink e as tabelas remotas aparecem com visual distinto (tracejado/cor própria das variáveis do tema), e dá para criar uma conexão DbLink do próprio diagrama (reuse o assistente `assistenteDbLink()` que já existe no `ui/index.html` — não duplique).

**Pesquisa**: o Adriano sugeriu buscar no GitHub referência de drag-and-drop de modelagem ER. Pesquise (WebSearch/WebFetch) para se inspirar em UX (como o dbdiagram/drawDB resolvem âncoras de linha, por exemplo), mas o CÓDIGO é escrito aqui, à mão: o console funciona offline, zero dependências também na UI — nenhum script externo entra.

## Regras da casa que mais mordem aqui

- **O CSS global morde todo componente novo**: `label{text-transform:uppercase}` e `input{width:100%}` — o próprio `diagrama-er.js`/CSS já tem comentário avisando que morde `input` dentro do SVG. Reuse classes existentes. Cores da ação: verde inclui, amarelo altera, rosa marca, vermelho exclui, azul consulta — sempre contorno, nunca fundo cheio. As cores do diagrama saem das variáveis do tema (claro E escuro — confira nos dois).
- **Interface só se prova exercitando**: Playwright (chromium `/opt/pw-browsers/chromium`, import `/opt/node22/lib/node_modules/playwright/index.mjs`; exemplos de script em `/tmp/claude-0/-home-user-adrianoboller/34595649-0af6-575a-8f79-80dbe8cb7a5d/scratchpad/*.mjs`, ex. `ver-dblink.mjs`) contra um `phxsqld` SEU nas portas **5307 (dados) / 5707 (web)** — compile release (`cargo build --release -p phxsql-server --bin phxsqld` — a UI é `include_str!`, recompila a cada mudança) e config própria (referência: `phxsql/exemplos/Config_exemplo_01.json`). Roteiro: arrastar tabela e a posição sobreviver ao recarregar; criar tabela nova pelo diagrama e ela existir de verdade (confira pela op); arrastar relacionamento e a FK aparecer declarada; abrir o assistente DbLink do diagrama. Screenshots de cada passo, OLHADAS de verdade — o drag é onde o defeito mora (offset do ponteiro, zoom, tabela fugindo). Pelo menos um defeito achado exercitando deve estar no relatório com o conserto (se não achar nenhum, desconfie do seu exercício). NUNCA mate um phxsqld que não seja o seu (demo no ar em 5199/5599 — não toque).
- Zero dependências; português; identificadores sem acento; comentário explica por quê.

## Portões antes do commit final

`cargo fmt --all` && `cargo clippy --workspace --all-targets` (ZERO avisos) && `cargo test --workspace` (a UI é embutida — o build tem de passar), exercício do navegador completo.

## O que NÃO fazer

Não atualize `PENDENCIAS.md`, `CHANGELOG.md` nem `docs/dossie/`. Não abra PR, não publique artifact, não faça push. Não inclua identificador de modelo em nada commitado.

## Entrega

UM commit final no worktree (mensagem em português: decisão e motivo). Relatório: o que fez, o que ficou honesto-mas-de-fora, os defeitos que o exercício achou e os consertos, screenshots gerados (caminhos), aprendizados, arquivos tocados, caminho do worktree.
```

---

## 11. Cluster com eleição automática  ·  29/08 12:14

```
Você é um agente do time PhxSql trabalhando num worktree isolado do repositório adrianoboller/adrianoboller. O projeto é o motor de dados em Rust em `phxsql/`. Leia o CLAUDE.md da raiz INTEIRO antes de começar — ele vale inteiro. Sua missão fecha o pedido 126: **clusterização com eleição e promoção automática**, agora com a decisão do Adriano na mesa: gestão de inatividade, aviso por e-mail a cada X minutos, endereço único para validação, eleição e promoção automáticas.

## O que já existe (leia antes de desenhar)

Replicação Master→Réplica funcionando: `.log` v2 com imagem da linha, ops `posicao`/`replicar`/`aplicar`, laço da réplica dentro do `phxsqld`, marca de posição do diário. Leia o módulo de replicação inteiro, `docs/` sobre replicação, e os `Config_exemplo_02/03.json` (réplica e origem). O cliente SMTP já existe (aviso de disco) — reuse.

## Missão

1. **Config `cluster`** no `config.json`: lista de nós `[{id, endereco, porta}]`, prioridade do nó, janela de inatividade (segundos), aviso por e-mail a cada X minutos, destinatário. **Sem o bloco `cluster`, NADA muda** — guarda nova entra pedida, não imposta; o teste do comportamento velho é o que mais importa.
2. **Pulso (heartbeat)**: op nova `cluster_pulso` entre nós pela porta de dados (autenticada como a réplica já se autentica), carregando id, papel (master/réplica) e posição do diário. Cada nó mantém o mapa vivo do cluster.
3. **Detecção de inatividade**: master sem pulso além da janela → os nós vivos abrem eleição.
4. **Eleição determinística e SEGURA contra split-brain no que dá sem quórum de log**: só promove se o nó enxerga a MAIORIA dos nós configurados; entre os elegíveis, vence a maior posição do diário; empate quebra por prioridade e depois por id. **Sem maioria visível, NÃO promove** — fica degradado e avisa (é o teste de proteção mais importante da bateria). Documente com honestidade que isto não é Raft: sem log replicado por quórum de escrita, uma partição pode perder as últimas escritas do master isolado — diga o que o operador deve saber.
5. **Promoção automática**: a réplica eleita vira master (descubra se já existe promoção manual e reuse); as demais réplicas passam a apontar para o novo; o master antigo, se voltar, se vê destronado pelo pulso (posição/época menor — use uma época/geração incrementada a cada eleição para resolver «dois masters») e rebaixa-se sozinho a réplica.
6. **Endereço único para validação**: op `cluster_estado` respondendo em QUALQUER nó quem é o master atual (endereço:porta, época, mapa dos nós com papel/atraso/último pulso) + o protocolo devolvendo erro claro de redirecionamento (`REDIRECIONA host:porta`) quando uma escrita chega numa réplica — o cliente valida com um endereço qualquer e é apontado ao certo. Documente que VIP de rede é infraestrutura, não banco — o que entregamos é a semântica de endereço único pelo protocolo.
7. **Aviso por e-mail a cada X minutos** enquanto o cluster estiver degradado (nó caído, sem maioria, ou promoção ocorrida — este último avisa uma vez), reutilizando o SMTP existente. Sem config de e-mail, sem e-mail — e nada mais muda.
8. Operações novas → entrada no `catalogo.rs` (teste trava). Ops de cluster exigem a mesma autenticação da replicação, nunca anônimas.

## Prova real (o critério da entrega)

Bancada python no estilo `phxsql/bancada/` (leia a de replicação com quatro servidores): erga **3 ou 4 `phxsqld` seus nas portas 5310-5319** (dados e web separados, configs próprias). Roteiro mínimo, com o resultado esperado escrito ANTES de rodar: (a) cluster sobe, `cluster_estado` igual nos nós; (b) escreve no master, réplicas acompanham; (c) MATA o master (só o SEU, pelo PID), mede o tempo até a promoção, confere que o novo master aceita escrita e as outras réplicas o seguem; (d) SMTP falso captura o e-mail de degradação e o de promoção, e o repetido a cada X (use X pequeno em segundos para teste, se o config permitir minutos fracionários — senão documente); (e) **partição sem maioria**: isole um nó (mate os outros) e prove que ele NÃO se promove; (f) o master antigo volta e se rebaixa sozinho; (g) sem bloco `cluster` no config, quatro servidores sobem como hoje e nada de novo acontece. Pelo menos um teste tem de FALHAR com um defeito reposto (ex.: eleição sem checar maioria) — registre qual. NUNCA toque no phxsqld demo que está no ar em 5199/5599.

## Regras da casa

Zero dependências, só `std`; português; identificadores sem acento; comentário explica por quê; senha nunca em texto em log/resposta. Doc da área: crie `phxsql/docs/CLUSTER.md` com o desenho, as garantias REAIS e as não-garantias (split-brain, perda de cauda), o roteiro de operação e os aprendizados da bancada (frutíferos ou não). Mexeu em formato de config → documente no MANUAL se houver seção. Antes do commit: `cargo fmt --all` && `cargo clippy --workspace --all-targets` (ZERO avisos) && `cargo test --workspace`.

## O que NÃO fazer

Não atualize `PENDENCIAS.md`, `CHANGELOG.md` nem `docs/dossie/`. Não abra PR, não publique artifact, não faça push. Não inclua identificador de modelo em nada commitado.

## Entrega

UM commit final no worktree. Relatório: desenho escolhido e por quê, números da bancada (tempo de promoção, e-mails capturados), qual teste falhou com o defeito reposto, o que ficou de fora com motivo, aprendizados, arquivos tocados, caminho do worktree.
```

---

## 12. Blacklist de IPs e mensagens  ·  29/08 12:15

```
Você é um agente do time PhxSql trabalhando num worktree isolado do repositório adrianoboller/adrianoboller. O projeto é o motor de dados em Rust em `phxsql/`. Leia o CLAUDE.md da raiz INTEIRO antes de começar — ele vale inteiro.

## Missão (duas partes que se tocam)

**Parte A — Firewall/blacklist de IPs por comandos proibidos.**

1. **Config** `config.json`, bloco novo (ex.: `seguranca`): `comandos_proibidos` (lista de nomes de operação do protocolo e/ou primeiras palavras SQL, ex.: `"excluir_tabela"`, `"DROP"`), `tentativas_para_bloqueio`, `duracao_bloqueio_min` (0 = permanente até soltar), `whitelist` de IPs/CIDRs que nunca bloqueiam. **Sem o bloco, NADA muda** — guarda nova entra pedida, não imposta; o teste do comportamento velho (`sem_bloco_seguranca_nada_muda`) é o que mais importa.
2. **No servidor**: pedido de comando proibido → recusa com erro claro + soma tentativa por IP (o servidor já loga IP por conexão — leia o log de acessos existente, que foi desenhado «para caber fail2ban»). Excedeu as tentativas → o IP entra na **blacklist**: conexões futuras são recusadas na porta com erro nomeando o bloqueio e a duração. Persistência em arquivo próprio (ex.: `bloqueios.json`) para sobreviver reinício — arquivo novo no layout → nota no `docs/FORMATO.md` ou MANUAL. Whitelist vence sempre. Decida e documente o tratamento de `127.0.0.1` (sugestão: whitelist implícita, senão o operador se tranca fora — mas documente e teste).
3. **Integração com firewall de verdade, honesta**: sem rodar como root, o servidor não mexe em iptables. Entregue a exportação: op/arquivo com a blacklist em formato consumível por `nftables`/`iptables`/`fail2ban` (uma linha por IP, e o doc mostra o comando de aplicar). Documente em `phxsql/docs/SEGURANCA.md` (se existir, amplie; senão crie).
4. **Operações**: listar bloqueados (IP, motivo, quando, quantas tentativas), soltar um IP, ver/editar whitelist — com permissão de administrador. Entradas no `catalogo.rs` (teste trava). Op sem campo `"tabela"` → portão próprio dentro da op (lição do juntar/unir do CLAUDE.md).
5. **Tela**: a árvore de administração já tem «Bloqueios» — leia o que existe hoje e amplie: bloqueados com motivo e botão soltar (rosa/vermelho conforme a convenção), whitelist, e a configuração do bloco `seguranca`. Cores da ação sempre contorno; cuidado com o CSS global (`input{width:100%}`, `label` maiúsculo) — reuse classes existentes.

**Parte B — Gestão das mensagens de erro, no menu superior.**

1. Levante os códigos/mensagens de erro que o servidor devolve (ex.: 4002 `EM_CARGA`; erros de permissão; nao-encontrado...). Catalogue-os num lugar só se já não houver.
2. **Personalização**: o operador pode trocar o TEXTO apresentado por código (config e/ou arquivo próprio), para dar ao usuário final o motivo adequado na língua da empresa. O código e o campo estruturado do erro NÃO mudam (cliente antigo continua tratando pelo código — comportamento velho intacto); só o texto humano é personalizável. Sem personalização, textos de hoje, byte a byte.
3. **Tela no menu superior**: item novo (ex.: Configurações → Mensagens de erro) listando código, texto padrão e texto personalizado editável; salvar aplica sem reiniciar se possível (senão diga na tela que exige reinício — honestidade).
4. **Senha nunca em mensagem**: o teste existente que trava vazamento continua valendo; nenhum texto personalizado pode interpolar dados sensíveis novos.

## Prova real (o critério da entrega)

Bancada python por soquete contra um `phxsqld` SEU nas portas **5321 (dados) / 5721 (web)**, config própria (referência: `phxsql/exemplos/Config_exemplo_01.json`): (a) comando proibido recusa e conta; (b) na enésima tentativa o IP bloqueia e a PRÓXIMA CONEXÃO é recusada; (c) soltar desbloqueia; (d) whitelist nunca bloqueia; (e) exportação gera o formato prometido; (f) **sem bloco `seguranca`, tudo como antes**; (g) mensagem personalizada aparece no lugar da padrão e o CÓDIGO não muda; (h) sem personalização, texto padrão intacto. Pelo menos um teste tem de FALHAR com o defeito reposto (ex.: whitelist ignorada) — registre qual. Tela exercitada com Playwright (chromium `/opt/pw-browsers/chromium`; exemplos de script em `/tmp/claude-0/-home-user-adrianoboller/34595649-0af6-575a-8f79-80dbe8cb7a5d/scratchpad/*.mjs`), screenshots olhadas. A UI é `include_str!` — recompile release antes de exercitar. NUNCA mate o phxsqld demo (5199/5599).

## Regras da casa

Zero dependências, só `std`; português; identificadores sem acento; comentário explica por quê. Aprendizados no doc da área. Antes do commit: `cargo fmt --all` && `cargo clippy --workspace --all-targets` (ZERO avisos) && `cargo test --workspace`.

## O que NÃO fazer

Não atualize `PENDENCIAS.md`, `CHANGELOG.md` nem `docs/dossie/`. Não abra PR, não publique artifact, não faça push. Não inclua identificador de modelo em nada commitado.

## Entrega

UM commit final no worktree. Relatório: o que fez, provas com números (e qual teste falhou com o defeito reposto), decisões tomadas (localhost, duração, formatos de exportação) com motivo, aprendizados, arquivos tocados, caminho do worktree.
```

---

## 13. Wizard e modos de replicação  ·  29/08 12:23

```
Você é um agente do time PhxSql trabalhando num worktree isolado do repositório adrianoboller/adrianoboller. O projeto é o motor de dados em Rust em `phxsql/`. Leia o CLAUDE.md da raiz INTEIRO antes de começar — ele vale inteiro.

## Contexto

O PhxSql tem replicação Master→Réplica funcionando em streaming: `.log` v2 com imagem da linha, ops `posicao`/`replicar`/`aplicar`, laço da réplica dentro do `phxsqld`, marca de posição do diário, bancada com quatro servidores. Leia o módulo de replicação inteiro, os docs de replicação e os `Config_exemplo_02/03.json` antes de desenhar. O Adriano estudou o Centro de Controle do HFSQL(R) (Replicação → configurar) e pediu o equivalente no PhxSql, com nomes mais explícitos e um modo a mais.

## Missão

**1. O Wizard de replicação na tela** (botão «Replicação» já existe na barra de ferramentas — descubra o que a tela mostra hoje). Assistente no estilo do `assistenteDbLink()` que já existe no `ui/index.html` (leia-o e siga o padrão: diálogo `.sobre > .caixa.larga`, passos numerados, **cada passo só avança com o anterior PROVADO**). Passo 1 é a escolha do modo, com cartões visuais (o padrão dos cartões de junção `.venn` serve de referência — desenho SVG com as cores das variáveis do tema, claro E escuro):

- **A) Primary → Replica** (unidirecional): A→B, distribuição/cópia, filial, datacenter secundário. É o que já existe — o wizard passa a configurá-lo sem editar config na mão.
- **B) Multi-Master ↔ Multi-Master** (bidirecional): os dois recebem escrita e trocam alterações.
- **C) Primary → Standby / Failover** (spare): reserva de contingência que NÃO aceita trabalho de cliente; existe para assumir quando o primário morrer.
- **D) Read Replica**: réplica otimizada para consulta — aceita LEITURA de cliente, recusa escrita com erro claro, serve relatório e balanceamento de leitura.

Passos seguintes por modo: os servidores (endereço, porta, credencial de replicação — descubra como a réplica se autentica hoje e reuse), o que replicar (tudo/databases), **streaming ou agendado** (veja item 3), a política de conflito quando for o modo B (mostrada com todas as letras), e o passo final que PROVA: testa a conexão com o outro servidor, inicia, e mostra a posição/atraso de cada lado. Cores da ação (contorno, nunca fundo cheio), cuidado com o CSS global (`input{width:100%}`, `label` maiúsculo) — reuse `form-dbl`, `linha-chk`, `pino`, `aviso`.

**2. Os papéis novos no motor**:
- **Read Replica**: papel explícito na config; escrita de cliente recusada com erro claro que aponta o primário (código estruturado + texto). Descubra primeiro o que a réplica de hoje faz com escrita de cliente — se já recusa, o papel só formaliza e nomeia; se aceita, isso é um defeito a fechar.
- **Spare/Standby**: como a Read Replica, mas recusa TAMBÉM a leitura de cliente comum (só administração/monitoramento enxergam) — reserva é reserva. E a operação **`spare_promover`** (o equivalente do HRSTransformSpareIntoServer do HFSQL(R)): para o laço de réplica, o servidor passa a aceitar tudo, e o papel vira primário — operação LOCAL e manual, com permissão de administrador. IMPORTANTE: outro agente do time (frente cluster) está construindo eleição e promoção AUTOMÁTICA em worktree separado — você NÃO constrói eleição, quórum, heartbeat nem e-mail; sua `spare_promover` é o degrau manual, e a integração vai ligar a promoção automática dele ao seu caminho. Deixe a função de promover coesa e chamável por dentro (uma função no módulo, a op como casca fina).

**3. Agendamento**: hoje a réplica é streaming (o laço puxa continuamente). Entra a opção de replicar **agendado**: a cada X minutos, de hora em hora, ou diária a uma hora marcada (ex.: à noite) — campo na config da réplica (ausente/0 = streaming, comportamento velho intacto, byte a byte; é o teste que mais importa). Implemente no próprio laço da réplica (dormir até a janela, puxar tudo, dormir de novo) — NÃO acople ao subsistema de jobs (outro agente está mexendo nele).

**4. Bidirecional (Multi-Master) — a parte funda.** Dois servidores, cada um réplica do outro. Os dois problemas reais, que você resolve e DOCUMENTA:
- **Laço infinito**: a alteração que A aplicou vinda de B não pode voltar para B. Os eventos precisam carregar a ORIGEM (id do servidor de origem no evento do diário ou no envelope da replicação): ao replicar para fora, eventos cuja origem é o próprio destino não viajam. Se isso exigir campo novo no formato do `.log` v2 ou do envelope, é mudança de formato: entra cedo (não há dado de produção), e `docs/FORMATO.md` atualiza NO MESMO commit. Cada servidor ganha um id estável (config).
- **Conflito**: o mesmo registro alterado dos dois lados antes de sincronizar. Política: **modificação mais recente vence**, pelo carimbo de data/hora que o `.log` já tem por evento — e o doc diz com todas as letras que isso exige relógios sincronizados entre os servidores (NTP), e o que acontece se não estiverem (o lado com relógio adiantado vence sempre). Empate de carimbo: desempata por id de servidor (determinístico, documentado).
- **Identidade da linha**: CUIDADO — aqui mora o risco de estrago silencioso. A replicação de hoje replica pela imagem da linha e o rowid segue o `.reg` do master. Com escrita nos DOIS lados, os rowids divergem (o insert local de A e o de B podem ganhar o mesmo rowid). **A ordem de digitação é sagrada em cada servidor** — cada `.reg` mantém a SUA ordem de chegada; a identidade entre servidores tem de ser a CHAVE do registro, não o rowid (o mesmo desenho da sincronia do DbLink em `crates/phxsql-server/src/dblink/sincronia.rs` — leia como ela casa por chave e por nome). Consequência honesta: o modo bidirecional exige tabela com chave única — sem chave, o wizard recusa a tabela com o motivo escrito. Documente essa exigência (o HFSQL(R) também impõe identificador adequado para replicar).
- Exclusão: decida e documente com honestidade o que viaja no bidirecional (o evento de exclusão existe no diário, então PODE viajar — diga como conflito exclusão×alteração se resolve pela mesma regra do mais recente).

**5. Operações novas → `catalogo.rs`** (teste trava operação sem descrição). Ops de replicação autenticam como a replicação de hoje autentica, nunca anônimas. `docs` da área: amplie o doc de replicação existente (ou crie `phxsql/docs/REPLICACAO.md` se não houver um) com os quatro modos, os diagramas em texto, a política de conflito, a exigência de chave, a exigência de relógio, e os aprendizados da bancada — frutíferos ou infrutíferos.

## Prova real (o critério da entrega)

Bancada python no estilo da de replicação existente (leia-a), com servidores SEUS nas portas **5330-5339** — NUNCA toque no demo que está no ar em 5199/5599; mate só PIDs que você criou. Resultado esperado escrito ANTES de rodar cada estágio:
(a) modo A pelo wizard-equivalente por soquete (as mesmas ops que o wizard chama): A→B replica, B não devolve;
(b) agendado: com cada_minutos=1 (ou janela pequena), a alteração NÃO aparece antes da janela e aparece depois — e sem o campo, streaming como hoje;
(c) bidirecional: insert em A aparece em B, insert em B aparece em A, e NÃO volta (prova do laço morto — conte os eventos);
(d) conflito: mesmo registro alterado nos dois lados, vence o carimbo mais recente nos DOIS servidores (convergência — os dois terminam iguais);
(e) tabela sem chave única: o modo B recusa com o motivo;
(f) spare: cliente comum não lê nem escreve; `spare_promover` e ele vira primário aceitando tudo;
(g) read replica: leitura ok, escrita recusada com o erro que aponta o primário;
(h) comportamento velho: configs de réplica de hoje (`Config_exemplo_02/03`) sobem e replicam exatamente como antes.
E **o exercício do wizard no navegador** com Playwright (chromium `/opt/pw-browsers/chromium`, import `/opt/node22/lib/node_modules/playwright/index.mjs`; exemplos em `/tmp/claude-0/-home-user-adrianoboller/34595649-0af6-575a-8f79-80dbe8cb7a5d/scratchpad/*.mjs`, ex. `ver-dblink.mjs`) — a UI é `include_str!`, recompile release antes; screenshots de cada passo, OLHADAS. Pelo menos um teste tem de FALHAR com um defeito reposto (ex.: supressão de origem desligada → evento volta) — registre qual.

## Regras da casa

Zero dependências, só `std`; código/comentários/commits em português, identificadores sem acento; comentário explica por quê; senha nunca em texto em log/resposta; interface só se prova exercitando; medidor com binário velho mede o passado (recompile release antes de medir). Antes do commit: `cargo fmt --all` && `cargo clippy --workspace --all-targets` (ZERO avisos) && `cargo test --workspace`.

## O que NÃO fazer

Não construa eleição/heartbeat/quórum/e-mail (frente cluster) nem tela de jobs (frente jobs). Não atualize `PENDENCIAS.md`, `CHANGELOG.md` nem `docs/dossie/`. Não abra PR, não publique artifact, não faça push. Não inclua identificador de modelo em nada commitado. Cite HFSQL(R), MySQL(R), PostgreSQL(R) com a marca.

## Entrega

UM commit final no worktree (mensagem em português: decisão e motivo). Relatório: os quatro modos e o que cada um provou (números da bancada), as decisões de desenho (origem no evento, conflito, exigência de chave, exclusão) com motivo, qual teste falhou com o defeito reposto, o que ficou de fora e por quê, aprendizados, arquivos tocados, caminho do worktree.
```

---

## 14. Wizard de replicação (tela)  ·  29/08 12:25

```
Você é um agente do time PhxSql trabalhando num worktree isolado do repositório adrianoboller/adrianoboller. O projeto é o motor de dados em Rust em `phxsql/`. Leia o CLAUDE.md da raiz INTEIRO antes de começar — ele vale inteiro. Sua missão é o **Wizard de configuração da replicação** no Centro de Controle — SÓ a tela e o exercício dela; o motor dos modos novos está sendo construído em paralelo por outro agente.

## Contexto e divisão de trabalho

O PhxSql tem replicação Master→Réplica funcionando (ops `posicao`/`replicar`/`aplicar`, laço da réplica no `phxsqld`, `Config_exemplo_02/03.json` de réplica e origem). Um agente irmão está construindo NO MOTOR, no worktree `/home/user/adrianoboller/.claude/worktrees/agent-aeba5ba7fe4b19f92` (LEIA-O como referência, NUNCA escreva nele): os papéis Read Replica e Spare (+ op `spare_promover`), o agendamento (streaming quando ausente), e o modo bidirecional (Multi-Master) com origem no evento, conflito «mais recente vence» e exigência de chave única. O contrato das operações dele sai do `catalogo.rs` do worktree dele (nome, resumo, parâmetros) — alinhe o wizard a esse contrato, relendo o worktree quando for montar cada passo, porque ele evolui enquanto você trabalha. Se um contrato de que você precisa ainda não existir lá, monte a chamada pelo desenho combinado abaixo e marque no relatório o que precisa conferir na integração.

## Missão

O wizard, no botão «Replicação» da barra (descubra o que a tela mostra hoje e integre — o wizard entra como o assistente do DbLink entrou na tela DbLink). Siga o padrão do `assistenteDbLink()` que já existe no `ui/index.html` (leia-o inteiro): diálogo `.sobre > .caixa.larga`, molde com passos numerados, **cada passo só avança com o anterior PROVADO** (testar conexão antes de configurar; um assistente que deixa pular o teste é um cadastro com etapas).

**Passo 1 — a escolha do modo**, com quatro cartões visuais no padrão dos cartões de junção `.venn` (SVG desenhado à mão, cores SÓ das variáveis do tema — confira no claro E no escuro):

- **A) Primary → Replica** — A→B, distribuição/cópia: central→filial, relatórios, datacenter secundário. (Existe hoje no motor.)
- **B) Multi-Master ↔ Multi-Master** — os dois recebem escrita e trocam alterações; o cartão e o passo de configuração DIZEM a política de conflito («modificação mais recente vence — exige relógios sincronizados/NTP») e a exigência de chave única por tabela.
- **C) Primary → Standby / Failover** — reserva de contingência: não atende cliente; existe para assumir no desastre; o passo final mostra onde fica a promoção (`spare_promover`).
- **D) Read Replica** — réplica de consulta: aceita leitura, recusa escrita apontando o primário; para relatórios e balanceamento de leitura.

**Passos seguintes por modo** (adapte ao contrato do motor): os servidores (endereço, porta, credencial de replicação — descubra como a réplica se autentica hoje), o que replicar, **streaming ou agendado** (a cada X minutos / de hora em hora / diária a uma hora marcada — ausente = streaming), e o passo final que PROVA: testa a conexão com o outro servidor, aplica a configuração pelas ops reais, e mostra a posição/atraso de cada lado com o resultado na cara (o padrão do passo «Pronto» do assistente DbLink, com a primeira rodada mostrada).

**Regras de tela da casa**: cores da ação (verde inclui, amarelo altera, rosa marca, vermelho exclui, azul consulta — sempre contorno, nunca fundo cheio); o CSS global MORDE (`label{text-transform:uppercase}`, `input{width:100%}`) — reuse `form-dbl`, `linha-chk`, `pino`, `aviso`, `aviso.bom`, `lista-limpa` em vez de inventar classe; texto de interface em português (pode ter acento), identificadores sem.

## Prova real (o critério da entrega)

**Interface só se prova exercitando.** Playwright (chromium `/opt/pw-browsers/chromium`, import `/opt/node22/lib/node_modules/playwright/index.mjs`; exemplos de roteiro em `/tmp/claude-0/-home-user-adrianoboller/34595649-0af6-575a-8f79-80dbe8cb7a5d/scratchpad/*.mjs`, ex. `ver-dblink.mjs` e o padrão de login). A UI é `include_str!` — `cargo build --release -p phxsql-server --bin phxsqld` a cada mudança antes de exercitar.

1. **Modo A de ponta a ponta AGORA**: o motor de hoje já faz Primary→Replica. Erga DOIS `phxsqld` seus nas portas **5340-5349** (um primário, um réplica — os `Config_exemplo_02/03.json` mostram como), rode o wizard no navegador configurando A→B de verdade, grave uma linha no primário e MOSTRE no passo final a réplica acompanhando (posição/atraso). Screenshots de cada passo, OLHADOS — conserte o que estiver feio ou errado antes de entregar.
2. **Modos B, C e D**: exercite o wizard até o fim do fluxo de configuração com screenshots (cartões, passos, textos de política de conflito e exigência de chave legíveis nos dois temas). A prova de ponta a ponta desses três contra o motor novo acontece na INTEGRAÇÃO — deixe o roteiro Playwright deles PRONTO para rodar (arquivo `.mjs` comentado dizendo o que espera), e marque no relatório exatamente o que ficou aguardando o motor.
3. Pelo menos um defeito achado exercitando deve estar no relatório com o conserto — se não achou nenhum, desconfie do seu exercício (a casa já achou três defeitos em cinco minutos de vídeo).
4. NUNCA toque no phxsqld demo que está no ar em 5199/5599; mate só PIDs que você criou.

## Portões antes do commit final

`cargo fmt --all` && `cargo clippy --workspace --all-targets` (ZERO avisos) && `cargo test --workspace` (a UI é embutida, o build tem de passar) && o exercício do modo A passando de ponta a ponta.

## O que NÃO fazer

Não mexa no motor de replicação (módulos Rust da replicação são do agente irmão — seu commit não pode conflitar com o dele em nada além de, no máximo, `servidor.rs`/`catalogo.rs` se precisar de uma op de leitura de estado que não exista; prefira não precisar). Não escreva no worktree do irmão. Não atualize `PENDENCIAS.md`, `CHANGELOG.md` nem `docs/dossie/`. Não abra PR, não publique artifact, não faça push. Não inclua identificador de modelo em nada commitado. Cite HFSQL(R) com a marca se citar.

## Entrega

UM commit final no worktree (mensagem em português: decisão e motivo). Relatório: o fluxo de cada modo com os screenshots (caminhos), o que o modo A provou de ponta a ponta (números: posição, atraso), os defeitos que o exercício achou e os consertos, o que ficou aguardando o motor da frente irmã (lista exata de ops/contratos a conferir na integração), aprendizados, arquivos tocados, caminho do worktree.
```

---

## 15. Revisão das telas de configuração  ·  29/08 12:39

```
Você é um agente do time PhxSql trabalhando num worktree isolado do repositório adrianoboller/adrianoboller. O projeto é o motor de dados em Rust em `phxsql/`. Leia o CLAUDE.md da raiz INTEIRO antes de começar — ele vale inteiro. Sua missão: **ajuste e revisão das telas de configuração, global e local, conferindo item por item contra o `config.json` e contra o código**.

## A lição que rege esta missão

O CLAUDE.md registra: «Configuração que não é lida mente.» O `recursos.cache_paginas` ficou da 0.13.0 em diante no `config.json`, no MANUAL e na tela dizendo «4096 páginas do .ndx em memória» — e NENHUMA linha de código o lia. Campo de configuração sem leitor é pior que campo ausente. Sua revisão existe para que isso não esteja acontecendo de novo em nenhum canto.

## Missão

**1. A tabela-verdade de três colunas, campo por campo.** Para a configuração GLOBAL (`config.json`: bind, portas, recursos, e-mail, réplica, etc.) e para a LOCAL (o que se configura por database e por tabela: diretivas de acesso, geometria/volumes, chaves — descubra tudo lendo as telas «Config», «Configurações e diretivas da tabela», «Diretivas de acesso ao banco» e o código que as serve): levante para CADA item — (a) existe no arquivo/estrutura de config? (b) existe LEITOR no código (aponte arquivo:linha)? (c) aparece na tela? Toda célula divergente é um defeito com conserto obrigatório e decisão documentada:
   - na tela e sem leitor → ou ganha leitor de verdade (se o efeito prometido é implementável nesta rodada) ou SAI da tela, do MANUAL e dos `exemplos/Config_exemplo_0*.json`, com o motivo escrito;
   - lido pelo código e fora da tela → entra na tela com a explicação do ajuste (o padrão do pedido 129: tela de configuração explicando cada ajuste);
   - nomes divergentes entre arquivo/tela/MANUAL → unifica (o config já avisa campo com nome errado — confira que o aviso cobre tudo).

**2. Os itens têm de FUNCIONAR.** Descubra o estado real: o dossiê registra que as telas de configuração «leem, não gravam». O Adriano quer os itens funcionando com o arquivo json:
   - onde a tela permite editar, o salvar tem de gravar no `config.json` (escrita ATÔMICA — grava arquivo temporário e renomeia, o padrão que o cadastro do DbLink já usa; permissão de administrador; NUNCA ecoar senha/token de volta — há teste que trava vazamento de ficha, e a tela mostra segredo como •••);
   - o efeito: campo que aplica a quente, aplica a quente e a prova confirma; campo que exige reinício diz isso NA TELA ao lado do campo (honestidade em vez de promessa) — levante qual é qual lendo o código de arranque;
   - configuração local (diretivas por database/tabela): mesma régua — editar, gravar, aplicar, e o teste do comportamento velho (arquivo antigo sem os ajustes novos abre igual).

**3. Revisão de layout.** Exercite TODAS as telas de configuração no navegador, nos DOIS temas (claro e escuro), com screenshots OLHADOS um a um: o CSS global morde (`label{text-transform:uppercase}` mente sobre o dado, `input{width:100%}` estoura checkbox/radio em célula); grupos com `gap` e não margens empilhadas; números com `tabular-nums`; cores da ação (verde inclui, amarelo altera, rosa marca, vermelho exclui, azul consulta — sempre contorno, nunca fundo cheio); reuse as classes existentes (`form-dbl`, `linha-chk`, `pino`, `aviso`, `fichas`) em vez de inventar. Conserte o que estiver mordido, desalinhado ou ilegível — e liste cada conserto no relatório.

**4. Fronteiras com o resto do time (importante).** Outros agentes, em worktrees separados, estão ACRESCENTANDO blocos novos de config nesta mesma rodada: `seguranca` + idioma de mensagens (um agente), `cluster` (outro), agendamento de replicação (outro). NÃO crie telas nem campos para esses blocos novos — cada frente faz a sua. Sua revisão cobre o que EXISTE hoje no master (seu worktree parte dele). Se a moldura da tela de configuração precisar de um lugar natural para seções novas se encaixarem, deixe a moldura pronta e diga no relatório onde cada bloco novo vai plugar.

## Prova real (o critério da entrega)

Bancada por soquete + navegador contra um `phxsqld` SEU nas portas **5350 (dados) / 5750 (web)**, config própria (referência: `phxsql/exemplos/Config_exemplo_01.json`). A UI é `include_str!` — `cargo build --release -p phxsql-server --bin phxsqld` antes de cada exercício. Roteiro mínimo, com o esperado escrito ANTES:
(a) a tabela-verdade completa no relatório (e as divergências ANTES dos consertos — é a fotografia do problema);
(b) editar um campo global pela tela → o `config.json` no disco mudou (diff do arquivo) → o efeito é observável (escolha um campo de efeito mensurável a quente; se nenhum aplicar a quente hoje, o reinício aplica e a tela avisa — prove os dois lados);
(c) editar uma diretiva local → gravou → aplicou → **o teste do comportamento velho**: config/arquivos de antes abrem byte a byte como antes;
(d) segredo nunca volta: a resposta da op de configuração e o HTML da tela não carregam senha/token em claro (grep na resposta e no DOM — e há teste Rust que trava isso: confira que ele cobre a op de gravar);
(e) campo-sem-leitor: para CADA um que você achar, ou a prova do efeito novo (leitor implementado, teste falha com o leitor removido — a prova nos dois sentidos) ou a remoção completa (tela+MANUAL+exemplos);
(f) screenshots de todas as telas de configuração nos dois temas, antes e depois dos ajustes de layout.
Playwright: chromium `/opt/pw-browsers/chromium`, import `/opt/node22/lib/node_modules/playwright/index.mjs`; exemplos de roteiro em `/tmp/claude-0/-home-user-adrianoboller/34595649-0af6-575a-8f79-80dbe8cb7a5d/scratchpad/*.mjs`. NUNCA toque no phxsqld demo em 5199/5599; mate só PIDs seus. Pelo menos um teste tem de FALHAR com um defeito reposto — registre qual.

## Regras da casa

Zero dependências, só `std`; português; identificadores sem acento; comentário explica por quê; mensagem de commit conta decisão e motivo. Operação nova → entrada no `catalogo.rs` (teste trava); op de configuração sem campo `"tabela"` → portão próprio de administrador dentro da op. Mexeu no MANUAL/exemplos → no mesmo commit. Aprendizados (frutíferos ou infrutíferos) no doc da área. Antes do commit: `cargo fmt --all` && `cargo clippy --workspace --all-targets` (ZERO avisos) && `cargo test --workspace`.

## O que NÃO fazer

Não atualize `PENDENCIAS.md`, `CHANGELOG.md` nem `docs/dossie/`. Não abra PR, não publique artifact, não faça push. Não inclua identificador de modelo em nada commitado.

## Entrega

UM commit final no worktree. Relatório: a tabela-verdade (as três colunas, com as divergências achadas), cada conserto de funcionamento e de layout com antes/depois, qual teste falhou com o defeito reposto, os campos que saíram e por quê, onde os blocos novos das outras frentes vão plugar, aprendizados, arquivos tocados, caminho do worktree.
```

---

## 16. Análise do manual CQL do Cassandra  ·  29/08 12:48

```
Você é um agente do time PhxSql trabalhando num worktree isolado do repositório adrianoboller/adrianoboller. O projeto é o motor de dados em Rust em `phxsql/`. Leia o CLAUDE.md da raiz INTEIRO antes de começar — ele vale inteiro. Sua missão é de PESQUISA E PROPOSTA, não de implementação: **baixar e analisar o manual do Cassandra(R) — a linguagem CQL e a documentação oficial — e produzir a lista de sprints que o PhxSql poderia executar, para APROVAÇÃO do Adriano**. Nada do que você propuser será executado sem o sim dele — deixe isso claro no próprio documento.

## Missão

**1. Baixe o manual.** A documentação oficial do Apache Cassandra(R) está em cassandra.apache.org (a referência CQL — `docs/latest/cassandra/developing/cql/` — e as seções de arquitetura: storage engine, consistency, replication, compaction, secondary indexes, materialized views, TTL, lightweight transactions/Paxos, batches, counters, collections, UDT, SASI/SAI). Use WebFetch/WebSearch pela rede do ambiente (há proxy configurado — se um fetch falhar, tente outra página; não desative TLS). Salve os trechos que fundamentam cada afirmação sua — **toda afirmação sobre o Cassandra(R) no seu documento carrega a fonte (URL e seção)**; afirmação sem fonte não entra. Cite Cassandra(R) sempre com a marca.

**2. Leia o que a casa JÁ sabe e JÁ está fazendo, para não propor o repetido:**
   - `phxsql/docs/CASSANDRA.md` e `phxsql/docs/CONCORRENTES.md` — a análise do caminho de inserção já foi feita (memtable/commit log/SSTable) e a pergunta do quórum já foi respondida (o OK de QUORUM não significa disco no modo padrão);
   - `phxsql/docs/PENDENCIAS.md` — o que está feito, parcial e planejado;
   - `phxsql/docs/DESEMPENHO.md`, `phxsql/bancada/resultados.json` — os números medidos (a bancada de 10M: insert já vence o MySQL(R); excluir ainda perde);
   - E as NOVE frentes em andamento AGORA por outros agentes (não proponha sprint do que já está em construção; proponha no máximo a evolução por cima): triggers+stored procedures estilo MySQL(R); gestão de jobs com e-mail; driver ODBC; editor ER com arrastar; cluster com eleição/promoção automática e aviso por e-mail; blacklist de IPs + tabela de mensagens multilíngue; motor de replicação em 4 modos (Primary→Replica, Multi-Master com mais-recente-vence, Spare/Failover, Read Replica) com agendamento; wizard de replicação na tela; revisão das telas de configuração.

**3. A análise.** Do manual, extraia o que o Cassandra(R) tem que o PhxSql não tem (ou tem diferente), e AVALIE cada item contra a realidade da casa:
   - **a regra de ouro do projeto**: «Receita de fora se mede contra o nosso gargalo antes de virar plano» — a arquitetura LSM inteira já foi avaliada uma vez e o resultado está no DESEMPENHO.md (o gargalo daqui era o `.ndx`, não o fsync); não repita a recomendação sem confrontá-la com os números atuais;
   - **zero dependências, só `std`** — o que exigiria crate não cabe; diga o que caberia escrito à mão;
   - o modelo de arquivos separados e **a ordem de digitação sagrada** do `.reg` — o que conflita com isso precisa ser dito;
   - candidatos prováveis a avaliar (confirme ou descarte com fonte e com o número da casa quando existir): TTL por linha/coluna; tombstones e o que ensinam sobre o nosso excluir (que ainda perde na bancada — 6,27 contra 4,73 s); counters; collections (list/set/map) e UDTs; batches (logged/unlogged) contra o nosso BULKINSERT; lightweight transactions (IF NOT EXISTS / Paxos) contra a nossa janela de conflito; materialized views; índices secundários (SAI) contra os nossos `.ndx`; snitch/topologia contra o nosso cluster nascente; níveis de consistência (ONE/QUORUM/ALL) contra a réplica nossa; compaction e o custo de espaço; paginação por token contra a nossa por cursor/rownum; vector search se o manual atual cobrir. Para cada um: o que é, o que resolveria AQUI, qual é a PREMISSA A MEDIR antes de implementar (a lição do pedido 113: a premissa se mede antes do item), e o custo/risco.

**4. O entregável: `phxsql/docs/SPRINTS-CASSANDRA.md`** — a proposta de sprints, PRIORIZADA, para aprovação. Formato por sprint:
   - **Sprint N — título curto**
   - O que entra (escopo concreto e fechado, do tamanho de uma rodada de trabalho);
   - Por que agora (o que do Cassandra(R) inspira, com a fonte; o que a casa ganha, com o número da casa quando houver);
   - **Premissa a medir primeiro** (a medição que pode matar o sprint — e matar é resultado válido, como manda o CLAUDE.md);
   - Dependências (de outra frente em andamento ou de outro sprint);
   - O que NÃO entra (a fronteira honesta).
   Ordene por valor medível para o projeto — desempenho do excluir, integridade, operação — e não por brilho. Feche o documento com a tabela-resumo (sprint, tamanho estimado P/M/G, premissa, dependência) e a frase de que a execução aguarda aprovação do Adriano, sprint a sprint.
   Se quiser rodar alguma MEDIÇÃO pequena para fundamentar premissa (ex.: perfil do excluir atual), pode — portas **5360/5760** são suas, `cargo build --release --examples -p phxsql-store` antes de medir (binário velho mede o passado), e NUNCA toque no demo em 5199/5599. Medição grande não: registre como premissa a medir no sprint.

## Regras da casa

Documento em português; Cassandra(R)/MySQL(R)/PostgreSQL(R) com a marca; número citado é número que não se mede — todo número da casa que você usar sai de `bancada/resultados.json`/DESEMPENho.md ou de medição sua reproduzível, com o comando; não repita «ACID compliant» nem «built-in replication» como se fossem verdade da casa. Nada de implementar funcionalidade nesta frente. Antes do commit: se tocou em algo de código (não deveria além de exemplo de medição), `cargo fmt`/clippy/test; para o doc, revise os links.

## O que NÃO fazer

Não atualize `PENDENCIAS.md`, `CHANGELOG.md` nem `docs/dossie/`. Não abra PR, não publique artifact, não faça push. Não inclua identificador de modelo em nada commitado.

## Entrega

UM commit final no worktree com o `SPRINTS-CASSANDRA.md` (e os apoios que quiser em `docs/`). Relatório: os 5 achados mais valiosos com uma linha cada, a lista dos sprints propostos em ordem (número, título, tamanho, premissa a medir), o que você descartou de propósito e por quê (tão importante quanto o proposto), as fontes principais, aprendizados, caminho do worktree.
```

---

## 17. Análise do manual do Redis  ·  29/08 12:49

```
Você é um agente do time PhxSql trabalhando num worktree isolado do repositório adrianoboller/adrianoboller. O projeto é o motor de dados em Rust em `phxsql/`. Leia o CLAUDE.md da raiz INTEIRO antes de começar. Sua missão é de PESQUISA E PROPOSTA, não de implementação: **baixar e analisar a documentação oficial do Redis(R) e produzir a lista de sprints que o PhxSql poderia executar, para APROVAÇÃO do Adriano** — nada executa sem o sim dele, e o documento diz isso.

Uma honestidade de partida que o seu documento deve registrar: o Redis(R) NÃO tem SQL — o «manual do Redis SQL» pedido é a documentação oficial de comandos, tipos e arquitetura (redis.io/docs e redis.io/commands). Analise o que ele É, sem fingir que é outra coisa.

## Missão

**1. Baixe o manual** (WebFetch/WebSearch; há proxy — se um fetch falhar, tente outra página; nunca desative TLS). Seções que interessam: tipos e comandos (string/hash/list/set/sorted set, streams); expiração/TTL e políticas de eviction; persistência **RDB e AOF** (o AOF é um diário append-only — parente direto do nosso `.log`: fsync always/everysec/no, rewrite/compactação do AOF); pub/sub e keyspace notifications; pipelining; transações MULTI/EXEC; scripting Lua; Redis Cluster (slots, resharding); replicação. **Toda afirmação sobre o Redis(R) carrega fonte (URL e seção)** — sem fonte, não entra. Marca sempre: Redis(R).

**2. Leia o que a casa já sabe e já está fazendo:** `phxsql/docs/PENDENCIAS.md`, `docs/DESEMPENHO.md`, `docs/CONCORRENTES.md`, `bancada/resultados.json` (a bancada de 10M: insert vence o MySQL(R); excluir ainda perde — 6,27 contra 4,73 s). E DEZ frentes rodando agora em paralelo por outros agentes — não proponha o que já está em construção: triggers+stored procedures estilo MySQL(R); gestão de jobs com e-mail; driver ODBC; editor ER; cluster com eleição/promoção; blacklist de IPs + tabela de mensagens multilíngue; motor de replicação em 4 modos (inclusive Multi-Master); wizard de replicação; revisão das telas de configuração; e uma frente irmã analisando o manual do Cassandra(R) (outra o do MariaDB(R)) — sobreposição de candidato (ex.: TTL aparece nos dois mundos) você ANOTA como «candidato compartilhado com a análise X» em vez de esconder; a consolidação das listas é da integração.

**3. A análise.** O que o Redis(R) tem que renderia aqui, avaliado contra a realidade da casa: a regra «receita de fora se mede contra o NOSSO gargalo antes de virar plano» (a arquitetura LSM já foi avaliada e recusada com números — DESEMPENHO.md); zero dependências, só `std`; o modelo de arquivos separados e a ordem de digitação sagrada do `.reg`. Candidatos prováveis (confirme ou descarte com fonte): TTL por linha com expiração; política do fsync do AOF (everysec) contra a nossa janela de durabilidade e o §4.9 pendente; rewrite do AOF contra o corte do nosso diário por volume; **pub/sub / keyspace notifications** contra a tela (a grade poderia assinar mudanças em vez de recarregar) e contra a replicação; pipelining contra o nosso protocolo linha-a-linha; cache em memória com eviction contra o nosso cache de páginas do `.ndx` (2,40× já medido — o que mais renderia?); MULTI/EXEC contra o nosso BULKINSERT e a janela de conflito; slots do cluster contra a nossa partição alfanumérica `.pag`. Para cada um: o que é, o que resolveria AQUI, **a premissa a medir primeiro** (a medição que pode matar o sprint — matar é resultado válido), custo/risco.

**4. Entregável: `phxsql/docs/SPRINTS-REDIS.md`** — sprints priorizados por valor medível, no formato: título; escopo fechado do tamanho de uma rodada; por que agora (fonte + número da casa quando houver); premissa a medir primeiro; dependências; o que NÃO entra. Tabela-resumo no fim (sprint, tamanho P/M/G, premissa, dependência) e a frase de que a execução aguarda aprovação do Adriano, sprint a sprint. Também registre o que você DESCARTOU de propósito e por quê. Medição pequena para fundamentar premissa pode (portas **5362/5762**; `cargo build --release --examples -p phxsql-store` antes de medir; NUNCA toque no demo em 5199/5599); medição grande vira premissa escrita.

## Regras e limites

Documento em português; número citado é número que não se mede (números da casa saem da bancada ou de medição sua com o comando); não repita «ACID compliant»/«built-in replication» como verdade da casa; nada de implementar. Não atualize PENDENCIAS/CHANGELOG/dossiê; não abra PR; não publique artifact; não faça push; sem identificador de modelo em nada commitado.

## Entrega

UM commit final no worktree com o SPRINTS-REDIS.md. Relatório: os 5 achados mais valiosos (uma linha cada), a lista dos sprints em ordem (número, título, tamanho, premissa), os descartes com motivo, os candidatos compartilhados com as análises irmãs, fontes principais, aprendizados, caminho do worktree.
```

---

## 18. Análise do manual do MariaDB  ·  29/08 12:50

```
Você é um agente do time PhxSql trabalhando num worktree isolado do repositório adrianoboller/adrianoboller. O projeto é o motor de dados em Rust em `phxsql/`. Leia o CLAUDE.md da raiz INTEIRO antes de começar. Sua missão é de PESQUISA E PROPOSTA, não de implementação: **baixar e analisar o manual SQL do MariaDB(R) — a Knowledge Base oficial (mariadb.com/kb) — e produzir a lista de sprints que o PhxSql poderia executar, para APROVAÇÃO do Adriano** — nada executa sem o sim dele, e o documento diz isso.

## Missão

**1. Baixe o manual** (WebFetch/WebSearch; há proxy — se um fetch falhar, tente outra página; nunca desative TLS). Seções que interessam na Knowledge Base: a referência SQL (statements, funções, tipos); **sequences** (CREATE SEQUENCE); colunas geradas/virtuais; CHECK constraints; **tabelas com versionamento de sistema** (system-versioned/temporal tables — `AS OF`); funções JSON; window functions e CTEs (WITH/RECURSIVE); particionamento; ALTER TABLE online (ALGORITHM=INSTANT/INPLACE); o event scheduler; roles no modelo de permissão; collations; fulltext; EXPLAIN/ANALYZE e o otimizador; e o que a MariaDB(R) tem que o MySQL(R) não tem (é o diferencial que interessa). **Toda afirmação carrega fonte (URL e seção)** — sem fonte, não entra. Marcas sempre: MariaDB(R), MySQL(R).

**2. Leia o que a casa já sabe e já está fazendo — aqui o risco de repetir é o MAIOR das três análises**, porque a casa já bebeu muito dessa fonte: `phxsql/docs/PENDENCIAS.md` (132 pedidos), `docs/CONCORRENTES.md` (a análise do caminho de inserção do MariaDB(R)/MySQL(R) JÁ FOI FEITA — não a repita), `docs/DESEMPENHO.md` e `bancada/resultados.json` (insert já vence o MySQL(R) na bancada de 10M; excluir ainda perde — 6,27 contra 4,73 s), `docs/SQL.md` (o que a camada SQL já sabe que precisa). E DEZ frentes rodando agora por outros agentes — não proponha o que já está em construção: **triggers e stored procedures estilo MySQL(R)/MariaDB(R) (frente inteira dedicada — não proponha de novo; proponha no máximo o degrau seguinte por cima, ex.: functions, cursors, handlers)**; gestão de jobs com e-mail (cobre o terreno do event scheduler — diga apenas o que faltaria); driver ODBC; editor ER; cluster com eleição/promoção; blacklist + mensagens multilíngues; motor de replicação em 4 modos (inclusive Multi-Master com mais-recente-vence); wizard de replicação; revisão das telas de configuração; e duas frentes irmãs analisando Cassandra(R) e Redis(R) — sobreposição de candidato você ANOTA como «candidato compartilhado com a análise X»; a consolidação é da integração.

**3. A análise.** Avalie cada candidato contra a realidade da casa: «receita de fora se mede contra o NOSSO gargalo antes de virar plano»; zero dependências, só `std`; arquivos separados e a ordem de digitação sagrada do `.reg`; guarda nova entra pedida, não imposta. Candidatos prováveis (confirme ou descarte com fonte): sequences contra o nosso rowid/identificadores; system-versioned tables contra os nossos `.trash`/`.reason` (a casa já guarda a linha inteira antes de sumir — quanto falta para um `AS OF`?); colunas geradas; CHECK; window functions e CTEs na nossa camada SQL nascente; ALTER TABLE online contra o nosso alterar-estrutura (que hoje quase não existe — o editor ER esbarrou nisso); particionamento nativo contra o `.pag` alfanumérico; roles contra o nosso modelo de direitos por base/tabela; EXPLAIN contra o nosso Profiler; fulltext contra o `.memo`. Para cada um: o que é, o que resolveria AQUI, **a premissa a medir primeiro** (a medição que pode matar o sprint — matar é resultado válido), custo/risco.

**4. Entregável: `phxsql/docs/SPRINTS-MARIADB.md`** — sprints priorizados por valor medível, formato: título; escopo fechado do tamanho de uma rodada; por que agora (fonte + número da casa quando houver); premissa a medir primeiro; dependências (inclusive de frente em andamento — ex.: «depende da frente triggers entregar o interpretador»); o que NÃO entra. Tabela-resumo no fim (sprint, tamanho P/M/G, premissa, dependência) e a frase de que a execução aguarda aprovação do Adriano, sprint a sprint. Registre também o que DESCARTOU de propósito e por quê. Medição pequena pode (portas **5364/5764**; `cargo build --release --examples -p phxsql-store` antes de medir; NUNCA toque no demo em 5199/5599); medição grande vira premissa escrita.

## Regras e limites

Documento em português; número citado é número que não se mede; não repita «ACID compliant»/«built-in replication» como verdade da casa; nada de implementar. Não atualize PENDENCIAS/CHANGELOG/dossiê; não abra PR; não publique artifact; não faça push; sem identificador de modelo em nada commitado.

## Entrega

UM commit final no worktree com o SPRINTS-MARIADB.md. Relatório: os 5 achados mais valiosos (uma linha cada), a lista dos sprints em ordem (número, título, tamanho, premissa), os descartes com motivo, os candidatos compartilhados com as análises irmãs, fontes principais, aprendizados, caminho do worktree.
```

---

## 19. Análise do manual Teradata SQL  ·  29/08 12:51

```
Você é um agente do time PhxSql trabalhando num worktree isolado do repositório adrianoboller/adrianoboller. O projeto é o motor de dados em Rust em `phxsql/`. Leia o CLAUDE.md da raiz INTEIRO antes de começar. Sua missão é de PESQUISA E PROPOSTA, não de implementação: **baixar e analisar o manual SQL do Teradata(R) — a documentação oficial em docs.teradata.com — e produzir a lista de sprints que o PhxSql poderia executar, para APROVAÇÃO do Adriano** — nada executa sem o sim dele, e o documento diz isso.

## Missão

**1. Baixe o manual** (WebFetch/WebSearch; há proxy — se um fetch falhar, tente outra página ou espelhos públicos da documentação; nunca desative TLS). O Teradata(R) é um banco MPP shared-nothing de data warehouse — o mundo MAIS DIFERENTE da casa entre as análises do time, e é aí que mora o valor. Seções que interessam: a arquitetura (AMPs, distribuição por hash do PRIMARY INDEX, BYNET); FALLBACK e journals; tabelas SET vs MULTISET; a cláusula **QUALIFY**; funções OLAP/window; tipos PERIOD e tabelas temporais; **macros** (consultas parametrizadas salvas — mais leves que stored procedures); MERGE; queue tables; índices secundários (USI/NUSI) e join indexes; COLLECT STATISTICS e o otimizador; **compressão multi-valor (MVC)** de coluna; identity columns; integridade referencial soft (declarada e não imposta); os utilitários de carga **FastLoad/MultiLoad/TPT**; workload management (TASM); row-level security. **Toda afirmação carrega fonte (URL e seção)** — sem fonte, não entra. Marca sempre: Teradata(R).

**2. Leia o que a casa já sabe e já está fazendo:** `phxsql/docs/PENDENCIAS.md`, `docs/CONCORRENTES.md`, `docs/DESEMPENHO.md`, `docs/SQL.md`, `bancada/resultados.json` (bancada de 10M: insert vence o MySQL(R); excluir ainda perde — 6,27 contra 4,73 s). Paralelos que a casa JÁ tem e você deve citar como ponto de partida, não como proposta: a FK daqui já é declarada-e-não-imposta (o mesmo espírito da soft RI do Teradata(R), com teste que trava a meia-verdade); o `BULKINSERT` já reserva a tabela para carga (compare com o modo de FastLoad e diga o que faltaria); a partição alfanumérica `.pag` existe (compare com a distribuição por hash do PRIMARY INDEX — premissa a medir, não recomendação cega); a compactação do DIÁRIO já foi medida e recusada DUAS vezes com números (DESEMPENHO.md) — se propuser compressão, é a MVC de coluna, que é outra coisa, e diga por que a conta seria diferente. E ONZE frentes rodando agora por outros agentes — não proponha o que está em construção: triggers+stored procedures estilo MySQL(R) (macros podem ser o degrau POR CIMA — anote a dependência); gestão de jobs com e-mail; driver ODBC; editor ER; cluster com eleição/promoção; blacklist + mensagens multilíngues; motor de replicação em 4 modos; wizard de replicação; revisão das telas de configuração; e três frentes irmãs analisando Cassandra(R), Redis(R) e MariaDB(R) — sobreposição de candidato (ex.: identity columns × sequences do MariaDB(R); temporal tables aparece no MariaDB(R) também) você ANOTA como «candidato compartilhado com a análise X»; a consolidação é da integração.

**3. A análise.** Avalie cada candidato contra a realidade da casa: «receita de fora se mede contra o NOSSO gargalo antes de virar plano» (a LSM já foi avaliada e recusada com números); zero dependências, só `std`; arquivos separados e a ordem de digitação sagrada do `.reg`; guarda nova entra pedida, não imposta. Para cada candidato: o que é, o que resolveria AQUI, **a premissa a medir primeiro** (a medição que pode matar o sprint — matar é resultado válido), custo/risco.

**4. Entregável: `phxsql/docs/SPRINTS-TERADATA.md`** — sprints priorizados por valor medível, formato: título; escopo fechado do tamanho de uma rodada; por que agora (fonte + número da casa quando houver); premissa a medir primeiro; dependências (inclusive de frente em andamento); o que NÃO entra. Tabela-resumo no fim (sprint, tamanho P/M/G, premissa, dependência) e a frase de que a execução aguarda aprovação do Adriano, sprint a sprint. Registre o que DESCARTOU de propósito e por quê — num mundo MPP, muito NÃO vai caber num motor de arquivos separados de um nó, e o descarte fundamentado é metade do valor desta análise. Medição pequena pode (portas **5366/5766**; `cargo build --release --examples -p phxsql-store` antes de medir; NUNCA toque no demo em 5199/5599); medição grande vira premissa escrita.

## Regras e limites

Documento em português; número citado é número que não se mede; não repita «ACID compliant»/«built-in replication» como verdade da casa; nada de implementar. Não atualize PENDENCIAS/CHANGELOG/dossiê; não abra PR; não publique artifact; não faça push; sem identificador de modelo em nada commitado.

## Entrega

UM commit final no worktree com o SPRINTS-TERADATA.md. Relatório: os 5 achados mais valiosos (uma linha cada), a lista dos sprints em ordem (número, título, tamanho, premissa), os descartes com motivo, os candidatos compartilhados com as análises irmãs, fontes principais, aprendizados, caminho do worktree.
```

---

## 20. Design responsivo e painel retrátil  ·  29/08 16:54

```
Você é o agente de DESIGN do time PhxSql, num worktree isolado do repositório adrianoboller/adrianoboller (projeto em `phxsql/`). Leia o CLAUDE.md da raiz INTEIRO antes de começar — ele vale inteiro, em especial a marca oficial (`phxsql/marca/LEIA-ME.md`: Exo 2, fundo #010418, e as adaptações já decididas), as cores da ação (verde inclui, amarelo altera, rosa marca, vermelho exclui de vez, azul consulta — SEMPRE contorno, nunca fundo cheio) e as três lições de tela já pagas caro: «interface só se prova exercitando», «o CSS global morde todo componente novo» (`input{width:100%}` virou bolinha gigante; `label{text-transform:uppercase}` fez «Blumenau» virar «BLUMENAU», que é MENTIRA SOBRE O DADO) e «componente novo se abre no navegador e se olha».

O Centro de Controle é o `phxsql/crates/phxsql-server/ui/index.html` (UI embutida por `include_str!` — `cargo build --release -p phxsql-server --bin phxsqld` a cada mudança).

## Missão

**1. Revisão de design de TODAS as telas, com sugestões e consertos.** Percorra o console inteiro (painel, bancos, tabelas, grade, query, pivot, junção, importar/exportar, usuários, acessos, conexões, config, jobs, backup, lixeira, transações, DbLink, diagrama ER, LGPD, replicação, profiler, diretivas, ajuda…), nos DOIS temas. Entregue: (a) um documento `phxsql/docs/DESIGN.md` com o sistema visual REAL do console (tokens de cor, escala tipográfica, espaçamentos, os componentes que existem — `.botao`, `.pino`, `.aviso`, `.fichas`, `.rolo`, `.form-dbl`, `.linha-chk`, `table.conf`… — e quando usar cada um), as inconsistências que você achou e o que consertou; (b) os consertos aplicados. Priorize o que é ERRO (ilegível, mordido pelo CSS global, contraste abaixo de 4,5:1, dado deformado por CSS) sobre o que é gosto. Não redesenhe o que já está bom — a marca manda, e a economia de mudança é uma virtude aqui.

**2. Responsividade real: celular, tablet e desktop.** O console hoje assume desktop (`body{overflow:hidden}`, barra de ferramentas de duas linhas, árvore lateral fixa de 268px, grades largas). Entregue um console que serve nos três tamanhos:
   - **Celular (≤ 640px)**: navegação sem árvore fixa (gaveta sobreposta que abre e fecha), barra de ferramentas rolável ou em menu, formulários de uma coluna, grades com rolagem horizontal PRÓPRIA (`overflow-x:auto` no contêiner, o corpo da página NUNCA rola de lado), alvos de toque de pelo menos 40px.
   - **Tablet (641–1024px)**: árvore recolhível, duas colunas onde couber.
   - **Desktop**: como hoje, melhorado.
   Use as unidades relativas e `clamp()` onde ajudar; nada de biblioteca externa (zero dependências vale para a UI também).

**3. O painel lateral esquerdo RETRÁTIL e PINÁVEL** (pedido explícito do Adriano): botão para recolher/expandir; estado «pinado» que mantém o painel fixo, e despinado que o deixa sobrepor e sumir para dar TELA CHEIA ao conteúdo; a escolha é lembrada (localStorage — e a tela diz que é por navegador); atalho de teclado; transição respeitando `prefers-reduced-motion`; e o botão de reabrir SEMPRE visível quando recolhido (painel que some sem volta é armadilha). Largura ajustável por arrasto é bem-vinda se couber com qualidade.

## Prova real (o critério da entrega)

**Interface só se prova exercitando.** Playwright (chromium `/opt/pw-browsers/chromium`, import de `/opt/node22/lib/node_modules/playwright/index.mjs`; exemplos de roteiro em `/tmp/claude-0/-home-user-adrianoboller/34595649-0af6-575a-8f79-80dbe8cb7a5d/scratchpad/*.mjs`, veja `ver-dblink.mjs` para o login) contra um `phxsqld` SEU nas portas **5370 (dados) / 5770 (web)**, com config própria (referência `phxsql/exemplos/Config_exemplo_01.json`) e dados semeados (crie tabelas e linhas para as grades não ficarem vazias). Roteiro mínimo:
- as telas principais capturadas em TRÊS viewports (390×844 celular, 820×1180 tablet, 1440×900 desktop) e nos DOIS temas — e OLHADAS uma a uma;
- o painel retrátil exercitado: recolhe, expande, pina, despina, sobrevive ao recarregar, e o botão de reabrir existe em todos os estados;
- **nenhuma tela pode ter rolagem horizontal do corpo** em 390px: prove medindo `document.documentElement.scrollWidth <= innerWidth` em cada tela e relate a lista;
- contraste: meça os pares texto/fundo dos componentes principais nos dois temas e relate os que ficaram abaixo de 4,5:1 (e conserte).
Pelo menos um defeito real achado exercitando tem de estar no relatório com o conserto — se não achar nenhum, desconfie do exercício. NUNCA mate um phxsqld que não seja o seu (o demo em 5199/5599 e os de outros agentes: confira o `--config` no `ps` antes de matar).

## Fronteiras (outros agentes trabalham em paralelo)

NÃO crie telas novas de funcionalidade: um agente irmão está construindo a tela de TELEMETRIA (SQL Check) em arquivo próprio, e outros mexem em jobs, configuração, DbLink e replicação. Seu terreno é o CSS global, a moldura (cabeçalho, barra, painel lateral, área de conteúdo) e o polimento das telas existentes. Se um componente novo precisar existir para a responsividade, crie-o genérico e documente em DESIGN.md para os outros reusarem.

## Portões antes do commit final

`cargo fmt --all` && `cargo clippy --workspace --all-targets` (ZERO avisos) && `cargo test --workspace` (a UI é embutida: o build tem de passar) && o exercício dos três viewports completo.

## O que NÃO fazer

Não atualize `PENDENCIAS.md`, `CHANGELOG.md` nem `docs/dossie/`. Não abra PR, não publique artifact, não faça push. Não inclua identificador de modelo em nada commitado. Código/comentários/commits em português, identificadores sem acento; comentário explica por quê.

## Entrega

UM commit final no worktree. Relatório: o sistema de design documentado, a lista de consertos (erro × gosto), como ficou a responsividade nos três tamanhos, o painel retrátil/pinável, os defeitos achados exercitando, os números de contraste e de rolagem horizontal, os caminhos das capturas, aprendizados, arquivos tocados, caminho do worktree.
```

---

## 21. Telemetria estilo SQL Check  ·  29/08 16:55

```
Você é o agente de TELEMETRIA do time PhxSql, num worktree isolado do repositório adrianoboller/adrianoboller (projeto em `phxsql/`). Leia o CLAUDE.md da raiz INTEIRO antes de começar — vale inteiro. Sua missão: **a tela de telemetria do banco, no espírito do SQL Check da Idera(R)**, mais o **gestor de threads** e o **encerrar processo problemático**.

## O que o Adriano pediu (com a referência na mão)

Ele mostrou o SQL Check da Idera(R): um painel com faixas de gráficos no topo (Waits empilhados, Physical R/W, CPU Usage, Throughput) e, embaixo, um grande painel de **Processos como BOLHAS**: cada processo é uma bolha com o identificador dentro, **o tamanho proporcional ao peso** dele, **ordenadas por tamanho**, e a cor dizendo o estado: **azul = normal, amarelo = uso alto (CPU, memória ou disco), vermelho = servidor em stress**. Clicar numa bolha abre o **descritivo completo** do que ela é. E deve dar para **parar um processamento que esteja causando problema** — ele lembrou que o MS SQL Server(R) tem o *pid* que permite dar kill numa atividade anormal e perguntou se conseguimos o equivalente. Além disso: **gestor de multithreads** para ver o que roda em background, com **as threads bem documentadas nas suas finalidades**.

## Missão

**1. Backend das métricas.** Descubra e REUSE o que já existe (leia antes de escrever): os monitores de máquina (CPU, memória, discos, rede), o Profiler (o que chega pela porta), a tela de Conexões, as estatísticas do servidor, o cache de páginas do `.ndx`. Entregue uma operação de telemetria (nome à sua escolha, no `catalogo.rs` — há teste que trava operação sem descrição; permissão de administrador; e como é operação sem campo `"tabela"`, portão próprio dentro dela — a lição do juntar/unir) devolvendo, numa amostra: as séries do topo (esperas, leituras/escritas físicas, CPU, vazão de operações, acertos de cache) e a lista de **atividades vivas** — cada uma com identificador estável, usuário, IP, operação, database/tabela, início, duração, estado e o peso que dá o tamanho da bolha. **O caminho quente não pode pagar por isso**: a lição do Profiler no CLAUDE.md é que o portão vem ANTES do trabalho — instrumentação desligada custa zero, e mesmo ligada a coleta tem de ser barata (contadores atômicos, amostragem, nada de `Json::analisar` no caminho da operação).

**2. Encerrar uma atividade — com honestidade sobre o que é possível.** Este é o ponto de arquitetura mais delicado da missão, e a casa tem regra: **nunca comprometer o dado**. Rust não mata thread no meio com segurança, e uma escrita interrompida no meio corromperia arquivo — então a resposta certa é **cancelamento cooperativo**: uma marca por atividade que os laços longos consultam em pontos seguros (entre linhas de uma varredura, entre lotes de uma carga, entre páginas de um `.ndx`), abortando com erro claro e deixando o arquivo consistente. Entregue a op de encerrar (permissão de administrador; registro no log de acessos: quem matou o quê e quando), documente **exatamente o que é cancelável e o que não é** (uma escrita já dentro do ponto crítico termina — e a tela tem de dizer isso ao operador em vez de mentir que matou), e faça a tela mostrar o estado real («encerrando…» → «encerrada» ou «não cancelável nesta fase»). Nunca ofereça um botão que não cumpre o que promete.

**3. Gestor de multithreads.** Levante TODAS as threads que o `phxsqld` cria (atendimento de conexões, laço de réplica, laço de cluster/pulso, jobs, vigias, sincronia de DbLink, o que mais achar) e entregue: um registro central de threads com **nome e FINALIDADE escritos** (o Adriano pediu explicitamente que as threads sejam bem documentadas), estado, início, e o que cada uma está fazendo agora; a tela que as lista; e a documentação em `phxsql/docs/TELEMETRIA.md`. Se achar thread sem dono claro ou que ninguém acompanha, isso é achado de valor — relate.

**4. A tela.** Crie-a em **arquivo próprio** (ex.: `ui/telemetria.js`, servido como o `ui/diagrama-er.js` já é — veja o `http.rs`), para não disputar o `index.html` com os outros agentes; no `index.html` entra só o botão/rota. Conteúdo: as faixas de gráficos no topo e o painel de bolhas embaixo. Regras do desenho:
   - bolhas **ordenadas por tamanho**, tamanho proporcional ao peso, com o identificador dentro; layout que não sobreponha (empacotamento simples escrito à mão — zero dependências, nada de d3);
   - **cores com significado**: azul normal, amarelo alto uso, vermelho stress — e como a casa exige acessibilidade, a cor NÃO pode ser o único sinal (borda/rótulo/ícone acompanham), e as três têm de passar em contraste nos DOIS temas (as cores saem das variáveis do tema; no tema claro elas escurecem, como o vermelhão da marca já faz);
   - clicar na bolha abre o **descritivo completo** da atividade (usuário, IP, operação, alvo, início, duração, estado, o que está esperando) com o botão de encerrar quando aplicável — nas cores da ação, contorno nunca fundo cheio;
   - atualização periódica (o Adriano vai querer saber se há atraso: meça e mostre o instante da última amostra), com pausa/retomada, e sem piscar a tela inteira a cada volta;
   - legenda, como o SQL Check tem.

## Prova real (o critério da entrega)

Servidor SEU nas portas **5372 (dados) / 5772 (web)**, config própria. NUNCA mate um phxsqld que não seja o seu (demo em 5199/5599 e os de outros agentes — confira o `--config` no `ps`).
- **Carga real**: gere atividade de verdade (várias conexões inserindo/varrendo em paralelo, uma consulta longa proposital) e prove que as bolhas aparecem, crescem, mudam de cor e se ORDENAM; capture.
- **Encerrar**: prove que a atividade longa é encerrada de fato (some da lista, o cliente recebe o erro claro) **e que o arquivo continua íntegro depois** (reabrir a tabela, conferir contagem/CRC — a prova que separa cancelamento de estrago).
- **Custo**: meça o servidor com a telemetria desligada e ligada (`cargo build --release --examples -p phxsql-store` antes de medir — binário velho mede o passado) e relate o número; se ligada custar caro, conserte antes de entregar.
- **Atraso da atualização**: meça o tempo entre o evento acontecer e aparecer na tela; relate.
- Pelo menos um teste tem de FALHAR com um defeito reposto (ex.: cancelamento que não confere a marca em ponto seguro, ou a coleta que trabalha antes de olhar o próprio interruptor) — registre qual.
- Exercício no navegador com Playwright (chromium `/opt/pw-browsers/chromium`, import `/opt/node22/lib/node_modules/playwright/index.mjs`), capturas nos dois temas, OLHADAS.

## Fronteiras

Um agente irmão está fazendo o design global e a responsividade (CSS global, moldura, painel lateral) — não mexa no CSS global nem na moldura; sua tela reusa as classes e traz o próprio estilo escopado. Outros agentes mexem em jobs, config, replicação e DbLink — não entre neles.

## Regras da casa

Zero dependências (só `std` no Rust; nada de biblioteca na UI); português; identificadores sem acento; comentário explica por quê; senha/token nunca em telemetria, log ou descritivo de bolha. Antes do commit: `cargo fmt --all` && `cargo clippy --workspace --all-targets` (ZERO avisos) && `cargo test --workspace`.

## O que NÃO fazer

Não atualize `PENDENCIAS.md`, `CHANGELOG.md` nem `docs/dossie/`. Não abra PR, não publique artifact, não faça push. Não inclua identificador de modelo em nada commitado. Cite Idera(R), MS SQL Server(R) com a marca.

## Entrega

UM commit final no worktree. Relatório: as métricas escolhidas e por quê, o desenho do cancelamento cooperativo com a lista do que é e do que NÃO é cancelável, o inventário das threads com finalidade, os números (custo ligado/desligado, atraso da atualização), qual teste falhou com o defeito reposto, as capturas, aprendizados, arquivos tocados, caminho do worktree.
```

---

## 22. Tela de login: idiomas e conexões  ·  29/08 16:59

```
Você é um agente do time PhxSql, num worktree isolado do repositório adrianoboller/adrianoboller (projeto em `phxsql/`). Leia o CLAUDE.md da raiz INTEIRO antes de começar — vale inteiro. Sua missão são dois pedidos do Adriano, os dois na TELA DE LOGIN do Centro de Controle (`phxsql/crates/phxsql-server/ui/index.html`, seção `#entrada` — a UI é embutida por `include_str!`, então `cargo build --release -p phxsql-server --bin phxsqld` a cada mudança).

## Pedido 1 — As bandeiras dos idiomas no login

«Na tela de login ter as bandeiras das linguagens para deixar o ambiente adequado à linguagem escolhida entre: Português, Francês, Inglês, Italiano, Alemão, Espanhol. Fazer a carga da tabela de linguagem e ter backup em caso de desastre e um botão carga padrão caso a tradução não tenha ficado boa.»

Contexto que você PRECISA ler antes: um agente irmão está construindo, em worktree separado, a **tabela de mensagens multilíngue** — colunas `id` (UUID, chave fixa da programação), `TextName` (chave fixa) e `Portugues`, `Frances`, `Ingles`, `Italiano`, `Alemao`, `Espanhol`, mais o campo de idioma no `config.json` (vazio = Português) e a cadeia de resolução (idioma → vazio cai para Português → linha ausente cai para o texto de fábrica). O worktree dele é `/home/user/adrianoboller/.claude/worktrees/agent-af10b6f797860b6a7` — **leia-o (só leitura, NUNCA escreva nele)** para casar os nomes de tabela/colunas/ops. Se a peça dele ainda não estiver pronta quando você precisar, implemente contra o contrato descrito aqui e liste no relatório o que precisa ser conferido na integração.

O que entregar:
- **Seletor de idioma com bandeiras** na tela de login (SVG desenhado à mão — zero dependências, nada de emoji de bandeira, que não desenha em toda plataforma; bandeiras simples e reconhecíveis, com o nome do idioma ao lado e `aria-label`). A escolha vale para a sessão e é lembrada (localStorage — diga na tela que é por navegador).
- **Carga da tabela de idiomas**: operação que semeia/recarrega a tabela de mensagens (uma linha por `TextName` conhecido, `id` UUID, `Portugues` = texto de fábrica). Permissão de administrador; como é op sem campo `"tabela"`, portão próprio dentro dela (a lição do juntar/unir); entrada no `catalogo.rs` (há teste que trava).
- **Backup em caso de desastre**: exportar a tabela de mensagens para arquivo (formato do próprio projeto — JSON escrito à mão) e importar de volta, para o operador guardar as traduções fora do banco.
- **Botão «carga padrão»**: volta os textos de fábrica «caso a tradução não tenha ficado boa» — e aqui a decisão importa: ofereça restaurar **só o idioma escolhido** ou **tudo**, e deixe claro na tela o que será sobrescrito, pedindo confirmação (a casa não aceita botão que apaga trabalho em silêncio — é a mesma lição do merge de conflito que marca quem MEXEU).

## Pedido 2 — O histórico de conexões no login, como no HFSQL(R)

«Na tela de login adicionar um grid do histórico das conexões igual do HFSQL(R) para escolher, que possa dar um nome pra conexão assim: base da farmácia, base do açougue… facilitando com esse nome a conexão. Esse grid pode ser retrátil para não estragar o visual ou simplificar a tela.»

- Grade das conexões já usadas, cada uma com **apelido dado pelo usuário** (o texto que ele quiser: «base da farmácia»), endereço/porta, usuário e quando foi usada por último; clicar preenche o login; dá para renomear e remover uma entrada.
- **Retrátil**: fechada por padrão (não estraga o visual da tela de entrada), abre num toque, e o estado é lembrado.
- Onde guardar: **no navegador** (localStorage), nunca no servidor — é o histórico de quem senta na máquina. E a regra que não se quebra: **senha nunca em texto puro**, nem aqui. Guarde apelido, host, porta, usuário e data; NUNCA a senha nem o token, e escreva na tela que a senha não é guardada. (Se você achar que vale oferecer «lembrar usuário», tudo bem; senha, não.)

## Prova real (o critério da entrega)

Servidor SEU nas portas **5374 (dados) / 5774 (web)**, config própria (referência `phxsql/exemplos/Config_exemplo_01.json`). NUNCA mate um phxsqld que não seja o seu (o demo em 5199/5599 e os de outros agentes — confira o `--config` no `ps` antes de matar).
Playwright (chromium `/opt/pw-browsers/chromium`, import `/opt/node22/lib/node_modules/playwright/index.mjs`; exemplos em `/tmp/claude-0/-home-user-adrianoboller/34595649-0af6-575a-8f79-80dbe8cb7a5d/scratchpad/*.mjs`):
- escolher um idioma no login e ver o ambiente naquele idioma (ao menos as mensagens que a tabela cobre); trocar e ver mudar;
- semear a tabela, editar uma tradução, exportar, estragar de propósito, importar o backup e conferir que voltou;
- «carga padrão» restaurando os textos de fábrica com a confirmação aparecendo antes;
- salvar duas conexões com apelidos («base da farmácia», «base do açougue»), fechar o navegador (novo contexto), reabrir e clicar numa: os campos preenchem, e **grep no localStorage prova que não há senha guardada**;
- o grid retrátil abrindo/fechando e o estado sobrevivendo ao recarregar; capturas nos dois temas, OLHADAS.
Pelo menos um defeito achado exercitando no relatório, com o conserto. Pelo menos um teste tem de FALHAR com um defeito reposto (ex.: fallback de idioma devolvendo texto vazio, ou a senha vazando para o localStorage) — registre qual.

## Fronteiras

Um agente irmão faz o **design global e a responsividade** (CSS global, moldura, painel lateral): não mexa no CSS global — seu estilo é escopado à tela de entrada, e o seletor/grade têm de funcionar em celular também (alvos de toque grandes). Outro faz a **telemetria** em arquivo próprio. Não entre nos worktrees deles.

## Regras da casa

Zero dependências (Rust só `std`; UI sem biblioteca); português; identificadores sem acento; comentário explica por quê; senha nunca em arquivo, log, resposta ou localStorage. Antes do commit: `cargo fmt --all` && `cargo clippy --workspace --all-targets` (ZERO avisos) && `cargo test --workspace`.

## O que NÃO fazer

Não atualize `PENDENCIAS.md`, `CHANGELOG.md` nem `docs/dossie/`. Não abra PR, não publique artifact, não faça push. Sem identificador de modelo em nada commitado. Cite HFSQL(R) com a marca.

## Entrega

UM commit final no worktree. Relatório: como ficaram os dois pedidos, as decisões (o que a «carga padrão» sobrescreve; o que o histórico guarda e o que recusa guardar), o que depende do agente da tabela de mensagens (lista exata para a integração), o defeito achado exercitando, qual teste falhou com o defeito reposto, capturas, aprendizados, arquivos tocados, caminho do worktree.
```

---

## 23. Integração com a Claude no console  ·  29/08 18:00

```
Você é um agente do time PhxSql, num worktree isolado do repositório adrianoboller/adrianoboller (projeto em `phxsql/`). Leia o CLAUDE.md da raiz INTEIRO antes de começar — vale inteiro. Sua missão: **integrar a Claude (API da Anthropic) ao Centro de Controle**, com a configuração no menu Configurações e o uso na tela de Query.

## A decisão de arquitetura JÁ FOI TOMADA pelo Adriano — não a reabra

A API da Claude é **HTTPS obrigatório** e a `std` do Rust **não tem TLS**. Das três saídas possíveis, o Adriano escolheu: **a chamada sai DIRETO DO NAVEGADOR** (`fetch` da própria tela). Consequências que o seu desenho tem de respeitar:

- **O servidor PhxSql nunca vê a chave da API, nunca faz a chamada, e não precisa de TLS.** Nenhuma linha de Rust fala com a Anthropic.
- A chave mora **no navegador de quem usa** (`localStorage`), e é dele. Cada pessoa usa a própria chave.
- **Regra da casa que vale aqui inteira**: senha nunca em texto puro em arquivo, log ou resposta do protocolo. A chave da API é segredo do mesmo naipe: ela **não pode** ser enviada ao servidor em nenhum pedido, aparecer em nenhum log do servidor, nem ser gravada em nada que o servidor escreva. Escreva um teste que PROVA isso (o agente da tela de login fez igual: grep no localStorage e no que trafega).
- Na tela, o campo da chave é `type=password`, mostra só os últimos 4 caracteres depois de salva, e tem botão de remover.

## O contrato da API (confirmado na referência oficial — não invente)

- Endpoint: `POST https://api.anthropic.com/v1/messages`
- Cabeçalhos: `content-type: application/json`, `x-api-key: <a chave>`, `anthropic-version: 2023-06-01`
- Modelo: **`claude-opus-5`** (string exata, sem sufixo de data). Deixe o modelo **escolhível** na configuração, com esse como padrão; ofereça também `claude-sonnet-5` e `claude-haiku-4-5` para quem quiser gastar menos, dizendo na tela que a escolha é de custo.
- `max_tokens`: ~4000 basta para SQL e explicação; se usar streaming pode subir.
- **Streaming (recomendado)**: `"stream": true` e leitura do SSE (`event: content_block_delta` traz os pedaços em `delta.text`) — a resposta aparece enquanto sai, em vez de a tela ficar parada. Trate também `message_stop`.
- **NÃO use prefill de assistente** (é rejeitado com 400 nos modelos atuais) — para controlar o formato, instrua no `system`.
- `thinking`: pode omitir. Se usar, é `{"type":"adaptive"}` — **`budget_tokens` foi removido e devolve 400**.
- **CORS**: chamada de navegador para a API exige um cabeçalho próprio de acesso direto do navegador. **CONFIRME o nome exato dele na documentação oficial** (WebFetch em `https://docs.anthropic.com/en/api/` ou na página de CORS/browser da Anthropic) **antes de escrever** — não escreva de memória, e se a documentação disser que não é suportado, relate isso em vez de inventar contorno. Se o ambiente não deixar buscar a doc, implemente com o cabeçalho que a doc oficial indicar quando você conseguir ler; se não conseguir, diga no relatório que ficou por confirmar e deixe o ponto isolado numa constante única no código.
- Erros: trate e mostre com clareza **401** (chave inválida/ausente), **429** (limite de uso), **5xx/529** (sobrecarga — sugira tentar de novo), e falha de rede. Mensagem que diz o que fazer, não "erro".
- **Custo é do usuário**: mostre na tela os tokens consumidos de cada chamada (`usage.input_tokens` / `usage.output_tokens`) — quem paga tem direito de ver.

## O que construir

**1. Menu Configurações → «Integração com a Claude»** (item novo): a chave, o modelo, ligar/desligar, e o texto que explica com honestidade o que acontece: *a chave fica neste navegador, as perguntas e o que você mandar de contexto vão para a Anthropic, e o servidor PhxSql não participa*. Botão «Testar» que faz uma chamada mínima e diz se a chave funciona.

**2. Tela de Query — os quatro recursos que o Adriano pediu:**
   - **Texto → SQL**: descreve em português, recebe o SQL. **O SQL gerado NUNCA executa sozinho** — cai no editor para revisão, e quem aperta Executar é a pessoa. Escreva isso na tela.
   - **Explicar o SQL**: cola a consulta, recebe a explicação em português.
   - **Sugerir índice / desempenho**: a partir da consulta e do esquema, sugere índices ou reescritas — e diz que é sugestão a medir, não verdade (a casa mede antes de aceitar receita de fora).
   - **Ajudar a modelar tabelas**: descreve o negócio, recebe proposta de tabelas/colunas/relacionamentos **no vocabulário do PhxSql** (os tipos reais do motor — Int4, Str(n), Decimal{precisao,escala}, Date, Memo, Bin, Uuid…, chaves e índices `.ndx`). Leia `docs/FORMATO.md` e o esquema real para o `system` ensinar os tipos certos — proposta com tipo que não existe aqui é lixo.

**3. O contexto que vai junto — e o limite de privacidade, que é INEGOCIÁVEL:**
   - Mandar o **ESQUEMA** (nomes de tabelas, colunas, tipos, chaves) é o que faz a IA acertar. O servidor já sabe responder isso (veja as ops de esquema/SysTables/catálogo).
   - **Dados de linhas NÃO viajam por padrão.** Se você oferecer «mandar N linhas de exemplo», tem de ser opt-in explícito por chamada, com aviso na cara de que o dado sai da máquina. O projeto tem tela de LGPD e uma regra dura sobre dado pessoal: o padrão é **não vazar**.
   - Mostre ANTES de enviar (ou num painel «o que foi enviado») exatamente o que sobe. Ninguém deve descobrir depois que mandou o esquema inteiro do cliente para fora.

## Regras da casa que mordem aqui

- **Zero dependências também na UI**: `fetch` nativo, nada de SDK, nada de biblioteca — o console funciona offline e nenhum script externo entra (a CSP da página e o hábito da casa).
- **O CSS global morde** (`input{width:100%}`, `label{text-transform:uppercase}`): reuse `form-dbl`, `linha-chk`, `pino`, `aviso`, `aviso.bom`. Cores da ação: azul consulta, verde inclui — **sempre contorno**. Confira nos DOIS temas.
- Português na interface e nos comentários; identificadores sem acento; comentário explica **por quê**.
- Se precisar de op nova no servidor (por exemplo, uma que devolva o esquema num formato bom para o prompt), entrada no `catalogo.rs` (há teste que trava) e portão próprio se a op não tiver campo `"tabela"` (a lição do juntar/unir). Mas prefira **reusar** o que já existe.

## Fronteiras (três agentes trabalham em paralelo)

Um faz o **design global e a responsividade** (CSS global, moldura, painel lateral), outro a **telemetria** (arquivo próprio), outro a **revisão das telas de configuração**. Para não brigar pelo `index.html`: ponha a lógica da integração em **arquivo próprio** (ex.: `ui/claude.js`, servido como `ui/diagrama-er.js` já é — veja o `http.rs`), e no `index.html` deixe só o item de menu e o botão da tela de Query. Não mexa no CSS global.

## Prova real (o critério da entrega)

Servidor SEU nas portas **5376 (dados) / 5776 (web)**, config própria. **NUNCA mate um phxsqld que não seja o seu** — há outros agentes com servidores no ar; case o padrão pelo caminho do SEU worktree ou mate só pelo PID que você criou (um agente já derrubou os dos outros com um `pkill` largo demais).
- **Sem chave da API você não consegue chamar a Anthropic de verdade — e não invente uma.** Prove o que dá para provar sem ela: (a) um **servidor falso** local que fale o formato da API (inclusive o SSE) e responda como a Anthropic responderia, com a tela apontada para ele por uma constante de endereço — assim o caminho inteiro (montar o pedido, ler o streaming, mostrar o SQL, mostrar tokens) é exercitado de ponta a ponta; (b) os erros 401/429/5xx encenados pelo servidor falso e a tela reagindo a cada um; (c) o teste que prova que **a chave nunca sai do navegador** (grep no que o servidor recebeu e no log dele); (d) o teste do comportamento velho: **sem chave configurada, a tela de Query funciona exatamente como hoje** — os botões da IA não aparecem ou dizem o que falta, e nada mais muda.
- Playwright (chromium `/opt/pw-browsers/chromium`, import de `/opt/node22/lib/node_modules/playwright/index.mjs`; exemplos em `/tmp/claude-0/-home-user-adrianoboller/34595649-0af6-575a-8f79-80dbe8cb7a5d/scratchpad/*.mjs`), capturas nos dois temas, OLHADAS.
- Pelo menos um teste tem de FALHAR com um defeito reposto (candidato natural: a chave escapando para o servidor, ou o SQL gerado executando sozinho) — registre qual.
- A UI é `include_str!`: `cargo build --release -p phxsql-server --bin phxsqld` antes de cada exercício.

## Portões antes do commit final

`cargo fmt --all` && `cargo clippy --workspace --all-targets` (ZERO avisos) && `cargo test --workspace` && as provas acima.

## O que NÃO fazer

Não atualize `PENDENCIAS.md`, `CHANGELOG.md` nem `docs/dossie/`. Não abra PR, não publique artifact, não faça push. Não inclua identificador de modelo em nada commitado **no repositório** — a exceção óbvia é o ID de modelo da API (`claude-opus-5`), que é configuração do produto e precisa estar no código. Documente tudo em `phxsql/docs/CLAUDE-IA.md`: o desenho, por que a chamada sai do navegador (o obstáculo do TLS), o que viaja e o que não viaja, os limites e os aprendizados.

## Entrega

UM commit final no worktree. Relatório: o que ficou pronto, o cabeçalho de CORS que a documentação confirmou (ou o que ficou por confirmar), o que viaja para a Anthropic e o que você barrou, qual teste falhou com o defeito reposto, as capturas, aprendizados, arquivos tocados, caminho do worktree.
```

---

## 24. Trilha LGPD no arquivo .lgpd  ·  29/08 18:51

```
Você é um agente do time PhxSql, num worktree isolado do repositório adrianoboller/adrianoboller (projeto em `phxsql/`). Leia o CLAUDE.md da raiz INTEIRO antes de começar — vale inteiro. Sua missão é **estrutural**: o arquivo **`.lgpd`**, a trilha de auditoria das colunas marcadas como dado sensível.

## O pedido do Adriano, nas palavras dele

«Todas as colunas ter um atributo LGPD (x): se marcado é um dado sensível e deve guardar no arquivo `.lgpd` quando — data e hora, registro único, valor antes, valor depois, IP e quem acessou ou alterou. **No insert e delete e soft delete não precisa.**»

## O que JÁ existe — não crie um segundo atributo

A marca de coluna **já está no formato**: PSCH **v6**, um byte por coluna no fim do bloco, com três graus — `0 nao`, `1 pessoal`, `2 sensivel` (`docs/FORMATO.md` §"A marca de dado pessoal (LGPD / GDPR), v6"). Já há tela de LGPD no console. **Reuse isso.** Um atributo novo ao lado do que existe criaria duas verdades sobre a mesma coluna, e a que alguém esquecesse de marcar viraria o vazamento que ninguém acha por leitura.

O que falta é a **trilha**: hoje a marca classifica e não registra nada.

## O desenho

**1. O arquivo `.lgpd`** — mais um diário append-only por tabela, ao lado do `.log`, `.trash` e `.reason`. Siga o desenho deles (leia os três antes de escrever o seu): cabeçalho com assinatura e versão, registros de tamanho conhecido, CRC, e a regra de que arquivo ausente = tabela sem trilha, nunca erro. Formato em disco novo → **`docs/FORMATO.md` no mesmo commit**, e "mudança de formato entra cedo" é a regra: agora é barato.

**2. O que cada registro guarda** (o que ele pediu): quando (data e hora), **qual linha** (o identificador único — decida entre rowid e a chave, e diga por quê; se for rowid, lembre que ele é local do servidor), **qual coluna**, **valor antes**, **valor depois**, **IP** e **quem** (o login). O `.reason` já resolveu o problema de "identidade da linha + quem + quando": copie a solução dele em vez de inventar outra.

**3. Quando grava, e quando NÃO grava.** Ele foi explícito: **insert, delete e soft delete não precisam**. Respeite — e escreva no doc **por que** isso é coerente, não só que foi pedido: o `.log` já registra toda inclusão e exclusão com data e hora, o `.trash` guarda a linha inteira antes de sumir e o `.reason` guarda quem excluiu e por quê. A trilha nova cobre o que **falta**: a alteração (com antes e depois por coluna) e o **acesso**.

**4. O acesso — e aqui está a decisão difícil, que é sua.** "Quem acessou" significa registrar leitura de coluna sensível. Uma varredura de 10.000 linhas com uma coluna sensível não pode virar 10.000 registros de trilha: isso multiplicaria o arquivo e o tempo da consulta. **Proponha o desenho, meça o custo e defenda com número** — o caminho que eu apostaria é registrar **por operação** (quem, quando, IP, quais tabelas e colunas sensíveis foram lidas, quantas linhas) e não por linha, porque é isso que um auditor pergunta ("quem viu o prontuário do fulano?" continua respondível se a operação registrar o filtro/chave usado). Se você achar caminho melhor, use — mas meça os dois e mostre a conta.

**5. Custo zero quando não há coluna sensível.** É a lição do Profiler, que o CLAUDE.md registra: o portão vem ANTES do trabalho. Tabela sem nenhuma coluna marcada não abre arquivo, não monta registro, não paga nada além de um teste barato. **Meça e prove** (`cargo build --release --examples -p phxsql-store` antes de medir — binário velho mede o passado).

**6. O perigo que este arquivo cria, e que você tem de tratar.** O `.lgpd` guarda **valores sensíveis em claro** — é, por construção, o arquivo mais perigoso da tabela: concentra exatamente o que a LGPD manda proteger. Então:
   - permissão restrita no disco (`0600`), como o cadastro de ligações já faz;
   - **cifra**: o projeto já tem ChaCha20-Poly1305 (RFC 8439, vetores oficiais) ligada aos diários, opt-in por `config.json` — o `.lgpd` entra nessa mesma chave e no mesmo interruptor, e **diga no doc** que guardar trilha de dado sensível em claro é risco de vazamento concentrado;
   - **senha nunca em texto puro**: se uma coluna marcada como sensível guardar senha ou hash, o valor **não** vai para a trilha — grave o tamanho ou uma marca de "redigido", nunca o conteúdo. É a regra da casa e o corolário do Profiler: **redigir ANALISANDO, nunca recortando**;
   - quem pode LER a trilha: administrador. E a leitura da trilha **também** é acesso a dado sensível — decida se ela se registra a si mesma (e o que isso implica de recursão) e documente a escolha.

**7. Ligar/desligar.** «Guarda nova entra pedida, não imposta» é a regra da casa — mas aqui há uma exigência legal, e o Adriano pediu que grave. O caminho: nasce **ligada para colunas `sensivel`** (é o pedido), com interruptor no `config.json` para quem precise desligar, e o grau `pessoal` (grau 1) só entra se ele pedir — proponha, não imponha. E o teste que mais importa é o do **comportamento velho**: tabela sem coluna marcada, e arquivo gravado antes desta rodada, abrem e operam **exatamente** como hoje.

**8. Na tela**: a LGPD já tem tela — mostre a trilha lá (quem alterou o quê, quando, de que IP), com a marca de coluna editável se ainda não for. Cores da ação, contorno nunca fundo cheio; o CSS global morde (`input{width:100%}`, `label` maiúsculo) — reuse `form-dbl`, `linha-chk`, `pino`, `aviso`. Confira nos dois temas.

## Prova real (o critério da entrega)

Servidor SEU nas portas **5378 (dados) / 5778 (web)**, config própria. **NUNCA mate um phxsqld que não seja o seu** — case o padrão pelo caminho do SEU worktree ou mate pelo PID que criou (um agente já derrubou os dos outros com um `pkill` largo demais).
Roteiro mínimo, com o esperado escrito ANTES: (a) tabela com coluna sensível — alterar grava antes/depois, com IP e quem; (b) **insert, delete e soft delete NÃO gravam** (conte os registros e prove que não mexeu); (c) leitura registra pelo desenho que você escolheu; (d) tabela sem coluna sensível — nenhum arquivo criado, custo medido ~zero; (e) valor de coluna que guarda senha **não aparece** na trilha (grep no arquivo); (f) com a cifra ligada, o `.lgpd` no disco não entrega o valor a um `grep`; (g) comportamento velho intacto. Pelo menos um teste tem de **falhar com o defeito reposto** — candidato natural: o portão do custo-zero removido, ou o insert voltando a gravar. Tela exercitada com Playwright (chromium `/opt/pw-browsers/chromium`), capturas olhadas.

## Portões antes do commit final

`cargo fmt --all` && `cargo clippy --workspace --all-targets` (ZERO avisos) && `cargo test --workspace` && as provas acima. Aprendizados (frutíferos e infrutíferos) no doc da área — `docs/LGPD.md` se existir, ou crie.

## O que NÃO fazer

Não atualize `PENDENCIAS.md`, `CHANGELOG.md` nem `docs/dossie/`. Não abra PR, não publique artifact, não faça push. Sem identificador de modelo em nada commitado. Zero dependências, só `std`. Português, identificadores sem acento, comentário explica por quê.

## Entrega

UM commit final no worktree. Relatório: o formato do `.lgpd` (com o porquê de cada campo), a decisão sobre o registro de ACESSO com os dois custos medidos, o que você barrou de ir para a trilha (senha/hash) e como, o custo medido com e sem coluna sensível, qual teste falhou com o defeito reposto, as capturas, aprendizados, arquivos tocados, caminho do worktree.
```

---

## 25. Restaurar backup  ·  29/08 19:21

```
Você trabalha no PhxSql (motor de dados em Rust, modelo de arquivos separados), em `phxsql/`. LEIA `CLAUDE.md` na raiz do repositório ANTES de tudo e siga as regras da casa à risca.

Regras que mais pegam nesta frente:
- **Zero dependências externas. Só a `std`.** Se algo parecer exigir uma crate, NÃO acrescente — resolva com a `std` ou pare e escreva no relatório por quê.
- **A ordem de digitação é sagrada:** o `.reg` nunca reaproveita slot excluído.
- **Guarda nova entra pedida, não imposta:** nada pode quebrar cliente antigo nem arquivo antigo. O teste que mais importa é o do comportamento VELHO.
- **Toda bateria de teste tem prova real:** o teste novo tem de FALHAR com o defeito reposto e passar com o conserto. Teste que passa por engano é pior que teste que falta.
- Código, comentários e mensagem de commit em **português**; identificadores e comentários **sem acento**; comentário diz **por que**, não o quê.
- Portões antes de commitar: `cargo fmt --all`, `cargo clippy --workspace --all-targets` com **zero avisos**, `cargo test --workspace` verde.
- Se subir servidor, use portas na faixa **6100–6149**. Mate só o SEU processo, pelo PID que você anotou. **Nunca `pkill -f`** — isso já matou o servidor de outros agentes nesta máquina.
- **Não faça push e não abra PR.** Commite no seu branch de worktree; a integração é minha.
- Ao terminar tudo, rode `rm -rf phxsql/target` para liberar disco (está apertado).

## A sua tarefa: RESTAURAR UM BACKUP

Hoje o backup existe e a restauração NÃO. Veja `phxsql/crates/phxsql-server/ui/index.html` por volta da linha 5030 (`verBackupRestaure`): o botão "Restaurar" é um `afazer` que abre a tela do "ainda não existe", e o texto de lá já enuncia o problema em aberto:

> "com o servidor no ar e a trava tomada, precisa de um desenho — parar o serviço, restaurar ao lado e trocar, ou restaurar com outro nome"

O dono do projeto acabou de reclamar exatamente disso: **"falta o botão restaurar"**. Backup que não restaura não é backup.

O que fazer:

1. **Leia primeiro** como o backup grava hoje (procure a operação de backup no servidor, o manifesto SHA-256 e a conferência), e `phxsql/docs/FORMATO.md`. Entenda os sete arquivos por tabela mais `.pag`/`.bkp` antes de escrever uma linha.

2. **Escolha o desenho e justifique no comentário.** As três saídas que a tela já cita são: (a) parar o serviço e trocar por cima; (b) restaurar ao lado e trocar os diretórios; (c) restaurar com OUTRO nome de database. Minha recomendação, que você deve conferir e pode contrariar com motivo: **(c) como caminho principal** — restaurar com outro nome não precisa da trava global nem de parar o serviço, não destrói nada, e transforma "restaurar" numa operação segura de se errar; e **(b) como o "restaurar por cima"**, com o serviço de dados parado (a interface web continua no ar, é assim que o Start/Stop já funciona). Não implemente (a) sem necessidade.

3. **Confira antes de escrever.** A restauração TEM de validar o manifesto SHA-256 do backup antes de tocar em qualquer arquivo de destino. Backup corrompido não pode virar database restaurado pela metade.

4. **Operação no protocolo** com nome coerente com as que já existem, passando pelo portão de permissão único (`despachar`). Atenção à lição da casa: **quando o portão olha um campo, procure quem não tem esse campo** — se a sua operação recebe o database de destino num campo diferente de `"tabela"`/`"database"`, ela precisa da conferência própria dentro dela.

5. **Tela**: substitua o `afazer` por uma tela que funcione — escolher o backup, ver o que ele contém, escolher restaurar com outro nome ou por cima, e o aviso claro do que vai acontecer. Siga a convenção de cores da casa (verde inclui, amarelo altera, rosa marca, vermelho exclui de vez, azul consulta; sempre CONTORNO, nunca fundo cheio) e cuide do tema claro.

6. **E ponha o botão onde se acha.** Acabei de consertar um caso em que a tela existia e ninguém achava o botão: hoje "Backup" está na barra de ferramentas e "Restaurar" não está em lugar nenhum além do interior da tela de backup. Coloque a restauração no menu **Arquivo** (junto de "Conferir um backup…") e avalie se merece botão na barra.

7. **Prove exercitando**, não só por teste unitário:
   - testes Rust cobrindo: restaurar com outro nome cria o database íntegro; manifesto adulterado é RECUSADO (reponha o defeito: mude um byte e veja o teste falhar sem a conferência); restaurar por cima com o serviço no ar é recusado com erro claro; e o comportamento VELHO — quem não usa a operação nova não vê diferença nenhuma.
   - navegador com Playwright (`/opt/node22/lib/node_modules/playwright/index.mjs`, Chromium já instalado, NÃO rode `playwright install`), subindo um servidor seu na faixa 6100–6149, criando um banco com dado, fazendo backup, restaurando com outro nome e CONFERINDO os dados restaurados na tela. Guarde as capturas em `/tmp/claude-0/-home-user-adrianoboller/34595649-0af6-575a-8f79-80dbe8cb7a5d/scratchpad/restaurar/` (nos dois temas).
   - lembre que a interface é `include_str!`: depois de mexer no `ui/index.html` é preciso `cargo build --release -p phxsql-server --bin phxsqld` antes de subir o servidor, senão você exercita a página velha.

8. **Documente**: atualize `phxsql/docs/` (o documento da área; crie `RESTAURACAO.md` se não houver lugar melhor) com o desenho escolhido, o que a restauração garante e o que ela NÃO garante. Se alguma hipótese sua morrer no caminho, escreva a recusa com o número — na casa, hipótese infrutífera medida é resultado válido.

No relatório final, diga em texto corrido: o desenho escolhido e por quê, quais defeitos você repôs para provar cada teste, o que a restauração não faz, e onde ficaram as capturas.
```

---

## 26. Gráfico bolha no molde do SQL Check  ·  29/08 19:22

```
Você trabalha no PhxSql, em `phxsql/`. LEIA `CLAUDE.md` na raiz ANTES de tudo e siga as regras da casa.

Regras que mais pegam nesta frente:
- **Zero dependências externas** — na interface isso quer dizer: nada de D3, nada de biblioteca de gráfico, nada de CDN. O desenho é SVG/Canvas escrito à mão, como o resto da tela já faz.
- **O CSS global morde todo componente novo da tela.** `input{width:100%}` e `label{text-transform:uppercase}` são certos num formulário e errados dentro de uma tabela — a folha da telemetria já é escopada em `.tlm` justamente por isso. Mantenha o escopo.
- **Interface só se prova exercitando** — abrir no navegador e OLHAR. Ler o código não acha o que este pedido está pedindo.
- Português, identificadores e comentários **sem acento**, comentário diz **por que**.
- Portões: `cargo fmt --all`, `cargo clippy --workspace --all-targets` zero avisos, `cargo test --workspace` verde.
- Portas na faixa **6150–6199**. Mate só o SEU processo pelo PID. **Nunca `pkill -f`.**
- **Sem push, sem PR.** Commite no seu branch de worktree.
- Ao terminar, `rm -rf phxsql/target` (disco apertado).

## A sua tarefa: O GRÁFICO BOLHA TEM DE FICAR NO MOLDE DO SQL CHECK

O dono do projeto pediu a tela de telemetria "igual ou similar ao SQL Check da Idera(R)", com "gráficos bolha que ficam ordenados por tamanho, cores azul normal, amarelo alto uso de cpu, memória e disco, vermelho stress do servidor", podendo "parar um processamento que esteja causando um problema" e "visualizar só clicar nas bolhas um descritivo completo".

Ele acabou de olhar o que entregamos e disse: **"O gráfico do idera SQL check é diferente precisa melhorar"**.

Ele está certo. O estado de hoje (`phxsql/crates/phxsql-server/ui/telemetria.js` e `telemetria.css`, tela ligada em `index.html` na função `telaTelemetria`): o painel de atividades é uma caixa grande e quase VAZIA, com uma bolha média no meio e uma minúscula ao lado, rótulo pequeno, sem legenda, sem eixo, sem empacotamento. Parece um esboço, não o painel de um monitor de servidor.

O que o SQL Check faz e nós não:
- as bolhas **ocupam o painel** — empacotamento apertado, não duas bolinhas perdidas num retângulo;
- **ordenadas por tamanho** de forma visível (a maior domina a vista; a ordem se lê);
- **tamanho mínimo legível** — bolha pequena demais não mostra rótulo nem se clica;
- **cor por estado** com faixas declaradas (azul normal / amarelo uso alto de CPU, memória ou disco / vermelho estresse), e a faixa tem de estar ESCRITA numa legenda, não adivinhada;
- **rótulo dentro da bolha** quando cabe, e fora ou em tooltip quando não cabe;
- **painel de detalhe** ao clicar, com o descritivo completo da atividade;
- o conjunto **respira**: quando há uma atividade só, ela não pode ficar sozinha num vazio de mil pixels — o painel se ajusta.

O que fazer:

1. **Leia primeiro** `phxsql/docs/TELEMETRIA.md`, `ui/telemetria.js` e `ui/telemetria.css` inteiros. O módulo já foi desenhado para não falar com o servidor por conta própria — ele recebe `api` e um retrato. Use isso: você consegue exercitar o desenho com um retrato INVENTADO, com 1, 3, 12 e 40 atividades de pesos muito diferentes, sem precisar de carga real. Faça isso antes de mexer, para ver o defeito com os próprios olhos.

2. **Reescreva o empacotamento das bolhas.** Sugestão que você deve avaliar e pode contrariar com motivo: empacotamento por círculos com raio proporcional à RAIZ do peso (área proporcional ao peso — raio proporcional ao peso mente sobre a proporção), ordenado do maior para o menor, colocado em espiral ou em faixa a partir do centro, com raio mínimo e máximo em função da caixa. Escreva no comentário por que a raiz, senão alguém "conserta" isso depois.

3. **Faixas de cor declaradas.** Escreva os limiares no código UMA vez, use-os no desenho e na legenda, e documente-os em `TELEMETRIA.md`. Número que aparece na tela e número que decide a cor têm de sair da mesma constante.

4. **Responsivo.** A tela tem de servir em celular, tablet e desktop — foi pedido explicitamente. Painel estreito não pode virar bolhas ilegíveis nem barra de rolagem horizontal.

5. **Os dois temas.** Verifique claro e escuro. A marca manda: fundo `#010418`, e no tema claro as cores escurecem por contraste (verde e rosa claros não passam de 4,5:1 sobre papel). Confira contraste do rótulo DENTRO da bolha nos dois temas — é o ponto onde este tipo de gráfico costuma falhar.

6. **Não quebre o que funciona**: o encerrar cooperativo de uma atividade, o gestor de threads e as faixas de série do topo já existem e foram provados. Se você mexer neles, prove de novo.

7. **Prove exercitando, com captura.** Playwright em `/opt/node22/lib/node_modules/playwright/index.mjs` (Chromium já instalado; NÃO rode `playwright install`). Suba um servidor seu na faixa 6150–6199, gere carga de verdade para ter atividades vivas, e capture: 1 atividade, várias atividades, uma atividade em amarelo, uma em vermelho, o painel de detalhe aberto, nos dois temas, e em largura de celular. Guarde em `/tmp/claude-0/-home-user-adrianoboller/34595649-0af6-575a-8f79-80dbe8cb7a5d/scratchpad/bolhas/`. Lembre que a interface é `include_str!`: `cargo build --release -p phxsql-server --bin phxsqld` depois de cada mudança em `ui/`, senão você exercita a página velha.

8. **Documente** em `TELEMETRIA.md` o que mudou no desenho e por quê, incluindo o que você decidiu NÃO copiar do SQL Check e o motivo.

Observação honesta que você deve levar em conta: eu não tenho mais as capturas do SQL Check que o dono mandou — elas saíram do meu contexto. Trabalhe pelo que está escrito acima e pelo que você souber do produto; se alguma decisão depender de detalhe visual que só a referência resolveria, IMPLEMENTE a sua melhor leitura e ANOTE no relatório qual detalhe ficou em aberto, para eu perguntar a ele.

No relatório final: o que estava errado no desenho antigo (com o que você viu na captura), o que mudou, quais decisões de desenho você tomou e por quê, o que ficou em aberto por falta da referência, e onde ficaram as capturas.
```

---

## 27. Bateria de testes backend e frontend  ·  29/08 19:22

```
Você trabalha no PhxSql, em `phxsql/`. LEIA `CLAUDE.md` na raiz ANTES de tudo e siga as regras da casa.

Regras que mais pegam nesta frente:
- **Toda bateria de testes tem prova real e aprendizado documentado — frutífero ou infrutífero.** Prova real é nos dois sentidos: o teste novo tem de FALHAR com o defeito reposto e passar com o conserto. Já houve teste que passava por engano nesta casa, e ele é pior que teste que falta.
- **Teste unitário não prova queda de conexão — soquete prova.** O que depende do sistema operacional se prova contra o sistema operacional.
- **Interface só se prova exercitando.** Gravar a tela achou três defeitos em cinco minutos que ler o código não acharia.
- Zero dependências além da `std`. Português, identificadores e comentários sem acento.
- Portões: `cargo fmt --all`, `cargo clippy --workspace --all-targets` zero avisos, `cargo test --workspace` verde.
- Portas na faixa **6200–6249**. Mate só o SEU processo pelo PID. **Nunca `pkill -f`.**
- **Sem push, sem PR.** Commite no seu branch de worktree.
- Ao terminar, `rm -rf phxsql/target`.

## A sua tarefa: A BATERIA DE TESTES DO BACKEND E DO FRONTEND, E A AVALIAÇÃO DO DESIGN

O dono do projeto pediu, na letra C da lista dele: "Bateria de testes backend e frontend e avaliação do design".

Hoje o projeto tem ~1.106 testes no `cargo test --workspace` e provas de navegador espalhadas em scripts de rascunho que NÃO são versionados — ou seja, o frontend não tem bateria que rode sozinha. É esse o buraco.

O que fazer:

1. **Levante o que já existe.** Rode `cargo test --workspace` e mapeie a cobertura por área (motor, servidor, protocolo, replicação, cluster, jobs, triggers, procedures, LGPD, telemetria, configuração, mensagens). Diga com número, não com impressão, onde está rala. **Número citado é número que não se mede** — meça.

2. **Ache os buracos que importam** e escreva os testes que faltam, cada um com o defeito reposto provando que ele pega mesmo. Priorize por risco, não por facilidade: perda de dado, vazamento de credencial, portão de permissão, comportamento velho quebrado por guarda nova. Se você achar um caminho que hoje passa por engano, esse é o achado mais valioso da frente — persiga.

3. **Crie a bateria de FRONTEND que roda sozinha.** Hoje não há. Use Playwright (`/opt/node22/lib/node_modules/playwright/index.mjs`; Chromium já instalado, NÃO rode `playwright install`). Faça dela algo versionado e repetível, dentro de `phxsql/`, com um jeito claro de rodar (documente o comando). Ela precisa, no mínimo:
   - subir um servidor próprio numa porta da sua faixa e derrubá-lo no fim, pelo PID;
   - entrar pela tela de login;
   - percorrer as telas principais e FALHAR se qualquer uma soltar erro de página (`pageerror`) — esse laço sozinho vale mais que dez asserções bonitas;
   - conferir os fluxos que já quebraram antes nesta casa: incluir e salvar pela tela (quebrou inteiro quando o `rownum` entrou, porque alguém usou `find(...)` onde devia usar `filter(...)`), a árvore remontando quando um banco novo aparece, e a grade com coluna de sistema nova;
   - rodar nos dois temas.

4. **Avalie o design** das telas com olho crítico e com CAPTURA, não por leitura: responsividade (celular, tablet, desktop — foi pedido explicitamente), contraste no tema claro E no escuro, a convenção de cores de ação da casa (verde inclui, amarelo altera, rosa marca, vermelho exclui de vez, azul consulta; sempre contorno, nunca fundo cheio), e o painel lateral retrátil e pinável. Duas armadilhas já pagas aqui, procure-as: `input{width:100%}` deformando controle dentro de tabela, e `label{text-transform:uppercase}` transformando «Blumenau» em «BLUMENAU» — que é uma MENTIRA SOBRE O DADO, porque quem olha não sabe se está gravado assim. Guarde as capturas em `/tmp/claude-0/-home-user-adrianoboller/34595649-0af6-575a-8f79-80dbe8cb7a5d/scratchpad/bateria/`.

5. **Conserte o que achar** que for pequeno e claro. O que for grande, ANOTE com precisão (arquivo:linha, o que quebra, como reproduzir) em vez de sair reformando — e diga no relatório.

6. **Documente o aprendizado**, inclusive o infrutífero: hipótese que morreu medida é resultado válido nesta casa, e é o que impede a mesma ideia de voltar sem medição. Escreva no documento da área (crie `phxsql/docs/TESTES.md` se não houver lugar melhor) como a bateria roda, o que ela cobre e o que ela deliberadamente NÃO cobre.

Atenção à interface ser `include_str!`: depois de mexer em `ui/` é preciso `cargo build --release -p phxsql-server --bin phxsqld` antes de subir o servidor, senão você exercita a página velha — esta casa já perdeu uma rodada inteira de ganhos por medir com binário velho.

No relatório final: a cobertura medida por área (com números), os buracos que você fechou, os defeitos que repôs para provar cada teste novo, o que achou na avaliação de design com a captura correspondente, e o que ficou anotado por ser grande demais para esta frente.
```

---

## 28. Profiler validado e log em txt  ·  29/08 19:23

```
Você trabalha no PhxSql, em `phxsql/`. LEIA `CLAUDE.md` na raiz ANTES de tudo e siga as regras da casa.

Regras que mais pegam nesta frente (o Profiler é a origem de duas delas — leia com atenção):
- **Senha nunca em texto puro.** Nem em arquivo, nem em log, nem em resposta do protocolo.
- **Funcionalidade que mostra texto cru redige ANALISANDO, nunca recortando.** Recortar depende de o pedido estar escrito de um jeito; analisar e reserializar não. O que não se analisa não vira texto — vira o tamanho em bytes.
- **Instrumentação desligada tem de custar zero — e o portão que decide isso vem ANTES do trabalho.** O Profiler desligado já cobrou 7% da carga pela rede porque o ponto de captura fazia dois `Json::analisar` do corpo inteiro, três `String` e um mutex, e só então perguntava se estava ligado.
- **Diagnóstico plausível não é diagnóstico medido.** Aqui já se escreveu que "o mutex era o pior pedaço"; medido, o `lock` sem disputa custa 13,2 ns e o parse do lote custa 3.456 µs — 262.000× mais.
- Zero dependências além da `std`. Português, identificadores e comentários sem acento.
- Portões: `cargo fmt --all`, `cargo clippy --workspace --all-targets` zero avisos, `cargo test --workspace` verde.
- Portas na faixa **6250–6299**. Mate só o SEU processo pelo PID. **Nunca `pkill -f`.**
- **Sem push, sem PR.** Commite no seu branch de worktree. Ao terminar, `rm -rf phxsql/target`.

## A sua tarefa: VALIDAR O PROFILER E O LOG EM TXT

O dono do projeto pediu, na letra E da lista dele: "Validar o profiler e o log dele em txt".

O Profiler existe (`phxsql/crates/phxsql-server/src/profiler.rs`, tela em `index.html` na função `verProfiler`). O que você tem de descobrir e provar é se ele **cumpre o que promete**, e se o log em txt é confiável.

O que fazer:

1. **Leia** `profiler.rs` inteiro, a tela, e o que houver em `phxsql/docs/` sobre ele. Entenda o que ele captura, quando captura, e o que ele redige.

2. **Prove que a redação funciona, repondo o defeito.** A regra da casa é dura aqui: o Profiler redige ANALISANDO. Escreva testes que mandem senha e token pelos caminhos torcidos — chave com espaço antes, corpo com a senha dentro de um valor de texto que contém aspas escapadas, JSON aninhado, campo de senha com nome diferente, lote grande, corpo malformado, corpo que não é JSON. Em NENHUM caso a credencial pode aparecer no que o Profiler mostra ou grava. Reponha o defeito (troque a análise por um recorte de texto) e veja o teste falhar — se ele não falhar, o teste não presta.

3. **Meça o custo com o Profiler DESLIGADO** e prove que é zero, ou perto disso, com número. Use o medidor que a casa já tem (`--example onde-doi` e o que houver em `phxsql/docs/DESEMPENHO.md`) e lembre da armadilha: `cargo build --release` NÃO recompila os examples — rode `cargo build --release --examples -p phxsql-store` antes de medir, senão você mede o passado. Se o custo desligado não for zero, ache o que ele faz antes de olhar o próprio interruptor e conserte.

4. **O log em txt**: confira que ele existe, onde grava, o que grava, se rotaciona ou cresce sem fim, se sobrevive a reinício, o que acontece com disco cheio ou caminho sem permissão, e se ele redige a credencial com o mesmo rigor da tela. Um log que redige na tela e vaza no arquivo é pior que não ter log. Prove por SOQUETE, não só por teste unitário: suba um servidor seu, mande pedidos de verdade, e leia o arquivo que ficou no disco.

5. **Conserte o que achar.** Se achar defeito grande demais para esta frente, anote com precisão (arquivo:linha, o que quebra, como reproduzir).

6. **Documente o aprendizado no documento da área** — `phxsql/docs/` — incluindo o infrutífero: se você formular uma hipótese sobre custo ou vazamento e ela MORRER medida, escreva a recusa com o número. Nesta casa isso é resultado válido e é o que impede a ideia de voltar sem medição.

7. Se mexer na tela, lembre que a interface é `include_str!`: `cargo build --release -p phxsql-server --bin phxsqld` antes de subir o servidor. Exercite no navegador com Playwright (`/opt/node22/lib/node_modules/playwright/index.mjs`; Chromium já instalado, NÃO rode `playwright install`) e guarde capturas em `/tmp/claude-0/-home-user-adrianoboller/34595649-0af6-575a-8f79-80dbe8cb7a5d/scratchpad/profiler/`.

No relatório final: o que o Profiler promete, o que ele cumpre, o que você provou repondo qual defeito, o custo medido com ele desligado e ligado (com número e com o binário certo), o que o log em txt faz e o que ele não faz, e o que ficou anotado.
```

---

## 29. Bateria dos 12 itens parte A  ·  29/08 19:23

```
Você trabalha no PhxSql, em `phxsql/`. LEIA `CLAUDE.md` na raiz ANTES de tudo e siga as regras da casa.

Regras que mais pegam nesta frente:
- **Toda bateria de testes tem prova real e aprendizado documentado — frutífero ou infrutífero.** O teste novo tem de FALHAR com o defeito reposto.
- **A ordem de digitação é sagrada** — o `.reg` nunca reaproveita slot excluído.
- **Medidor com binário velho mede o passado.** Antes de medir: `cargo build --release --examples -p phxsql-store`.
- **Número citado é número que não se mede.**
- Zero dependências além da `std`. Português, identificadores e comentários sem acento.
- Portões: `cargo fmt --all`, `cargo clippy --workspace --all-targets` zero avisos, `cargo test --workspace` verde.
- Portas na faixa **6300–6349**. Mate só o SEU processo pelo PID. **Nunca `pkill -f`.**
- **Sem push, sem PR.** Commite no seu branch de worktree. Ao terminar, `rm -rf phxsql/target`.

## A sua tarefa: A BATERIA DE PONTA A PONTA QUE O DONO PEDIU (itens 1 a 6)

O dono do projeto pediu uma bateria de doze itens. Estes seis são seus, e ele quer que sejam feitos **de ponta a ponta, como um usuário faria**, não como teste unitário isolado:

1. **Criar um database.**
2. **Criar tabelas** dentro dele.
3. **UUID v7 como chave** e **relacionamentos 1:N** entre as tabelas.
4. **Triggers** — o recurso existe (procure em `phxsql/docs/TRIGGERS.md` e no código); exercite de verdade.
5. **Stored procedures** — idem (`phxsql/docs/` tem o documento da área).
6. **Carga de 5.000 registros.**

O que fazer:

1. **Faça a bateria pelo caminho de verdade**, pelo SOQUETE e pela TELA, não só por API interna. Teste unitário não prova o caminho do usuário — nesta casa já aconteceu de os dez testes do `BULKINSERT` passarem enquanto a queda de conexão não soltava a reserva, e só o soquete mostrou.

2. **Meça e registre**: quanto leva a carga de 5.000, quanto custa cada trigger disparando, o que a procedure custa. Com o binário CERTO (veja a regra do example acima). Compare com o que `phxsql/docs/DESEMPENHO.md` já afirma — e se algum número de lá não bater, esse é um achado importante: registre com o número novo e diga por que o velho estava errado.

3. **Ache o que quebra.** Esse é o objetivo real da frente. Procure especificamente:
   - coluna de sistema nova quebrando quem filtra pela primeira (a lição do `rownum`: procure `find(...)` onde devia ser `filter(...)`);
   - o UUID v7 sendo crescente de verdade (é o que o formato promete) e a chave estrangeira 1:N segurando o que promete;
   - trigger que dispara duas vezes, ou que não dispara, ou que dispara dentro da própria trava e trava o servidor;
   - procedure que vê dado que o usuário dela não podia ler — **portão de permissão é UM só, e o campo que ele lê é o furo**: se a procedure não tem o campo `"tabela"` no pedido, ela precisa de conferência própria dentro dela. Essa foi exatamente a porta dos fundos que `juntar` e `unir` abriram.
4. **Cada defeito que achar vira teste com o defeito reposto** e conserto — ou, se for grande demais, anotação precisa (arquivo:linha, como reproduzir, o que quebra) no relatório.

5. **Exercite também pela TELA** com Playwright (`/opt/node22/lib/node_modules/playwright/index.mjs`; Chromium já instalado, NÃO rode `playwright install`), porque metade dos defeitos desta casa só apareceu assim. Suba um servidor seu na faixa 6300–6349. Capturas em `/tmp/claude-0/-home-user-adrianoboller/34595649-0af6-575a-8f79-80dbe8cb7a5d/scratchpad/bateria12/`. Se mexer em `ui/`, `cargo build --release -p phxsql-server --bin phxsqld` antes de subir.

6. **Deixe a bateria repetível e versionada** dentro de `phxsql/`, com o comando para rodar documentado — o valor dela é rodar de novo daqui a três meses, não rodar uma vez.

7. **Documente o aprendizado** no documento da área, inclusive o infrutífero: hipótese que morreu medida é resultado válido e impede a ideia de voltar sem medição.

No relatório final: o que passou, o que quebrou, cada defeito com o teste que o pega, os números medidos (e com qual binário), e o que ficou anotado por ser grande demais.
```

---

## 30. Consolidar sprints e PENDENCIAS  ·  29/08 19:24

```
Você trabalha no PhxSql, em `phxsql/`. LEIA `CLAUDE.md` na raiz ANTES de tudo e siga as regras da casa.

Esta frente é de DOCUMENTO e de CONTAGEM. Você não precisa compilar nada pesado, e não deve mexer em código de produção.

Regras que mais pegam aqui:
- **Número digitado à mão envelhece calado.** O selo da capa do dossiê passou quatro lançamentos dizendo a versão errada. **Todo número visível ou sai de um gerador, ou está errado e ninguém percebeu ainda.**
- **A lista do que falta também é palpite até alguém medir.** O pedido 113 tinha o alvo certo e a causa errada; medir a premissa do item vem antes de implementar o item.
- **Número citado é número que não se mede.** Se você repetir um número de outro documento, confira a fonte dele — esta casa já teve um número RETRATADO (16,61 µs, que na verdade era 7,92 µs, medido com binário velho) circulando em quatro citações e quase fundamentando dois sprints.
- Português; identificadores e comentários sem acento; documento pode ter acento.
- **Sem push, sem PR.** Commite no seu branch de worktree. Ao terminar, `rm -rf phxsql/target` se existir.

## A sua tarefa: UMA LISTA SÓ DE SPRINTS, E O `PENDENCIAS.md` DE VOLTA À VERDADE

### Parte 1 — consolidar as quatro listas de sprints

Existem quatro propostas de sprint, feitas por agentes dedicados que leram os manuais de quatro motores:

- `phxsql/docs/SPRINTS-CASSANDRA.md` (5 sprints propostos)
- `phxsql/docs/SPRINTS-REDIS.md` (4)
- `phxsql/docs/SPRINTS-MARIADB.md` (13)
- `phxsql/docs/SPRINTS-TERADATA.md` (9)

São 31 propostas esperando a aprovação do dono, e hoje ele teria de ler quatro documentos e cruzar tudo de cabeça. Faça a lista ÚNICA que ele pode aprovar item a item:

1. **Leia os quatro por inteiro.**
2. **Ache as sobreposições** — quando dois motores diferentes apontam para a mesma melhoria, isso é sinal forte, e a lista tem de dizer isso em vez de listar a mesma coisa duas vezes.
3. **Ache as contradições** — quando dois sprints puxam o desenho para lados opostos, o dono precisa saber ANTES de aprovar os dois.
4. **Ache o que já existe.** Muita coisa foi feita desde que aquelas listas nasceram (triggers, procedures, jobs, ODBC, diagrama ER, cluster, replicação com assistente, telemetria, integração com a Claude, LGPD). Sprint proposto para algo que já está pronto tem de sair da lista, dizendo isso.
5. **Ache o que morre na regra da casa** — qualquer proposta que exija dependência externa, ou que quebre a ordem de digitação, ou que quebre cliente antigo, é recusa fundamentada, não item de lista. Escreva a recusa com o motivo; nesta casa recusa fundamentada é resultado válido.
6. **Ordene por (valor ÷ custo)**, dizendo de onde vem cada estimativa. Onde você não tiver medição, DIGA que é julgamento e não número — não invente precisão.
7. Grave em `phxsql/docs/SPRINTS.md`, com referência cruzada para os quatro documentos de origem (que continuam existindo).

### Parte 2 — `PENDENCIAS.md` de volta à verdade

`phxsql/docs/PENDENCIAS.md` está DESATUALIZADO: ainda declara "123 feitos · 5 parciais · 4 planejados" e lista como não começados itens que já foram entregues (triggers, stored procedures, ODBC, cluster, entre outros).

1. **Confira item a item contra o código**, não contra a memória do documento. Um item só está "feito" se você achou onde ele está implementado — cite `arquivo:linha`.
2. **Feche o que virou recusa fundamentada.** Dois casos que eu já identifiquei e você deve conferir: os pedidos **#95 e #106 (MULTILINK)** foram efetivamente resolvidos pelo DbLink nativo — se você confirmar, feche-os dizendo isso. O pedido **#18 (GitHub)** está travado por uma credencial que não é do dono (a sessão autentica como outra identidade, sem direito de escrita) — registre a causa real.
3. **Acrescente o que apareceu novo** e ainda falta, incluindo: restaurar backup (não existe hoje — `ui/index.html` por volta da linha 5047 é um `afazer`), e as 14 tomadas diretas da trava de dados que continuam fora do ponto único `travar_dados()`.
4. **A contagem tem de sair de gerador, não do teclado.** Existe `phxsql/docs/dossie/pagina-dos-pedidos.py`, que gera `pedidos.html` a partir do `PENDENCIAS.md` e conta os três estados sozinho. Rode-o e confira que a contagem bate. Se o formato do `PENDENCIAS.md` mudar de um jeito que o script não entenda, conserte o script — nunca o número à mão.

**Não publique nada na web** (nem dossiê nem página de pedidos) — só gere os arquivos e commite. A publicação é minha.

No relatório final: quantas propostas sobreviveram e por quê, quais eram duplicatas, quais contradiziam outras, quais já estavam prontas, quais viraram recusa fundamentada, e como ficou a contagem do `PENDENCIAS.md` (com o número saído do gerador).
```

---

## 31. Cifra dos dados em repouso  ·  29/08 19:57

```
Você trabalha no PhxSql, em `phxsql/`. LEIA `CLAUDE.md` na raiz ANTES de tudo e siga as regras da casa. Esta frente é de CRIPTOGRAFIA, então três regras valem em dobro:

- **Zero dependências externas. Só a `std`.** Nenhuma crate, nem "só esta".
- **Criptografia se confere contra vetor oficial.** Nada de "parece certo": FIPS 180-4, RFC 4231, RFC 8439, RFC 9106 — o vetor publicado, no teste.
- **Senha nunca em texto puro.** Nem em arquivo, nem em log, nem em resposta do protocolo. E agora um corolário novo: **chave nunca em texto puro** pelos mesmos três caminhos.

Mais: **guarda nova entra pedida, não imposta** — nada pode quebrar arquivo antigo nem cliente antigo, e o teste que mais importa é o do comportamento VELHO. E **mudança de formato entra cedo**: enquanto não há dado em produção, mudar é barato; depois vira migração.

Portões: `cargo fmt --all`, `cargo clippy --workspace --all-targets` zero avisos, `cargo test --workspace` verde. Portas na faixa **6400–6449**. Mate só o SEU processo pelo PID; **nunca `pkill -f`**. Sem push, sem PR — commite no seu branch de worktree. Ao terminar, `rm -rf phxsql/target` (o disco está apertado).

## O contexto: o que JÁ existe aqui

Não comece do zero, e não reescreva o que já foi conferido contra vetor:

- `crates/phxsql-core/src/cifra.rs` — **ChaCha20-Poly1305 (RFC 8439) escrito aqui**, com todos os vetores do RFC nos testes. O cabeçalho explica por que não é AES: AES portátil usa tabela, tabela em cache vaza chave por tempo, e o PhxSql não sabe se vai rodar onde há AES-NI.
- `crates/phxsql-core/src/hash.rs` (SHA-256, HMAC, PBKDF2, comparação em tempo constante), `sha512.rs`, `sha1.rs`, `senha.rs`, `ed25519.rs`, `base64.rs`, `crc.rs`.
- `crates/phxsql-store/src/cofre.rs` — o cofre que sela e abre com chave derivada de senha.
- Configuração `cifra.{ligada,senha,senha_env,iteracoes}` em `config.rs`, **desligada por padrão**, cobrindo `.log`, `.trash` e `.reason` (pedido 101).

## O buraco, que é o pedido

**Os arquivos de DADOS não são cifrados.** Hoje o diário é cifrado e a tabela não: `.reg`, `.ndx`, `.memo`, `.bin`, `.pag` e `.bkp` ficam em claro no disco. Quem copiar o diretório lê tudo. É isso que "criptografia de dados" quer dizer, e é o que você vai resolver.

## O material de referência que o dono mandou (leia, mas cuidado)

Os fontes do **psig 1.0.0-rc.41** (ferramenta dele, de assinatura de código) estão em
`/tmp/claude-0/-home-user-adrianoboller/34595649-0af6-575a-8f79-80dbe8cb7a5d/scratchpad/psig/fontes/psig-1.0.0-rc.41-fontes/`.

**Não dá para ligar o psig ao PhxSql**, e eu já conferi: o `Cargo.toml` dele traz cerca de **60 crates** (rsa, p256, ed25519-dalek, aes-gcm, argon2, ml-kem, fips204/205, x509-cert, cms, ureq, clap, serde…). Ligar acabaria com o `cargo build --offline` e com a compilação cruzada para Windows que funcionou de primeira. É a mesma lição do MULTILINK, que está no `PENDENCIAS.md` #95: o caminho que funciona é **ler e portar o que se precisa**, não linkar.

E a maior parte do psig **não serve a um banco**: Authenticode, CMS, X.509, carimbo RFC 3161 e assinatura pós-quântica são assinatura de artefato, não cifra de dado. Use-o como catálogo de escolhas, não como fonte de código.

Do documento comparativo dele, três algoritmos são portáveis e valem a pena — todos com vetor oficial publicado:
- **XChaCha20-Poly1305** — nonce de 192 bits, que pode ser sorteado sem medo de colisão. Aqui isso importa muito: com nonce de 96 bits derivado de contador, um erro de rebobinar reusa nonce e o Poly1305 quebra inteiro. É **HChaCha20 + o AEAD que já existe** — poucas dezenas de linhas em cima do `cifra.rs`.
- **Argon2id (RFC 9106)** — o `senha.rs` usa PBKDF2 hoje. Argon2id é o recomendado atual e resiste a GPU, que o PBKDF2 não resiste. **Guarda nova entra pedida**: hash antigo tem de continuar abrindo, e o teste que mais importa é `senha_velha_continua_entrando`.
- **BLAKE3** — muito mais rápido em arquivo grande; o manifesto do backup e a soma de verificação seriam os beneficiados. **Meça antes**: se o SHA-256 que já existe não for o gargalo do backup, isto morre com o número na mesa, e isso é resultado válido.

## O que fazer, nesta ordem

**1. Meça primeiro, projete depois.** Antes de qualquer desenho, meça o custo do `cifra.rs` que já existe: quantos MB/s o ChaCha20-Poly1305 faz nesta máquina, e quanto isso representa de uma inserção (que custa ~7,5 µs) e de uma leitura. Sem esse número, o desenho é chute. **Atenção à armadilha da casa: `cargo build --release --examples -p phxsql-store` antes de medir — `cargo build --release` não recompila os examples, e uma rodada inteira de ganhos já ficou invisível por causa disso.**

**2. Enfrente a restrição de formato, que é o coração do problema.** O `.reg` endereça por rowid em O(1): posição = função do rowid e do tamanho fixo do slot. Um AEAD acrescenta **nonce e etiqueta** a cada pedaço cifrado, e isso não cabe no slot sem mudar o formato. As saídas que eu enxergo, e você deve avaliar todas e escolher com motivo escrito:
   - **(a) por slot**, com o slot crescendo para caber a etiqueta (nonce derivado de volume+rowid, não sorteado — e aí o XChaCha não ajuda, mas o determinismo tem de ser provado contra reuso);
   - **(b) por página do `.ndx`** e por bloco do `.memo`/`.bin`, que já têm cabeçalho e CRC e são o lugar natural para uma etiqueta;
   - **(c) por COLUNA marcada**, cifrando só o que é sensível — o que casa com a marca LGPD que já existe no esquema (grade por coluna, PSCH v6);
   - **(d) o arquivo inteiro em volumes**, cifrando na escrita e decifrando na leitura, o que é simples e destrói o O(1).
   Diga qual escolheu, o que ela custa, e **o que ela deixa em claro** — porque toda escolha aqui deixa algo em claro, e esconder isso seria pior que não cifrar.

**3. O que fica em claro tem de estar ESCRITO.** Mesmo com o `.reg` cifrado, o `.ndx` guarda as chaves; mesmo com a coluna cifrada, o índice sobre ela vaza a ordem. Escreva isso no documento antes de escrever o código. Um banco que diz "cifrado" e vaza a chave pelo índice está mentindo para o usuário.

**4. A chave.** Hoje a senha da cifra vive no `config.json` ou numa variável de ambiente. Chave ao lado do dado protege pouco. Proponha o que dá para fazer sem dependência: derivação por Argon2id, chave da tabela envelopada por chave mestra (para poder trocar a mestra sem reescrever a tabela), e o que acontece quando alguém erra a senha. **Rotação** tem de estar no desenho, mesmo que não entre agora.

**5. Implemente a escolha**, com testes contra vetor oficial e com o teste do comportamento VELHO: arquivo gravado sem cifra continua abrindo, cliente que não sabe de cifra continua funcionando, e desligar a cifra num servidor que já cifrou tem de dizer claramente o que acontece (o `config.rs` já tem uma limitação parecida escrita para o diário — leia).

**6. Tela e documento.** A configuração precisa aparecer na tela de configurações, com a convenção de cores da casa. Documente em `phxsql/docs/SEGURANCA.md` (que já existe e já registra "Sem TLS. O tráfego vai em claro") e no `FORMATO.md` se o formato mudar — no MESMO commit, que é regra da casa.

## Duas coisas que NÃO são desta frente

- **TLS no fio.** É buraco maior e é decisão do dono; não comece.
- **A trilha `.lgpd`** está com outro agente AGORA. Se a sua escolha for a (c), por coluna, você vai encostar no mesmo esquema — então **não mexa em `usuarios.rs` nem no arquivo `.lgpd`**, e diga no relatório onde encostou, para eu resolver na integração.

No relatório final: os números medidos (com qual binário), a escolha de formato e o motivo, **o que continua em claro**, quais vetores oficiais você usou, quais defeitos repôs para provar cada teste, e o que ficou para depois.
```

---

## 32. Recursos dos data grids  ·  29/08 20:34

```
Você trabalha no PhxSql, em `phxsql/`. LEIA `CLAUDE.md` na raiz ANTES de tudo e siga as regras da casa.

Regras que mais pegam nesta frente:
- **Zero dependências externas.** Na interface: nada de CDN, nada de biblioteca. O `phx-grid` é ES5 estrito, arquivo único, zero dependência — e continua assim.
- **O CSS global morde todo componente novo da tela.** `input{width:100%}` e `label{text-transform:uppercase}` são certos num formulário e errados dentro de uma tabela: o rádio já virou uma bolinha do tamanho da célula, e «Blumenau» já apareceu como «BLUMENAU» — que é uma **mentira sobre o dado**. Isso reapareceu TRÊS vezes em duas rodadas. Procure ativamente.
- **Interface só se prova exercitando.** Abrir no navegador e olhar. E **coluna de sistema nova quebra quem filtra pela primeira**: quando entrar peça no fim de uma lista, procure quem usa `find(...)` onde devia usar `filter(...)`.
- Português, identificadores e comentários **sem acento**, comentário diz **por que**.
- Portões: `cargo fmt --all`, `cargo clippy --workspace --all-targets` zero avisos, `cargo test --workspace` verde.
- Portas na faixa **6450–6499**. Mate só o SEU processo pelo PID. **Nunca `pkill -f`.**
- **Sem push, sem PR.** Commite no seu branch de worktree. Ao terminar, `rm -rf phxsql/target` (disco apertado).
- A interface é `include_str!`: depois de mexer em `ui/` é preciso `cargo build --release -p phxsql-server --bin phxsqld` antes de subir o servidor, senão você exercita a página velha.

## A sua tarefa: "os data grids devem ter esses recursos"

O dono mandou um pacote de modelos e disse isso, sem detalhar quais recursos. Está extraído em
`/tmp/claude-0/-home-user-adrianoboller/34595649-0af6-575a-8f79-80dbe8cb7a5d/scratchpad/grids/novo/`.

Eu já abri e há **duas coisas diferentes** lá dentro, e a distinção decide o seu trabalho:

**1. `phx-grid` v0.6.0 e v0.7.0** — a grade ES5 de arquivo único, que é a MESMA que o PhxSql já embute em `crates/phxsql-server/ui/grid/`. Conferi: **a nossa é a 0.8.0, mais nova que as duas**, porque tem o motor de agrupamento (`agrupa`, que a 0.7.0 não tem). Então aqui não há o que atualizar — mas há duas coisas a fazer:
   - O cabeçalho do nosso `phx-grid.js` diz `v0.1.0 — Núcleo (S01)` enquanto o `CHANGELOG-phx-grid.md` ao lado documenta até a 0.8.0. **Número digitado à mão envelheceu calado**, que é regra da casa. Confira qual é a verdade lendo o código contra o changelog, e conserte o que estiver mentindo.
   - As **demos** da 0.7.0 (`v070/phx-grid/demos/`) mostram os recursos em uso: núcleo, células, filtros, busca, bandas, excel, frow. Abra-as no navegador e compare com o que as nossas telas realmente ligam.

**2. `phoenix_data_grid_x_v1` a `v38`** — um produto COMPLETAMENTE diferente: Rust com DataFusion, PostgreSQL, Timescale, OTLP/Prometheus/Grafana, Dioxus/WASM, detecção de anomalia, pivot com arrastar-e-soltar, colunas calculadas, cadeia de auditoria com rotação de chave, eleição de líder. **Não dá para linkar nada disso** — DataFusion sozinha traria centenas de crates, e seria o fim do `cargo build --offline` e da compilação cruzada. É a mesma parede do MULTILINK (`PENDENCIAS.md` #95): o caminho é **ler e portar a ideia**, não o código. Use as 38 versões como **catálogo de recursos**, lendo os `RELEASE_NOTES_*.md` e o `ROADMAP.md`.

## O que fazer, nesta ordem

**1. Levante o que as NOSSAS telas realmente usam.** O console tem várias grades (`PhxGrid.criar` aparece em `ui/index.html` em pelo menos quatro lugares: conteúdo da tabela, DbLink, consulta SQL e outra). Descubra, tela por tela, **quais opções cada uma liga e quais deixa desligadas**. Eu já vi que quatro ligam `agrupavel` e `buscaGlobal`; o resto é com você. Esse levantamento é o coração da frente, porque a suspeita — e o padrão desta casa, já visto no botão de telemetria, na marca LGPD e no cache de páginas — é que **a capacidade existe e a tela não a liga**.

**2. Faça a lista dos recursos dos modelos**, das duas fontes, e cruze com o que temos. Para cada recurso, uma de três respostas, com evidência:
   - **já existe e está ligado** — diga onde;
   - **existe no `phx-grid` e a tela não liga** — LIGUE, se fizer sentido naquela tela (e diga por que não faz, onde não fizer);
   - **não existe** — diga o que custaria e se cabe na regra de zero dependências. Recurso que exige dependência é **recusa fundamentada**, não item de lista: escreva o motivo.

**3. Implemente o que for barato e claro**, priorizando o que um usuário de banco de dados usa todo dia. Meu palpite, que você deve conferir e pode contrariar: **congelar coluna** (olhar a linha 40 sem perder de vista o nome), **filtro por coluna** no cabeçalho, **seleção de linhas com ação em lote**, **exportar o que está na tela** (a tela de Exportar existe, mas exportar a VISTA — com filtro, ordem e agrupamento aplicados — é outra coisa), **redimensionar e reordenar coluna com o estado lembrado**, e **edição na célula** onde a grade já é editável. Meça o que precisar medir.

**4. Cuide da conversa entre a grade e o servidor.** O `phx-grid` tem contrato remoto (`grupos/recolhidos/aggCols/tiposCampos`, e a busca serializa `{campo:"*", termo, campos}`) — é *pushdown* previsto. Veja o que o servidor já atende e o que ele ignora em silêncio. **Filtro que a tela aplica e o servidor ignora é filtro que mente quando a página vira** — se achar isso, é o achado mais importante da frente.

**5. Prove exercitando**, com Playwright (`/opt/node22/lib/node_modules/playwright/index.mjs`; Chromium instalado, NÃO rode `playwright install`). Servidor seu na faixa 6450–6499, com dado suficiente para paginar (alguns milhares de linhas). Capture cada recurso funcionando, **nos dois temas** e em **largura de celular** — responsividade foi pedida explicitamente. Guarde em `/tmp/claude-0/-home-user-adrianoboller/34595649-0af6-575a-8f79-80dbe8cb7a5d/scratchpad/grades/`.

**6. Documente** em `phxsql/docs/` (crie `GRADE.md` se não houver lugar melhor): o que a grade faz, o que cada tela liga, o que ficou de fora e por quê. E se alguma hipótese sua morrer medida, escreva a recusa com o número — nesta casa isso é resultado válido.

Atenção a duas coisas que estão acontecendo em paralelo: outro agente mexe na **telemetria** (`ui/telemetria.js`) e outro na **cifra**. Não encoste nesses arquivos. Se precisar tocar em algo que suspeite ser deles, diga no relatório em vez de resolver por conta.

No relatório final: o levantamento tela por tela, a lista cruzada dos recursos com as três respostas, o que você ligou, o que implementou, o que recusou com motivo, e onde ficaram as capturas.
```

---

## 33. Ultrawide e multi-monitor  ·  29/08 21:58

```
Você trabalha no PhxSql, em `phxsql/`. LEIA `CLAUDE.md` na raiz ANTES de tudo e siga as regras da casa.

Regras que mais pegam nesta frente:
- **Interface só se prova exercitando.** Abrir no navegador, nas larguras de verdade, e OLHAR.
- **O CSS global morde todo componente novo.** Já mordeu quatro vezes: `input{width:100%}` deformando controle dentro de tabela, e `label{text-transform:uppercase}` transformando «Blumenau» em «BLUMENAU», que é **mentira sobre o dado**.
- Zero dependência externa. Português, identificadores e comentários **sem acento**, comentário diz **por que**.
- Portões: `cargo fmt --all`, `cargo clippy --workspace --all-targets` zero avisos, `cargo test --workspace` verde, **e a bateria de tela**: `node phxsql/testes-web/bateria.mjs` tem de dar 22/22.
- A interface é `include_str!`: `cargo build --release -p phxsql-server --bin phxsqld` depois de mexer em `ui/`, senão você exercita a página velha. A bateria já recusa binário velho.
- Portas na faixa **6500–6549**. Mate só o SEU processo pelo PID. **Nunca `pkill -f`.**
- **Sem push, sem PR.** Commite no seu branch de worktree. Ao terminar, `rm -rf phxsql/target`.

## A sua tarefa: o console em tela larga, e em vários monitores

O dono pediu os prints do console em celular, tablet, desktop e "desktop gamer", e escreveu: **"É importante poder usar as telas em multi-monitores"**. Eu fotografei antes de te chamar, e medi. As capturas estão em
`/tmp/claude-0/-home-user-adrianoboller/34595649-0af6-575a-8f79-80dbe8cb7a5d/scratchpad/telas/`
(`1-celular`, `2-tablet`, `3-tablet-deitado`, `4-desktop` 1920, `5-gamer` 3440 ultrawide, `6-dois-monitores` 5120). **Olhe todas antes de mexer em qualquer coisa.**

O que a medição diz:

| largura | rolagem lateral do corpo | maior parágrafo |
|---|---|---|
| 3440 (ultrawide) | não | **2.668 px** |
| 5120 (dois monitores) | não | **4.348 px** |

Ou seja: **a responsividade segura** — não há rolagem lateral em largura nenhuma, e isso é mérito do trabalho anterior. O que não existe é **limite superior**. Uma linha de texto de 4.348 px tem umas 400 letras; ninguém lê isso. E há coisa pior, que você vai ver nas imagens:

1. **Um defeito de sobreposição que já aparece a 1920**, e fica grotesco a 5120: no cartão «A máquina», o caminho do diretório de dados **passa por cima** do texto «livres de 37,0 GB · 214,9 GB reservados». Veja `4-desktop-painel-escuro.png` e `6-dois-monitores-painel-escuro.png`. Isto é defeito, não questão de gosto — conserte primeiro.

2. **Dois regimes de escala convivendo mal.** Os textos em SVG (o caminho do disco, o rótulo «operações por hora», os números do gráfico) crescem com o contêiner por causa do `viewBox`, enquanto o texto em HTML fica do mesmo tamanho. A 5120 o resultado é um caminho de arquivo gigante ao lado de um menu de 4 px. Decida uma regra e escreva o porquê.

3. **A tela de telemetria a 3440** (`5-gamer-telemetria-escuro.png`) mostra o outro sintoma: os pares rótulo→valor do descritivo se esticam por mais de mil pixels, com o valor lá na ponta direita — o olho perde a linha. E metade inferior da tela fica vazia.

## O que fazer

**1. Conserte a sobreposição** do cartão «A máquina». Prove com captura antes e depois, a 1920 e a 5120.

**2. Decida o que a largura extra deve fazer, e defenda a escolha no comentário.** As três saídas que eu enxergo — avalie todas, escolha uma, e escreva por quê:
   - **(a) teto e centraliza**: o conteúdo para de crescer depois de um limite e fica no meio. Simples, e desperdiça a tela que a pessoa comprou.
   - **(b) mais colunas**: a largura extra vira mais colunas de cartões e painéis lado a lado, em vez de esticar os mesmos. É o que uma tela larga é boa em fazer.
   - **(c) misto**: texto corrido com teto (a legibilidade tem limite físico: ~75 caracteres por linha), e grade/painéis aproveitando a largura toda.
   Minha leitura, que você deve conferir e pode contrariar com motivo: **(c)**. Texto corrido tem limite de legibilidade que nenhuma tela grande muda; tabela e grade não têm.

**3. Cuide dos pares rótulo→valor** (a ficha da telemetria, as fichas de configuração, o «Quem sou eu»). Valor a mil pixels do rótulo não se lê. Colunas em vez de esticar.

**4. Não estrague o que funciona.** Celular, tablet e 1920 estão bons — a bateria de tela mede responsividade em três larguras e tem de continuar passando 22/22. Se você mudar as regras de largura, **acrescente 3440 e 5120 à bateria**, para o dia em que alguém quebrar isso o teste contar.

**5. O que "multi-monitores" pode querer dizer, e é decisão de desenho sua avaliar:** uma janela esticada por dois monitores tem uma emenda física no meio, e conteúdo centrado cai justamente nela. Vale considerar que o layout, acima de certa largura, prefira **duas colunas com uma calha larga** a um bloco central. Se você achar que isso resolve, faça; se achar que é enfeite, escreva por que e não faça.

**6. Documente** em `phxsql/docs/DESIGN.md` (existe) as faixas de largura e o que cada uma faz, com os números medidos.

Atenção: outro agente está rodando uma bateria de ponta a ponta e pode subir servidores; não mate processo que não seja seu.

No relatório final: o que estava errado com captura, a saída que você escolheu e por quê, o que mudou por faixa de largura, e as capturas novas nas seis larguras.
```

---

## 34. Modo multitela do console  ·  29/08 22:01

```
Você trabalha no PhxSql, em `phxsql/`. LEIA `CLAUDE.md` na raiz ANTES de tudo e siga as regras da casa.

Regras que mais pegam nesta frente:
- **Zero dependência externa.** Tudo aqui é API nativa do navegador. Nada de CDN, nada de biblioteca de janelas.
- **Interface só se prova exercitando.** Abrir no navegador e usar.
- **Guarda nova entra pedida, não imposta.** Quem nunca usar o modo multitela não pode notar diferença nenhuma — e o teste que mais importa é o do comportamento VELHO.
- **Senha nunca em texto puro**, e o corolário: **ficha de sessão também não vai para o disco do navegador**.
- Português, identificadores e comentários **sem acento**; comentário diz **por que**.
- Portões: `cargo fmt --all`, `cargo clippy --workspace --all-targets` zero avisos, `cargo test --workspace` verde, e `node phxsql/testes-web/bateria.mjs` 22/22.
- A interface é `include_str!`: `cargo build --release -p phxsql-server --bin phxsqld` depois de mexer em `ui/`.
- Portas **6550–6599**. Mate só o SEU processo pelo PID; **nunca `pkill -f`**.
- **Sem push, sem PR.** Commite no seu branch de worktree. Ao terminar, `rm -rf phxsql/target`.

## A sua tarefa: MODO MULTITELA, no molde do WINDEV(R)

O dono considera isto **obrigatório**, e descreveu o que quer:

- arrastar uma tela/editor para outro monitor;
- destacar uma aba e transformá-la em **janela independente**;
- Designer no monitor 1, Código no 2, Banco/SQL no 3, Debug/Logs/IA no 4;
- **memorizar posição, tamanho e monitor** de cada janela;
- **restaurar o workspace** na abertura seguinte;
- tratar monitores com **DPI e resoluções diferentes** (o WINDEV(R) tem DPI independente por monitor).

## O que eu já apurei, para você não gastar rodada descobrindo

**É possível no navegador, e o caminho é a Window Management API.** O console é servido em `http://127.0.0.1:5799`, e **localhost é contexto seguro** — então `window.getScreenDetails()` está disponível no Chrome/Edge 100+ depois da permissão `window-management`. Ela devolve a lista de monitores com `left/top/width/height`, **`devicePixelRatio` por monitor**, `isPrimary` e `label`. Com isso dá para abrir a janela JÁ no monitor certo, em vez de pedir para a pessoa arrastar.

As outras peças, todas nativas:
- `window.open(url, nome, "left=…,top=…,width=…,height=…,popup=yes")` para criar a janela na posição calculada;
- **`BroadcastChannel`** (mesma origem) para as janelas conversarem — é por aqui que a árvore avisa que um banco nasceu, e que a sessão viaja;
- `localStorage` para o **workspace** (que tela, em que monitor, posição e tamanho);
- DPI por monitor: `window.devicePixelRatio` muda quando a janela troca de tela, e dá para escutar com `matchMedia(\`(resolution: ${dpr}dppx)\`)` — o evento `change` avisa a troca.

**Fallback obrigatório:** Firefox e Safari **não têm** a Window Management API. Lá o modo tem de continuar funcionando com `window.open` simples, a pessoa arrastando, e a posição sendo lembrada de `screenX`/`screenY`. Detecte a capacidade e **diga na tela** o que muda — não finja que é igual.

**O pré-requisito que dá o trabalho:** hoje o console é UMA página só, sem rota. Não há `?tela=`, não há `window.open`, não há `BroadcastChannel` — conferi. Para destacar uma tela é preciso que ela seja alcançável por URL (algo como `/?tela=telemetria&db=loja&destacada=1`) e que a página saiba subir em **modo janela destacada**: sem a moldura inteira, só a folha pedida, com um cabeçalho mínimo dizendo qual banco e qual servidor.

**A sessão é o ponto delicado.** O `api()` manda `X-Sessao` de `est.sessao`, que vive só em memória. Uma janela destacada precisa da sessão **sem pedir login de novo** e **sem gravar a ficha no `localStorage`** — o disco do navegador é lido por qualquer outra aba e sobrevive ao fechamento. Passe pelo `BroadcastChannel` no instante da abertura (a janela nova pede, a mãe responde), e se a mãe morrer, a filha pede login. Escreva esse desenho no documento.

## O que fazer

1. **Leia primeiro** a moldura da página: `montarMenu`, `montarFerramentas`, `folha(...)`, `irPara(...)` e o estado `est`. Entenda como uma tela é montada hoje antes de propor rota.

2. **Rota por URL** para as telas que valem destacar. Não precisa ser todas — comece pelas que o dono nomeou: Query/SQL, Diagrama ER (Designer), Telemetria e Profiler (Debug/Logs), e a integração com a Claude (IA). Diga no relatório quais ficaram de fora e por quê.

3. **O botão de destacar**, na folha, e o caminho de volta (fechar a janela devolve a tela para a mãe, ou não — decida e escreva o porquê).

4. **O workspace**: gravar qual tela em qual monitor, com posição e tamanho, e restaurar na abertura seguinte — com o cuidado óbvio de que **o arranjo de monitores pode ter mudado**. Monitor que sumiu não pode fazer a janela abrir fora da tela; caia para o primário e diga.

5. **DPI diferente por monitor**: quando a janela muda de tela, o desenho tem de se refazer. Prove com dois monitores simulados de DPI diferente (o Playwright deixa criar contexto com `deviceScaleFactor`; e a `getScreenDetails` pode ser dublada num teste de navegador para exercitar o caminho sem hardware).

6. **Prove exercitando.** Playwright em `/opt/node22/lib/node_modules/playwright/index.mjs` (Chromium instalado; NÃO rode `playwright install`). O Chromium do Playwright aceita `--enable-features` e permissões — investigue como conceder `window-management` no contexto; se não der, duble a API e exercite o resto, dizendo no relatório o que ficou sem prova real. Capturas em `/tmp/claude-0/-home-user-adrianoboller/34595649-0af6-575a-8f79-80dbe8cb7a5d/scratchpad/multitela/`.

7. **Documente** em `phxsql/docs/` (crie `MULTITELA.md`): o desenho, o que funciona em qual navegador, como a sessão viaja, o que acontece quando um monitor some, e **o que este modo NÃO faz**.

**Atenção — outro agente está mexendo em largura de tela AGORA** (faixas de largura, ultrawide, o cartão «A máquina» que se sobrepõe). **Não mexa nas regras de largura nem no CSS de layout responsivo.** Se precisar, diga no relatório e eu resolvo na integração.

No relatório final: o que ficou possível de verdade e em qual navegador, o que precisou de dublê para provar, como a sessão viaja, o que acontece com monitor que some ou muda de DPI, e o que ficou de fora.
```

---

## 35. Cores das bolhas configuráveis  ·  29/08 22:31

```
Você trabalha no PhxSql, em `phxsql/`. LEIA `CLAUDE.md` na raiz ANTES de tudo e siga as regras da casa.

Regras que mais pegam nesta frente:
- **Configuração que não é lida mente.** `recursos.cache_paginas` esteve no `config.json`, no MANUAL e na tela por versões inteiras **sem uma linha de código que o lesse**. Campo de configuração sem leitor é pior que campo ausente. O que você acrescentar tem de ser lido de verdade, e provado por teste.
- **Guarda nova entra pedida, não imposta.** Quem não configurar cor nenhuma continua vendo exatamente o que vê hoje, e o teste que mais importa é o do comportamento VELHO.
- **Número que aparece na tela e número que decide a cor têm de sair da mesma constante** — a legenda não pode divergir do desenho.
- Zero dependência externa. Português; identificadores e comentários **sem acento**; comentário diz **por que**.
- Portões: `cargo fmt --all`, `cargo clippy --workspace --all-targets` zero avisos, `cargo test --workspace` verde, e `node phxsql/testes-web/bateria.mjs` 22/22.
- A interface é `include_str!`: `cargo build --release -p phxsql-server --bin phxsqld` depois de mexer em `ui/`.
- Portas **6600–6649**. Mate só o SEU processo pelo PID; **nunca `pkill -f`**.
- **Sem push, sem PR.** Commite no seu branch de worktree. Ao terminar, `rm -rf phxsql/target`.

## A sua tarefa: as cores do SQL Check configuráveis

O dono pediu: **"Permitir mudar as cores do SQL check bolhas pelo config.json e pela tela de configuração."**

O estado de hoje: a tela de telemetria (`ui/telemetria.js`, tela ligada em `index.html` por `telaTelemetria`) desenha as bolhas em quatro estados, com cor **e** um sinal que não é cor:

| estado | cor | traço da borda | glifo |
|---|---|---|---|
| normal | `var(--reg)` (azul) | cheia | — |
| uso alto | `var(--ambar)` (amarelo) | tracejada | ▲ |
| stress | `var(--vermelho)` | pontilhada | ■ |
| encerrando | `var(--acao-marcar)` (rosa) | traço longo | ✕ |

**Essa segunda coluna é decisão de projeto e NÃO se perde**: 8% dos homens não distinguem vermelho de amarelo, e a legenda diz as três coisas juntas («amarelo · uso alto ▲ borda tracejada»). Cor configurável não pode virar cor como único sinal.

O que fazer:

1. **Leia primeiro** `ui/telemetria.js` (as constantes dos estados e a legenda), `docs/TELEMETRIA.md`, e como o `config.rs` declara bloco novo — em especial `CAMPOS_CONHECIDOS`, `CAMPOS_EDITAVEIS` e `editaveis_json()`, que é o **ponto único** onde um bloco novo se pluga. Há um teste (`os_exemplos_nao_tem_campo_estranho`) que reprova campo desconhecido: ele é seu amigo, não obstáculo.

2. **O bloco no `config.json`**, com as quatro cores e os **limiares** (o servidor já manda `limiares` na resposta da telemetria — confira e reuse em vez de inventar outro). Campo ausente = cor de fábrica; é assim que o velho continua valendo.

3. **A leitura de verdade.** O servidor entrega as cores para a tela pelo mesmo caminho por onde já entrega o resto da configuração. Nada de a tela ler o arquivo.

4. **A tela de configuração**: seletor de cor para cada estado, com a **amostra da bolha ao lado** — cor se escolhe vendo, não lendo hexadecimal. Botão de voltar às cores de fábrica.

5. **A conferência de contraste é obrigatória, e é o coração da frente.** Cor escolhida pela pessoa pode ficar ilegível: o rótulo vai DENTRO da bolha, e branco sobre amarelo claro é 2,x:1. Esta casa já pagou isso duas vezes (o botão de excluir com fundo laranja e texto escuro, e o branco sobre `--vermelho` dando 2,98:1 na própria tela de bolhas). Então:
   - calcule a razão WCAG das duas tintas possíveis e use a maior — o código de hoje já faz isso, **não desfaça**;
   - e **avise na tela de configuração** quando a cor escolhida não alcançar 4,5:1 com nenhuma das duas tintas. Avisar, não proibir: quem manda é o dono do servidor.
   - Confira nos **dois temas** — no claro as cores escurecem, pela mesma razão do vermelhão da marca (`#C63C0A`).

6. **Prove exercitando**: Playwright (`/opt/node22/lib/node_modules/playwright/index.mjs`, Chromium instalado, NÃO rode `playwright install`), servidor seu na faixa 6600–6649, com carga que produza os três estados. Capture cor de fábrica e cor trocada, nos dois temas, e o aviso de contraste aparecendo. Guarde em `/tmp/claude-0/-home-user-adrianoboller/34595649-0af6-575a-8f79-80dbe8cb7a5d/scratchpad/cores/`.

7. **Teste com o defeito reposto** para cada guarda: sem o leitor no servidor, a cor configurada não chega (o teste tem de cair); sem a conferência de contraste, o aviso não aparece; e `sem_cor_configurada_nada_muda` para o comportamento velho.

8. **Documente** em `TELEMETRIA.md` e no MANUAL, e acrescente o bloco aos `Config_exemplo_*.json` se for o padrão da casa.

**Atenção aos vizinhos:** dois agentes estão mexendo na interface agora — um em **faixas de largura/ultrawide**, outro em **abas, painéis e janelas soltas**. Você mexe em `ui/telemetria.js`, no bloco de configuração e na tela de configuração. **Não mexa no layout nem na moldura.** Se encostar, diga no relatório.

No relatório final: o bloco criado, como ele chega à tela, o que acontece com cor ilegível, os defeitos que repôs, e onde ficaram as capturas.
```

---

## 36. Revisão de multilíngua  ·  29/08 22:32

```
Você trabalha no PhxSql, em `phxsql/`. LEIA `CLAUDE.md` na raiz ANTES de tudo e siga as regras da casa.

Regras que mais pegam nesta frente:
- **Guarda nova entra pedida, não imposta.** Quem não escolhe idioma continua vendo português, e o teste que mais importa é o do comportamento VELHO.
- **Funcionalidade que mostra texto redige ANALISANDO, nunca recortando** — e aqui vale o primo: **texto de interface se resolve por CHAVE, nunca por comparação da frase**. O dia em que alguém melhorar a redação, o que compara frase quebra calado.
- **Toda bateria de teste tem prova real**: o teste novo tem de FALHAR com o defeito reposto.
- Zero dependência externa. Português; identificadores e comentários **sem acento**.
- Portões: `cargo fmt --all`, `cargo clippy --workspace --all-targets` zero avisos, `cargo test --workspace` verde, e `node phxsql/testes-web/bateria.mjs` 22/22.
- A interface é `include_str!`: `cargo build --release -p phxsql-server --bin phxsqld` depois de mexer em `ui/`.
- Portas **6650–6699**. Mate só o SEU processo pelo PID; **nunca `pkill -f`**.
- **Sem push, sem PR.** Commite no seu branch de worktree. Ao terminar, `rm -rf phxsql/target`.

## O contexto: uma regra que o dono acabou de tornar PÉTREA

Palavras dele: **"o agente multi linguagem deve fazer uma revisão constante para manter a possibilidade de mudar entre português, inglês… pelo login e pela tela de configuração. A cada nova implementação esse agente tradutor deve atualizar strings fixas por variáveis de multi linguagem. Isso é pétreo."**

A máquina existe: tabela de mensagens `phxsys.mensagens` (`id` Uuid, `TextName`, e as seis colunas de idioma — Português, Francês, Inglês, Italiano, Alemão, Espanhol), `src/mensagens.rs` com a `FABRICA`, `src/idiomas.rs` com a `FABRICA_TELA` e os textos de tela, o `config.json` dizendo o idioma padrão (vazio = português), e a tela de login com as bandeiras.

**E aqui está o buraco, medido por mim antes de te chamar: o `ui/index.html` tem 11.987 linhas e apenas 16 atributos `data-txt`.** Ou seja, a interface é quase inteiramente português cravado no código. A máquina de tradução funciona e quase nada passa por ela.

## O que fazer

**1. Meça primeiro, e o número é o produto principal desta frente.** Levante quantos textos visíveis existem na interface e quantos já passam pela fábrica. Escreva um **conferidor automático** que rode sempre (junto dos testes) e que responda: quantos textos de tela estão fora da fábrica, e onde. Sem esse conferidor, a regra "pétrea" do dono vira promessa — porque a próxima frente acrescenta tela e ninguém percebe.

Já existe o laço do outro lado: `todo_data_txt_da_pagina_existe_na_fabrica` reprova `data-txt` que não existe na `FABRICA_TELA`. **Falta o laço inverso**: texto na tela que não está na fábrica. Esse é o que segura a regra.

**2. Traga para a fábrica o que esta rodada acrescentou**, que é muito e é recente: telemetria e o painel de bolhas, trilha e tela de LGPD, restaurar backup, integração com a Claude, as grades (filtro, congelar, seleção, exportar a vista), cifra, jobs, cluster, replicação, ODBC, diagrama ER. Priorize pelo que a pessoa mais vê: **títulos de tela, rótulos de botão, cabeçalhos de coluna, e as mensagens de erro** — nesta ordem.

**Não traduza tudo de uma vez se não couber.** Melhor entregar as telas mais usadas 100% traduzidas e o conferidor dizendo com precisão o que falta, do que tudo pela metade sem conferidor.

**3. Cuide de três armadilhas que esta casa já pagou:**
   - **Caixa alta mente sobre o dado.** «Blumenau» virando «BLUMENAU» já apareceu QUATRO vezes. Texto de interface pode ter transformação; **dado, nunca**. Ao mover uma string para a fábrica, confira se ela é rótulo ou dado.
   - **Três mensagens de erro NÃO se traduzem, de propósito**, e estão assim documentadas em `mensagens.rs`: `erro.redireciona` (o cliente recorta o prefixo — é protocolo vestido de texto), `erro.sinal` (a `MESSAGE_TEXT` é escrita pelo dono do banco) e `erro.cancelado` (o texto já vem montado do ponto de cancelamento). **Não as traduza**, e se você achar uma quarta do mesmo naipe, documente a decisão em vez de traduzir.
   - **Acento em identificador, nunca**; em texto de interface, sim.

**4. O caminho da escolha tem de funcionar dos dois lados**, e prove pelo navegador: pelo **login** (as bandeiras) e pela **tela de configuração**. Trocar o idioma tem de mudar a tela sem recarregar coisa alguma à mão, e a escolha tem de sobreviver ao próximo login.

**5. Deixe a regra escrita onde as próximas frentes a leiam.** Acrescente ao `CLAUDE.md` a regra pétrea, com a mesma voz das outras (o porquê, e o defeito que ela evita), e ao documento da área (`docs/MENSAGENS.md`) o procedimento: como acrescentar um texto novo, e o que o conferidor reprova.

**6. Prove exercitando.** Playwright (`/opt/node22/lib/node_modules/playwright/index.mjs`; Chromium instalado, NÃO rode `playwright install`), servidor seu na faixa 6650–6699. Capture a mesma tela em português e em inglês, nos dois temas, e o caminho pelo login e pela configuração. Guarde em `/tmp/claude-0/-home-user-adrianoboller/34595649-0af6-575a-8f79-80dbe8cb7a5d/scratchpad/idiomas/`. E cuide do que só aparece traduzindo: **texto que estica** — alemão é ~30% mais longo que português, e botão que cabia deixa de caber. Capture uma tela em alemão para provar.

**Atenção aos vizinhos:** três agentes mexem na interface agora — faixas de largura/ultrawide, abas e painéis, e cores das bolhas. Você mexe em TEXTO. Se um deles trocar uma string que você acabou de mover para a fábrica, o conflito é meu para resolver: diga no relatório quais arquivos você tocou e em que região.

No relatório final: o número medido antes e depois, o conferidor que você criou e o que ele reprova, quais telas ficaram 100%, o que falta com precisão, e as capturas — incluindo a do alemão esticando.
```

---

## 37. Dossiê refeito com capturas  ·  30/08 02:05

```
Você trabalha no PhxSql, em `phxsql/`. LEIA `CLAUDE.md` na raiz ANTES de tudo — e nesta frente ele importa mais que nunca, porque o dossiê é a página que o dono usa para enxergar o projeto inteiro, e ela já mentiu várias vezes.

Regras que mandam aqui:
- **Os números do painel são MEDIDOS, nunca estimados.** Já saíram errados três vezes: arredondamento para cima, depois 276 testes quando eram 280, depois um rodapé inteiro parado numa versão anterior.
- **Número digitado à mão envelhece calado.** O selo da capa passou **quatro lançamentos** dizendo 0.11.0. **Todo número visível ou sai de um gerador, ou está errado e ninguém percebeu ainda.**
- Os dois scripts de números aceitam o caminho do HTML como argumento: `docs/dossie/numeros-da-bancada.py` e `docs/dossie/numeros-do-projeto.py`. Há também `docs/dossie/cobertura-por-area.py` e `docs/dossie/pagina-dos-pedidos.py`.
- **Medidor com binário velho mede o passado**: antes de qualquer medição, `cargo build --release --examples -p phxsql-store`.
- A marca **manda**: arquivos em `phxsql/marca/`, especificação em `phxsql/marca/LEIA-ME.md`. Tipografia Exo 2, fundo `#010418`, assinatura *Built to store. Engineered to scale.* Duas adaptações já decididas: corpo de texto longo não usa Exo 2, e o vermelhão escurece para `#C63C0A` no tema claro.
- **A folha de marca afirma *ACID compliant* e isso continua FALSO** (não há transação). Não repita em documento técnico. Já *built-in replication* deixou de ser falso — a replicação funciona e está medida.
- Português; identificadores e comentários sem acento; documento pode ter acento.
- **Sem push, sem PR.** Commite no seu branch de worktree. Ao terminar, `rm -rf phxsql/target`.
- **NÃO publique nada na web.** Eu publico depois, da árvore integrada. Publicar do seu worktree cairia na URL compartilhada com números que só valem aí.

## A sua tarefa: o dossiê refeito

O dono pediu, na letra F item 11: o dossiê está **desatualizado**, falta o `.bkp`, **não é responsivo**, precisa de **download**, e quer **capturas do login até replicação, profiler e SQL Check**.

A fonte é `phxsql/docs/dossie/dossie-phxsql-0.15.html` (versionada de propósito, para qualquer sessão conseguir atualizá-la). Instruções e armadilhas de estilo em `phxsql/docs/dossie/LEIA-ME.md` — leia antes.

O que fazer:

**1. Confira seção por seção contra o CÓDIGO**, não contra a lembrança do documento. Muita coisa entrou desde a última revisão: trilha `.lgpd` e a cifra por coluna marcada, restaurar backup, telemetria com bolhas 3D e três níveis, o modo multitela (abas, regiões lado a lado, janelas soltas), as grades com filtro/congelar/seleção/exportar-a-vista, integração com a Claude, ODBC, cluster, jobs, gatilhos e procedimentos, blacklist, mensagens em seis idiomas com catraca. Cada afirmação do dossiê precisa de `arquivo:linha` ou de número medido por trás.

**2. Os números saem dos geradores.** Rode os quatro e confira que a página bate com eles. Se algum número visível não tiver gerador, **ou você cria o gerador, ou tira o número**. Foi assim que o selo da capa parou de mentir.

**3. O `.bkp` que falta** — confira em `docs/FORMATO.md` o que ele é e onde entra no fluxograma dos arquivos, e ponha no diagrama junto dos outros (`.reg`, `.ndx`, `.bin`, `.memo`, `.log`, `.trash`, `.reason`, `.pag`, e agora `.lgpd`).

**4. Responsivo** — a página tem de servir em celular, tablet e desktop. E há regra recém-medida na casa que vale aqui: **texto corrido para em ~74 caracteres; a largura extra vira mais coluna, não linha mais comprida**; nada de centralizar bloco estreito em tela muito larga (a emenda física entre dois monitores cai no meio). Está em `docs/DESIGN.md` §4.

**5. Download** — o dono quer poder baixar. Veja o que dá para fazer numa página estática: imprimir para PDF com `@media print` decente é o caminho barato e confiável; um botão que monta um arquivo no navegador também é possível. Escolha, faça, e diga na página o que o botão faz.

**6. As capturas.** Suba um servidor seu (portas **6700–6749**, mate pelo PID, **nunca `pkill -f`**), popule com dado que não deixe as telas vazias, e capture com Playwright (`/opt/node22/lib/node_modules/playwright/index.mjs`; Chromium instalado, **NÃO** rode `playwright install`) o caminho que ele pediu: **login → painel → tabelas → grade → query → diagrama ER → telemetria (as bolhas) → profiler → replicação**, mais o **modo multitela com as quatro telas lado a lado**. Nos dois temas onde fizer sentido. Lembre que a interface é `include_str!`: `cargo build --release -p phxsql-server --bin phxsqld` antes de subir.

As imagens precisam viver **dentro** do HTML (data URI) ou ao lado dele em `docs/dossie/` — decida pelo tamanho: a página inteira não pode ficar impossível de abrir. Diga no relatório qual escolheu e quanto pesou.

**7. O que o dossiê NÃO pode esconder.** Ele é a vitrine, e vitrine que mente é pior que vitrine feia. Tem de dizer, sem rodeio: não há transação (logo, não é ACID), não há TLS (o tráfego vai em claro, a senha não), o `excluir` ainda perde para o MySQL(R) na bancada, e a interface está em 11% traduzida. Se você achar que falta algum "não faz" relevante, acrescente.

No relatório final: o que estava errado no dossiê antes (com o número certo ao lado do errado), quais números passaram a sair de gerador, o peso final da página, e onde ficaram as capturas.
```

---

## 38. Backup dos fontes e builds  ·  30/08 02:05

```
Você trabalha no PhxSql, em `phxsql/`. LEIA `CLAUDE.md` na raiz ANTES de tudo e siga as regras da casa.

Regras que mais pegam nesta frente:
- **Zero dependências externas. Só a `std`.** É exatamente o que faz esta frente ser possível: foi o que fez a compilação cruzada para Windows funcionar de primeira e o que permite `cargo build --offline`. Se algo parecer exigir uma crate, NÃO acrescente.
- **Número digitado à mão envelhece calado** — tamanho de pacote, contagem de arquivos e soma de verificação saem de comando, não do teclado.
- Português; identificadores e comentários sem acento.
- Portões: `cargo fmt --all`, `cargo clippy --workspace --all-targets` zero avisos, `cargo test --workspace` verde.
- **Sem push, sem PR.** Commite no seu branch de worktree. Ao terminar, `rm -rf phxsql/target`.
- Se subir servidor, portas **6750–6799**, morto pelo PID. **Nunca `pkill -f`.**

## A sua tarefa: o pacote dos fontes e os binários de Linux e Windows

O dono pediu, na letra A: **backup dos fontes** e os **builds de Linux e Windows**. Existe `./empacotar.sh` (o pedido 17 fala em «três zips conferidos») — **comece lendo o que ele já faz**, porque metade do trabalho pode já estar pronta e a outra metade desatualizada.

O que fazer:

**1. Confira o que existe.** Rode o empacotador e veja o que sai. Ele conhece os arquivos e crates de hoje? Muita coisa entrou nesta rodada (`phxsql-odbc`, `testes-web/`, `bancada/`, `ui/multitela.js`, `ui/telemetria.*`, `ui/claude.js`). Pacote de fontes que esquece um crate é pior que pacote nenhum, porque quem recebe descobre no `cargo build`.

**2. O build de Linux** — `--release`, e diga o alvo exato (`x86_64-unknown-linux-gnu` ou o que for) e a versão do `rustc`. Confira que o binário roda: suba, faça um pedido, derrube pelo PID.

**3. O build de Windows por compilação cruzada.** Confira se o alvo está instalado (`rustup target list --installed`); se não estiver, instale (`rustup target add x86_64-pc-windows-gnu` ou `-msvc`, conforme o que a máquina tem de linker — o `gnu` costuma precisar do `mingw-w64`). **Se não der para compilar aqui, isso é resultado válido**: diga exatamente o que falta, com o erro, e entregue o roteiro para o dono rodar na máquina dele. Não invente que funcionou.

**4. Confira o que você empacotou.** Cada zip com **manifesto SHA-256** e um conferidor — o projeto já tem essa disciplina no backup de dados (`backup.json` com SHA-256 por arquivo), e o pacote de distribuição merece a mesma. Prove que o conferidor pega adulteração: mude um byte e veja falhar.

**5. O que vai junto**: o MANUAL, o `README`, os `Config_exemplo_*.json`, a licença se houver, e uma nota de versão dizendo o que é cada zip e como rodar. Confira que o `Cargo.toml` e o MANUAL não estão dizendo versões diferentes.

**6. Teste o pacote de fontes de verdade**: descompacte num diretório limpo, fora da árvore do projeto, e rode `cargo build --offline --release`. Se falhar, o pacote está errado — e esse é o teste que vale, porque é o que o dono vai fazer.

**7. Documente** em `phxsql/docs/` (ou no `README`, se for o lugar) o que o empacotador produz, como se confere, e o que ele deliberadamente não inclui.

Uma coisa que o dono valoriza e vale confirmar com número: o projeto compila **offline** e **sem nenhuma crate externa**. Se isso continuar verdade depois de tudo o que entrou, diga com a prova (`cargo build --offline` num diretório limpo, e a contagem de dependências). Se tiver deixado de ser verdade, esse é o achado mais importante da frente.

No relatório final: o que o empacotador fazia e o que passou a fazer, o alvo e a versão de cada binário, o resultado do teste em diretório limpo, e — se o Windows não compilar aqui — o que exatamente falta.
```

---

## 39. Pacote dos fontes e binário Linux  ·  30/08 02:07

```
Você trabalha no PhxSql, em `phxsql/`. LEIA `CLAUDE.md` na raiz ANTES de tudo e siga as regras da casa.

Regras que mais pegam nesta frente:
- **Zero dependências externas. Só a `std`.** É o que faz esta frente ser possível: foi o que permitiu `cargo build --offline` e a compilação cruzada para Windows funcionar de primeira.
- **Número digitado à mão envelhece calado** — tamanho de pacote, contagem de arquivos e soma de verificação saem de comando, não do teclado.
- Português; identificadores e comentários sem acento.
- Portões: `cargo fmt --all`, `cargo clippy --workspace --all-targets` zero avisos, `cargo test --workspace` verde.
- **Sem push, sem PR.** Commite no seu branch de worktree. Ao terminar, `rm -rf phxsql/target`.

**Uma restrição desta sessão que você precisa respeitar:** **NÃO instale nada no sistema** — nem pacote do sistema, nem alvo de `rustup`, nem daemon. A camada de permissão desta sessão recusa instalação, e contornar isso não é opção. Onde faltar ferramenta, **diga exatamente o que falta e entregue o roteiro** para o dono rodar na máquina dele. Recusa com o motivo escrito é resultado válido nesta casa.

## A sua tarefa: o pacote dos fontes, e os binários

O dono pediu, na letra A: **backup dos fontes** e os **builds de Linux e Windows**. Existe `./empacotar.sh` (o pedido 17 fala em «três zips conferidos») — **comece lendo o que ele já faz**, porque metade pode estar pronta e a outra metade desatualizada.

**1. Confira o que existe.** Rode o empacotador e veja o que sai. Ele conhece os arquivos e crates de HOJE? Muita coisa entrou nesta rodada: `phxsql-odbc`, `testes-web/`, `bancada/`, `ui/multitela.js`, `ui/telemetria.*`, `ui/claude.js`, `docs/` novos. Pacote de fontes que esquece um crate é pior que pacote nenhum, porque quem recebe só descobre no `cargo build`.

**2. O binário de Linux** — `--release`. Diga o alvo exato e a versão do `rustc` (leia, não instale). Confira que ele roda de verdade: suba numa porta da faixa **6750–6799**, faça um pedido, derrube **pelo PID** (nunca `pkill -f`, que já matou servidor de outros agentes aqui).

**3. Windows.** Confira o que a máquina já tem: `rustup target list --installed` e se há linker para o alvo. **Se o alvo não estiver instalado, NÃO instale** — registre o que falta, com o comando exato que o dono precisaria rodar, e diga se o `empacotar.sh` já prevê esse caminho. Se por acaso o alvo já estiver presente, compile e confira o binário produzido (`file`, tamanho, e o que der para verificar sem Windows).

**4. Confira o que empacotou.** Cada zip com **manifesto SHA-256** e um conferidor — o projeto já tem essa disciplina no backup de dados (`backup.json` com SHA-256 por arquivo), e o pacote de distribuição merece a mesma. **Prove que o conferidor pega adulteração**: mude um byte e veja falhar.

**5. O que vai junto**: MANUAL, README, `Config_exemplo_*.json`, licença se houver, e uma nota dizendo o que é cada zip e como rodar. Confira que `Cargo.toml` e MANUAL não estão dizendo versões diferentes.

**6. O teste que vale**: descompacte o pacote de fontes num diretório limpo, FORA da árvore do projeto, e rode `cargo build --offline --release`. Se falhar, o pacote está errado — e é exatamente o que o dono vai fazer.

**7. Documente** o que o empacotador produz, como se confere, e o que ele deliberadamente não inclui.

E confirme com prova uma coisa que o dono valoriza: o projeto compila **offline** e **sem nenhuma crate externa**. Se continuar verdade depois de tudo o que entrou nesta rodada, diga com o número (a contagem de dependências e o `--offline` num diretório limpo). **Se tiver deixado de ser verdade, esse é o achado mais importante da frente** — e vem antes de qualquer zip.

No relatório final: o que o empacotador fazia e o que passou a fazer, o alvo e a versão do binário de Linux, o resultado do teste em diretório limpo, e o que exatamente falta para o Windows compilar aqui.
```

---

## 40. Replicação em Docker  ·  30/08 02:10

```
Você trabalha no PhxSql, em `phxsql/`. LEIA `CLAUDE.md` na raiz ANTES de tudo e siga as regras da casa.

**NOTÍCIA BOA: o daemon do Docker está NO AR nesta máquina.** Eu subi (`dockerd` 29.3.1, containerd 2.2.2, storage `overlayfs`, cgroup v1) e provei com `docker run --rm alpine:3` — o contêiner rodou e a imagem baixou pelo proxy. Você pode usar `docker` e `docker compose` normalmente.

Regras que mais pegam nesta frente:
- **Bancada compara trabalho igual, não só pergunta igual.** As quatro regras estão em `phxsql/bancada/LEIA-ME.md` — leia antes de medir qualquer coisa. Os dois erros que a casa já cometeu apontavam para lados opostos e nenhum aparecia no número.
- **Medidor com binário velho mede o passado**: `cargo build --release --examples -p phxsql-store` antes de medir.
- **Número citado é número que não se mede.**
- **Toda bateria tem prova real e aprendizado documentado — frutífero ou infrutífero.** Hipótese que morre medida é resultado válido.
- Zero dependência externa no código do PhxSql. (Imagem de contêiner não é dependência do binário — mas prefira imagem enxuta e diga qual usou.)
- Português; identificadores e comentários sem acento.
- Portões: `cargo fmt --all`, `cargo clippy --workspace --all-targets` zero avisos, `cargo test --workspace` verde.
- **Sem push, sem PR.** Commite no seu branch de worktree. Ao terminar, `rm -rf phxsql/target` e **remova os contêineres e volumes que você criar** (`docker compose down -v`), para não deixar lixo na máquina.
- Portas do hospedeiro na faixa **6800–6899**. Mate só o SEU processo pelo PID; **nunca `pkill -f`**.
- **Não mexa no daemon** (não pare, não reconfigure) — outros trabalhos podem depender dele.

## A sua tarefa: os quatro tipos de replicação, em Docker

O dono pediu isso na letra F, item 7, e agora repetiu: **testar os 4 tipos de replicação em Docker**.

Os quatro modos estão documentados em `phxsql/docs/REPLICACAO.md` §9 e o assistente em `docs/ASSISTENTE-REPLICACAO.md`. Existe `phxsql/bancada/replicacao/` com uma bancada que sobe quatro servidores **como processos** (`montar.py`) — **leia-a primeiro**: ela já sabe o roteiro, e o seu trabalho é levá-lo para contêineres, não reinventá-lo.

O que fazer:

**1. Por que Docker muda alguma coisa** — e isto tem de estar escrito no documento. Com processos na mesma máquina, tudo se enxerga por `127.0.0.1`, e a bancada não prova nada sobre **endereço**, **firewall** e **isolamento de rede**. O `REPLICACAO.md` §7 descreve justamente um desenho em que o Source aceita entrada só do IP da Réplica e **não alcança ninguém**. Em contêiner dá para provar isso de verdade: rede própria, nomes de serviço, e regras de quem alcança quem. **Essa é a razão de a frente existir** — se você só reproduzir o mesmo teste dentro de contêineres, entregou pouco.

**2. Os quatro modos, cada um com o seu `compose`** e o seu roteiro de prova:
   - **Primary → Replica** (a réplica em somente-leitura; provar que a escrita nela é recusada com o erro certo, e que o dado chega);
   - **Multi-Master / bidirecional** (o laço infinito e a origem no evento; o conflito por «mais recente vence»; a identidade pela chave e não pelo rowid — está em `REPLICACAO.md` §12);
   - **Spare / Failover** (o `SpareEmEspera`, a promoção, e o `REDIRECIONA host:porta`);
   - **Read Replica**.

**3. Prove a convergência com número, não com impressão**: soma de verificação das tabelas idêntica nos dois lados, contagem de linhas, e o **atraso** medido. A bancada de processos já registra master 28.914 linhas/s e réplica 4.357 eventos/s com atraso de 1,3 a 2,1 s — **compare com o que sai em contêiner** e explique a diferença, se houver. Se a rede do Docker custar, isso é resultado, não defeito.

**4. Prove o que só o contêiner prova**: derrube um nó (`docker stop`) e veja o que acontece; volte-o e veja se ele alcança; corte a rede entre dois e veja o comportamento. **A queda de um nó é o teste que a bancada de processos nunca fez direito** — e lembre da lição do `BULKINSERT`: teste unitário não prova queda de conexão, soquete prova; e aqui, contêiner prova melhor ainda.

**5. Deixe repetível e versionado** dentro de `phxsql/bancada/replicacao/docker/` (ou onde fizer sentido), com um `LEIA-ME.md` dizendo o comando único para rodar tudo e quanto demora. O dono vai querer rodar na máquina dele.

**6. Documente o aprendizado** em `docs/REPLICACAO.md` — inclusive o infrutífero. Se algum modo não se sustentar no isolamento de rede, esse é o achado mais valioso da frente e tem de estar escrito com o motivo.

No relatório final: o que cada modo provou, os números de convergência e atraso, o que aconteceu na queda de nó, o que o Docker mostrou que os processos escondiam, e o comando para refazer tudo.
```

---

## 41. GPU CUDA medido  ·  30/08 02:11

```
Você trabalha no PhxSql, em `phxsql/`. LEIA `CLAUDE.md` na raiz ANTES de tudo e siga as regras da casa.

Regras que MANDAM nesta frente, e ela existe por causa delas:
- **Zero dependências externas. Só a `std`.** CUDA fere isso de frente: exige o toolkit da NVIDIA(R) e, em Rust, uma crate de ligação. **Se algo parecer exigir uma crate, primeiro pergunte — não acrescente.** Você NÃO vai acrescentar nada; você vai medir e escrever.
- **Receita de fora se mede contra o NOSSO gargalo antes de virar plano.** Foi assim com o WAL/LSM: chegou uma arquitetura inteira para acelerar escrita, e a medição mostrou que **83,5% do tempo de uma inserção está no `.ndx`** e que o arquivo de dados custa 16,5%. Das dez propostas, cinco já existiam, duas miravam um problema que não temos, uma quebraria a ordem de digitação, e duas eram reais.
- **Diagnóstico plausível não é diagnóstico medido.**
- **Hipótese que morre medida é resultado válido** — e é o que impede a ideia de voltar sem medição.
- **NÃO instale nada no sistema** (a camada de permissão desta sessão recusa, e contornar não é opção). Medir e escrever, sem instalar.
- Português; identificadores e comentários sem acento.
- Portões: `cargo fmt --all`, `cargo clippy` zero avisos, `cargo test --workspace` verde.
- **Sem push, sem PR.** Commite no seu branch de worktree. Ao terminar, `rm -rf phxsql/target`.
- Portas **6900–6949** se precisar subir servidor; morto pelo PID, **nunca `pkill -f`**.

## A sua tarefa: GPU/CUDA — responder com número, não com opinião

O dono pediu: **"Gpu cuda ativar para ajudar em processamento pesado"**. Ele quer o ganho, e o pedido é legítimo. A sua tarefa é responder à pergunta que vem antes: **onde está o processamento pesado, e ele é do tipo que uma GPU acelera?**

Dois fatos desta máquina, que eu já apurei e você deve confirmar:
- **não há GPU aqui**: nenhum `/dev/nvidia*`, nenhum `nvidia-smi`, nenhum `nvcc`; a CPU é um Xeon de 4 núcleos a 2,10 GHz;
- portanto **ativar CUDA nesta sessão é impossível de fato** — não é escolha minha nem sua. O que dá para entregar é a análise medida e o desenho, para ele decidir com número na mão.

O que fazer:

**1. Meça onde o tempo REALMENTE está**, no caminho de cada operação pesada. O medidor existe: `cargo run --release --example onde-doi -p phxsql-store` e o que houver em `docs/DESEMPENHO.md`. **Antes de medir: `cargo build --release --examples -p phxsql-store`** — a casa já perdeu uma rodada inteira medindo com binário velho. Cubra pelo menos: inserção, varredura com soma, busca por chave, junção, ordenação, o CRC-32 das páginas, o SHA-256 do backup, e a cifra ChaCha20-Poly1305 que entrou agora.

**2. Separe o que é I/O do que é CPU, e o que é CPU do que é PARALELIZÁVEL EM SIMD.** É a pergunta que decide tudo: GPU ganha em trabalho aritmético, uniforme, sobre muitos dados, com pouca dependência entre eles e **pouca ida e volta de memória**. Se 83,5% do custo é descer uma B+tree lendo página por página (ponteiro atrás de ponteiro, ramificação a cada nível), GPU não ajuda — e o número é que diz isso, não eu.

**3. Onde a GPU PODERIA ajudar, se ajudar em algum lugar**, e o candidato tem de sair da sua medição, não de uma lista genérica. Suspeitos plausíveis a conferir com número: CRC-32 e SHA-256 em lote; a cifra de muitos slots; a varredura com filtro sobre coluna (o `WHERE` que é o sprint 10); agregação de muitas linhas; ordenação grande. **Para cada um: quanto custa hoje, quantos bytes teriam de atravessar o barramento, e a partir de que tamanho a transferência se paga.** Uma conta de ida e volta PCIe honesta mata a maioria dos candidatos, e mata com número.

**4. Compare com a alternativa que NÃO fere a regra da casa**: paralelismo em CPU (a `std` tem threads, e o projeto já tem `paralelo.rs` — leia) e SIMD (que em Rust estável tem caminho por `std::arch`, sem crate). **Se um deles der a maior parte do ganho sem dependência nenhuma, a resposta prática ao pedido do dono é essa** — e ela vale mais que um plano de CUDA que ele não pode compilar offline.

**5. Escreva o custo REAL de adotar CUDA**, sem retórica: o toolkit deixa de compilar offline; a compilação cruzada para Windows que funcionou de primeira passa a exigir toolchain de GPU dos dois lados; o binário deixa de rodar em máquina sem placa, ou precisa de dois caminhos e de um teste para cada; e a casa passa a ter uma dependência externa, que é a regra fundadora. **Diga isso como custo mensurável, não como objeção moral** — a decisão é do dono.

**6. Entregue o veredito em três linhas no topo do documento**, com o número que o sustenta, e depois a análise. Se a resposta for «não compensa», ela precisa dizer **a partir de que tamanho de dado compensaria**, porque um dia pode compensar. Se for «compensa em X», diga o que teria de ser feito e quanto custa.

Documente em `phxsql/docs/DESEMPENHO.md` (seção nova) ou em `docs/GPU.md`, como preferir — e registre no `PENDENCIAS.md`.

No relatório final: onde o tempo está (com número), quais candidatos sobreviveram à conta da transferência, o que CPU paralela e SIMD dariam sem violar a regra, o custo declarado de adotar CUDA, e o veredito com o limiar em que ele mudaria.
```

---

## 42. Bateria única com prova real  ·  30/08 02:35

```
Você trabalha no PhxSql, em `phxsql/`. LEIA `CLAUDE.md` na raiz ANTES de tudo. Esta frente existe por causa de uma regra dele, e a regra é o trabalho inteiro:

> **Toda bateria de testes tem prova real e aprendizado documentado — frutífero ou infrutífero. Prova real é nos dois sentidos: o teste novo tem de FALHAR com o defeito reposto e passar com o conserto (já houve teste que passava por engano, e ele é pior que teste que falta).**

Outras que pegam:
- **Teste unitário não prova queda de conexão — soquete prova.** O que depende do sistema operacional se prova contra o sistema operacional.
- **Interface só se prova exercitando.**
- Zero dependência externa. Português; identificadores e comentários sem acento.
- Portões: `cargo fmt --all`, `cargo clippy --workspace --all-targets` zero avisos, `cargo test --workspace` verde.
- **NÃO instale nada no sistema** (a camada de permissão recusa; contornar não é opção).
- Portas **6950–6999**. Mate só o SEU processo pelo PID; **nunca `pkill -f`**.
- **Sem push, sem PR.** Commite no seu branch de worktree. Ao terminar, `rm -rf phxsql/target`.
- Há **quatro outras frentes rodando** (dossiê, empacotamento, replicação em Docker, GPU). O daemon do Docker está no ar e **não é seu** — não pare, não reconfigure.

## O estado de hoje, medido por mim agora

| bateria | comando | resultado |
|---|---|---|
| unitários e integração | `cargo test --workspace` | **1.229 aprovados, 0 falharam** |
| frontend | `node phxsql/testes-web/bateria.mjs` | **24/24** |
| idiomas | `node phxsql/testes-web/prova-idiomas.mjs` | **7/7** |
| ponta a ponta | `python3 phxsql/bancada/bateria/prova-bateria.py --tela` | passou (93 conferências) |

E há mais espalhado: `bancada/profiler/` (6 sondas), `bancada/telemetria/` (2 conferidores + a prova das cores), `bancada/dblink/prova-sincronia.py`, `bancada/replicacao/`, `bancada/carga/`, o conferidor de textos fora da fábrica, e os geradores de números do dossiê.

**O buraco é este: são oito lugares, oito comandos, e nenhum relatório único.** Quem chega no projeto não sabe o que rodar, e ninguém sabe dizer, num só lugar, se o projeto está verde.

## A sua tarefa, em duas metades

### Metade 1 — a bateria única

Um comando que roda TUDO e imprime um relatório: o que passou, o que falhou, quanto demorou cada parte, e o que foi **pulado** e por quê (por exemplo: a replicação precisa de quatro servidores; o DbLink precisa de um MySQL(R) vivo). **Pular tem de aparecer no relatório** — bateria que esconde o que não rodou mente por omissão.

Requisitos:
- Recusar rodar com **binário velho** (a página é `include_str!`) — a bateria de frontend já faz isso; herde a regra.
- Cada parte com o seu tempo, e o total.
- Código de saída honesto: falhou é diferente de pulou.
- **Não reescreva as baterias que existem** — orquestre-as. Elas foram provadas e têm dono.

### Metade 2 — a que importa: PROVAR QUE A PROVA PEGA

A regra da casa diz que todo teste novo tem de falhar com o defeito reposto. Isso hoje é feito **à mão, por frente, e depois se perde**: ninguém consegue dizer, hoje, quais das 1.229 asserções ainda pegariam o defeito que as motivou.

Construa o mecanismo que responde isso: **repor o defeito automaticamente e conferir que o teste cai.**

O caminho que eu recomendo, e você pode contrariar com motivo: um catálogo versionado de **defeitos repostos** — cada um com o arquivo, o trecho a trocar, o trecho de substituição, e **quais testes têm de falhar**. O executor aplica um por vez numa cópia da árvore, roda só os testes nomeados, confere que caem, desfaz, e segue. No fim, o relatório diz quantas guardas foram provadas e **quais não foram**.

Comece pelas que a casa já pagou caro, e cada uma está documentada com o defeito exato:
- o Profiler **recortando** em vez de analisar (7 testes têm de cair);
- o portão do Profiler ausente (o leitor lê o pedido alheio);
- `pivotar`, `sequencias` e `posicao` sem conferência própria — a família do `juntar`/`unir`;
- `descarregar_sujas` chamado com a trava na mão (o servidor trava; o teste tem **prazo**, senão pendura em vez de falhar);
- a cadeia de gatilhos sem teto (o binário **aborta**, exit 134);
- `excluir_tabela` com a lista curta de extensões;
- a conferência de SHA-256 do backup desligada;
- o AAD fora do slot cifrado;
- o cache de chaves derivadas não limpo na troca de senha;
- a catraca dos textos fora da fábrica.

**Se algum defeito reposto NÃO derrubar o teste que deveria, esse é o achado mais valioso da frente** — é um teste que passa por engano, e a casa considera isso pior que teste que falta. Persiga, e conserte ou registre com precisão.

Cuidados de execução:
- Trabalhe numa **cópia** da árvore, nunca na de verdade — e garanta que a árvore volta ao estado original mesmo se algo estourar no meio.
- Rode só os testes nomeados por cada defeito; rodar tudo a cada mutação custaria horas.
- **Um defeito que pendura em vez de falhar precisa de prazo**, senão a bateria trava. O teste do deadlock já aprendeu isso.

### Documente
Em `phxsql/docs/TESTES.md` (existe): o comando único, o que cada parte cobre, o que se pula e por quê, e a tabela das guardas provadas. E o aprendizado, **inclusive o infrutífero**.

No relatório final: o comando único e o que ele imprime, quantas guardas você conseguiu provar automaticamente, quais falharam em falhar (se houver), o que ficou de fora e por quê, e quanto tempo a bateria inteira leva.
```

---

## 43. Transações  ·  30/08 05:05

```
Você trabalha no PhxSql, em `phxsql/`. LEIA `CLAUDE.md` na raiz ANTES de tudo. Esta é a maior frente que o projeto tem pela frente, e a que mais pode estragar se for feita com pressa.

Regras que MANDAM aqui:
- **A ordem de digitação é sagrada.** O `.reg` nunca reaproveita slot excluído. **Qualquer desenho de transação que precise disso tem de ser recusado, não negociado.**
- **Guarda nova entra pedida, não imposta.** Quem nunca abrir transação tem de ver EXATAMENTE o comportamento de hoje, com o mesmo custo. O teste que mais importa é `sem_transacao_nada_muda`.
- **Mudança de formato entra cedo** — e esta mexe no `.log`, que é a fonte da replicação. Nos dois lados: réplica que não conhece a versão nova **continua aplicando**.
- **Toda bateria tem prova real:** o teste novo FALHA com o defeito reposto.
- **`Mutex` da `std` não é reentrante.** Já travou o servidor três vezes neste projeto — a última em configuração padrão, com escrita comum em duas tabelas. Quem já tem a trava chama a variante `_com`; quem não tem, a outra.
- Zero dependências além da `std`. Português; identificadores e comentários **sem acento**.
- Portões: `cargo fmt --all`, `cargo clippy --workspace --all-targets` zero avisos, `cargo test --workspace` verde, `node phxsql/testes-web/bateria.mjs` 24/24, e `python3 phxsql/bancada/guardas/provar-guardas.py` sem regressão.
- Portas **7000–7049**. Mate só o SEU processo pelo PID; **nunca `pkill -f`**. Ao terminar, `rm -rf phxsql/target`.
- **Sem push, sem PR.** Commite no seu branch de worktree.

## A tarefa: TRANSAÇÕES

Hoje não há `BEGIN`, `COMMIT` nem `ROLLBACK` de várias operações. É o que mantém falsa a afirmação *ACID compliant* da folha de marca — sem transação não há o **A** nem o **I**. E o dano é visível: o `inserir_lote` responde, literalmente, *«nao ha transacao: as linhas gravadas antes do erro ficaram gravadas»*. Eu vi isso hoje, numa carga de 2.500 que falhou na linha 1.

### Primeiro: o pré-requisito que é seu

**As 13 tomadas diretas de `self.dados.lock()` fora do ponto único `travar_dados()`** (`servidor.rs`) são o terreno onde a transação vai morar, e o comentário do `travar_dados()` **ainda afirma ser «o único lugar que a toma»** — é mentira medida. Traga-as para o ponto único ANTES de projetar a transação, com o cuidado que a terceira reincidência ensinou: converter mecanicamente é o que quase reintroduziu o deadlock num merge. Cada tomada convertida precisa da pergunta «quem chama isto já tem a trava?».

Este pré-requisito sozinho já é entrega. Se ele consumir a rodada, **entregue ele bem-feito e o desenho da transação escrito** — vale mais que as duas pela metade.

### Depois: o desenho, e ele vem antes do código

Escreva `phxsql/docs/TRANSACOES.md` **antes** de implementar, respondendo com o formato na mão:

1. **O que uma transação abrange.** Uma conexão? Uma sessão? Várias tabelas do mesmo database? Vários databases? Diga o escopo e o que fica de fora.
2. **Como desfazer, sem reaproveitar slot.** O `.trash` já guarda a linha inteira antes de sumir e o `.log` já pode guardar a imagem anterior. **A pergunta difícil é o `rollback` de um `inserir`**: o slot foi escrito, o rowid foi consumido, e a ordem de digitação proíbe reusá-lo. Um slot marcado como «nasceu e morreu» é uma resposta possível; há outras. **Escolha e defenda.**
3. **O isolamento.** Hoje uma trava única serializa tudo, o que dá serialização de graça mas nenhum paralelismo. Diga qual nível você entrega e qual não.
4. **O que acontece se o processo morrer no meio.** Sem marca de «não fechei direito» (o Aria tem 3 bytes para isso, o InnoDB tem o LSN do checkpoint, nós não temos nada), a recuperação não sabe o que reverter. Isso é peça da transação, não detalhe.
5. **A replicação.** A réplica aplica evento a evento. Uma transação revertida no master **não pode chegar aplicada na réplica**. Diga como, e prove com a bancada de Docker que já existe (`bancada/replicacao/docker/provar.py`, o daemon está no ar).
6. **O custo.** Meça o que a transação acrescenta a quem NÃO a usa. Se acrescentar algo mensurável, o desenho está errado. E use o binário certo: `cargo build --release --examples -p phxsql-store` antes de medir.

### E o que a tela já promete

Existe *Ferramentas → Gestão de transações*, e ela hoje **diz o que existe e o que não existe em vez de fingir**. Quando algo passar a existir, é ela que muda — leia antes.

No relatório final: o que ficou pronto do pré-requisito, o desenho escolhido com os cinco pontos respondidos, o que foi implementado, o custo medido para quem não usa, e o que ficou para a próxima rodada com o motivo.
```

---

## 44. A trava presa atrás da rede  ·  30/08 05:06

```
Você trabalha no PhxSql, em `phxsql/`. LEIA `CLAUDE.md` na raiz ANTES de tudo.

Regras que mais pegam nesta frente:
- **Diagnóstico plausível não é diagnóstico medido.** Esta casa já escreveu «o mutex era o pior pedaço» e mediu depois: o `lock` sem disputa custa 13,2 ns e o parse do lote 3.456 µs — 262.000× mais.
- **`Mutex` da `std` não é reentrante**, e já travou o servidor três vezes aqui.
- **Guarda nova entra pedida, não imposta**; o teste que mais importa é o do comportamento velho.
- **Toda bateria tem prova real:** o teste novo FALHA com o defeito reposto — e defeito que PENDURA em vez de falhar precisa de **prazo**, senão a bateria trava em vez de reprovar (foi o que o teste do deadlock aprendeu).
- Zero dependências além da `std`. Português; identificadores e comentários **sem acento**.
- Portões: `cargo fmt --all`, `cargo clippy --workspace --all-targets` zero avisos, `cargo test --workspace` verde.
- Portas **7050–7099**. Mate só o SEU processo pelo PID; **nunca `pkill -f`**. Ao terminar, `rm -rf phxsql/target`.
- **Sem push, sem PR.** Commite no seu branch de worktree.
- O **daemon do Docker está no ar** e não é seu: use, não pare, não reconfigure. Outro agente também está usando a máquina.

## A tarefa: a trava de dados presa atrás de uma leitura de rede

A bancada de replicação em contêiner mediu, e o número é feio:

- com corte silencioso da rede, `varrer` esperou **29.456 ms** na réplica enquanto o `ping` respondia em **6 ms** — ou seja, o servidor parecia vivo e não atendia dado nenhum;
- no **bidirecional**, sem corte nenhum, os dois lados se trancam um ao outro: 100.000 linhas nos dois lados levaram **33,3 s** contra **~5,8 s** do modo simples, com `EAGAIN` no diário de cada um.

A causa está no laço da réplica (`crates/phxsql-server/src/replica.rs`, e o que ele chama no `servidor.rs`): ele **segura a trava de dados enquanto lê do soquete**. Uma rede lenta, ou um par bidirecional, congela o servidor inteiro.

E há uma hipótese já **derrubada** que você não deve repetir: «o corte silencioso demora pela espera do SYN» — falso, o diário mostrava sete `EAGAIN` de 30 s em cada lado.

O que fazer:

1. **Meça primeiro, e ache o ponto exato.** Qual chamada segura a trava, e por quanto tempo. Use o que já existe: a telemetria cronometra a espera na fila da trava (`travar_dados()`), e o `.log` da réplica mostra os `EAGAIN`. Escreva o número antes de propor conserto.

2. **Separe a leitura de rede do trabalho no dado.** O desenho óbvio — ler o lote inteiro do soquete PRIMEIRO, e só então tomar a trava para aplicar — precisa de duas conferências: quanto o lote pode crescer em memória (teto declarado, e recusa clara ao ultrapassar), e o que acontece se a conexão cair entre a leitura e a aplicação. **Teste unitário não prova queda de conexão — soquete prova**, e a lição do `BULKINSERT` está no `CLAUDE.md`: `socket.makefile()` do Python segura o descritor, e fechar só o soquete deixa o fd aberto.

3. **Prazo em toda leitura de rede que acontece com a trava na mão** — se sobrar alguma. Sem prazo, um par que não responde é um servidor parado.

4. **Prove no contêiner, que é onde o defeito apareceu.** A bancada existe: `phxsql/bancada/replicacao/docker/provar.py`. Refaça as duas medições (corte silencioso e bidirecional) antes e depois, e ponha os dois números no documento. **Se o conserto não mover o número, ele não é conserto** — e isso também é resultado.

5. **O teste que trava a regressão** tem de ter prazo e falhar com o defeito reposto. Acrescente-o ao catálogo de guardas (`bancada/guardas/catalogo.py`), que já existe e roda em 162 s.

6. **Documente** em `docs/REPLICACAO.md` e no `DESEMPENHO.md`, inclusive o infrutífero.

No relatório final: o ponto exato que segurava a trava, os números antes e depois nas duas medições, o que você fez com o teto de memória e com a queda de conexão, e a guarda nova.
```

---

## 45. fsync da exclusão e rotação do log  ·  30/08 05:06

```
Você trabalha no PhxSql, em `phxsql/`. LEIA `CLAUDE.md` na raiz ANTES de tudo.

Regras que MANDAM nesta frente:
- **Guarda nova entra pedida, não imposta.** Hoje um `excluir` que responde OK **já está no disco**. Se a janela de durabilidade passar a valer para a exclusão POR PADRÃO, o significado da resposta muda para todo cliente que já existe, sem ninguém ter pedido. Isso é o estrago, não a proteção. O teste que mais importa é o do comportamento VELHO.
- **Medidor com binário velho mede o passado**: `cargo build --release --examples -p phxsql-store` antes de medir.
- **Toda bateria tem prova real:** o teste novo FALHA com o defeito reposto.
- Zero dependências além da `std`. Português; identificadores e comentários **sem acento**.
- Portões: `cargo fmt --all`, `cargo clippy --workspace --all-targets` zero avisos, `cargo test --workspace` verde.
- Portas **7100–7149**. Mate só o SEU processo pelo PID; **nunca `pkill -f`**. Ao terminar, `rm -rf phxsql/target`.
- **Sem push, sem PR.** Commite no seu branch de worktree.

## Tarefa 1 (a principal): o `fsync` da exclusão

É o **sprint nº 1** de `docs/SPRINTS.md`, e o único da lista com número grande medido: **6,5 s → 0,83 s em 20.000 exclusões, 7,8×**. E importa porque **excluir é a única fase da bancada em que o PhxSql perde para o MySQL®**: 6,27 s contra 4,73 s em 20.000 linhas, na bancada de dez milhões.

A janela de durabilidade já existe para a escrita (`gravar_de_verdade`, `lote_operacoes`, `descarregar_sujas`/`descarregar_sujas_com` no `servidor.rs`) — **leia esse mecanismo inteiro antes de mexer**, inclusive o comentário que separa quem tem a trava de quem não tem. Foi ali que nasceu o travamento em configuração padrão.

O que fazer, e a ordem importa:

1. **Refaça a medição em máquina quieta** e confirme o 7,8×. O sprint traz o critério de morte combinado ANTES: **abaixo de 2× o item morre**, e a recusa com o número é resultado tão válido quanto o ganho. Não mude o critério depois de medir — isso é escolher o resultado.
2. **A exclusão entra na janela PEDIDA, não por padrão.** Quem não pedir continua com a garantia de hoje. O documento do sprint registra que a proposta original do Cassandra punha por padrão, e que a casa recusou por isto.
3. **Prove que não existe queda em que a linha suma dos dois lados** — é a conferência que o sprint exige. Com a exclusão na janela, há um intervalo entre responder OK e o dado estar no disco: diga exatamente o que se perde se o processo morrer nesse intervalo, e prove.
4. **A tela e o `config.json`** ganham o interruptor pelo ponto único (`CAMPOS_CONHECIDOS`, `CAMPOS_EDITAVEIS`, `editaveis_json()`), e o campo tem de ser **lido de verdade** — «configuração que não é lida mente» já apareceu três vezes neste projeto, uma delas num campo de segurança.
5. **Refaça a fase `excluir` da bancada** e diga o número novo contra os 4,73 s do MySQL®. Se virarmos a fase, é a primeira vez que o PhxSql ganha em todas.

## Tarefa 2 (menor, e independente): a rotação do `.txt` do Profiler

Anotada com número: **345 bytes por pedido, sem teto — 1,2 GB por hora** a mil pedidos/s. Enche disco em silêncio.

Faça a rotação por tamanho (e diga por que não por tempo, ou vice-versa), com teto e contagem de arquivos configuráveis pelo mesmo ponto único. Dois cuidados que o Profiler já pagou: **linha forjada** (um `"op"` com quebra de linha dentro deixava no arquivo uma segunda linha que se lia como evento de outro IP) e **disco cheio em silêncio** — a tela seguia dizendo «gravando em …» com 223 de 400 linhas gravadas. A rotação não pode reabrir nenhum dos dois.

Se as duas tarefas não couberem, **a 1 é a que importa**; entregue-a inteira e diga que a 2 ficou.

No relatório final: o número refeito em máquina quieta, se ele sobreviveu ao critério de morte, o que se perde na janela e como você provou, o número novo da fase `excluir` contra o MySQL®, e o que ficou da tarefa 2.
```

---

## 46. ALTER TABLE ADD COLUMN  ·  30/08 05:07

```
Você trabalha no PhxSql, em `phxsql/`. LEIA `CLAUDE.md` na raiz ANTES de tudo.

Regras que MANDAM nesta frente:
- **A ordem de digitação é sagrada** — o `.reg` nunca reaproveita slot excluído, e **o rowid é endereço**. Qualquer desenho que renumere rowid está errado antes de começar.
- **Coluna de sistema nova quebra quem filtra pela primeira.** É a lição do `rownum`, que quebrou *todo salvar e todo incluir* pela tela. E ela vale em dobro aqui: **a coluna nova entra ANTES de `softdeleted` e `rownum`**, deslocando as colunas de sistema. Procure quem usa `find(...)` onde devia usar `filter(...)`, e quem assume posição fixa.
- **Guarda nova entra pedida, não imposta**; tabela que ninguém altera tem de custar o mesmo de hoje.
- **Mudança de formato entra cedo**, e esta mexe no bloco de esquema (PSCH) e possivelmente no `.reg`. Documente no `FORMATO.md` **no mesmo commit**.
- **Toda bateria tem prova real:** o teste novo FALHA com o defeito reposto.
- Zero dependências além da `std`. Português; identificadores e comentários **sem acento**.
- Portões: `cargo fmt --all`, `cargo clippy --workspace --all-targets` zero avisos, `cargo test --workspace` verde, `node phxsql/testes-web/bateria.mjs` 24/24.
- Portas **7150–7199**. Mate só o SEU processo pelo PID; **nunca `pkill -f`**. Ao terminar, `rm -rf phxsql/target`.
- **Sem push, sem PR.** Commite no seu branch de worktree.

## A tarefa: `ALTER TABLE ADD COLUMN`, preservando o rowid

É o sprint **25** de `docs/SPRINTS.md` e é o que falta ao pedido **127** (o editor de modelo). Hoje **não dá para acrescentar coluna a uma tabela que já tem dado** — e isso é o que qualquer sistema em produção precisa no segundo mês. O cartão da tela já diz isso em vez de fingir; leia antes.

O ponto difícil está no formato: o `.reg` tem **slots de largura fixa**, e o endereço do registro sai de uma multiplicação. Acrescentar coluna aumenta a largura do slot, então **todo o arquivo se move** — e o rowid, que é o endereço, **não pode mudar**.

O que fazer:

1. **Leia o formato primeiro**: `docs/FORMATO.md` §1 (o `.reg`), o bloco de esquema PSCH e as versões dele (a v6 trouxe a marca LGPD por coluna, a v5 do `.reg` trouxe a etiqueta da cifra). E leia como o `duplicar_tabela` e o `restaurar_backup` copiam — os dois já passeiam pelo arquivo inteiro e podem ser o modelo.

2. **Escolha o desenho e defenda.** As saídas que eu enxergo, avalie todas:
   - **(a) reescrever a tabela** num arquivo novo, slot por slot, na mesma ordem, e trocar no fim — o rowid se preserva porque a ordem se preserva. Custa uma passada inteira, e a casa dos minutos para dez milhões (**inferido, não medido — meça**);
   - **(b) coluna «à direita» com valor ausente**, sem reescrever, se o formato permitir slot de duas larguras convivendo (provavelmente não permite — confira, não presuma);
   - **(c) só em tabela vazia**, que é o que o Aria exige para desligar índice — resposta honesta e pequena, mas resolve pouco.
   Diga o custo de cada uma **com número**, e o que cada uma NÃO faz.

3. **A coluna nova nasce com quê?** Valor padrão declarado, ou nulo? Se a coluna for obrigatória, o que acontece com as linhas que já existem? Essa é a pergunta que a maioria das implementações erra, e ela é de desenho, não de código.

4. **O que acontece se o processo morrer no meio da reescrita.** Sem isso, `ALTER TABLE` é uma roleta. O `restaurar_backup` já resolveu um problema parecido com um palco fora da raiz e troca no fim — leia como ele faz, e diga se serve.

5. **Os arquivos irmãos**: `.ndx` (os índices apontam para rowid — se o rowid não muda, eles sobrevivem?), `.bkp` (o espelho é clone byte a byte do `.reg`), `.lgpd`, `.pag`. Diga o que acontece com cada um.

6. **Prove exercitando**, e não só por teste: tabela com dado de verdade, coluna acrescentada, e depois **ler, inserir, atualizar, excluir, restaurar do backup e replicar** — a replicação porque o esquema mudou dos dois lados. Playwright em `/opt/node22/lib/node_modules/playwright/index.mjs` para a tela; o daemon do Docker está no ar e a bancada de replicação em contêiner existe.

7. **A tela**: o editor de modelo já existe e já diz o que falta. Ligue o que passar a existir.

Se o desenho concluir que a rodada não cabe, **entregue o desenho medido com os números das três saídas** — isso vale mais que meia implementação, e é o que permite decidir.

No relatório final: a saída escolhida com o custo medido, o que acontece com cada arquivo irmão, o que a coluna nova recebe nas linhas antigas, a resposta para a morte no meio, e o que ficou.
```

---

## 47. Aperto de mão Noise  ·  30/08 05:20

```
Voce trabalha no PhxSql, motor de dados em Rust do Adriano Boller, em `phxsql/` do repositorio. LEIA `CLAUDE.md` na raiz ANTES de qualquer coisa: as regras de la mandam sobre este pedido inteiro.

Use as portas 7200 a 7249 nas suas provas. NUNCA mate processo `phxsqld` de outro agente: `pkill -f` e proibido, mate so por PID que voce mesmo iniciou.

# A frente: cifra do fio por aperto de mao estilo Noise

O dono decidiu, entre tres alternativas: **aperto de mao estilo Noise (X25519 + HKDF + ChaCha20-Poly1305)**. Nao e TLS, e navegador nao fala isso — isso e limite aceito e tem de ficar escrito, nao escondido.

## O que ja existe aqui, conferido contra vetor oficial

- `crates/phxsql-core/src/cifra.rs`: ChaCha20-Poly1305 (RFC 8439, cinco vetores), XChaCha20/HChaCha20
- SHA-256 (FIPS 180-4), HMAC-SHA256 (RFC 4231), PBKDF2 — todos com vetor
- O login por desafio-resposta (`docs/SEGURANCA.md`)

**Falta so a troca de chaves.** Nao reescreva o que ja existe; reaproveite.

## Escreva o desenho ANTES de escrever codigo

Crie `phxsql/docs/CIFRA-DO-FIO.md` respondendo, com argumento e nao com assercao:

1. **Qual padrao Noise.** O servidor tem chave estatica X25519; o cliente a fixa (estilo `known_hosts` do SSH) ou aprende na primeira vez (TOFU)? Autenticacao mutua por chave substituiria ou conviveria com o desafio-resposta de usuario que ja existe? Recomende um e diga o que o descartado dava e voce abriu mao.

2. **O problema do rebaixamento — este e O ponto.** A regra da casa diz «guarda nova entra pedida, nao imposta». Mas cifra *pedida* e cifra que o atacante ativo simplesmente apaga do pedido, e ai a protecao vira zero contra quem esta no meio. Resolva assim e argumente se concorda: opcao `exigir_cifra` na configuracao do servidor, **desligada por padrao** — cliente velho continua exatamente como hoje, que e a regra petrea — e ligada recusa texto claro. E **escreva em palavras claras** que com ela desligada a protecao vale contra escuta **passiva** apenas. Nao deixe o documento sugerir mais do que entrega.

3. **Disciplina do nonce.** Contador por direcao, nunca reusado. O que acontece no esgotamento do contador — rechaveia ou fecha? Decida e implemente o que decidir.

4. **Truncamento e repeticao.** O hash da transcricao tem de cobrir o aperto de mao inteiro. A camada de registro tem de distinguir «fim de conversa» de «fio cortado no meio» — fio cortado e erro, nunca sucesso silencioso.

5. **Onde vale e onde nao vale.** Protocolo binario da porta 5000 e transporte da replicacao: sim. Interface web: **nao**, e diga por que (o navegador fala TLS ou nada). Nao venda o que nao entrega.

## O codigo

- **X25519 conforme RFC 7748**, tempo constante, sem tabela. Confira contra **todos** os vetores oficiais que der: os dois de multiplicacao escalar da secao 5.2, o Diffie-Hellman da 6.1, e as iteracoes de 1 e de 1.000 vezes. Recuse o segredo compartilhado todo-zeros (ponto de ordem pequena) — isso e teste, nao comentario.
- **HKDF conforme RFC 5869**, sobre o HMAC que ja existe. Os tres casos de teste SHA-256 do anexo A.
- Zero dependencias. So a `std`. `cargo build --offline` tem de funcionar. Se algo parecer exigir uma crate, **pare e me pergunte** em vez de acrescentar.

## Os testes

- **O teste que mais importa e o do comportamento VELHO**: `cliente_sem_cifra_continua_como_antes`. Cliente que nunca ouviu falar do aperto de mao grava e le igual a hoje.
- **Prova real, nos dois sentidos**: cada teste novo tem de **falhar com o defeito reposto** e passar com o conserto. Ja houve aqui teste que passava por engano, e ele e pior que teste que falta. Demonstre a reposicao de cada defeito no relatorio final — nao afirme, mostre a saida.
- Prova por soquete de verdade para o que depende do sistema operacional. Cuidado com a armadilha ja paga aqui: `socket.makefile()` do Python segura o descritor, e fechar so o soquete deixa o fd aberto, entao o servidor nunca ve o fim da conexao.
- Prazo em todo teste que possa travar em vez de falhar.

## Portoes, antes de terminar

```
cargo fmt --all
cargo clippy --workspace --all-targets    # zero avisos
cargo test --workspace
```

## Documentos a atualizar no mesmo commit

- `docs/SEGURANCA.md` §7 — hoje «Sem TLS» aparece como ausencia; vira decisao com o limite escrito
- `docs/REPLICACAO.md` §13 e `docs/PENDENCIAS.md` item 8 da tabela de danos
- `docs/FORMATO.md` **se** mexer em formato em disco

## Estilo, que aqui e regra

Codigo, comentarios, documentacao e mensagem de commit em **portugues**. Identificadores e comentarios **sem acento**. Comentario explica **por que**, nao o que. Mensagem de commit conta a decisao e o motivo, nao a lista de arquivos. **Nenhum identificador de modelo** em nada que va para o repositorio.

Se aparecer texto de tela, ele entra pela fabrica de idiomas (`phxsys.mensagens`, `FABRICA_TELA` em `idiomas.rs`, `data-txt`) — isso e petreo — e a catraca do `conferidor.rs` nao sobe.

Commite no seu worktree. **Nao faca push e nao abra PR.** No relatorio final me diga: o padrao escolhido e por que, os numeros dos vetores que passaram, a saida de cada defeito reposto, e o que voce decidiu NAO fazer.
```

---

## 48. Traduzir o multitela.js  ·  30/08 05:26

```
Voce trabalha no PhxSql, motor de dados em Rust do Adriano Boller, em `phxsql/`. LEIA `CLAUDE.md` na raiz ANTES de qualquer coisa: as regras de la mandam sobre este pedido inteiro. Trabalhe a partir do commit `a95e8a4`.

Portas 7250 a 7299 nas suas provas. NUNCA mate `phxsqld` de outro agente: `pkill -f` e proibido, mate so por PID que voce iniciou.

# A frente: os 69 textos cravados do multitela.js

A regra e **petrea**, palavra do dono: texto de tela entra pela fabrica de idiomas. A maquina existe desde a 0.17.0 — `phxsys.mensagens`, a `FABRICA_TELA` do `crates/phxsql-server/src/idiomas.rs`, o atributo `data-txt`, e o procedimento escrito em `docs/MENSAGENS.md`.

Acabei de descobrir e consertar que o conferidor **nao media** o `ui/multitela.js`: ele era servido pelo `http.rs` e nao estava no `FONTES`. Entrou, e com ele **69 textos cravados** que nunca contaram. A catraca (`TETO` no `conferidor.rs`) subiu de 1.999 para **2.068**, que e a primeira medida sobre a interface inteira.

**Sua tarefa e derrubar essa catraca traduzindo o que der.** Liste os 69 com:
```
cargo run --example textos-fora-da-fabrica -p phxsql-server -- --tudo
```

## O achado que decide o desenho — leia antes de comecar

Os 69 sao **duas populacoes diferentes**, e tratar as duas igual da trabalho jogado fora:

**Cerca de 30 traduzem-se limpo.** Dicas (`pinar aqui`, `devolver`, `Redimensionar`, `Alinhar`), os nomes das telas (`Painel`, `Query`, `Diagrama ER`, `Telemetria`, `Profiler`, `Usuarios` — **confira se ja nao existem chaves para esses**, porque chave duplicada e pior que chave faltando), e os recados (`o navegador bloqueou a janela — libere o popup desta origem`, `a janela solta nao cabia onde estava guardada — foi presa dentro da area visivel`).

**Cerca de 39 NAO traduzem como estao.** Sao uma frase **picada pela marcacao**, nas linhas 1090-1101 e 1401-1447:
```
"Abas vivas e regioes lado a lado funcionam em" <b>qualquer navegador</b> "— e layout. Destacar em janela tambem, com" <code>...</code> ...
```
Traduzir o fragmento `"— e layout. Destacar em janela tambem, com"` sozinho e **impossivel**: a ordem das palavras muda de idioma para idioma, e em alemao o verbo vai para o fim. Frase picada nao se traduz.

**O desenho certo: a frase inteira vira UMA chave**, com o destaque como marcador dentro dela — algo como `Abas vivas e regioes lado a lado funcionam em <b>qualquer navegador</b> — e layout.` numa chave so, e o tradutor move o `<b>` para onde o idioma dele pede. Decida o mecanismo (a fabrica ja aceita marcacao no valor? Se nao, o que muda?), **argumente a escolha em `docs/MENSAGENS.md`**, e registre a licao: *frase picada por marcacao e intraduzivel por construcao — o corte tem de acontecer depois da traducao, nunca antes.*

Se algum bloco for grande demais para caber bem nesta rodada, **deixe-o de fora nomeando cada texto no `docs/PENDENCIAS.md`** — foi o que a frente anterior fez com os paragrafos da tela de cores, e o precedente esta escrito no comentario do `TETO`. Melhor deixar seis na fila que traduzir seis as pressas.

## As tres armadilhas que esta frente ja pagou

- **Rotulo se traduz; dado, nunca.** E a licao do «Blumenau» virando «BLUMENAU». O que a pagina interpola (`${…}`) nao se toca.
- **Texto se resolve por CHAVE, nunca por comparacao da frase.** No dia em que alguem melhorar a redacao, quem compara frase quebra calado, mostrando portugues.
- **Chave morta e pior que chave faltando.** O tradutor a ve na tabela, traduz nos seis idiomas, e nada muda na tela. Ha teste para os dois lados do laco.

Sao **seis idiomas**: pt, fr, en, it, de, es. Traduza de verdade — nao deixe portugues repetido nas outras colunas.

## Baixe a catraca no mesmo commit

Traduziu, mede de novo e poe o `TETO` no numero medido. **Catraca frouxa nao segura nada** — ha um `assert` que reprova quem traduz e esquece de baixar. Nao invente o numero: rode o exemplo e leia.

## Prove exercitando, nao lendo

**Interface so se prova exercitando** — regra da casa, e ela nasceu de tres defeitos achados em cinco minutos de video que ler o codigo nao acharia. Suba o servidor numa porta sua, abra o modo multitela no navegador, **troque o idioma** e confira que os textos mudam de verdade. O CSS global morde componente novo: confira que nada ficou em MAIUSCULA por causa de `label{text-transform:uppercase}` nem esticado por `input{width:100%}`.

## Portoes

```
cargo fmt --all
cargo clippy --workspace --all-targets    # zero avisos
cargo test --workspace
```
Mexeu no `index.html` ou em qualquer `ui/`? `cargo build --release -p phxsql-server --bin phxsqld` antes de exercitar, senao voce testa o binario velho — armadilha ja paga tres vezes aqui.

## Estilo

Codigo, comentarios, documentacao e mensagem de commit em **portugues**. Identificadores e comentarios **sem acento** (texto de interface pode ter). Comentario explica **por que**. Mensagem de commit conta a decisao e o motivo, nao a lista de arquivos. **Nenhum identificador de modelo** em nada que va para o repositorio.

Commite no seu worktree. **Nao faca push e nao abra PR.** No relatorio final: quantos traduziu, o numero medido da catraca, como resolveu a frase picada, e o que deixou na fila com o motivo.
```

---

## 49. BEGIN COMMIT ROLLBACK SAVEPOINT  ·  30/08 14:54

```
Voce trabalha no PhxSql, motor de dados em Rust do Adriano Boller, em `phxsql/`. LEIA `CLAUDE.md` na raiz ANTES de qualquer coisa. Portas 7300 a 7349. NUNCA `pkill -f`; mate so por PID que voce iniciou. Trabalhe a partir do commit `0541f88`.

# A frente: BEGIN / COMMIT / ROLLBACK / SAVEPOINT

O terreno ja esta pronto (ponto unico de trava, `travar_dados()`), e **o desenho ja esta escrito e decidido** em `docs/TRANSACOES.md`. **Leia-o inteiro antes de escrever uma linha.** Voce implementa o que esta la; nao reabra as decisoes ja fundamentadas.

O resumo do que ja foi decidido: **nada vai a disco antes do `COMMIT`.** A transacao empilha o conjunto de escrita em RAM; desfazer e jogar a lista fora. Nenhum slot queimado, nenhum rowid consumido, a **ordem de digitacao intacta** -- que e a regra petrea do projeto. Escopo por conexao, varias tabelas, um database. Marca `transacao_<id>.tx` para a queda no meio, com recuperacao que anda para a frente. O `.log` **nao muda**.

## O dono mandou um capitulo de manual sobre transacoes nos cinco SGBDs

Ele estudou MySQL/InnoDB, MariaDB, PostgreSQL, SQL Server e Oracle, e recomendou uma arquitetura. **Nem tudo dela cabe aqui, e o que cabe e otimo.** Abaixo, o que ENTRA nesta rodada, o que ENTRA MEDIDO, e o que NAO entra e por que -- cada um desses "nao" tem de ficar escrito no `TRANSACOES.md`, com o motivo, para ninguem propor de novo sem medir.

### Entra, direto

1. **A maquina de estados com `ABORT_ONLY`.** E a melhor ideia do capitulo e custa quase nada. Estados: `IDLE`, `ACTIVE`, `FAILED`/`ABORT_ONLY`, `COMMITTING`, `COMMITTED`, `ROLLING_BACK`, `ROLLED_BACK`. Depois de erro grave, um `COMMIT` **recusa** dizendo que a transacao nao pode ser confirmada e exige `ROLLBACK` -- em vez de confirmar trabalho meio invalido. E o `XACT_STATE()` do SQL Server casado com o estado abortado do PostgreSQL. Exponha o estado ao cliente (ele listou um conjunto de campos: `transaction_id`, `transaction_state`, `transaction_start_time`, `transaction_isolation`, `transaction_read_only`, idade, contagem de linhas empilhadas -- entregue os que fazem sentido aqui e diga quais nao fazem).

2. **Classes de erro** (o §29 dele): erro de INSTRUCAO (chave duplicada) cancela a instrucao e a transacao segue `ACTIVE`; erro de TRANSACAO leva a `ABORT_ONLY`; queda da conexao desfaz sozinha. A aplicacao precisa saber diferenciar, entao o erro tem de **dizer qual e**.

3. **`SAVEPOINT` pela ideia do §27 dele, que encaixa perfeitamente aqui**: nao se copia a transacao, guarda-se um **indice na lista** de escrita empilhada. `ROLLBACK TO SAVEPOINT` trunca a lista naquele ponto e a transacao **continua aberta**. Num desenho em que tudo esta em RAM isso e quase de graca -- diga no documento que foi ele quem tornou barato. Aceite `SAVEPOINT nome`, `ROLLBACK TO SAVEPOINT nome`, `RELEASE SAVEPOINT nome`.

4. **Sinonimos de abertura**: `START TRANSACTION`, `BEGIN`, `BEGIN TRANSACTION`.

5. **Autocommit ligado por padrao.** Isso E a regra petrea da casa dita com outras palavras: cliente que nunca manda `BEGIN` continua exatamente como hoje. **O teste que mais importa e o do comportamento VELHO**: `sem_transacao_nada_muda`.

6. **O relatorio de recuperacao** que ele desenhou (o bloco "PHXSQL Recovery" com transacoes achadas, confirmadas recuperadas, incompletas revertidas). Adapte ao que esta marca `.tx` realmente sabe -- **nao invente linha que voce nao mede**. Se nao ha pagina refeita, nao imprima "Pages redone".

### Entra so se a MEDIDA sustentar

7. **Group commit.** Regra da casa: *receita de fora se mede contra o nosso gargalo antes de virar plano* -- e ja houve um precedente exato, uma arquitetura de escrita inteira que chegou pronta e da qual so duas de dez propostas eram reais aqui. **Meça antes.** Dois fatos para partir: o `.ndx` caiu de 83,5% para **63,6%** do tempo de uma insercao depois do cache de paginas (`docs/DESEMPENHO.md`), e a rodada passada mediu o `fsync` da exclusao valendo **3,10x** quando agrupado -- ou seja, **o `fsync` importa em alguns caminhos e nao em outros**, e a resposta e por operacao. Acorde o **criterio de morte antes de medir** (sugiro: abaixo de 1,5x a hipotese morre) e escreva a recusa com o numero no `DESEMPENHO.md` se ela morrer. Recusa medida e resultado tao valido quanto ganho.

### NAO entra, e o motivo vai escrito

8. **MVCC.** Aqui o rowid **e o endereco** -- e o que da o O(1). Uma segunda versao da linha pede um segundo slot, logo um segundo rowid, e isso quebra a regra petrea da ordem de digitacao E quebra a replicacao, cujo `aplicar_evento` para quando o rowid diverge. Nao implemente. Escreva no documento por que, com essas palavras.

9. **WAL, undo log, PageLSN, full-page-write, VACUUM.** Todos existem para o problema que o desenho de "nada a disco antes do COMMIT" **nao tem**: nao ha pagina suja confirmada para refazer, nem versao velha para limpar. Um ponto que vale conferir e escrever: o `.reg` guarda slots de tamanho fixo **com CRC-32**, entao uma escrita rasgada e **detectavel**, e nao silenciosa -- confira se um slot pode cruzar fronteira de setor e diga o que isso significa para a necessidade de full-page-write aqui. Se descobrir que precisa, diga com a medida.

10. **Deteccao de deadlock.** O desenho usa **reserva de tabela SEM espera**, entao nao ha grafo de espera nem abraco mortal possivel. Isso e uma resposta mais forte que detectar, e tem de ficar dito -- nao omitido.

11. **DDL transacional.** O `ALTER TABLE ADD COLUMN` que acabou de entrar ja tem duas fases e ponto de compromisso (escreve todos os `*.novo`, sincroniza, e so entao troca com `rename`, volume 1 primeiro). Diga o quanto isso ja e, e o que falta para ser DDL dentro de transacao. **Nao implemente nesta rodada.**

## O contrato que ele escreveu, e que vale como criterio

> «Se o computador perder energia exatamente nesta instrucao, depois de reiniciar o banco ele conseguira determinar de forma inequivoca se esta transacao foi COMMITTED ou ABORTED?»

Passe cada ponto do seu codigo por essa pergunta e **escreva a resposta**. Onde a resposta for "nao", diga -- a §5.4 do documento ja tem um lugar para o que continua sem cobertura.

## Provas

- **Prova real nos dois sentidos**: todo teste novo tem de **falhar com o defeito reposto** e passar com o conserto. Mostre a saida de cada reposicao no relatorio; nao afirme.
- **Prova por soquete** para o que depende do sistema operacional -- inclusive matar o processo com `SIGKILL` no meio de um `COMMIT` e conferir que o banco reabre sabendo dizer o que aconteceu. Cuidado com a armadilha ja paga aqui: `socket.makefile()` do Python segura o descritor, e fechar so o soquete deixa o fd aberto.
- Prazo em todo teste que possa **travar** em vez de falhar.
- Registre as guardas novas no `bancada/guardas/catalogo.py` (hoje 37) e a parte nova no `provar.py` (hoje 20 partes).

## Tela

A tela de Gestao de transacoes **nao foi tocada de proposito** enquanto nada existia. Agora passa a existir, entao ela pode entrar -- e **todo texto entra pela fabrica de idiomas**, que e petreo: `phxsys.mensagens`, `FABRICA_TELA` do `idiomas.rs`, `data-txt`, procedimento em `docs/MENSAGENS.md`. A catraca do `conferidor.rs` esta em **1.996** e **nao sobe**. Nos seis idiomas: pt, fr, en, it, de, es. E **interface so se prova exercitando** -- abra no navegador, troque o idioma, olhe. O CSS global morde componente novo (`input{width:100%}`, `label{text-transform:uppercase}`).

## Portoes

```
cargo fmt --all
cargo clippy --workspace --all-targets    # zero avisos
cargo test --workspace                    # hoje 1.328 verdes
```
Mexeu em `ui/`? `cargo build --release -p phxsql-server --bin phxsqld` antes de exercitar.

## Documentos

`docs/TRANSACOES.md` (o principal), `docs/SQL.md`, `docs/PENDENCIAS.md`, `CHANGELOG.md`. Mexeu no formato em disco? `docs/FORMATO.md` no mesmo commit.

**E o mais importante:** so escreva *ACID compliant* se as quatro letras estiverem entregues e provadas. A folha de marca afirma isso e o `CLAUDE.md` diz que e falso. Se esta rodada tornar o A e o I verdadeiros, diga exatamente o que passou a valer e o que ainda nao -- com o nivel de isolamento chamado pelo nome certo, sem enfeite. **Nao e ANSI SERIALIZABLE e nao pode ser chamado assim.**

## Estilo

Portugues em codigo, comentarios, documentacao e mensagem de commit. Identificadores e comentarios **sem acento**. Comentario explica **por que**. **Nenhum identificador de modelo** em nada que va para o repositorio.

Commite no seu worktree. **Nao faca push e nao abra PR.** No relatorio final: o que entrou, o numero do group commit (ganhou ou morreu), a saida de cada defeito reposto, e o que ficou de fora com o motivo.
```

---

## 50. PhxSql embutido via FFI  ·  30/08 15:44

```
Voce trabalha no PhxSql, motor de dados em Rust do Adriano Boller, em `phxsql/`. LEIA `CLAUDE.md` na raiz ANTES de qualquer coisa. Portas 7400 a 7449. NUNCA `pkill -f`; mate so por PID que voce iniciou. Parta do commit `409c2ae`.

**Disco esta apertado (~5 GB livres, outro agente compilando).** Remova `phxsql/target` do seu worktree ao terminar, e evite compilar alvos que nao precisa.

# A frente: PhxSql Embutido -- o motor como biblioteca, com FFI em C

## O pedido do dono, e a correcao de rumo que ele precisa ouvir

Ele escreveu: *«no HFSQL(R) nao roda o servidor, apenas as tabelas soltas sem cuidado, mas julgo que poderia ter um mini servidor para rodar no Android e no iOS off-line e se conectar por TCP/IP MULTILINK DATABASE/dblink com o servidor»*.

O objetivo esta certo: **banco local no aparelho, offline, que sincroniza com o servidor central**. A FORMA -- «mini servidor escutando porta» -- e a peca a corrigir, e o motivo e do sistema operacional, nao nosso:

- **iOS nao permite** processo de longa duracao em segundo plano nem app escutando porta para outros apps usarem. Um «mini servidor» ali nao e dificil: e proibido.
- **Android mata** processo em segundo plano com liberdade. Um daemon que escuta porta sobrevive mal fora do Termux.

**A forma certa e a mesma maquina com outra porta de entrada: biblioteca EMBUTIDA no processo do app** (sem porta, sem daemon), mais um **cliente de sincronia** que fala TCP com o servidor central quando ha rede. O motor e o mesmo; o que muda e quem chama.

E aqui esta a boa noticia que voce deve conferir e escrever: **o `phxsql-store` JA E o banco embutido.** O `phxsql-server` e um envelope de rede em volta dele. Entao a frente nao e reescrever nada -- e expor o que existe por uma ABI de C.

## O que entregar

### 1. Um crate novo, `phxsql-ffi`

`cdylib` (o `.so` do Android) **e** `staticlib` (o `.a` que o iOS exige, porque a Apple nao aceita biblioteca dinamica de terceiros no app). Nao mexa no `phxsql-server`; isto e porta de entrada nova, nao troca.

Zero dependencias externas continua valendo -- inclusive aqui, e e o que torna o iOS plausivel.

### 2. A superficie da ABI

Desenhe e justifique em `docs/EMBUTIDO.md` **antes** de escrever. No minimo: abrir e fechar base, criar tabela, inserir, atualizar, excluir, ler por rowid, varrer com cursor, e os ganchos de replicacao. Decida e escreva:

- **Nenhum `panic` atravessa a fronteira.** `catch_unwind` em toda funcao exportada; um panic virando *undefined behavior* no app do cliente e o pior defeito possivel. Isso e teste, nao comentario.
- **Como o erro volta**: codigo de retorno mais uma funcao de ultimo-erro, ou struct de resultado. Escolha uma e diga por que.
- **Quem aloca e quem libera**: buffer do chamador, ou nosso com funcao de liberar. Vazamento em app de celular aparece como o app sendo morto pelo sistema.
- **Seguranca de thread**: diga o que vale, com todas as letras. Nao prometa o que nao testou.
- **Strings**: UTF-8 com tamanho explicito, nao `NUL`-terminado sozinho -- dado de cliente tem byte zero.

### 3. Prova real

- Um programa em **C** que liga contra a biblioteca, cria tabela, grava e le. Compile e RODE.
- Rode tambem contra a `.so` **ARM64**, sob `qemu-aarch64-static` (ja instalado): ha bancada em `bancada/arm/provar.sh` mostrando o caminho.
- Todo teste novo tem de **falhar com o defeito reposto** e passar com o conserto. Mostre a saida.
- Guardas novas no `bancada/guardas/catalogo.py` (hoje 37).

### 4. O que NAO fazer nesta rodada

- Camada JNI para Android e camada Swift/ObjC para iOS -- **so o desenho**, escrito, dizendo o que falta. A ABI de C e o degrau que serve aos dois.
- Nao toque na cifra nem na replicacao alem de expor o que ja existe.

## Portoes

```
cargo fmt --all
cargo clippy --workspace --all-targets    # zero avisos
cargo test --workspace                    # hoje 1.328 verdes
```

## Estilo

Portugues em codigo, comentarios, documentacao e commit. Identificadores e comentarios **sem acento**. Comentario explica **por que**. **Nenhum identificador de modelo** em nada que va para o repositorio.

Commite no seu worktree. **Nao faca push e nao abra PR.** No relatorio: a ABI escolhida e por que, a saida do programa em C rodando (x86 e ARM), cada defeito reposto, e o que ficou de fora com o motivo.
```

---

## 51. PhxSql contra SQLite, medido  ·  30/08 15:45

```
Voce trabalha no PhxSql, motor de dados em Rust do Adriano Boller, em `phxsql/`. LEIA `CLAUDE.md` na raiz ANTES de qualquer coisa. Portas 7450 a 7499. NUNCA `pkill -f`; mate so por PID que voce iniciou. Parta do commit `409c2ae`.

**Disco apertado (~5 GB, dois agentes compilando).** Esta frente e de MEDICAO e DOCUMENTO: use o binario ja compilado em `target/release/phxsqld` e nao recompile o mundo. Se precisar compilar, so o que faltar. Remova qualquer `target` extra ao terminar.

# A frente: PhxSql no celular contra o SQLite, medido -- e o Windows sob Wine

O dono perguntou: *«como o PhxSql mobile pode ser melhor que o SQLite e o HFSQL(R) no celular?»*. A resposta so vale medida, e ela **tem de dizer tambem onde o SQLite ganha** -- um documento que so elogia a casa nao serve para decidir nada.

## Parte 1 -- a bancada contra o SQLite

O `sqlite3` vem na biblioteca padrao do Python, entao nao ha o que instalar.

**A regra que esta frente NAO pode quebrar** (`bancada/LEIA-ME.md`): *bancada compara trabalho igual, nao so pergunta igual*. Esta casa ja errou duas vezes nisso, para os dois lados -- um `WHERE id IN (...)` contra vinte mil buscas separadas (41x a favor do outro), e um `COUNT(*)+SUM` sobre 1.250.000 linhas contra a leitura de 20.000 (5x a favor do nosso). **Leia as quatro regras de la antes de escrever uma linha de bancada.**

Cuidados especificos deste par:

- O SQLite e **biblioteca em processo**; o PhxSql de hoje e **servidor por soquete**. Comparar chamada de funcao com ida e volta de rede nao e trabalho igual. Meça o custo do transporte SEPARADO e diga quanto dele e do soquete -- e note que a frente do PhxSql Embutido (FFI) esta tirando exatamente essa diferenca.
- O SQLite por padrao faz `fsync` por transacao. Se voce agrupar de um lado e nao do outro, o numero mente. Deixe explicito o modo de durabilidade dos dois (`journal_mode`, `synchronous`) e meça com o mesmo compromisso.
- Rode cada medida **mais de uma vez** e publique a mediana e a dispersao. A maquina nao esta quieta.

Meça pelo menos: inserir uma a uma, inserir em lote, atualizar, excluir, ler por chave, varrer faixa, e o tamanho em disco do mesmo dado.

## Parte 2 -- o que decide no celular, e nao aparece em micro-bancada

Aqui esta o argumento de verdade, e ele nao e velocidade. Investigue e escreva com honestidade, conferindo cada afirmacao no codigo antes de escrever:

**A favor do PhxSql:**
- **Replicacao embutida e medida.** O SQLite nao tem replicacao nenhuma -- sincronizar com servidor e codigo escrito a mao, ou produto pago de terceiro. Aqui ha quatro modos, `.log` v2 com a imagem da linha, «mais recente vence», spare e promocao, tudo com bancada. Num app que fica offline e reconecta, ISSO e o problema, e ele ja esta resolvido no motor. Confira em `docs/REPLICACAO.md`.
- **O `.log` por tabela ja e o diario do que aconteceu offline**, em forma que a replica sabe aplicar. No SQLite isso seria uma tabela de saida escrita a mao.
- **A janela de conflito de escrita por versao** (pedido 123) e exatamente o que um protocolo de sincronia precisa.
- **Cifra em repouso** ja escrita e conferida contra vetor do RFC 8439, e agora a **cifra do fio**. No SQLite, cifra e extensao paga (SEE) ou fork de terceiro (SQLCipher).
- **Trilha LGPD** (`.lgpd`) num aparelho que carrega dado pessoal.
- **Arquivos separados por tabela** permitem sincronizar uma tabela e nao outra. O SQLite e um arquivo so.

**A favor do SQLite, e isto tem de estar escrito com a mesma clareza:**
- **Ja esta no aparelho.** Android e iOS trazem SQLite; usa-lo custa **zero byte**. O PhxSql soma ~6,8 MB.
- **Maturidade.** O SQLite tem uma das suites de teste mais completas que existem e roda em bilhoes de aparelhos ha vinte anos. Aqui sao 1.328 testes.
- **SQL muito mais completo.**
- **E ACID de verdade.** O PhxSql **nao e** -- nao ha transacao ainda (esta sendo feita agora, noutra frente). Nao escreva *ACID compliant* sobre o PhxSql: o `CLAUDE.md` diz que e falso e continua falso.
- **E biblioteca, sem porta e sem daemon** -- o que no iOS e decisivo.

**Sobre o HFSQL(R):** seja preciso. O dono disse que la sao «tabelas soltas sem cuidado». O HFSQL Classic embutido no WINDEV(R) Mobile existe e tem replicacao com o HFSQL Client/Server. Pesquise no material que ja esta em `docs/HFSQL.md` e escreva o que se sustenta -- nao repita a frase do dono como se fosse fato apurado, e nao invente o que nao conferiu. Onde nao souber, diga que nao sabe.

## Parte 3 -- o Windows sob Wine

O dono autorizou criar VM para testar Windows e Android. **VM completa esta fora**: nao ha `/dev/kvm` nesta maquina e nem flag de virtualizacao no processador -- ela propria e uma VM sem aninhamento. Ja provei que o binario ARM roda sob `qemu-user-static` (ver `bancada/arm/provar.sh`).

Falta o equivalente para Windows: **tente o Wine** (`apt-get install -y wine` ou `wine64`). **Confira o tamanho antes de instalar** e desista dizendo o motivo se nao couber no disco. Se instalar, rode `target/x86_64-pc-windows-gnu/release/phxsqld.exe` e faca a mesma prova que a bancada ARM faz -- subir, login, criar tabela, gravar e ler. Se der certo, deixe como `bancada/windows/provar.sh` no mesmo molde.

## O documento

Crie `docs/MOBILE.md` com tudo isso: a tabela medida, o argumento que nao e velocidade, onde o SQLite ganha, e a forma certa do PhxSql no aparelho (biblioteca embutida mais cliente de sincronia -- NAO mini-servidor escutando porta, porque o iOS proibe e o Android mata). Atualize `docs/EMPACOTAMENTO.md` §7.7 e o `docs/PENDENCIAS.md`.

## Portoes

```
cargo fmt --all
cargo clippy --workspace --all-targets    # zero avisos
cargo test --workspace
```

## Estilo

Portugues em codigo, comentarios, documentacao e commit. Identificadores e comentarios **sem acento**. **Nenhum identificador de modelo** em nada que va para o repositorio. Numero visivel sai de medicao, nunca digitado.

Commite no seu worktree. **Nao faca push e nao abra PR.** No relatorio: a tabela medida com a dispersao, onde o SQLite ganhou, o veredito do Wine, e o que voce NAO conseguiu apurar.
```

---

## 52. Webservice REST com OpenAPI  ·  30/08 15:48

```
Voce trabalha no PhxSql, motor de dados em Rust do Adriano Boller, em `phxsql/`. LEIA `CLAUDE.md` na raiz ANTES de qualquer coisa. Portas 7500 a 7549. NUNCA `pkill -f`; mate so por PID que voce iniciou. Parta do commit `409c2ae`.

**Disco apertado (~4 GB livres, tres agentes compilando).** Remova `phxsql/target` do seu worktree ao terminar.

# A frente: webservice REST com OpenAPI/Swagger

O dono pediu: *«servidor webservice swagger phxsql para Windows, Linux, macOS, IoT, Android e iOS»*.

## O que ja existe e voce NAO refaz

Ha um servidor HTTP em `crates/phxsql-server/src/http.rs` (serve a interface web) e um protocolo JSON por soquete com dezenas de operacoes, despachadas num ponto so. Sobre isso e que entra o REST -- **nao invente um segundo caminho de dados**, e sobretudo **nao contorne o portao de permissao**.

Leia a regra da casa sobre isso antes: *«Portao de permissao e UM so -- e o campo que ele le e o furo»*. Ja houve quatro furos por operacao que nao tinha o campo `"tabela"`. Um caminho REST novo que despache por fora do portao seria a porta dos fundos maior que este projeto ja teve. **Toda rota REST passa pelo mesmo despachar.**

## 1. A especificacao SAI DO CODIGO, nunca da mao

Esta e a decisao mais importante da frente. Sao dezenas de operacoes; uma spec OpenAPI digitada a mao envelhece na primeira operacao nova e **passa a mentir** -- e mentir com aparencia de documento oficial. A casa ja pagou por isso duas vezes: o rodape do dossie publicou 780 KiB quando eram 1.032, porque o gerador tinha uma lista copiada; e o conferidor de idiomas media cinco sextos da tela porque a lista de arquivos era digitada.

**A regra: quando um gerador depende de uma lista, a lista tem de sair do codigo.** Entao o `openapi.json` se gera da tabela de despacho real. E ha DUAS guardas obrigatorias, uma para cada lado do laco -- e o precedente e o `conferidor.rs`, que ja faz exatamente isso para os textos de tela:

- **operacao que existe e a spec nao documenta** -- reprova
- **rota que a spec documenta e nao existe** -- reprova

Sem as duas, a spec vira chave morta: alguem le, acredita, e nada corresponde.

## 2. O Swagger UI, e a tensao que voce tem de resolver com numero

O Swagger UI e um pacote JavaScript de varios MB. Medi hoje: o binario do servidor tem **6,8 MB**, e isso importa porque acabei de provar que ele roda em ARM e cabe em placa pequena -- em ESP32 de 4 MB de flash **ja nao cabe**. Embutir o Swagger UI dobraria isso.

Meça o tamanho real das opcoes e decida com o numero na mao:
- embutir, e quanto cresce o binario
- servir so a spec e apontar para CDN -- **quebra o uso offline**, que e justamente o caso do IoT
- escrever um visualizador minimo aqui, sem dependencia externa

Seja qual for a escolha, o Swagger UI **nao pode ser obrigatorio**: quem sobe numa placa quer o REST sem o visualizador. Faca disso uma opcao (feature de compilacao ou campo de configuracao) e diga qual e o padrao e por que.

## 3. A guarda nova entra PEDIDA, nao imposta

Regra petrea: o REST nasce **desligado** por configuracao. Servidor que ja roda hoje nao passa a expor porta nova numa atualizacao -- isso seria abrir superficie de ataque sem ninguem pedir. O teste que mais importa e o do comportamento velho: `config_sem_a_secao_rest_nao_escuta`.

## 4. Autenticacao

Aproveite o que existe -- token e o login por desafio-resposta -- e descreva na spec o esquema de seguranca de verdade. **Nao invente credencial nova.** E lembre: `Bearer` sobre HTTP em claro entrega o token; a §7 do `docs/SEGURANCA.md` diz o que vale e o que nao vale, e o REST tem de repetir esse limite em vez de escondê-lo.

## 5. Onde roda -- responda por plataforma, com honestidade

- **Linux, Windows, IoT (ARM64/ARMv7)**: e o mesmo binario, entao vem de graca. Confira rodando o REST sob `qemu-aarch64-static` -- ha bancada pronta em `bancada/arm/provar.sh` mostrando o caminho.
- **macOS**: **nao ha alvo** hoje e nao da para compilar aqui (o SDK da Apple so existe em macOS). Diga isso em vez de deixar parecer entregue.
- **Android e iOS**: o servidor REST **nao e** a forma certa la -- o iOS proibe app escutando porta para outros apps e o Android mata processo em segundo plano. Nesses dois o caminho e a biblioteca embutida, que outra frente esta construindo agora (`phxsql-ffi`). **Nao duplique esse trabalho**; so diga no documento como as duas pecas se encaixam.

## Provas

- Prova por soquete de verdade, exercitando as rotas com um cliente HTTP escrito na hora (nada de so testar por unitario).
- Todo teste novo tem de **falhar com o defeito reposto** e passar com o conserto -- mostre a saida, nao afirme.
- Guardas novas no `bancada/guardas/catalogo.py` (hoje 37) e parte nova no `provar.py` (hoje 20 partes).
- Se houver texto de tela, ele entra pela fabrica de idiomas (petreo) e a catraca do `conferidor.rs`, em **1.996**, nao sobe.

## Portoes

```
cargo fmt --all
cargo clippy --workspace --all-targets    # zero avisos
cargo test --workspace                    # hoje 1.328 verdes
```

## Documento

`docs/REST.md` novo, e atualize `docs/PENDENCIAS.md` e `CHANGELOG.md`.

## Estilo

Portugues em codigo, comentarios, documentacao e commit. Identificadores e comentarios **sem acento**. Comentario explica **por que**. **Nenhum identificador de modelo** em nada que va para o repositorio.

Commite no seu worktree. **Nao faca push e nao abra PR.** No relatorio: como a spec e gerada, o numero das opcoes de Swagger UI e a escolhida, a saida de cada defeito reposto, e o que ficou de fora com o motivo.
```

---

## 53. Revisor multilíngue  ·  30/08 15:51

```
Voce e o **agente revisor multilingue** do PhxSql, motor de dados em Rust do Adriano Boller, em `phxsql/`. LEIA `CLAUDE.md` na raiz ANTES de qualquer coisa. Portas 7550 a 7599. NUNCA `pkill -f`; mate so por PID que voce iniciou. Parta do commit `409c2ae`.

**Disco apertado (~6 GB, quatro agentes compilando).** Remova `phxsql/target` do seu worktree ao terminar.

# O papel, nas palavras do dono

> *«O agente multi linguagem deve fazer uma revisao constante para manter a possibilidade de mudar entre portugues, ingles… pelo login e pela tela de configuracao. A cada nova implementacao esse agente tradutor deve atualizar strings fixas por variaveis de multi linguagem. Isso e petrio.»*

A maquina existe desde a 0.17.0: `phxsys.mensagens`, a `FABRICA_TELA` do `crates/phxsql-server/src/idiomas.rs`, o atributo `data-txt`, as bandeiras do login, e o conferidor com catraca em `crates/phxsql-server/src/conferidor.rs`. O procedimento de acrescentar um texto esta em `docs/MENSAGENS.md`. **Leia os dois antes de comecar.**

## O que eu ja medi, para voce nao repetir

- Catraca (`TETO`): **1.996** textos ainda cravados. Na fabrica: **303** chaves.
- Distribuicao: `index.html` **1.806** · `claude.js` **126** · `telemetria.js` **38** · `grid/phx-grid.js` **24** · `diagrama-er.js` **2**. O `multitela.js` esta em **zero** -- foi o primeiro a fechar.
- **A qualidade do que ja esta traduzido esta boa.** Medi: **zero** chaves com os seis idiomas identicos, e **zero** frases longas repetidas em tres ou mais idiomas. O problema NAO e traducao ruim, e cobertura.
- Cuidado com a medicao ingenua: contei 33 chaves com espanhol identico ao portugues, e a maioria esta CERTA -- `Database`, `Profiler`, `Pivot`, `Servidor`, e `Menu principal` que em frances e exatamente isso. **Uma guarda de «igual = nao traduzido» reprovaria o correto.**

## ESCOPO desta rodada -- leia com atencao, ha quatro frentes trabalhando ao lado

**NAO toque no `ui/index.html`.** Quatro agentes estao em voo agora (transacoes, biblioteca embutida, comparacao com SQLite, REST/OpenAPI) e pelo menos dois vao mexer nele. Editar 1.806 textos la agora garantiria um conflito enorme na integracao. O `index.html` fica para a proxima rodada, depois que elas caírem.

**Trabalhe nos outros quatro arquivos:** `claude.js` (126), `telemetria.js` (38), `grid/phx-grid.js` (24), `diagrama-er.js` (2) -- **190 textos**. Traduza o que der nos seis idiomas: pt, fr, en, it, de, es.

## As tres armadilhas que esta frente ja pagou, e a quarta

1. **Rotulo se traduz; dado, nunca.** E a licao do «Blumenau» virando «BLUMENAU». O que a pagina interpola (`${…}`) nao se toca.
2. **Texto se resolve por CHAVE, nunca por comparacao da frase.** No dia em que alguem melhorar a redacao, quem compara frase quebra calado -- mostrando portugues.
3. **Chave morta e pior que chave faltando.** O tradutor a ve na tabela, traduz nos seis idiomas, e nada muda na tela. Ha teste para os dois lados do laco.
4. **Frase picada por marcacao e intraduzivel por construcao** -- a licao que o `multitela.js` pagou. `"funciona em"` + `<b>qualquer navegador</b>` + `"— e layout"` nao se traduz em pedacos, porque a ordem das palavras muda de idioma para idioma e em alemao o verbo vai para o fim. A frase inteira vira UMA chave, com a enfase como marca dentro do texto (`**assim**`), e o corte em etiquetas acontece DEPOIS da traducao. O mecanismo ja existe -- o `marcado()` -- e esta descrito no `docs/MENSAGENS.md`. Use, nao reinvente.

E as **tres mensagens que nao se traduzem de proposito** continuam assim, com o motivo no `mensagens.rs`: `erro.redireciona`, `erro.sinal`, `erro.cancelado`. Achou uma quarta do mesmo naipe? **Documente a decisao** em vez de traduzir.

## A parte que torna a revisao CONSTANTE, e nao dependente de alguem lembrar

Esta e a metade mais importante da frente. Um agente que revisa quando alguem chama nao e revisao constante -- e revisao ocasional. O que torna constante e o **portao que roda junto dos testes**, e ele hoje so conta. Acrescente ao `conferidor.rs`:

- **Guarda do texto colado**: chave em que os SEIS idiomas sao identicos e o texto tem mais de 3 caracteres. Hoje o numero e **zero**, entao ela entra como catraca em zero e pega o dia em que alguem colar portugues seis vezes. **Nao** use «igual ao portugues» como criterio -- eu medi e daria falso positivo em `Database`, `Profiler`, `Menu principal`.
- **Guarda da frase longa**: frase com mais de 25 caracteres identica em tres ou mais idiomas. Hoje tambem **zero**.
- Se ja existirem guardas para chave morta e chave faltando, confirme que pegam e **nao duplique**.

Cada uma dessas guardas tem de **falhar com o defeito reposto** -- ponha uma chave colada de proposito, veja reprovar, tire. Mostre a saida no relatorio; nao afirme. Registre no `bancada/guardas/catalogo.py` (hoje 37 guardas).

## Baixe a catraca no mesmo commit

Traduziu, mede de novo com `cargo run --example textos-fora-da-fabrica -p phxsql-server` e poe o `TETO` no numero **medido**. Nao invente: rode e leia. Ha um `assert` que reprova quem traduz e esquece de baixar -- **catraca frouxa nao segura nada**.

## Prove exercitando

**Interface so se prova exercitando** -- a regra nasceu de tres defeitos achados em cinco minutos de video que ler o codigo nao acharia. Suba o servidor numa porta sua, abra as telas que voce traduziu (Query/Claude, Telemetria, a grade, o diagrama ER), **troque o idioma** e confira que muda de verdade. O CSS global morde componente novo: `input{width:100%}` e `label{text-transform:uppercase}` ja produziram uma bolinha do tamanho da celula e um «BLUMENAU».

Mexeu em `ui/`? `cargo build --release -p phxsql-server --bin phxsqld` antes de exercitar, senao voce testa o binario velho -- armadilha ja paga tres vezes aqui.

## Portoes

```
cargo fmt --all
cargo clippy --workspace --all-targets    # zero avisos
cargo test --workspace                    # hoje 1.328 verdes
```

## Estilo

Portugues em codigo, comentarios, documentacao e commit. Identificadores e comentarios **sem acento** (texto de interface pode ter). Comentario explica **por que**. **Nenhum identificador de modelo** em nada que va para o repositorio.

Commite no seu worktree. **Nao faca push e nao abra PR.** No relatorio: quantos traduziu por arquivo, o numero medido da catraca, a saida de cada guarda nova com o defeito reposto, e o que deixou na fila com o motivo.
```

---

