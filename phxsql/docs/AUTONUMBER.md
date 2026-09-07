# O fluxo do auto number id e do sequence — como está e como seria o ideal

*Pedido do dono, 07/09/2026: «Fluxo do funcionamento do auto number id e do
sequence como está e como seria o ideal.»*

**Tudo o que este documento afirma foi medido em 2026-09-07 17:39 UTC, commit
`395848f`, contra o motor vivo, por `bancada/sequencias/sonda.py`** — 25 blocos,
gravados em `bancada/sequencias/resultados.json` com a data da corrida. Nenhum
número aqui foi digitado de memória, e os que não puderam ser medidos aparecem
dizendo que não foram.

A resposta curta já publicada está em `docs/pdf/respostas/00-sequencia.md`
(«o sequence do PostgreSQL e o nosso»); este documento é a metade que faltava —
o **fluxo**, de ponta a ponta, e o **ideal**.

---

# PARTE A — COMO ESTÁ

## A.1 São TRÊS números, não um

Confundi-los foi o primeiro erro desta bancada, e a confusão é fácil: os três
sobem, os três nunca voltam, e **dois deles moram no mesmo cabeçalho de 128
bytes**.

| | **`rowid`** | **`rownum`** | **`Sequence`** |
|---|---|---|---|
| o que é | posição física do slot no `.reg` | ordem de chegada da linha | o identificador do usuário |
| de quem | do motor, invisível ao esquema | coluna de **sistema**, visível | coluna do **usuário** |
| onde mora o contador | `slot_count`, byte **20** | `proximo_rownum`, byte **92** | `proxima_sequencia`, byte **36** |
| tipo | — | `UInt8` não nula | `Sequence` = `u64`, 8 bytes (código 21, `types.rs:69`) |
| quantas por tabela | uma | uma, sempre | **no máximo uma** (`schema.rs:635`) |
| quem preenche | o motor, ao anexar o slot | o motor, sempre (`table.rs:2105`) | o motor **quando o valor chega nulo** (`table.rs:2307`) |
| o cliente pode escolher | não | **não** — valor de fora é ignorado | **sim**, e o contador acompanha |
| se ajusta | não | **não** | `ajustar_sequencia` (exige `administrar`) |
| reaproveita depois de excluir | nunca | nunca | nunca (a não ser por ajuste manual) |

A frase que resume o desenho: **`rowid` é onde a linha está, `rownum` é quando
ela chegou, `Sequence` é como o mundo lá fora a chama.**

A `Sequence` **não** pode ser a coluna de sistema, e o motivo está no
`FORMATO.md` §«A coluna de sistema `rownum`»: o contador do `.reg` é único, e
reservar a única vaga para o motor tiraria do usuário um tipo que é dele. Por
isso o `rownum` tem contador próprio, nos bytes 92..100.

## A.2 O fluxo de uma inserção, passo a passo

Os números entram **antes** de qualquer byte ir para o disco, e a ordem entre
eles não é arbitrária.

```
op_inserir (servidor.rs:11270)
  └─ trava de dados  ── abrir_travada (servidor.rs:6513)
       │  a trava cobre ABRIR E GRAVAR como um bloco só: o cabeçalho traz
       │  slot_count e proxima_sequencia, e duas operações que abrissem a
       │  tabela ao mesmo tempo gravariam no MESMO rowid, em silêncio.
       └─ Table::inserir (table.rs:2591)
            1. conferir_aridade
            2. conferir_fks            ── a regra primordial da integridade
            3. completar               (table.rs:2073)  softdeleted=false, rownum=0
            4. numerar_linha           (table.rs:2105)  rownum = proximo_do_rownum()
            5. numerar                 (table.rs:2307)  Sequence:
                 · Value::Null  → proxima_da_sequencia()   (reg.rs:693)
                 · Value::UInt(n) → anotar_sequencia(n)     (reg.rs:758)
            6. todas_as_chaves + conferência de unicidade  ← ANTES de gravar
            7. montar_payload, ponteiros (.bin/.memo)
            8. reg.inserir* → rowid,  e gravar_contadores(1)  (reg.rs:799)
            9. ndx.inserir_ja_conferido (com desfazer se falhar)
           10. indexar_texto (.fts), anotar (.log)
```

