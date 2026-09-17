# Parecer do papel C (DBA) — adiar o `.ndx` durante a reserva do `BULKINSERT`

**Data:** 17/09/2026. **Papel:** C (DBA sênior), escalão forte.
**Pergunta do orquestrador:** o dono decidiu adiar o `.ndx` durante a reserva do
`BULKINSERT` — carregar as linhas sem tocar o índice e reconstruí-lo no fim. A
medição do ganho está com outra frente. **Este parecer não mede ganho: ele
responde se a proposta quebra alguma garantia.**

**Método:** leitura do código, com arquivo e linha em cada resposta. Onde a
resposta só sai medindo, está escrito que só sai medindo, e o que a bancada
teria de fazer está na §8. Nenhum número deste documento foi digitado de
memória: cada um traz a origem e a data.

---

## 0. O que já estava decidido, e que a decisão do dono reabre

Antes de qualquer coisa, o registro — porque ele é a metade da resposta que
poupa tempo:

* **`docs/PENDENCIAS.md:123`, pedido 114, está FECHADO como «medido e
  recusado».** A peça que faltava (`construir_em_lote`) foi feita; o adiamento
  em si foi medido e ficou de fora.
* **`docs/DESEMPENHO.md` §4.4 (linhas 684–718)** traz o número, do commit
  `8808e23` de **29/08/2026** (*«Adiar o indice: medido, e ele quase nunca
  compensa»*): carregando M linhas numa tabela de N=200.000, o ganho é
  **1,22× quando M=N**, **1,10× em M=N/2**, e vira **prejuízo (0,86×) abaixo de
  M ≈ N/3**, chegando a **0,22×** em M=4.000. O ponto de virada fica perto de
  **M ≈ N/3**.
* **`docs/DESEMPENHO.md:536–548`** já separava a proposta em duas, com veredito
  diferente: *«Índice NÃO único, adiado — seguro»* e *«Índice ÚNICO, adiado —
  não»*.
* O mesmo §4.4 já nomeava o preço de formato: *«adiar exigiria marcar índice
  suspenso no formato do `.ndx`, porque uma queda no meio da carga deixaria uma
  árvore com chaves faltando e nada dizendo isso — busca respondendo errado em
  silêncio»*.

Reabrir é direito do dono, e este parecer não discute a reabertura. Mas o
número de 29/08/2026 continua sendo o número, e a frente que mede o ganho tem
de dizer **contra qual N e qual M** mediu — senão ela vai remedir o caso da
tabela vazia (1,59×) e publicar como se fosse o caso da carga incremental
(0,22× a 1,22×). *Medir a premissa do item vem antes de implementar o item.*

O cabeçalho de `crates/phxsql-server/src/carga.rs:12-16` diz que a objeção
histórica caiu: *«A objecao registrada em `docs/DESEMPENHO.md` contra adiar era
que a leitura veria um indice defasado e `buscar` responderia errado em
silencio. Com a tabela reservada nao ha leitura para ver.»*

**A premissa dessa frase é falsa em quatro pontos, e é o que a §2 mede.**

---

## 1. A tomada cai no meio da carga adiada. O que fica no disco?

### 1.1 Hoje, com índice inline

O `.ndx` acompanha o `.reg`, e a marca de sujo (byte 52) é o que torna a queda
**detectada**:

* `crates/phxsql-store/src/ndx.rs:745` — `gravar_pagina` levanta a marca **antes**
  de existir a primeira página suja, e grava o cabeçalho no arquivo;
* `crates/phxsql-store/src/ndx.rs:562` — `abrir` lê `cab[52]`;
* `crates/phxsql-store/src/ndx.rs:618` — `precisa_reconstruir: sujo`;
* `crates/phxsql-store/src/ndx.rs:886-894` — `conferir_confiavel` recusa;
* `crates/phxsql-store/src/ndx.rs:912-917` — e o portão mora num lugar só, o
  `descritor`, de propósito.

Medido na bancada «chutar a tomada» (`bancada/tomada/resultados.json`,
**16/09/2026**, 0.18.0, commit `29a333fe2aee`):

| ponto | corridas | `byte52 == 1` depois da queda |
|---|---:|---:|
| `bulk` (um `inserir` por pedido) | 72 | **1** |
| `bulk_lote` (`inserir_lote` de 250) | 72 | **35** |
| `reindexar` | 120 | **111** |

