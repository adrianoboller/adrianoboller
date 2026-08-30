# Os sprints, numa lista só — para aprovar item a item

Quatro agentes leram quatro manuais e escreveram quatro propostas: **31
sprints** esperando o seu sim. Ler os quatro e cruzar de cabeça é trabalho
seu; este documento faz esse trabalho uma vez.

Os quatro continuam existindo, e cada item daqui aponta para o seu:

| origem | propostas | onde |
|---|--:|---|
| Cassandra(R) | 5 | `docs/SPRINTS-CASSANDRA.md` |
| Redis(R) | 4 | `docs/SPRINTS-REDIS.md` |
| MariaDB(R) | 13 | `docs/SPRINTS-MARIADB.md` |
| Teradata(R) | 9 | `docs/SPRINTS-TERADATA.md` |
| | **31** | |

**Nada aqui está autorizado por existir aqui.** Cada linha da §6 se aprova,
recusa ou reordena sozinha.

### O que este documento fez com as 31

| | quantas | onde |
|---|--:|---|
| viraram item aprovável | **27** | §6 |
| ↳ das quais **fundidas** (dois motores, uma melhoria) | 2 pares → 2 itens | §3 |
| encolheram porque **metade já existe** | 1 (mais 1 capacidade já pronta) | §4 |
| saíram da lista: **não são sprint, são medição** | 2 | §5 |
| **contradições** que precisam de decisão sua antes | 6 | §2 |

Nenhuma das 31 morreu na regra de zero dependências, nenhuma quebra a ordem de
digitação — os quatro autores já filtraram por isso, e eu reconferi. **Uma
delas quebra cliente antigo do jeito que está escrita**, e está reescrita na
§2.1.

---

## 1. Como esta lista foi montada, e o que nela é medido

A régua é a da casa, e ela vale contra os quatro documentos-fonte também:

- **número citado é número que não se mede.** Onde repeti um número dos
  quatro, fui à fonte dele. Um foi conferido e vale (os 7,8× do `fsync` da
  lixeira, com a condição de medição escrita); dois estavam certos e são de
  segunda mão (o 1,45–1,58× do pipelining, medido com `load` ~4 — o próprio
  autor diz que não é publicável); **duas citações cruzadas não conferem** e
  estão na §2.6.
- **medir a premissa do item vem antes de implementar o item.** Cinco
  premissas dos quatro documentos se respondem **lendo o código**, sem
  bancada. Fiz isso, com `arquivo:linha`, e o resultado mudou três itens (§4 e
  §5).
- **o custo (P/M/G) é julgamento dos autores, não medição.** Eu o repito como
  veio e digo que é julgamento. O **valor** de quase todos também é
  julgamento; a §7 separa os poucos que têm número.

---

## 2. As contradições — o que decidir antes de aprovar dois itens juntos

Esta seção existe porque aprovar item a item, sem ela, aprova coisas que se
desfazem entre si.

### 2.1 Os dois `fsync` puxam para lados opostos sobre quem escolhe

O Cassandra(R) §Sprint 1 e o Redis(R) §Sprint 1 chegaram ao mesmo alvo — tirar
o `fsync` do caminho de quem escreve — e **discordam na regra da casa**.

O Redis(R) escreve a regra certa e a obedece: *«o modo entra pedido, não
imposto: o padrão continua `por_lote`; quem quiser a janela escreve o modo
novo no `config.json`. Guarda nova entra pedida — e afrouxar guarda, com mais
razão ainda.»*

O Cassandra(R) escreve o oposto sem perceber. O escopo dele diz: *«`por_lote`
(o padrão) — a exclusão entra na janela que já existe»*. Hoje, num servidor
com a configuração padrão, um `excluir` que responde OK **já está no disco**
(`lixeira.rs`, dentro de `guardar`, com o motivo escrito ali). Depois dessa
mudança, não estaria — e ninguém pediu. É a versão espelhada do defeito que a
casa já escreveu: **proteção que quebra todo cliente antigo não é proteção, é
estrago** — e retirar proteção sem pedido é o mesmo estrago pelo outro lado.

**A reescrita, e é assim que o item 1 da §6 está escrito:** a exclusão passa a
respeitar `recursos.durabilidade` **quando o dono pedir**, por um campo
próprio (`durabilidade.exclusao`, ou o modo novo do Redis(R) §Sprint 1). Sem o
campo, byte por byte como hoje. O ganho medido não muda; muda quem escolhe. E
o teste que importa continua sendo o do comportamento **velho**.

### 2.2 Cinco propostas independentes dizem «PSCH v7»

É a maior descoberta desta consolidação, e ela só aparece cruzando os quatro:

| proposta | o que ela diz precisar |
|---|---|
| MariaDB(R) 4 — `CHECK` | «Muda o formato: esquema `PSCH` v7» |
| MariaDB(R) 5 — colunas geradas | «As mesmas do sprint 4, e o mesmo `PSCH` v7» |
| Redis(R) 2 — TTL por linha | «Formato (`PSCH` v7, com a disciplina do v6)» |
| Teradata(R) 3 — dicionário de coluna | «PSCH v6 → v7, com o campo no fim do bloco» |
| Teradata(R) 8 — tipo `PERIOD` | «Mudança de formato no bloco de esquema» |

Aprovadas item a item, em rodadas diferentes, elas custam **v7, v8, v9, v10 e
v11**: cinco migrações, cinco testes de «quem lê a versão velha para antes»,
cinco chances de errar o mesmo byte. Aprovadas juntas, custam **uma**.

Isto **não** é motivo para aprovar as cinco. É motivo para a decisão ser
«quais delas, na mesma rodada de formato» — e para nenhuma delas entrar
sozinha só porque é pequena. A §6 as agrupa por isso, e cada uma continua
aprovável sozinha *dentro* do grupo.

### 2.3 Duas propostas independentes dizem «`.log` v4»

Mesma armadilha, no outro arquivo. O Teradata(R) 2 quer a **imagem anterior**
no diário (v3 → v4). O MariaDB(R) 7 quer ler o histórico da linha, e a
premissa que pode matá-lo é que a **imagem posterior** nasce **desligada** num
servidor isolado (`config.rs`: `replicacao.imagem_da_linha` segue o papel).
São dois pedidos ao mesmo arquivo, na mesma rodada, e o preço de ligar a
imagem já está medido (`FORMATO.md`: 21.740 → 19.531 linhas/s, 44 → 223 bytes
por evento). Decidir os dois de uma vez é uma conversa; decidir um de cada vez
são duas migrações e duas conversas sobre o mesmo diário.

### 2.4 O QUALIFY é proposto por um documento e recusado por outro

O Teradata(R) 5 propõe `QUALIFY` com `ROW_NUMBER()`/`RANK()`. O MariaDB(R), na
lista de descartes, recusa **exatamente isso**, pelo nome: *«Propor
`ROW_NUMBER() OVER (PARTITION BY …)` antes de o `AND` funcionar é construir o
telhado antes da parede. Volta quando o sprint 1 estiver feito e o `GROUP BY`
existir.»*

Não é desacordo de mérito — é de **ordem**, e o MariaDB(R) tem razão pelo
substrato: hoje o `WHERE` aceita **uma** comparação
(`crates/phxsql-sql/src/sintaxe.rs:534`) e a camada SQL não tem `GROUP BY`.
Resolvido na §6 por dependência: o `QUALIFY` vem depois do item 10.

### 2.5 Estatística: um documento propõe, o outro já recusou

O MariaDB(R) 2 escreve, no «o que NÃO entra»: *«Estatísticas persistidas para
o planejador — `ANALYZE TABLE` no sentido do MariaDB(R). `docs/COMPARACAO.md`
já recusou com o motivo certo: sem planejador, estatística é arquivo para
manter atualizado sem ninguém ler.»* O Teradata(R) 4 propõe cardinalidade por
índice para o tradutor escolher.

**Não se anulam, e a razão é de código:** o Teradata(R) 4 não coleta nada. A
cardinalidade que ele quer **já está gravada** — `qtd_chaves` por índice, no
cabeçalho do `.ndx` (`crates/phxsql-store/src/ndx.rs:88` e `:596`). Não há
arquivo novo para manter atualizado; há um número para ler. Mas, se o item 3
for aprovado, **a frase do MariaDB(R) 2 precisa ser reescrita**, senão os dois
documentos ficam dizendo coisas contrárias sobre a mesma decisão.

### 2.6 Duas citações cruzadas que não conferem

O MariaDB(R), na tabela de «candidatos compartilhados», afirma duas
sobreposições com o documento do Cassandra(R):

- *«Papéis (sprint 9) — compartilhado com a análise do Cassandra(R). Lá também
  há papéis concedidos a papéis.»*
- *«Índice invertido (sprint 13) — possivelmente a análise do Cassandra(R).»*

Conferido: o `SPRINTS-CASSANDRA.md` tem cinco sprints (`fsync` da lixeira,
retenção do diário, posição da réplica, interseção de rowids, TTL) e dez
descartes, e **nenhum deles é papéis nem índice invertido**. As duas
sobreposições não existem.