**Por que os passos 4 e 5 vêm antes do 6.** Se a `Sequence` estiver num índice —
e ela é a chave primária de quase toda tabela nascida pela tela —, a chave
conferida tem de ser a **do número que vai ser gravado**, e não a do nulo. O
comentário do `table.rs:2605` diz exatamente isso.

**O `rownum` ignora o que vem de fora.** `numerar_linha` chama
`proximo_do_rownum()` sempre que `anterior.is_none()` — ou seja, em toda
inserção, inclusive nas que a replicação aplica. Um número escolhido à mão seria
uma ordem de chegada inventada.

**A `Sequence` faz o contrário: ela obedece e depois acompanha.** Valor nulo
ganha o próximo do contador; valor escolhido **empurra** o contador para depois
dele (`anotar_sequencia`). É o que faz `id=100` seguido de um automático dar
**101** — medido no bloco 3 — e é onde o PhxSql diverge do PostgreSQL, que não
avança a sequência num `INSERT` com valor explícito e por isso colide na
inserção seguinte.

**Quanto o contador custa por linha: um `write` de 128 bytes, e nenhum `fsync`.**
`gravar_contadores` (reg.rs:799) regrava só o cabeçalho a cada inserção — não o
bloco de esquema, que é imutável. O `fsync` espera o fecho da janela de
durabilidade (`reg.rs:2088`). Medido nesta corrida:

| operação | mediana | min | max | amostras |
|---|---:|---:|---:|---|
| `write` de 128 bytes (o que o contador paga) | **0,50 µs** | 0,48 | 0,60 | 9 × 2.000 |
| `fdatasync` | **83,5 µs** | 72,6 | 125,0 | 9 × 200 |
| `fsync` | **82,1 µs** | 80,0 | 142,9 | 9 × 200 |

**Um sincronizar custa 164× o `write`** (151× no piso da faixa). O contêiner é
compartilhado — a carga na corrida era 3,60 —, e é por isso que a faixa vai
publicada junto: em qualquer ponto dela a conclusão é a mesma, duas ordens de
grandeza.

E o custo **marginal** da coluna `Sequence` não se distingue do ruído: 400
inserções pela rede com ela custaram **163,3 µs** (151,4–174,2) e com um `Int8`
mandado à mão **163,8 µs** (160,0–195,2) — as faixas se cruzam, e pela regra do
pedido 155 **não há vencedor**. Faz sentido: o cabeçalho ia ao disco de todo
jeito, por causa do `slot_count`.

## A.3 Cenário a cenário — o que acontece com cada um dos três

Cada linha traz o bloco da sonda que a mediu. Todos os blocos estão em
`bancada/sequencias/sonda.py` e a saída crua em
`bancada/sequencias/resultados.json`, corrida de 2026-09-07 17:39 UTC.