O `bulk` quase nunca deixa o índice sujo porque **o servidor abre e fecha a
tabela a cada operação** (`crates/phxsql-store/src/ndx.rs:105-107`), e o `Drop`
do `NdxFile` chama `fechar`, que desce as páginas e baixa a marca
(`ndx.rs:866-874`). Ou seja: **hoje o `.ndx` volta a bater com o `.reg` na
fronteira de cada pedido.** É essa propriedade que o adiamento revoga.

### 1.2 Com o índice adiado, na forma ingênua («pular `ndx.inserir`»)

**O `.reg` fica com N linhas e o `.ndx` fica LIMPO com zero chaves novas.** Não
é uma suposição: é a consequência direta de `ndx.rs:745` só levantar a marca
dentro de `gravar_pagina`. Se ninguém grava página, ninguém levanta a marca; e
`fechar` (`ndx.rs:866-874`) baixa `sujo` e regrava o cabeçalho no fim de cada
pedido, porque `precisa_reconstruir` é falso.

O banco reabre e responde: **`buscar` cala, `intervalo` cala, `inserir` aceita
chave repetida em índice único, e só `verificar` acusa.** É exatamente o defeito
que a casa chama de «busca respondendo errado em silêncio», e a própria bancada
já tem uma classe de veredito reservada para ele —
`*** VAZIO_OU_PARCIAL_EM_SILENCIO ***`, em
`bancada/tomada/chutar-a-tomada.py:685`.

**Resposta à pergunta «o byte 52 já marca índice sujo?»: NÃO, e não marcaria.**
A marca é acesa por escrita de página, não por intenção de adiar. Adiar é
justamente não escrever página.

### 1.3 Com o índice adiado e a marca levantada de propósito

Se a implementação levantar o byte 52 **antes da primeira linha ir ao `.reg`** e
o mantiver levantado durante toda a reserva, a queda passa a ser detectada — e o
banco reabre recusando **toda** operação de índice, com a mensagem de
`ndx.rs:888-891`. Isso é o comportamento certo, e é o único que preserva a
garantia. **Mas exige dois consertos que não são o adiamento:**

1. **`fechar` tem de aprender a não baixar a marca enquanto a suspensão durar.**
   Hoje `ndx.rs:870` baixa `self.sujo` em todo fechamento limpo. Como o servidor
   fecha a tabela a cada pedido, a marca cairia no fim do **primeiro**
   `inserir_lote` da carga — e o resto da carga correria sem rede.
2. **O estado «suspenso» tem de morar no disco, não na `Cargas`.** A reserva é
   memória de processo (`crates/phxsql-server/src/carga.rs:86-89`), e memória de
   processo morto não diz nada a quem reabre. Uma flag só em RAM reproduz
   exatamente o caso 1.2.

### 1.4 Quem lê o byte 52 no arranque?

**Ninguém.** É o pedido **255** (`docs/PENDENCIAS.md:279`), aberto: *«a
recuperação só reconstrói o índice de tabela nomeada numa marca; um `.ndx` sujo
sem marca fica recusando toda operação de índice — nunca em silêncio para o
cliente, mas silencioso para quem opera»*. A recuperação alcança só as tabelas
nomeadas numa marca `.tx` (`crates/phxsql-server/src/transacao.rs:1238`), e um
`BULKINSERT` fora de transação **não cria marca nenhuma**.

Consequência para esta proposta: hoje o 255 é um incômodo raro (queda dentro de
um pedido). **Com o adiamento ele vira a consequência normal de toda queda
durante qualquer carga**, e a tabela sai da queda recusando índice, sem nada no
arranque e nada no painel. **O 255 deixa de ser «decisão de desenho» e vira
pré-requisito.**

### 1.5 O buraco que o adiamento herda do `reindexar`, e que ele torna rotineiro

`Table::reindexar` (`crates/phxsql-store/src/table.rs:5091`) começa por
`NdxFile::criar`, que **trunca o arquivo e grava um cabeçalho LIMPO com árvore
vazia** (`ndx.rs:505` nasce com `sujo: false`, e `criar` termina em
`gravar_cabecalho`). A marca só sobe na primeira `gravar_pagina` do
`construir_em_lote`. **Entre um ponto e outro há uma janela em que o `.ndx` está
vazio e se declara limpo.**

Essa janela **não foi alcançada** nas 120 corridas do ponto `reindexar` de
16/09/2026 (classes observadas: 3 `INTEIRO_ANTES_DE_COMECAR`, 111
`SUJO_DETECTADO`, 6 `INTEIRO_DEPOIS_DE_TERMINAR`; zero
`VAZIO_OU_PARCIAL_EM_SILENCIO`). **Não alcançada não é inexistente** — está no
código, e a bancada a escreveu como hipótese antes de medir
(`bancada/tomada/chutar-a-tomada.py:48-54`).