Nada morre por causa disso, e o MariaDB(R) 9 e 13 continuam de pé pelos
próprios méritos. Fica registrado porque é a mesma família do defeito que esta
casa mais persegue: **afirmação citada é afirmação que não se conferiu.** Vale
para o documento irmão como vale para o número.

---

## 3. As duplicatas — dois motores, uma melhoria

Quando dois manuais diferentes apontam para o mesmo lugar, isso é sinal forte.
A lista diz isso uma vez, e não duas.

### 3.1 TTL por linha — Cassandra(R) 5 + Redis(R) 2 → **um item**

Os dois chegam pelo mesmo caminho e param no mesmo desenho: coluna de sistema
no fim do esquema (o padrão do `SOFTDELETED` e do `rownum`), vencimento
aplicando exclusão **suave** e não física, varredura pelo agendador que já
existe. O Redis(R) acrescenta as três decisões que o manual dele documenta e
que valem: prazo **absoluto** e não duração, leitura que **filtra** sem
gravar, e expiração **só na origem** — porque dois relógios decidindo o mesmo
vencimento é divergência garantida.

O MariaDB(R) chega ao mesmo assunto por um terceiro caminho (o
`DELETE HISTORY … BEFORE SYSTEM_TIME`, na tabela de compartilhados dele) e
escreve a frase que resume: *«vale desenhar uma vez, não três»*.

### 3.2 O `WHERE` — MariaDB(R) 1 + Cassandra(R) 4 → **um item, com bifurcação medida**

Os dois atacam a mesma recusa, escrita no código
(`crates/phxsql-sql/src/sintaxe.rs:534`):

> «o WHERE aceita UMA comparacao. Duas exigiriam interseccao de rowids, e nao
> ha planejador que decida por qual indice comecar»

O MariaDB(R) 1 quer a **varredura filtrada** (coluna sem índice); o
Cassandra(R) 4 quer a **interseção de rowids** (duas colunas indexadas). E o
próprio critério de morte do Cassandra(R) 4 desemboca no MariaDB(R) 1: *«se a
interseção nunca ganhar por mais de 1,2×, o sprint encolhe para “escolher o
índice mais seletivo e filtrar o resto”»* — que é, literalmente, o
MariaDB(R) 1 aplicado a duas colunas.

São **uma** medição e **uma** decisão. Separados, dois sprints mediriam a
mesma curva de seletividade e um deles descobriria que virou o outro.

### 3.3 O `fsync` — Cassandra(R) 1 + Redis(R) 1 → **dois passos do mesmo caminho**

Não são duplicatas: são o degrau e a escada. O Cassandra(R) 1 é **P** e tem
número medido (7,8× em 20.000 exclusões); o Redis(R) 1 é **M**, tira o `fsync`
de quem fecha a janela e traz um risco que o próprio autor nomeia (o p99 pode
piorar enquanto a média melhora). O primeiro fecha a única fase em que a
bancada perde; o segundo é decisão de contrato. **Nesta ordem, e não na
inversa** — o contrário deixaria a exclusão com uma política mais rígida que a
da inserção, que é exatamente a assimetria que o item 1 existe para tirar.

### 3.4 A mesma primitiva de espera, em dois usos

A `Condvar` que a tabela-fila do Teradata(R) 1 precisa (*«a transação espera
até que uma linha seja inserida»*) é a mesma que o `CASSANDRA.md` §6.2 já
propôs para o *long-poll* da replicação — inclusive com a mesma armadilha
escrita: **esperar fora da trava global**, senão o servidor congela. Quem
entrar primeiro entrega a peça para o outro.

### 3.5 A fronteira que NÃO se deve fundir

O Redis(R) 4 (`assinar`, a grade viva) e o empurrão de eventos para réplica
parecem a mesma coisa e não são. O documento do Redis(R) desenha a cerca com a
fonte na mão: *fire-and-forget* serve para tela (quem reconecta recarrega a
grade, e a perda custou um refresh) e **não serve para réplica**, que não pode
perder evento e retoma por posição. **Tela usa empurrão; réplica usa posição —
nunca misturar.**

---

## 4. O que já existe — sai da lista, ou encolhe

Os quatro autores conferiram contra o código e acertaram quase tudo. Sobraram
estes, e os três se respondem **lendo o código**, que é a medição mais barata
que existe.

### 4.1 O job que chama procedimento **já roda hoje** — MariaDB(R) 3 perde metade

O MariaDB(R) 3 pede duas coisas: **(a)** um job cujo corpo seja
`CALL procedimento(...)` e **(b)** o equivalente ao `DISABLE ON SLAVE`.

A metade (a) já funciona, e sem uma linha de código nova:

- o corpo de um job é **um pedido do protocolo**, e ele é despachado pelo
  despachar de sempre — `self.executar(op, &job.pedido, &sessao)`
  (`crates/phxsql-server/src/servidor.rs:3410`), com os portões antes
  (`:3407`, `:3409`);
- `CALL` entra pela op `sql`: o detector de comando de rotina roda antes do
  `SELECT` e desvia (`crates/phxsql-server/src/servidor.rs:6671` →
  `executar_rotina`, `:6711`; `Comando::Chamar` em `:6844`).

Ou seja, `{"op":"sql","database":"loja","texto":"CALL fecha_mes()"}` como
`pedido` de um job é um job que chama procedimento. **O sprint encolhe para a
metade (b)**, e a entrega do primeiro dia passa a ser o teste por soquete que
prova a metade (a) — porque ler o código não é provar, e esta casa já pagou
por confundir os dois.

### 4.2 O pipelining **já funciona** — Redis(R) 3 não é sprint de recurso

O próprio autor mediu e escreveu: *«o protocolo JSON Lines já atende pedidos
empilhados hoje, sem mudar uma linha do servidor»* — 2.000 respostas corretas
e na ordem, em quatro formas, nas duas rodadas. O que falta é **contrato
escrito, cliente e teste**, não capacidade. Ele continua na lista (item 8),
com o rótulo certo: é documentação com prova, não funcionalidade nova. E o
número dele (1,45–1,58×) saiu com `load` ~4 e **não é publicável** enquanto
não for refeito em máquina quieta — o autor diz isso primeiro que eu.

### 4.3 O que a rodada da telemetria entregou, e que muda uma premissa

A telemetria entrou depois dos quatro documentos. Ela não faz nenhum dos 31 —
mas entrega o instrumento de que a premissa do Redis(R) 1 precisa: *«medir o
p99 da latência de operação com a thread ligada contra o de hoje»*. A espera
na fila da trava passou a ser medida no ponto único `travar_dados()`
(`crates/phxsql-server/src/servidor.rs:719`).

Com uma ressalva que o `PENDENCIAS.md` desta rodada registra: **13 tomadas da
trava continuam fora desse ponto** — entre elas o `executar_rotina`, que é
onde um gatilho longo segura o servidor. Enquanto elas não entrarem, o p99 que
a telemetria mostra é o p99 de uma parte.

---

## 5. O que sai da lista: não é sprint, é medição

Recusa fundamentada é resultado válido aqui. Estes dois não são recusados por
mérito — são recusados como **rodada de trabalho**, porque o que falta neles é
um número, e o número custa uma tarde.

### 5.1 A retenção do diário no multi-master (Cassandra(R) 2)

O sprint escreve a fronteira de quanto um evento de exclusão precisa
sobreviver no `.log` antes de ser seguro apagá-lo — o `gc_grace_seconds` da
casa. É um bom problema, e o dado-zumbi é o pior defeito que este projeto já
teve três vezes.

Só que o próprio autor leu o código e achou a resposta: *«procurei expurgo de
volume em `crates/phxsql-store/src/` e não existe — o `.log` nunca apaga
volume; ao chegar em `max_arquivos` ele para com erro»*. **Sem expurgo não há
zumbi.** O que sobra é prevenção para um mecanismo que não existe e não está
planejado.

**O que vira:** a medição na bancada de replicação que já existe
(`bancada/replicacao/montar.py`, com `diario_volume_mib` pequeno), mais um
parágrafo no `REPLICACAO.md` e **um teste que trava**: no dia em que alguém
escrever o expurgo, ele falha. Isso não é uma rodada; é o começo da rodada de
espaço, e é lá que ele deve nascer.

### 5.2 O degrau seguinte do interpretador (MariaDB(R) 12)

`CASE`, `LOOP`/`REPEAT`, `HANDLER` e `CREATE FUNCTION` — as recusas mais
pedidas da lista de 17 que o `TRIGGERS.md` §8 publica.

O próprio sprint diz o que fazer antes: *«Este sprint só existe se alguém
esbarrar nas recusas, e o jeito de saber é contar: o Profiler já vê o que
chega pela porta antes de virar dado. Contar as recusas por nome durante um
tempo de uso real é mais barato que implementar as quatro.»*

**Concordo e levo até o fim:** implementar quatro verbos porque *poderiam*
faltar é o oposto de medir a premissa do item. E `CREATE FUNCTION` depende do
item 10 de qualquer jeito — a recusa dela é, literalmente, «a camada SELECT
não avalia expressão». Volta à lista com a contagem na mão.

---

## 6. A lista — 27 itens, cada um aprovável sozinho