| # | cenário | `rowid` | `rownum` | `Sequence` |
|---|---|---|---|---|
| 9 | inserção comum | 1,2,3 | 1,2,3 | 1,2,3 |
| 3 | inserção com valor à mão (`id=100`) | segue | segue | contador **acompanha**: o próximo sai **101** |
| 10 | `inserir_lote` de 5 | 1..5 | 1..5 | 1..5 |
| 10 | lote com um `id=900` no meio | segue | segue | 900 e depois **901** |
| 11 | `BULKINSERT` (reserva da tabela) | igual | igual | igual — o cabeçalho antes e depois de soltar é o **mesmo** |
| 12 | **`ROLLBACK`** | **devolve o slot** | **devolve o número** | **devolve o número** |
| 12 | dentro da transação, antes do `COMMIT` | previsto (`slots()+1`) | **`null`** | **`null`** |
| 12 | `COMMIT` | grava | numera aqui | numera aqui |
| 13 | exclusão **suave** | fica | fica | fica; `marcadas` 0→1 |
| 13 | **restaurar** | fica | fica | fica; `marcadas` 1→0 |
| 13 | exclusão **de vez** | slot morre, nunca volta | contador **não** volta | contador **não** volta |
| 14 | partição por **letra** | Silva=18001, Alves=1 | Silva=**1**, Alves=**2** | Silva=**1**, Alves=**2** |
| 14 | idem — onde mora o contador | — | no balde **`_A`** (o volume 1) | no balde **`_A`** |
| 14 | inserir na alfanumérica **dentro de transação** | — | — | **recusado**: o rowid alvo não é previsível fora do motor |
| 15 | partição por **quantidade** e por **período** | cresce entre volumes | cresce | cresce; contadores **só no volume 1** |
| 16 | `ajustar_sequencia` para trás **sem** índice único | — | — | **repete calado**: ids `[1, 2, 3, 1]` |
| 6 | idem **com** índice único | — | — | recusa na inserção: `SP000020 chave duplicada` |
| 17 | contador reposto para trás **no disco** | — | — | o **CRC-32 do cabeçalho pega**; `verificar`, `reparar` e `inserir` param |
| 18 | queda do processo (`SIGKILL`) | 7→8 | 7→8 | 7→8 — **nada volta atrás** |
| 19 | número acima de **2⁵³** pelo protocolo | — | — | **perde precisão em silêncio** (2 de 3 divergiram) |
| 20 | backup e restauração | volta ao instante | volta ao instante | **volta ao instante do backup** |
| 21 | `reindexar` e `verificar` | não tocam | não tocam | não tocam |
| 22 | **replicação** source → réplica | igual, e diverge = para | reatribuído localmente | **vem da origem**, e o contador acompanha |
| 23 | **promoção** de uma réplica atrasada | — | — | continua de onde **ela** parou: **5 números reemitidos** |
| 24 | **bidirecional**, mesma faixa | — | — | **4 inserções → 2 linhas**; duas somem |
| 24 | bidirecional, faixas disjuntas | — | — | funciona **uma** rodada; na segunda, 6 inserções → 5 linhas |
| 24 | bidirecional com chave **`Uuid` v7** | — | — | **4 inserções → 4 linhas**, dos dois lados |

### As sete que merecem parágrafo

**1. O `ROLLBACK` devolve o número, e o PostgreSQL não devolve.** Não é opção:
é consequência do formato. `docs/TRANSACOES.md` §3.2 e o `transacao.rs` abrem
com a frase «nada vai a disco antes do `COMMIT`» — porque o `.reg` nunca
reaproveita slot, e um `INSERT` gravado e depois revertido deixaria um buraco
permanente **e teria de deixar o MESMO buraco na réplica**. Como a linha não
chega ao disco, o `numerar` não roda; quem numera é o `aplicar_uma`
(`transacao.rs:1240`) na passada do commit. Medido: contador em 2 antes do
`BEGIN`, 2 durante, **2 depois do `ROLLBACK`**.

**2. E o preço disso é a ficha mestre-detalhe.** Dentro da transação, a linha
recém-inserida se lê com **`id: null` e `rownum: null`** — os números ainda não
existem. Quem quiser gravar a mãe e as filhas na mesma transação **não tem o id
da mãe para pôr nas filhas**. No PostgreSQL o `nextval` acontece no `INSERT`, e
o `RETURNING id` funciona lá dentro. É a divergência mais cara do desenho de
hoje, e é a que a Parte B ataca primeiro.

**3. A partição por letra é a exceção que justifica o `rownum`.** A Silva
digitada primeiro mora no `_S` com rowid 18001; a Alves digitada depois mora no
`_A` com rowid 1. Ordem física e ordem de chegada **divergem**, e é por isso que
`Table::posicao_e_rownum` (table.rs:3913) recusa a bisseção nesse modo. A
`Sequence` acompanha a chegada, não o rowid: Silva=1, Alves=2. E um detalhe que
só aparece no disco: **os contadores moram no balde `_A`**, que é o volume 1 —
o arquivo existe mesmo numa tabela sem nenhum nome começado por A.

**4. Ajustar para trás sem índice único repete, e ninguém avisa.** A resposta de
`ajustar_sequencia` traz o aviso certo («o contador andou para TRÁS…»), mas ele
fala de um índice único que pode não existir. Medido, numa tabela sem índice:
ids `[1, 2, 3, 1]`, quatro linhas, **um id repetido**, nenhum erro.

**5. A queda do processo não perde o contador — mas a queda da máquina pode.**
Com `SIGKILL` o contador voltou reaberto exatamente como estava (8): o `write`
do cabeçalho já estava no cache do sistema operacional, e matar o processo não
o apaga. O que o `SIGKILL` **não** prova é queda de energia, e aí o `fsync`
pendente é a diferença. *O que depende do sistema operacional se prova contra o
sistema operacional* — e este teste prova o que prova, não mais.