O que muda com o adiamento: hoje o `reindexar` é reparo, e roda raramente. Com o
adiamento, **toda carga termina num `reindexar`**, e a janela passa a ser
percorrida uma vez por carga em vez de uma vez por reparo.

---

## 2. A reserva é a única coisa que impede uma leitura de ver o índice vazio?

**NÃO. E a reserva, sozinha, nem sequer impede todas as leituras.** Quatro
furos, conferidos no código e não no comentário.

### 2.1 A MESMA ligação lê à vontade

`crates/phxsql-server/src/carga.rs:175`:

```rust
Some(d) if d.ligacao == ligacao => return None,
```

`barra` devolve `None` para a própria ligação, **por desenho** — é o que permite
o carregador escrever. Mas ele também permite ao carregador `buscar`, `varrer
por índice`, `contar` e `consultar` a mesma tabela no meio da carga. Com o
índice adiado e a marca levantada, essas leituras **recusam** (fecham certo); com
a marca baixa, **respondem errado**.

E isso não é hipotético para o próprio motor: o `inserir` da carga confere a
unicidade em `table.rs:3160` (`self.ndx.existe`), que é uma leitura do índice
feita pela mesma ligação. Ver a §5.

### 2.2 Três operações escondem a tabela do portão — e aqui a conferência
própria **não existe**

O portão da carga é **um só**, `crates/phxsql-server/src/servidor.rs:10006-10013`
(Portão 4), e ele lê `pedido.texto_ou("tabela", "")`. O `barrado_por_carga` é
chamado de **um lugar só** no repositório inteiro (`servidor.rs:10011`).

É o mesmo campo, e portanto o mesmo furo, que a pétrea do `CLAUDE.md` já nomeia
para o portão de permissão: `juntar` guarda as tabelas em `a.tabela`/`b.tabela`,
`unir` numa lista, `pivotar` põe a de fatos no campo de sempre e as de consulta
dentro de um `juntar` aninhado, e `diferencas` as põe em `"a"`/`"b"`.

**O portão de permissão paga conferência própria nas quatro** (por exemplo,
`servidor.rs:20441-20455`, com o comentário dizendo por que não é duplicação).
**O portão da carga NÃO paga em nenhuma.** A varredura completa de quem nomeia
tabela fora do campo `"tabela"` já existe e está pronta para ser reusada:
`crates/phxsql-server/src/direito_coluna.rs:437` (`tabelas_do_pedido`), que
inclusive **desce** nos sub-pedidos do `consultar` (linhas 486–499).

**Hoje isso é inofensivo**, porque durante a reserva o índice está correto: ler
pelo lado B de uma junção devolve dado certo, só possivelmente meio carregado.
**Com o índice adiado deixa de ser inofensivo**: a junção pelo lado B lê um
índice vazio e devolve zero linhas — ou, se a marca estiver levantada, devolve um
erro de corrupção que não é corrupção nenhuma.

Isto é uma pétrea da casa aplicada a um portão novo: *quando o portão passar a
olhar um campo novo, procure quem não tem esse campo.* Aqui o portão é velho e
o **significado** dele é que muda.

### 2.3 O `reindexar` de outra ligação é barrado — e isso é um problema, não um alívio

`reindexar` nomeia `"tabela"`, então o Portão 4 o barra enquanto a tabela estiver
reservada. Somado ao §1.4 (o arranque não avisa), o operador que quiser reparar
uma tabela presa numa reserva órfã recebe «tabela reservada» em vez de reparar —
até a reserva vencer pelo prazo.

### 2.4 A réplica NÃO passa pelo portão — mas isso salva, não fere

`aplicar_lote_da_replica` (`crates/phxsql-server/src/servidor.rs:2889-2900`) abre
a tabela direto com `db.abrir_qualificada`, **sem passar pelo `despachar`**, e
grava com `Table::aplicar_evento` (`servidor.rs:2941`). Ou seja, a réplica
escreve por dentro, ignorando o Portão 4.

Isso **não** é um furo desta proposta, porque a réplica escreve o índice **dela**,
inline — ela não sabe nem precisa saber que o master adiou. Ver a §3.

---

## 3. A replicação

### 3.1 Os eventos são os mesmos