Ordenada por **valor ÷ custo**. O custo (P/M/G) vem dos autores e é
**julgamento**; o valor tem número em três itens e é julgamento nos outros 24
— a coluna diz qual é qual, e a §7 explica cada um.

### Tier 1 — pequenos, e com defeito real ou número medido do lado de cá

| # | Sprint | Tam. | Origem | Valor | A premissa que pode matá-lo |
|--:|---|:--:|---|---|---|
| 1 | **O `fsync` da exclusão entra na janela — pedido, não imposto** | P | Cassandra(R) 1, reescrito pela §2.1 | **medido: 6,5 s → 0,83 s em 20.000 exclusões (7,8×)**, e é a única fase em que a bancada perde (6,27 s contra 4,73 s) | refazer em máquina quieta; **abaixo de 2× morre**. E provar que não existe queda em que a linha suma dos dois lados |
| 2 | **`DISABLE ON SLAVE`: o job que não roda na réplica** | P | MariaDB(R) 3(b) — a metade (a) já existe, §4.1 | julgamento, mas o defeito é de **hoje**: com os quatro modos de replicação no ar, um job de escrita ligado nos dois lados grava em dobro; num par bidirecional, cada lado sobrescreve o outro | subir um par e **contar**: a réplica já recusa a escrita por outro caminho? Se recusar, o sprint some quase inteiro |
| 3 | **Cardinalidade por índice: o tradutor escolhe** | P | Teradata(R) 4 | julgamento; o insumo **já está gravado** (`ndx.rs:88`, `:596`) e o ODBC trouxe quem não sabe nomear índice nenhum | diferença entre o índice certo e o errado: **abaixo de 2× morre** (uma regra a mais para errar). Acima de 10×, sobe |
| 4 | **Error table e a carga que se repete sem duplicar** | P | Teradata(R) 7 | julgamento; hoje as recusadas voltam só na resposta, e uma conexão cortada leva a única cópia | a chave externa sob índice único já torna a carga repetível? Se sim, **vira teste e documentação**, não código |
| 5 | **Macro: a consulta parametrizada salva** | P | Teradata(R) 6 | julgamento; o valor não é comodidade, é o parâmetro entrar como **valor** e não por concatenação | é mais barata que a procedure de uma instrução? Se ficar dentro do espalhamento, **morre com o número na mesa** |
| 6 | **`EXCEPT` e `INTERSECT` sobre o `unir`** | P | MariaDB(R) 6 | julgamento; a máquina está pronta (pedido 91: chave composta, nulo que não casa com nulo) | a comparação de linhas do `unir` serve como está? Responde-se **lendo o código** e escrevendo o teste do nulo |
| 7 | **A tabela-fila com pop atômico** | P | Teradata(R) 1 | julgamento; três consumidores de fila já existem (jobs, réplica, sincronia do DbLink) e não há fila | achar a próxima pendente com 1M consumidas custa como o `pular` (6 ms)? **Acima de 50 ms o desenho muda** |
| 8 | **Pipelining: contrato, cliente e número** | P | Redis(R) 3 — **capacidade já existe**, §4.2 | medido, mas **em condição ruim**: 1,45–1,58× no loopback com `load` ~4 | refazer em **máquina quieta** e com RTT real. Se não passar de poucos por cento, vira uma seção de documentação |
| 9 | **`EXPLAIN` e `ANALYZE`** | P | MariaDB(R) 2 | julgamento; rende **depois** do 3 e do 10, quando houver escolha a explicar | o `EXPLAIN` diz algo que as `notas` já não digam? Se não, encolhe para o verbo como sinônimo — meia hora |

### Tier 2 — médios, e é onde estão os que destravam outros