**6. O teto real do número não é o do formato.** `docs/FORMATO.md` publica
2⁶⁴−1 para o `rownum`, e a coluna `Sequence` é `u64`. Só que o `Json` desta casa
tem **um único tipo numérico, `Numero(f64)`** (`json.rs:20`), e acima de 2⁵³ o
inteiro deixa de ser representável. Medido, na resposta crua do servidor:

```
mandado 9007199254740992  →  gravado 9007199254740992   ok
mandado 9007199254740993  →  gravado 9007199254740992   PERDEU
mandado 9007199254740995  →  gravado 9007199254740996   PERDEU
```

O teto **prático** de uma `Sequence` pelo protocolo é **9.007.199.254.740.992**,
e não 18.446.744.073.709.551.615. Não é um problema teórico: a receita de
multi-master por faixa («o servidor 3 começa em 3×10¹⁵») cai bem dentro da
zona onde o número se corrompe calado.

**7. Na replicação, o valor vem da origem — e o `rownum` não.** O evento carrega
a **imagem da linha**, e `aplicar_evento_interno` (table.rs:3428) decodifica e
chama `inserir`. A `Sequence` chega preenchida, cai no ramo `Value::UInt` do
`numerar` e o contador da réplica **acompanha**: medido, os dois cabeçalhos
ficaram idênticos (`proxima_sequencia = 702` dos dois lados) mesmo depois de um
valor gravado à mão e de uma exclusão física. O `rownum`, ao contrário, é
**reatribuído localmente** — coincide porque a ordem de aplicação é a mesma, e
não porque foi copiado.

## A.4 O que a tela e o SQL veem hoje

- **`SELECT … WHERE id = 2` recusa quando a chave é `Sequence`** e passa quando
  é `Int8`. É o **pedido 223**, aberto: o alargamento de tipo do `docs/SQL.md`
  §5 (aceitar o literal como texto, que é como todo parâmetro chega de ODBC e do
  protocolo do PostgreSQL) alcançou `Int` e **não alcançou o irmão**
  (`valores.rs:707`). Na prática, um `WHERE id = 2` escrito por um driver falha
  na tabela mais comum que existe. Medido hoje: `esperado numero da sequencia,
  recebido Texto("2")`.
- **Não há `INSERT` na camada SQL** — «só `SELECT`». Logo, não há `RETURNING id`
  por SQL; pelo protocolo, o `inserir` devolve o **`rowid`**, e para saber a
  `Sequence` gerada é preciso reler a linha.
- **`sequencias`** lista tabela, coluna, próximo número e quantos registros;
  **`ajustar_sequencia`** zera ou pula, e exige `administrar`.

## A.5 O inventário do que NÃO existe hoje

| o que os outros têm | aqui | por quê |
|---|---|---|
| `CREATE SEQUENCE` solto, `nextval`/`currval`/`setval` | não | o contador é do `.reg`; uma sequência independente precisa do próprio arquivo — sprint 11 do `docs/SPRINTS-MARIADB.md` |
| mais de uma sequência por tabela | não | o contador do `.reg` é único (`schema.rs:635`) |
| `START WITH` / `INCREMENT BY` / `MINVALUE` / `MAXVALUE` / `CYCLE` | não | o passo é 1, o início é 1, não há teto declarado nem volta |
| `CACHE` (reserva em bloco) | não | e também não é preciso: o contador não paga `fsync` por número |
| `IDENTITY … GENERATED ALWAYS` (recusar valor do cliente) | não | hoje o valor do cliente é sempre aceito, e o contador o segue |
| `auto_increment_increment` / `_offset` (a paridade do MariaDB) | não | e a falta **custa dado** no modo bidirecional — §A.3, bloco 24 |
| `RETURNING id` | não | o protocolo devolve o `rowid`; a `Sequence` sai relendo |
| `OWNED BY` (a sequência morre com a tabela) | de graça | o contador **é** da tabela; some com ela |
| ver o próximo número sem consumir | sim | `sequencias` devolve `proxima` sem avançar |
| geração automática de `Uuid` quando o campo vem nulo | **não** | medido: `coluna id e obrigatoria e recebeu NULL`; o cliente tem de mandar `"novo"` |

---

# PARTE B — COMO SERIA O IDEAL

## B.1 A pergunta da casa, antes de qualquer proposta