Adiar o `.ndx` não toca o `.log`: o `anotar(Operacao::Inclusao, …)` continua
acontecendo por linha dentro do `inserir`. **O `.reg` e o `.log` do master ficam
byte a byte iguais ao que ficariam sem o adiamento** — e a ordem de digitação
não muda, porque nada aqui mexe em slot. Ver §7.

### 3.2 A réplica reconstrói o índice dela sozinha

Sim, e **nunca recebe índice**. `aplicar_evento` → `aplicar_evento_interno`
(`crates/phxsql-store/src/table.rs:4086`) chama `self.inserir(&valores)`, o
`inserir` inteiro. Logo, a réplica paga o índice inline, linha a linha, como
sempre.

Consequência: **o `.ndx` do master e o da réplica divergem em bytes** — o do
master sai de `construir_em_lote`, que enche folha a 80% (`ENCHIMENTO_PADRAO`,
medido em `docs/DESEMPENHO.md` §4.3); o da réplica sai de inserção incremental,
que assenta perto de 69%. **Isso não é divergência de dado**: o conteúdo lógico
da árvore é o mesmo, porque a chave completa carrega o rowid e a ordem é total.

### 3.3 A bancada pega ou não pega?

**Não pega — e não precisa pegar, porque não há o que pegar.** O retrato da
bancada é um SHA-256 de cada **linha**, lido pelo cursor do `varrer`
(`bancada/replicacao/medir.py:70-89` e `bancada/replicacao/achados-do-dba.py:170-177`),
e `Table::varrer_com` lê o `.reg` direto (`crates/phxsql-store/src/table.rs:4167-4171`),
sem tocar o índice. Então a bancada compara dado, e o dado é igual.

**O que ela não veria, e que importa:** uma divergência de índice que fosse
divergência de *conteúdo* — chave faltando na árvore de um dos lados — passaria
invisível ao retrato atual. Quem a pegaria é `op_verificar`
(`NdxFile::verificar`, `ndx.rs:1587`), que confere CRC de toda página e a
ordenação das folhas, e a bancada de replicação **não o chama**. Ver §8.

### 3.4 O caso que trava o par de servidores

Este é o achado desta seção, e ele é grave.

A conferência de unicidade em `table.rs:3160` **não é desligada na réplica**: ela
está fora do `julga_integridade()` que protege FK (`table.rs:3114`) e cascata
(`table.rs:3511`). Então:

> Se o adiamento deixar uma chave duplicada entrar no `.reg` do master, o evento
> correspondente entra no `.log` do master. Quando a réplica o puxar,
> `aplicar_evento` → `inserir` → `PhxError::Duplicado`, o `?` sobe por
> `aplicar_lote_da_replica`, a posição **não anda**, e o mesmo lote volta na
> rodada seguinte **para sempre**.

É literalmente o modo de falha que `table.rs:4008-4012` descreve para outro
caminho: *«Nao e uma linha perdida: e o par de servidores parado.»* E não há
como desfazer no master: **o `.reg` não reaproveita slot**, e o evento já está
no diário.

**Conclusão da §3: a replicação não impede o adiamento — mas ela transforma
qualquer falha de unicidade tardia de «a carga se perdeu» em «o cluster
parou».** Isso é o argumento decisivo contra a opção (a) da §5.

---

## 4. A integridade referencial — a pergunta que mais preocupava

**Resposta curta: com a marca levantada, a conferência RECUSA (fecha certo, com
a mensagem errada). Sem a marca, ela MENTE — e mente na direção que mata a
pétrea.**

### 4.1 O lado do `excluir`: «alguém aponta para esta linha?»

`Table::conferir_filhas_com`, `crates/phxsql-store/src/table.rs:1624`:

```rust
if !filha.buscar(&indice, &chave)?.is_empty() {
```

O `?` propaga. Então:

* **Índice da filha adiado COM a marca levantada** → `filha.buscar` devolve
  `Err` (`ndx.rs:912` → `conferir_confiavel`) → o `?` propaga → **a exclusão
  recusa**. Fail-closed. A pétrea sobrevive.
* **Índice da filha adiado SEM a marca** → `filha.buscar` devolve `Ok(vec![])` →
  `is_empty()` é verdadeiro → **a mãe é apagada com filhas vivas, em silêncio.**