| # | Sprint | Tam. | Origem | Valor | A premissa que pode matá-lo |
|--:|---|:--:|---|---|---|
| 10 | **O `WHERE` que filtra, e a segunda condição** | M | MariaDB(R) 1 + Cassandra(R) 4, fundidos (§3.2) | julgamento, mas o **número de dependentes é fato**: os itens 9, 23 e 24, o `CREATE FUNCTION` da §5.2 e dois descartes do MariaDB(R) (tipo `JSON`, window functions) nomeiam este item — cinco coisas esperando uma | a varredura filtrada é rápida o bastante para ser oferecida? E o **ponto de virada** por seletividade entre intersecar e filtrar. Se a interseção nunca ganhar 1,2×, sobra só a metade barata |
| 11 | **A posição confirmada de cada réplica, no source** | M | Cassandra(R) 3 | julgamento, com um argumento de correção: o `CLUSTER.md` §2.1 diz que promover é seguro «quando as réplicas estão na mesma posição», e **o cluster com eleição decide sem essa conferência** | a posição guardada bate com a informada dentro de um lote (500); e a taxa do master **não cai 1%** (34.048 linhas/s) |
| 12 | **O `fsync` fora do caminho de quem escreve** | M | Redis(R) 1 — **depois do item 1** (§3.3) | medido **em outro caminho**: `DESEMPENHO.md` §4.9 mede 16,13 → 7,99 µs/linha na inserção. Aqui é inferência | p99 sob a trava, em máquina quieta. **Morte: ganho de ponta a ponta < 2%, ou p99 piorando mais do que a média compra** |
| 13 | **`assinar`: a grade viva** | M/G | Redis(R) 4 | julgamento; é o único item de cara para o usuário — dois na mesma grade hoje só se descobrem no erro 3004 | carga em lote com o mecanismo presente e **zero assinantes**: diferença zero dentro do ruído (a lição dos 7% do Profiler). E 50 assinantes parados |
| 14 | **Papéis no modelo de direitos** | M | MariaDB(R) 9 | julgamento; com direito por tabela (pedido 124), dez pessoas do mesmo cargo são a mesma regra copiada dez vezes | não é de desempenho: é `sem_papel_nada_muda` **antes de qualquer código**. Regra que muda o significado da configuração existente tira direito de alguém sem ninguém pedir |
| 15 | **Sequência como objeto próprio** | M | MariaDB(R) 11 | julgamento; o limite é real e está escrito (`FORMATO.md` §12: uma sequência por tabela), e o caso é a numeração que atravessa duas tabelas | **é sua:** buraco na numeração é aceitável? Se não for, custa um `fsync` por número, e isso se mede antes de alguém usar em laço |

### Tier 3 — a rodada de formato do esquema (`PSCH` v7)

> **Trava de formato (§2.2).** Estes cinco mudam o bloco de esquema. Cada um
> se aprova sozinho, mas **os aprovados entram na mesma rodada** — cinco
> rodadas separadas são cinco migrações para o mesmo arquivo.

| # | Sprint | Tam. | Origem | Valor | A premissa que pode matá-lo |
|--:|---|:--:|---|---|---|
| 16 | **`CHECK` declarativo** | M | MariaDB(R) 4 | julgamento; o lugar já existe (o `BEFORE` roda com a trava na mão, entre a conversão e a gravação) e o avaliador exato já existe (`rotina.rs`, `i128` com escala, sem `f64`). É **tradução, não motor** | tabela sem `CHECK` custa exatamente o que custa hoje, pelo método intercalado dos gatilhos. E: **o que acontece com as linhas já gravadas que violam a regra nova** — escrito antes do código |
| 17 | **Colunas geradas `PERSISTENT`** | M | MariaDB(R) 5 | julgamento; o motor já preenche coluna sozinho (`rownum`, `softdeleted`) | custo por linha de avaliar a expressão na escrita. A inserção inteira custa 7,5 µs; 1 µs de expressão são 13%. **E antes de medir: `cargo build --release --examples -p phxsql-store`** |
| 18 | **TTL por linha, expirando para o `SOFTDELETED`** | M | Cassandra(R) 5 + Redis(R) 2, fundidos (§3.1) | julgamento — e a premissa mudou, ver ao lado | **a premissa do Redis(R) está morta por leitura de código:** ela dizia «um job com `UPDATE … SET SOFTDELETED = 1 WHERE prazo < NOW()` já faz isso hoje». **Não faz** — a camada SQL recusa `UPDATE` pelo nome (`sintaxe.rs:269`) e o corpo de rotina também (`TRIGGERS.md` §8). A pergunta que resta é a do Cassandra(R), e é **sua**: você tem esse caso de uso? Em ERP, dado que some sozinho costuma ser defeito |
| 19 | **Dicionário de coluna: a compressão que cabe num slot fixo** | M | Teradata(R) 3 | **derivado, não medido**: uma `Str(20)` com 8 valores distintos poupa ~15,6% do `.reg` e ~7% da tabela — aritmética do formato sobre uma largura medida, e a frase diz isso | **depende de dado seu:** quantas colunas de uma base real têm domínio ≤ 255? Abaixo de 10% da largura do slot, morre. E o custo de CPU na inserção: acima de 5%, o disco está sendo pago com o tempo que a casa acabou de comprar |
| 20 | **Tipo `PERIOD` e os dois predicados de vigência** | M ou P | Teradata(R) 8 | julgamento; vigência é problema de ERP, e a fronteira («o dia do fim entra?») erra sempre quando está em quarenta telas em vez de no motor | custa menos que as duas colunas `Date`, ou o mesmo? **A medição decide o tamanho do sprint**, não se ele existe: se custar o mesmo, é açúcar com valor de correção, e vira P |