> **Onde esta lógica DIVERGE da de origem, e qual restrição nossa causou a
> divergência?**

Se a resposta é «em lugar nenhum», a proposta passou pelos dedos e não pela
cabeça. Abaixo, cada item traz a divergência **e** a restrição que a causou. As
restrições em jogo são cinco, e nenhuma delas é negociável:

1. **A ordem de digitação é sagrada.** O `.reg` nunca reaproveita slot excluído.
2. **Zero dependências externas.** Só a `std`.
3. **O formato em disco muda cedo ou não muda.** Enquanto não há dado em
   produção é barato; depois vira migração.
4. **Replicação por *pull*, e cluster com eleição.** Mas o modo `multi` é
   bidirecional de verdade: **dois masters ao mesmo tempo existem hoje.**
5. **Guarda nova entra pedida, não imposta.** Proteção que quebra todo cliente
   antigo não é proteção, é estrago.

**A restrição 4 é uma correção de rota deste documento**, e vale registrá-la
porque eu concluí o contrário primeiro. A premissa que me deram — «o cluster faz
eleição, dois masters nunca ao mesmo tempo, então paridade não precisa» — está
certa para o cluster **e errada para o produto**: o papel `multi` do
`config.json` põe dois servidores recebendo escrita ao mesmo tempo, e o bloco 24
mediu o estrago. Não é hipótese: **4 inserções, 2 linhas sobreviveram.**

## B.2 O que se propõe, item a item

### B.2.1 A `Sequence` numerada no `INSERT`, e não no `COMMIT` — **primeiro item**

**O problema medido:** dentro de uma transação a linha se lê com `id: null`, e a
ficha mestre-detalhe não fecha.

**A proposta:** o `empilhar` (servidor.rs:8409) já reserva o **fim da tabela**
com uma trava exclusiva antes de calcular o rowid previsto — é por isso que
outra conexão levou `SP000006 tabela em transacao` na medição. Com essa trava na
mão, a transação também pode **reservar o número da sequência**: consome
`proxima_da_sequencia()` na instrução, guarda o número na `Escrita`, e o
`aplicar_uma` grava o valor reservado em vez de deixar o `numerar` sortear.

**A divergência, e a restrição:** o PostgreSQL numera no `INSERT` e **não
devolve** o número no `ROLLBACK` — a sequência dele é não-transacional de
propósito, para não serializar quem insere. Aqui a devolução seria de graça,
porque **a trava do fim da tabela já serializa a inserção** (restrição 1: sem
reaproveitar slot, o rowid tem de ser previsto, e prever exige travar o fim).
Ou seja: **numeramos como o PostgreSQL e desfazemos como ele não desfaz**, e a
razão é que a nossa concorrência já foi paga antes, pela ordem de digitação.

**O que muda para quem já usa:** nada. A linha empilhada passa a mostrar o
número em vez de `null` — quem lia `null` lia um valor que não servia para nada.
Não há mudança de formato.

**O que ainda precisa ser medido antes de implementar:** o custo da reserva
quando a transação é revertida. Hoje o `ROLLBACK` é «zero bytes de trabalho»;
com a reserva ele passa a ter de devolver o contador, e devolver **em ordem** —
duas transações que reservaram 7 e 8 e reverteram fora de ordem não podem deixar
o contador em 7. A saída barata é devolver **só se ninguém consumiu depois**, e
deixar o buraco quando alguém consumiu; a cara é uma lista de números livres,
que a ordem de digitação nos ensinou a não querer. *Medir a premissa do item vem
antes de implementar o item.*

### B.2.2 `inicio` e `passo` declarados no esquema — **e é o conserto do bidirecional**

**O problema medido:** dois servidores `multi` numerando a mesma faixa perdem
linha. Faixas disjuntas por `ajustar_sequencia` **funcionam uma rodada e morrem
na segunda**, porque `anotar_sequencia` empurra o contador local para depois do
maior valor **que chegou de fora** — e o de fora é justamente o da outra faixa.
Medido: alfa ajustada para 1, beta para 1.000.000; depois da primeira ida e
volta o contador de alfa estava em **1.000.002**, que é o próximo de beta.