O segundo caso é a quebra direta da regra primordial. E o código já se defendeu
de uma versão mais fraca do mesmo erro três linhas acima (`table.rs:1588-1593`):
*«`expect` e nao `unwrap_or(&[])`: chave vazia sairia daqui como “nao tem filha”,
que e a resposta errada na direcao errada — a petrea diz nunca matar pai que tem
filho.»* **Um índice adiado sem marca é exatamente esse `unwrap_or(&[])`, em
escala de tabela inteira.**

### 4.2 O lado do `inserir`: «existe este pai?»

`Table::conferir_fks_com` → `mae.buscar(&indice, chave)`, envolvido em
`crates/phxsql-store/src/table.rs:2062-2080`.

* **Índice da mãe adiado COM a marca** → `mae.buscar` devolve `Err` → a inserção
  da filha recusa. Fail-closed.
* **Índice da mãe adiado SEM a marca** → `Ok(vazio)` → **a filha legítima é
  recusada** com «o pai não existe», embora exista no `.reg`. Aqui o erro é para
  o lado seguro, mas é uma recusa falsa que ninguém entende.

### 4.3 O furo que a reserva NÃO tapa, e que é próprio desta proposta

**A reserva é da MÃE; o pedido que paga o preço nomeia a FILHA.** O Portão 4 lê
`"tabela"` do pedido, que num `inserir` de filha é a filha. **Reservar a mãe não
barra inserção na filha.** Então, enquanto a mãe carrega com índice adiado:

* toda inserção em **qualquer** filha dela, de **qualquer** ligação, cai em
  `conferir_fks` → `mae.buscar` → recusa;
* e a mensagem que sai é a de `table.rs:2066-2076`, que diz textualmente
  **«o arquivo está SÃO: não repare nada … confirme a mãe antes da filha»**.

Essa mensagem foi escrita para o limite de visibilidade de transação e está
**certa para o caso dela**. Para o caso do adiamento ela está **errada**: o
arquivo não está são, não há mãe em transação para confirmar, e o conselho não
conserta nada. É a armadilha que esta casa já pagou no pedido 176 — o recado que
dá uma ordem que a situação desmente.

Mesma coisa no caminho **irmão**: `planejar_ao_alterar` →
`filha.buscar(&indice, &antiga)`, `table.rs:2079-2096`, com a mensagem gêmea.

### 4.4 Veredito da §4

A integridade referencial **não é quebrada pelo adiamento em si**, desde que a
marca esteja levantada — mas ela **fica indisponível** para toda a família da
tabela enquanto a carga durar, e **a recusa sai com o motivo errado**. Isso
obriga a uma restrição, escrita na §9.

E a restrição não é opcional: o `.ndx` da mãe é exigido pela própria declaração
da chave (`table.rs:1615-1622`: *«não tem índice começando por (…) — crie o
índice na filha ou desligue `verificar` na chave»*). A chave conferida **precisa
de índice dos dois lados**; adiar um dos dois é desligar `verificar` sem que
ninguém tenha pedido.

---

## 5. A unicidade

`docs/DESEMPENHO.md:85` (tabela da §2, medida com `--example onde-doi -- 200000`,
re-medida em **08/09/2026** no binário da 0.18.0): conferir a chave única custa
**0,3 µs**, **1,9%** da inserção — porque o `.ndx` responde de dentro do cache de
páginas.

O motivo de a conferência acontecer **antes** de qualquer gravação está escrito
no código, e é de formato — `crates/phxsql-store/src/table.rs:3152-3157`:

> *«A conferencia acontece AQUI, antes de qualquer gravacao, e nao la dentro do
> `ndx.inserir`, por um motivo de formato: o `.reg` nunca reaproveita slot.
> Descobrir a duplicidade depois de gravar exigiria desfazer, e o slot desfeito
> ficaria morto para sempre.»*

### (a) Não conferir e deixar a reconstrução falhar no fim — **PROIBIDA**

`NdxFile::construir_em_lote` **detecta** a duplicata
(`crates/phxsql-store/src/ndx.rs:1251-1254`, `PhxError::Duplicado`). Mas detecta
**no fim**, e aí o estrago já é permanente:

1. As N linhas estão no `.reg`, e **o `.reg` nunca reaproveita slot**. A carga
   inteira vira buraco permanente, não uma linha.
2. `reindexar` já truncou o `.ndx` (`table.rs:5091`) e já sujou o arquivo com as
   páginas dos índices que construiu antes de falhar. A tabela sai **recusando
   índice** e só volta depois de alguém apagar as linhas duplicadas à mão.
3. **E, pela §3.4, os eventos já foram para o `.log` e a réplica trava.**