### Tier 4 — a rodada de formato do diário (`.log` v4)

> **Trava de formato (§2.3).** Os dois mexem no diário, que é a fonte da
> replicação. Nos dois, o teste que mais importa **não é o do recurso novo**:
> é o de que uma réplica que não conhece a versão nova continua aplicando.
>
> **E o item 21 deixou de ser «o primeiro tijolo da transação».** O desenho
> escolhido em `docs/TRANSACOES.md` §3 **não precisa da imagem anterior**:
> dentro de uma transação nada vai a disco antes do `COMMIT`, então não há
> estado anterior a guardar — desfazer é jogar fora uma lista em RAM. O item
> 21 continua valendo pelo que ele vale sozinho (auditoria, `AS OF`), e a
> pendência 3 não depende mais dele. *Medir a premissa do item vem antes de
> implementar o item — inclusive quando a premissa é «isto é pré-requisito
> daquilo».*

| # | Sprint | Tam. | Origem | Valor | A premissa que pode matá-lo |
|--:|---|:--:|---|---|---|
| 21 | **A imagem anterior no diário** | M | Teradata(R) 2 | julgamento; é literalmente o que a pendência 11 diz faltar («não há journal com a imagem anterior da linha»), e o Teradata(R) prova que a peça **se separa da transação** | custo por evento. Já medido o irmão: evento sem imagem 0,67 µs, com imagem posterior 1,61 µs. **Acima de 10% do `atualizar` (2,27 µs), nasce desligada** |
| 22 | **`AS OF` de uma linha** | M | MariaDB(R) 7 | julgamento; três quartos da matéria-prima já estão gravados (`.log` com imagem, `.trash`, `.reason`) e nenhum volume de diário é apagado | **a premissa é sua, e pode matá-lo:** a imagem da linha nasce **desligada** em servidor isolado. Ligar custa ~10% da vazão e 5× o diário (`FORMATO.md`). Aceita? Depois: quanto custa reconstruir uma linha num diário sem índice |

### Tier 5 — grandes, ou dependentes

| # | Sprint | Tam. | Origem | Valor | A premissa que pode matá-lo |
|--:|---|:--:|---|---|---|
| 23 | **Poda de volumes na varredura** | M | MariaDB(R) 8 — **depende do 10** | **inferido, não medido**: 12 volumes mensais, uma consulta de um mês lê 1 — 12× no papel | quantos volumes a pergunta típica dispensa, medido numa tabela particionada de verdade. E o alerta que a fonte dá de graça: com gatilho `BEFORE`, o MariaDB(R) **desiste da poda** — e gatilho acabou de entrar aqui |
| 24 | **`QUALIFY` + `ROW_NUMBER`/`RANK`** | M | Teradata(R) 5 — **depende do 10** (§2.4) | julgamento; «os 3 maiores pedidos de cada cliente» é o que uma ferramenta de BI gera sozinha pelo ODBC | o `PARTITION BY` cabe no `pivotar` que já existe? Se exigir ordenar a tabela em memória, **vira G e volta à mesa** |
| 25 | **`ALTER TABLE ADD COLUMN`, preservando o rowid** | G | MariaDB(R) 10 | julgamento, com um destravamento concreto: é o que falta ao editor de modelo (pedido 127), e o cartão da tela já diz isso em vez de fingir | custo de reescrever o `.reg` de uma tabela grande — **inferido, não medido** (a casa dos minutos para 10 milhões). E a armadilha: a coluna nova entra **antes** de `softdeleted` e `rownum`, deslocando as colunas de sistema — a família de defeito que já quebrou todo salvar e todo incluir |
| 26 | **Direito por linha** | G | Teradata(R) 9 | julgamento; a casa foi de direito por base a por tabela (124) e a marca por coluna (125); falta a linha | quanto custa por linha lida. **Se a varredura da bancada passar de 1,55 s (10%), o desenho está errado.** E o teste que importa é `sem_regra_de_linha_nada_muda` |
| 27 | **Índice de texto completo (`.fts`)** | G | MariaDB(R) 13 | **desconhecido — e o autor diz isso:** o número que justificaria ou mataria este sprint nunca foi medido | quanto custa **hoje** achar uma palavra num `.memo` de uma tabela de um milhão. Sem ele, «índice de texto completo» é desejo, não plano. É o sprint com mais chance de a premissa mudar a decisão |

---

## 7. De onde vem cada estimativa

A regra da casa é que número sem medição não vira plano. Esta seção separa.

**Com número medido nesta casa (3 de 27):**