**A proposta:** dois campos no esquema da coluna, `inicio` e `passo`, gravados
no `PSCH`. O contador anda de `passo` em `passo` a partir de `inicio`, e
`anotar_sequencia` passa a **arredondar para cima até o próximo valor da própria
faixa** em vez de tomar o valor alheio: com `inicio=1, passo=2` o servidor A dá
1, 3, 5; com `inicio=2, passo=2` o B dá 2, 4, 6; e o valor 1.000.000 vindo do
outro lado empurra A para 1.000.001, que continua sendo dele.

**A divergência, e a restrição:** o MariaDB põe isso em **variáveis de sessão do
servidor** (`auto_increment_increment` / `auto_increment_offset`), o que é
coerente lá — a configuração do servidor vale para todas as tabelas. Aqui vai no
**esquema da tabela**, e a restrição que causa a divergência é o formato: o
contador é do `.reg`, e um `.reg` restaurado noutro servidor precisa continuar
sabendo de que faixa ele é. Variável de servidor perderia a faixa numa
restauração de backup — e o bloco 20 mostra que a restauração devolve o contador
tal como estava. **A faixa tem de viajar com o arquivo.**

**Mudança de formato: entra CEDO ou não entra.** São dois `u64` por coluna
`Sequence` no `PSCH`, e o esquema em disco já carrega byte por chave desde a v7.
Enquanto não houver `Sequence` com faixa em produção, é grátis.

### B.2.3 `IDENTITY … GENERATED ALWAYS` — **guarda pedida, nunca imposta**

**A proposta:** um campo `sempre_do_motor` (`GENERATED ALWAYS`) na coluna, que
faz o `numerar` **recusar** valor mandado pelo cliente em vez de aceitá-lo e
seguir o contador.

**A divergência, e a restrição:** o padrão SQL tem os dois modos e o `BY DEFAULT`
é o padrão; aqui o padrão continua sendo o de hoje — **aceitar e acompanhar** —
e o `ALWAYS` é opt-in. A restrição é a lei da casa: *guarda nova entra pedida,
não imposta.* Ligar `ALWAYS` por padrão quebraria toda importação, toda
restauração linha a linha e toda réplica bidirecional, que é onde o valor chega
de fora **de propósito**. E o teste que mais importa aqui é o do comportamento
**velho**: `sem_identity_declarada_nada_muda`.

**Onde ele é obrigatório por dentro:** no caminho da replicação. `inserir` é a
mesma função para escrita local e para evento aplicado (`inserir_replicado`,
`aplicar_evento_interno`), e um `ALWAYS` que recusasse o valor do evento pararia
a replicação. A recusa tem de olhar `como_replica`, exatamente como
`julga_integridade` já faz.

### B.2.4 Sequência nomeada, fora da tabela — **vale a pena, e o número diz o preço**

O sprint 11 do `docs/SPRINTS-MARIADB.md` já a descreve e já faz **a pergunta
certa**: *quanto custa um `NEXTVAL` durável?* Este documento responde, medido.

- O contador de hoje é **de graça**: 0,50 µs de `write`, dentro do cabeçalho que
  a inserção grava de qualquer jeito.
- Uma sequência independente tem arquivo próprio e precisa do próprio
  sincronizar: **83,5 µs por número** de `fdatasync` (72,6–125,0). **167× mais
  caro** — e 164× pelo `fsync` cheio, 82,1 µs. Os dois números estão na mesma
  faixa porque o que domina é a ida ao disco, e não o metadado.
- Em disco, o objeto inteiro cabe num arquivo de 128 bytes por sequência —
  assinatura, versão, `atual`, `inicio`, `passo`, `minimo`, `maximo`, sinais
  (ciclo) e CRC-32. É a mesma anatomia do cabeçalho do `.reg`, e não precisa de
  nenhuma peça nova.
- Em operações, três: `criar_sequencia`, `proximo_da_sequencia`,
  `ajustar_sequencia` (que já existe e ganha o alvo nomeado).

**A divergência, e a restrição:** o MariaDB escapa do `fsync` por número com um
`CACHE` de 1.000 valores, e a documentação dele diz o preço com todas as letras
— *«FLUSH TABLES, desligar o servidor etc. descartam os valores em cache»*,
deixando buracos. Aqui o `CACHE` **entra desligado**, e a razão é o caso de uso
do dono: numeração de documento fiscal, onde buraco não é aceitável. Quem quiser
laço rápido liga o cache **sabendo** que uma queda come até `N` números. É a
mesma forma da janela de durabilidade, que já existe e já é escolha do
administrador.