É a pétrea da ordem de digitação usada ao contrário: ela existe para que a
posição de uma linha nunca minta, e a opção (a) a transforma em «a carga que
falhou fica no disco para sempre». **Recusada, e não por gosto: por uma decisão
já escrita no código, em `table.rs:3153`.**

### (b) Mapa em memória só da chave única — **permitida, com dois custos que a leitura não mede**

Não quebra pétrea nenhuma: a decisão continua acontecendo antes da gravação, só
muda a estrutura que responde. Os custos:

* **Semeadura.** A carga pode entrar em tabela que já tem linhas. O mapa teria de
  nascer com **todas** as chaves únicas já existentes, o que é uma varredura da
  árvore antiga (ou do `.reg`) no início da carga — custo proporcional a **N**,
  não a M, e portanto exatamente o custo que empurra o ponto de virada da §0
  para cima.
* **RAM.** É memória do processo, não paginada, viva durante toda a reserva, e
  **sem teto declarado**. O `.ndx` tem teto (`recursos.cache_paginas`,
  `ndx.rs:98-107`); um mapa de chaves não teria. Uma carga de dez milhões de
  linhas num contêiner de 560 MB livres é o cenário que o zelador já encontrou.

**Nenhum dos dois se responde lendo. Precisam de bancada** — ver §8.

### (c) Recusar adiar quando há índice único — **permitida, e é a que o formato e as pétreas sustentam sem dívida nenhuma**

É o que `docs/DESEMPENHO.md:543-548` já concluiu: *«Índice ÚNICO, adiado — não.
Ele é a própria decisão de aceitar ou recusar a linha.»*

O preço: sobra adiar só o não único. A medição de 29/08/2026 (`DESEMPENHO.md:718`)
diz que na forma da bancada isso vale **1,19 s de 3,93 s** — e a tabela de §4.3
dá **1,59×** no caso da tabela vazia. Contra os 3,28× do teto.

**Recomendação do papel C: (c) como padrão, (b) como pedido explícito e medido
depois.** (a) está fora.

---

## 6. Mudança de formato?

**Sim, uma — e ela é barata, cabe hoje, e NÃO precisa de bump de versão nenhum.**

### 6.1 O que falta é um byte no `.ndx`, não no `PSCH`

São arquivos diferentes e versões independentes: o `PSCH` v10 do pedido 314
(`docs/propostas/parecer-dba-psch-v10-2026-09-17.md`) é o bloco de esquema do
`.reg`; o `.ndx` tem `VERSAO: u16 = 1` própria (`ndx.rs:66`). **Nada aqui pede
`PSCH` novo, e nada aqui cabe no bump do 314.** Amarrar um ao outro só faria a
decisão do dono sobre a coluna de data/hora esperar por esta.

### 6.2 Onde o byte cabe

`docs/FORMATO.md:711-713`:

```
| 52 | 1 | **marca de sujo** (0 = fechado limpo) |
| 53 | 71 | reservado |
| 124 | 4 | CRC-32 dos bytes 0..124 |
```

**Há 71 bytes reservados.** O byte **53** serve, com a mesma semântica que fez o
byte 52 não precisar de migração (`FORMATO.md:741`): **0 = não suspenso**, que é a
verdade para todo arquivo já escrito. Zero migração, zero bump, banco antigo
continua legível.

O que ele guardaria: **«este índice está suspenso por uma carga»** — o estado que
a §1.3 mostrou não poder morar na `Cargas`, porque memória de processo morto não
fala.

### 6.3 O que o byte 53 compra, e o que ele NÃO compra

Compra **três** coisas que o byte 52 sozinho não dá:

1. **`fechar` sabe não baixar a marca** entre um pedido e o seguinte da mesma
   carga (o conserto obrigatório da §1.3.1), sem confundir «suspenso» com
   «caiu».
2. **O arranque sabe distinguir** «caiu no meio de uma carga adiada, reconstruo
   sozinho porque sei que é isso» de «write-back perdeu páginas» — que é
   literalmente o que o pedido 255 pede para decidir.
3. **A mensagem de erro pode dizer a verdade** em vez da mensagem de
   visibilidade de transação da §4.3.

**Não compra** a lista de linhas que faltam, e **não deve tentar**: guardar «estas
são as chaves que faltam» no cabeçalho recria um diário dentro do `.ndx`, e o
`reindexar` em lote já custa **0,31 s por milhão de chaves**
(`DESEMPENHO.md` §4.3, medido com `--example indice-em-lote`). Refazer inteiro é
mais barato que manter o registro do que falta.