| item | número | quem mediu, e em que condição |
|---|---|---|
| 1 | 6,5 s → 0,83 s (**7,8×**) | `--example custo-do-excluir 200000 20000`, no `SPRINTS-CASSANDRA.md` §3. **Condição ruim declarada**: máquina disputada, uma corrida de 48,8 s descartada. O que sustenta não é a mediana — é que a pior corrida sem `fsync` (0,891 s) ainda é 6,7× melhor que a melhor com ele (5,928 s) |
| 8 | **1,45×–1,58×** | medição do `SPRINTS-REDIS.md` §2, pelo soquete. `load average` 4,00 e 4,17, com três `phxsqld` de outros no ar. **Não publicável** até refazer em máquina quieta, e o loopback é onde o pipelining rende menos |
| 12 | 16,13 → 7,99 µs/linha (**2×**) | `DESEMPENHO.md` §4.9, `--example custo-do-fsync`. **É outro caminho** (a inserção), então aqui é inferência, não medida |

**Derivado da aritmética do formato, não medido (1):** o item 19 (~15,6% do
`.reg`), sobre uma largura que **foi** medida (`--example quanto-ocupa`).

**Inferido e dito como inferência (2):** os itens 23 (12× no papel) e 25 (a
casa dos minutos).

**Desconhecido, e o autor diz (1):** o item 27.

**Julgamento, sem número (20):** todos os outros. Onde há um fato duro por
trás do julgamento — «o insumo já está gravado», «o defeito existe hoje», «N
itens dependem deste» — o fato está na coluna de valor, com o `arquivo:linha`
quando é do código.

**E o custo (P/M/G) é julgamento em todos os 27.** Nenhum sprint desta casa
foi cronometrado antes de existir.

---

## 8. Uma ordem, se você perguntar

Não é a aprovação, é a defesa de uma ordem. A sua urgência de negócio manda
sobre ela.

1. **1, 2 e 3** primeiro. O 1 tem o único número grande e fecha a última fase
   em que a bancada perde. O 2 conserta um defeito que **existe hoje** e é
   quase de graça. O 3 lê um número que já está no disco.
2. **10** logo em seguida, porque é o item de que mais coisa depende — e
   porque apaga uma lista de recusas que a camada SQL carrega desde que
   nasceu.
3. **4, 5, 6, 7, 8, 9** em qualquer ordem: são pequenos, e **três deles podem
   terminar como um parágrafo em vez de código**. Isso é entrega completa, não
   fracasso.
4. **A rodada de formato** (16–20) só depois de você dizer **quais** entram —
   e as escolhidas entram juntas.
5. **11, 12, 13, 14, 15** conforme a urgência.
6. **21–22** como uma conversa só sobre o diário.
7. **23–27** por último; o 25 é o que destrava o pedido 127, e o 26 é o único
   que mexe em todo caminho de leitura.

---

## 9. A execução aguarda aprovação

**Nenhum dos 27 começa sem o seu sim, e o sim é item a item** — não em bloco,
com as duas exceções ditas: a trava de formato do esquema (§2.2) e a do
diário (§2.3), em que a decisão é «quais, na mesma rodada».

Três coisas que a aprovação deveria decidir junto:

1. **A ordem** — a da §8 é por valor ÷ custo, e não conhece a sua urgência.
2. **Os critérios de morte.** Cada item traz um número combinado **antes** da
   medição. Depois da medição, mudar o critério é escolher o resultado.
3. **O que pode virar só medição.** Os itens 4, 5, 6, 8 e 9 podem terminar
   como um parágrafo no documento da área. *A recusa com o número é resultado
   tão válido quanto o ganho* — e este documento já entregou cinco delas na §4
   e na §5, sem gastar uma rodada.

---

## Nota sobre os nomes

Apache Cassandra(R) é marca da Apache Software Foundation. Redis(R) é marca da
Redis Ltd. MariaDB(R) e Aria são marcas da MariaDB Corporation Ab. MySQL(R) e
InnoDB são marcas da Oracle Corporation. Teradata(R) e Teradata Vantage são
marcas da Teradata Corporation. PostgreSQL(R) é marca da PostgreSQL Community
Association of Canada. HFSQL(R) é marca da PC SOFT. Excel(R) é marca da
Microsoft Corporation. Idera(R) é marca da Idera, Inc.

Os quatro documentos-fonte leram **documentação pública** desses motores para
entender decisões de projeto; nenhum código foi copiado, e **nenhum dos 27
itens acima pede uma crate** — tudo seria escrito do zero, só com a `std` do
Rust.

Duas afirmações da folha de marca continuam **falsas** e não aparecem aqui:
*ACID compliant* e *built-in replication*. Não há transação, e a replicação é
funcionalidade construída, não propriedade herdada.