**O que se recusa junto:** `NEXT VALUE FOR` dentro de expressão SQL — depende do
interpretador que ainda não existe — e o `seq.nextval` do Oracle.

### B.2.5 O `Uuid` que nasce sozinho — **um item pequeno que fecha a saída de emergência**

Medido: `Sequence` nula ganha número; `Uuid` nulo dá erro. A assimetria não tem
motivo de formato — é só que ninguém a escreveu. Fechá-la custa uma linha na
mesma função (`numerar`, table.rs:2307) e faz do `Uuid` v7 uma alternativa
**usável** por quem tem dois masters: medido no bloco 24, estágio 4, **4
inserções → 4 linhas dos dois lados**, contra 2 de 4 com a `Sequence`.

**A divergência, e a restrição:** o Cassandra **não tem** auto-increment, e isso
é decisão, não falta: sem coordenação global não há como um nó saber qual é o
próximo número sem perguntar aos outros. A restrição que nos separa dele é a
regra primordial da integridade — aqui a chave estrangeira **é conferida na
gravação**, então precisamos de identidade estável, e o `Uuid` v7 dá isso
mantendo a ordenação temporal. **Convergimos com o Cassandra no meio de
transporte e divergimos no fim: eles não conferem, nós conferimos.**

### B.2.6 O teto do protocolo: dizer a verdade, ou consertá-la

O número acima de 2⁵³ se corrompe calado. Duas saídas, e a barata é a segunda:

1. **`Numero` inteiro no `Json`.** Uma variante `Inteiro(i64)` ao lado do
   `Numero(f64)`, com o analisador escolhendo pela presença de ponto ou expoente.
   É a correção de raiz, alcança todo o protocolo, e mexe num tipo que o
   repositório inteiro atravessa: **787 chamadas** a `inteiro_ou`, `de_u64`,
   `de_i64` e `inteiro()`, das quais **444 só no `servidor.rs`** (contado em
   07/09/2026). Não é item desta frente.
2. **Recusar cedo.** `ajustar_sequencia` e o `numerar` recusam valor acima de
   2⁵³ com a mensagem que explica: *«acima de 9.007.199.254.740.992 o protocolo
   perde precisão; use `Uuid256` se precisar dessa faixa»*. Custa duas
   comparações e transforma corrupção silenciosa em erro lido.

A escolha entre as duas é do papel C (DBA) e do B (engenharia), e a segunda
**não impede** a primeira depois.

### B.2.7 O que o `verificar` deveria saber recontar

O `verificar` **reconta** `marcadas` varrendo — `recontar_marcadas`,
`table.rs:4394`, «um contador de cache só serve enquanto alguém se dispõe a
conferi-lo» — e **não** reconta `proxima_sequencia`. Medido no bloco 17: com o contador reposto à mão o
CRC-32 do cabeçalho pegou e travou tudo — o que é a proteção certa —, mas
significa que **não existe caminho de reparo** para um contador atrás do maior
valor gravado. Se um dia ele ficar atrás (por ajuste manual, por restauração de
backup antigo, por promoção de réplica atrasada), o conserto é achar o maior
valor à mão.

**A proposta:** `verificar` passa a devolver `maior_sequencia_gravada` — que ele
já varre a tabela inteira de qualquer forma — e a comparar com o contador,
avisando quando o contador está **atrás**. `reparar` empurra o contador para
depois do maior. É `anotar_sequencia` chamado uma vez, no fim de uma varredura
que já acontece.

## B.3 O que se RECUSA, com o número

- **Catálogo reverso de sequências, ou uma sequência global do banco.** Custaria
  manutenção em toda criação e alteração de tabela — inclusive nas que não têm
  `Sequence` — para baratear uma operação que acontece uma vez por tabela. É o
  mesmo desenho que a casa já recusou para a busca reversa de chave
  estrangeira, e pelo mesmo motivo: *excluir é raro, inserir é o laço quente.*
- **Reaproveitar número de `Sequence` de linha excluída.** Nunca. Não é só a
  ordem de digitação: a `Sequence` viaja para fora do banco — em nota, em
  etiqueta, em recibo — e um número reemitido é dois documentos com a mesma
  identidade. O bloco 13 mostra que hoje ela não volta em nenhum dos três modos
  de exclusão, e isso está certo.