### 6.4 A pétrea «mudança de formato entra cedo» — está sendo cumprida aqui

É por isso que este parecer diz **agora**: o byte 53 custa uma linha hoje e uma
migração depois. Se o dono aprovar o adiamento, **o byte entra no mesmo commit em
que o adiamento entra, e o `FORMATO.md` muda junto** (regra do `CLAUDE.md`,
«Mexeu no formato em disco? Atualize `docs/FORMATO.md` no mesmo commit»).

### 6.5 O risco residual, nomeado

Um binário **anterior à 0.18.0** ignora o byte 52 e ignoraria o 53. Ele leria uma
árvore suspensa como se fosse boa. Isso **já é verdade hoje** para a marca de
sujo do write-back — a exposição não é nova e não cresce. Registrado, não
consertado.

---

## 7. A ordem de digitação

**NÃO quebra.** O adiamento não toca o `.reg`: `Table::inserir` continua chamando
`self.reg.inserir_no_balde` / `inserir_no_periodo` (`table.rs:3182-3185`), e o
rowid continua saindo do contador do `.reg`. Nenhum slot é reusado, nenhum é
pulado, nada é reordenado. O `reindexar` do fim varre na ordem de digitação
(`table.rs:5088-5100`).

**A única forma de esta proposta ferir a ordem de digitação é pela opção (a) da
§5** — e não por reuso, e sim por desperdício permanente: a carga inteira
morrendo em slots que nunca voltam. Por isso (a) está recusada.

---

## 8. O que eu NÃO consegui provar por leitura

| # | Pergunta | Por que a leitura não responde | Bancada |
|---|---|---|---|
| 1 | O ganho real, com N e M declarados | O número de 29/08/2026 (`DESEMPENHO.md` §4.4) é de antes do cache write-back e do CRC slice-by-16, que mudaram a repartição (`DESEMPENHO.md:29-42`, re-medido em 08/09) | `--example adiar-vale-quando` e `--example indice-adiado`, **no binário de hoje** e com `cargo build --release --examples` antes — medidor com binário velho mede o passado |
| 2 | Custo de RAM e de semeadura da opção (b) | Não há estrutura para medir; depende de M, de N e da largura da chave | medidor novo: semear o mapa das chaves únicas de uma tabela de N linhas, e medir pico de RSS e tempo |
| 3 | A janela «vazio e limpo» do `reindexar` (§1.5) é alcançável? | 120 corridas em 16/09/2026 não a alcançaram; a classe existe e ficou vazia | `bancada/tomada/chutar-a-tomada.py`, ponto `reindexar`, com a faixa de atrasos recalibrada para a janela entre `criar` e a primeira página |
| 4 | O `juntar`/`unir`/`pivotar`/`diferencas` realmente atravessa o Portão 4? | Li que `barrado_por_carga` só é chamado em `servidor.rs:10011` com o campo `"tabela"`; **não provei pelo soquete** | prova pelo soquete: reservar `A`, e de outra ligação pedir `juntar` com `b.tabela = A`. O que depende do sistema operacional se prova contra ele |
| 5 | O travamento da réplica da §3.4 | A leitura mostra o caminho (`table.rs:3160` fora do `julga_integridade`, `servidor.rs:2941` com `?`); não reproduzi | `bancada/replicacao`: forçar um `Duplicado` no `.log` do source e ver se a posição da réplica para |
| 6 | Custo de um `op_verificar` nas quatro réplicas no fim da bancada | — | `bancada/replicacao/medir.py`, acrescentar `verificar` ao retrato |

---

## 9. Veredito

### **SEGURO COM RESTRIÇÃO.** Cinco restrições, e nenhuma é negociável por desempenho.

Por garantia, item a item:

| Garantia | Quebra? |
|---|---|
| Ordem de digitação (`.reg` não reaproveita slot) | **NÃO** — §7. Salvo na opção (a) da §5, que por isso está recusada |
| Integridade referencial (nunca se mata o pai que tem filhos) | **NÃO com a marca levantada; SIM sem ela** — §4.1. Sem marca, `conferir_filhas` lê «não tem filha» e apaga a mãe |
| Chave conferida precisa de índice dos dois lados | **SIM, na prática** — §4.3: durante a carga um dos lados não responde, e a recusa sai com a mensagem errada |
| Unicidade conferida antes da gravação | **SIM na opção (a); NÃO em (b) e (c)** — §5 |
| Guarda nova entra pedida, não imposta | **NÃO** — §6.2: byte 53, zero = não suspenso, banco antigo intacto |
| Mudança de formato entra cedo | **cumprida por este parecer** — §6.4 |
| Replicação: os quatro iguais no fim | **NÃO** — §3.1/§3.2. Mas §3.4: a falha tardia de unicidade **para o par de servidores** |

### As cinco restrições

**R1 — O índice único não se adia.** É a opção (c) da §5, e é
`docs/DESEMPENHO.md:543-548` já escrito. Quem quiser a (b) pede, e só depois de a
bancada 2 da §8 responder.

**R2 — A marca vai ao disco ANTES da primeira linha, e o `fechar` não a baixa
enquanto a suspensão durar.** Byte 53 do `.ndx` (§6.2), com o byte 52 levantado
junto. Sem isto, §1.2 e §4.1 acontecem.

**R3 — Não se adia o índice de tabela que é mãe de alguém com `verificar`
ligado.** A recusa é na **declaração da carga**, não na gravação — pelo mesmo
motivo de sempre: a carga se pede uma vez e insere um milhão de vezes, e recusar
tarde custa a carga inteira. A busca é a mesma varredura por exclusão que
`conferir_filhas_com` já faz (`table.rs:1550`, `catalogo::tabelas_em`), e ela
cabe aqui porque **reservar é raríssimo** — é o mesmo argumento da pétrea: o
custo mora onde é raro.

**R4 — O Portão 4 passa a olhar onde a tabela se esconde.** `juntar`, `unir`,
`pivotar` e `diferencas` — reusando `direito_coluna::tabelas_do_pedido`
(`direito_coluna.rs:437`), não uma quinta cópia da lista. Sem isto, a §2.2 vira
leitura de índice vazio pelo lado B de uma junção.

**R5 — O pedido 255 vira pré-requisito, não «decisão de desenho».** O arranque
tem de ler o cabeçalho e dizer o que achou. Com a §1.4, toda queda durante uma
carga adiada deixa a tabela recusando e o operador sem aviso.

### As recusas, no molde desta casa

R3, na declaração da carga:

```
clientes nao pode carregar com o indice adiado: pedidos e faturas declaram
chave estrangeira conferida para clientes, e enquanto o indice estiver
suspenso a conferencia «existe este pai?» nao tem como responder -- carregue
sem adiar, ou desligue `verificar` nessas chaves antes.
```

R1, na declaração da carga:

```
clientes nao pode carregar com o indice adiado: porCpf e um indice UNICO, e
ele e a propria decisao de aceitar ou recusar a linha. Adiar seria gravar
primeiro e descobrir a chave repetida no fim -- e o `.reg` nunca reaproveita
slot, entao a carga inteira ficaria como buraco permanente. Carregue sem
adiar, ou tire o indice unico antes da carga e recrie-o depois.
```

R2, se alguém tentar carregar adiado num `.ndx` que já está sujo:

```
o indice de /dados/loja/clientes.ndx ficou para tras numa queda e nao e
confiavel: reconstrua com `reparar indice` antes de comecar a carga -- adiar
sobre um indice que ja nao presta apaga a unica marca que diz isso.
```

### E uma observação que não é minha de ofício, mas é de dever

A convergência dos quatro motores maduros não ajuda aqui, e vale dizer por quê:
PostgreSQL, MariaDB, MySQL e SQLite **todos** adiam ou desativam índice em carga
(`COPY`, `ALTER TABLE … DISABLE KEYS`, `PRAGMA`). **Mas os quatro o fazem com
transação por baixo** — no PostgreSQL o `COPY` está numa transação e o MVCC
segura a leitura; no MariaDB o `DISABLE KEYS` vale só para índice **não único**,
exatamente pela razão da §5. A convergência é, portanto, **a favor da restrição
R1, não contra ela**. Não há choque com pétrea nossa a levar à mesa: os quatro e
nós concordamos que o índice único não se adia.

---

## 10. Resumo de uma linha

Adiar o `.ndx` é **seguro se e somente se** a suspensão for **declarada no
disco** (byte 53), **o índice único ficar de fora**, e a carga **recusar na
declaração** quando a tabela for mãe de alguém com `verificar` ligado. Sem essas
três, o adiamento converte a regra primordial da integridade num
`unwrap_or(&[])` do tamanho de uma tabela — e a casa já escreveu no código, em
`table.rs:1588-1593`, por que isso é a resposta errada na direção errada.