- **`CACHE` ligado por padrão.** Medido: o contador de hoje custa 0,50 µs e não
  precisa de cache nenhum. Cache só faz sentido na sequência **nomeada**, que
  paga 83,5 µs por número, e mesmo lá entra desligado.
- **Paridade automática (o servidor descobre sozinho a própria faixa).** O
  MariaDB obriga o administrador a escolher `offset` e `increment`, e está
  certo: um servidor que escolhe a própria faixa por hash do `id_servidor`
  escolhe de novo, e diferente, no dia em que o `id_servidor` mudar. A faixa é
  declarada, e o motor **recusa** subir em `multi` com duas origens de mesma
  faixa — que é a conferência que ele já faz para o hash do `id_servidor`
  colidido.
- **Sequência transacional «à PostgreSQL» (não devolver no rollback) como
  padrão.** Devolver é o que o nosso formato permite de graça, e o buraco é
  visível para o usuário final na numeração de documento. Quem quiser o
  comportamento do PostgreSQL o terá pela sequência **nomeada** com `CACHE`
  ligado, onde o buraco é escolha declarada.

## B.4 O que entra primeiro, e o que é migração

| ordem | item | custo | muda formato? | quebra cliente? |
|---|---|---|---|---|
| 1 | **pedido 223** — `WHERE id = 2` com `Sequence` | 1 linha (`valores.rs:707`) + par de testes | não | não |
| 2 | **`Uuid` nasce sozinho** quando o valor chega nulo | 1 ramo no `numerar` | não | não |
| 3 | **recusar valor acima de 2⁵³** com a mensagem que explica | 2 comparações | não | não — hoje ele já se corrompe |
| 4 | **`verificar` reconta a `Sequence`** e `reparar` empurra o contador | dentro de uma varredura que já existe | não | não |
| 5 | **numerar no `INSERT`, dentro da transação** | reserva na `Escrita` + devolução | não | não (hoje se lê `null`) |
| 6 | **`inicio` e `passo` no esquema** — o conserto do bidirecional | **`PSCH` ganha 2 × `u64` por coluna `Sequence`** | **SIM — e por isso entra CEDO** | não (ausente = 1 e 1) |
| 7 | **`IDENTITY ALWAYS`** | 1 sinal no esquema + guarda no `numerar` | 1 byte no `PSCH` | não (nasce desligado) |
| 8 | **sequência nomeada** (`CREATE SEQUENCE`) | arquivo novo, 3 operações, `CACHE` desligado | arquivo NOVO, não altera os existentes | não |

**Os itens 6 e 7 são a linha de corte da migração.** Enquanto não houver
`Sequence` com faixa declarada em produção, acrescentar os campos ao `PSCH` é
uma versão nova do esquema que lê o antigo com os padrões — exatamente como o
`PSCH` v7 fez com o byte por chave. Depois, é migração de arquivo.

Os itens 1 a 5 **não tocam no formato** e podem entrar em qualquer ordem. O item
1 é o mais barato e o mais visível: hoje um `WHERE id = 2` de um driver ODBC
falha na tabela mais comum que existe.

---

## Como se refaz

```bash
# sem compilar, com o binário que já existe:
PHX_SONDA_PORTA=7830 PHX_SONDA_BIN=target/release/phxsqld \
  python3 bancada/sequencias/sonda.py
# grava bancada/sequencias/resultados.json com a data da corrida
```

## Onde mais isto está escrito

- `docs/FORMATO.md` — a tabela do cabeçalho (bytes 36 e 92) e a §«A coluna de
  sistema `rownum`».
- `docs/pdf/respostas/00-sequencia.md` — a resposta curta, e
  `docs/pdf/respostas/00-autonumber.md`, a deste documento.
- `docs/TRANSACOES.md` §3.2 — por que nada vai a disco antes do `COMMIT`.
- `docs/REPLICACAO.md` §12 — o bidirecional, o laço e o conflito.
- `docs/SPRINTS-MARIADB.md`, sprint 11 — a sequência como objeto próprio, com a
  premissa que este documento mediu.
- `docs/CASSANDRA.md` — por que eles não têm auto-increment, e onde isso nos
  serve.
- `docs/dossie/figuras/autonumber-como-esta.svg` e `autonumber-ideal.svg` — as
  duas figuras, uma afirmação cada.
