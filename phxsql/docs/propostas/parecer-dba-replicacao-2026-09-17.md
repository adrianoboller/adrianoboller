# Parecer do papel C (DBA) — as garantias de dado da replicação

**Data:** 17/09/2026, 02:39 UTC · **Ordem do dono:** *«Replicação: bateria de
testes, revisão e conclusão»* (17/09/2026 02:27 UTC) · **Escopo:** o formato em
disco e o que ele promete a uma réplica, ao cluster e à recuperação.
**Só leitura** — nenhuma linha de código foi escrita nesta frente.

Este parecer não conserta nada. Ele nomeia o que vale, o que **não** vale, o
cenário exato que quebra cada garantia que não vale, e o **não** do papel C.

## 0. Duas correções ao briefing, antes de qualquer conclusão

Confira, não confie — e duas coisas do enunciado não se sustentaram na leitura:

1. **O evento do `.log` tem 44 bytes de cabeçalho, não 36.** `EVENTO_CAB = 44`
   em `crates/phxsql-store/src/log.rs:81`. Os 36 são a fronteira do CRC
   (`crc32(&dst[..36])`, `log.rs:209`), não o tamanho do evento. Os 36 bytes de
   largura fixa foram a **versão 1**; o cabeçalho de hoje é 44 + corpo
   (`log.rs:38-43`).
2. **`reconciliar_sequencia` não mora no bidirecional**, e sim no store
   (`crates/phxsql-store/src/table.rs:3009`), chamada pelo `reparar`
   (`table.rs:668`). Ela conserta o contador contra o dado **local**; não fala
   com réplica nenhuma.

## 1. A tabela: garantia × vale? × quem prova × arquivo:linha

O que uma réplica entrega ao fim de um alcance, item por item.

| garantia ao fim do alcance | vale? | quem prova | arquivo:linha |
|---|---|---|---|
| **mesmo rowid** | **SIM**, e é imposta | `aplicar_evento` para no primeiro que não bate | `table.rs:4025-4030` |
| **mesma linha (colunas do usuário)** | **SIM** | bancada: SHA-256 por linha (`iguais_no_fim`) | `bancada/replicacao/medir.py:72-91,245` |
| **mesmo conteúdo de `.memo`** | **SIM** (coluna **não** marcada) | a imagem leva conteúdo, não ponteiro | `table.rs:4040-4083`; `log.rs:31-34` |
| **mesmo conteúdo de `.bin`** | **não provado** | ninguém — a tabela da bancada não tem coluna `Bin` | `bancada/replicacao/medir.py` (esquema sem `Bin`) |
| **mesmo `rownum`** | **NÃO** — ver §2.1 | ninguém; o portrait o veria, a carga não o exercita | medido: `table.rs:3074` + `reg.rs:764-768` |
| **mesma `versao`** | por construção, **nunca conferida** | ninguém: o campo viaja e é descartado | envia `servidor.rs:21854`; o leitor não tem o campo `replica.rs:330-338` |
| **mesmo `.trash` / `.reason`** | **NÃO**, por desenho | ninguém | `table.rs:4005` (`"replicacao"`), `lixeira.rs:417-419` (uuid e carimbo locais) |
| **mesmo carimbo no `.log`** | **NÃO** (unidirecional) | ninguém | o PITR força (`servidor.rs:18538`), a réplica **não** (`servidor.rs:2746`) |
| **mesma `origem` no `.log`** | **NÃO** (unidirecional) | ninguém | idem: o evento local nasce `origem = 0` |
| **integridade referencial na réplica** | **NÃO**, e é decisão registrada | `docs/INTEGRIDADE.md` §3, com os três números | `table.rs:1315-1317` |
| **unicidade na réplica** | **SIM** — e é um problema, ver §2.5 | medido nesta frente | `table.rs:3107-3115` |
| **atomicidade de um commit** | **NÃO** | ninguém | o `.log` não tem id de transação: `log.rs:134-158` |
| **a posição diz o atraso** | **SIM** por tabela; **NÃO** somada | ninguém | `servidor.rs:2710`; soma em `servidor.rs:3343-3376` |

## 2. As garantias que NÃO valem, e o cenário que quebra cada uma

### 2.1 «Mesmo `rownum`» — **falso**, e silencioso. *(o achado desta frente)*

**O cenário:** uma inserção **recusada** no source. Nada mais.

`numerar_linha` consome o contador (`table.rs:3074` → `reg.rs:764-768`) **antes**
da sequência (`table.rs:3080`), antes do `CHECK` (`table.rs:3092`) e antes da
conferência de unicidade que **retorna erro** (`table.rs:3107-3115`). O evento
do diário só é gravado no fim (`table.rs:3155`). Então a inserção recusada
**queima um `rownum` e não gera evento**: o contador do source anda, o da
réplica não.

E o `rownum` que chega na imagem é **descartado**: no `inserir`, `anterior` é
`None`, e a linha 3074/2374-2376 sobrescreve o valor recebido pelo contador
**local**.

**Medido** (17/09/2026, `/tmp/.../scratchpad/prova-rownum`, sem tocar no
repositório) — 3 linhas, **uma** inserção recusada por chave duplicada, 2 linhas:

```
  rowid |  id | rownum source | rownum replica
      1 |   1 |             1 |              1
      2 |   2 |             2 |              2
      3 |   3 |             3 |              3
      4 |   4 |             5 |              4  <<< DIVERGIU
      5 |   5 |             6 |              5  <<< DIVERGIU
rowids iguais: true
linhas com rownum DIFERENTE: 2 de 5
```

**Por que é grave, e não cosmético.** O `rownum` é a **ordem global de chegada**
e o cursor de paginação (`docs/FORMATO.md` §«A coluna de sistema `rownum`»,
linhas 474-512). Na **partição alfanumérica** ele é o **único** monotônico que
sobra, porque ali o rowid diz em que arquivo a linha mora e não quando ela
chegou (`FORMATO.md:1467-1476`). Uma réplica com `rownum` deslocado responde
`desde_rownum` com a linha errada, e o `varrer` por cursor paginado do cliente
pula ou repete registro — calado.

**E o `aplicar_evento` não vê.** Ele confere o **rowid**, e o rowid **não**
diverge: o slot só é consumido em `table.rs:3122-3127`, depois da recusa. O
fail-stop que existe justamente para não espalhar divergência passa ao lado
desta.

**O que a bancada prova e o que não prova:** o portrait SHA-256 hasheia
`rowid` + **todas** as colunas do esquema (`servidor.rs:16763-16767` →
`valores.rs:1062-1071`), e `Schema::new` sempre acrescenta `softdeleted` e
`rownum` (`schema.rs:557-586`). Então o `rownum` **está** no retrato: a bancada
**pegaria** esta divergência. Ela não pega porque **a carga nunca falha uma
inserção** — semeia ids únicos. É uma linha de carga, não uma linha de motor.

### 2.2 «Mesmo `.trash`/`.reason`» — **falso por desenho**, e sem ninguém dizendo

Três coisas diferem na lixeira da réplica:

- **o motivo**: `aplicar_evento` chama `excluir_de_vez(rowid, "replicacao")`
  (`table.rs:4005`) — o motivo que o operador escreveu no source **não viaja**,
  porque o evento de exclusão não leva imagem (`log.rs:36`) e o motivo não é
  campo do evento (`log.rs:134-158`). No bidirecional é
  `"replicacao bidirecional"` (`servidor.rs:4243`);
- **o uuid** e **o carimbo**: `Lixeira::guardar` sorteia `Uuid::v7()` e usa
  `agora_ms()` locais (`lixeira.rs:417-419`);
- **o usuário**: é o da sessão da réplica, não o do source.

**O cenário que isso quebra:** auditoria. Quem pergunta «por que esta linha foi
apagada?» na réplica lê **«replicacao»**, e não «cliente pediu cancelamento».
A trilha do *porquê* existe só no source. É defensável como decisão — a lixeira
é artefato local —, mas hoje **não está escrito em lugar nenhum** como limite, e
a `§3` do `INTEGRIDADE.md` promete «fidelidade conferida por SHA-256 de cada
linha» sem ressalvar que a lixeira fica fora do retrato (o `varrer` lê o `.reg`,
não o `.trash`).

### 2.3 «O `.log` da réplica é a mesma história» — **falso**, e o irmão já faz certo

O evento carrega o instante do **nascimento** da escrita e a **origem**
(`log.rs:136-150`, `registrar_detalhado` em `log.rs:448-480`). O fio transmite
os dois (`servidor.rs:21855-21857`) e o leitor os guarda
(`replica.rs:334-338`). E o aplicador unidirecional **não usa nenhum dos dois**:
`aplicar_lote_da_replica` chama `aplicar_evento` seco (`servidor.rs:2746`), e o
evento local nasce com `agora_ms()` e `origem = 0`.

O **PITR faz o contrário**, e está certo: `td.forcar_proximo_evento(e.carimbo,
e.origem)` antes de aplicar (`servidor.rs:18538`), com o motivo escrito — *«o
diário é trilha de auditoria, e um restaurado que jurasse que tudo aconteceu
agora destruiria justamente o que se foi buscar nele»*. **A frase vale igual
para a réplica, e a réplica não a cumpre.**

**O cenário que quebra:** topologia mista. `A → B` unidirecional, `B ↔ C`
bidirecional. O carimbo nascido em A é **lavado** no salto para B, e o conflito
em C passa a ser decidido pela hora em que **B sincronizou** — que é exatamente
a injustiça que `docs/REPLICACAO.md` §12 e `bidirecional.rs:21-23` dizem que o
desenho existe para evitar. Também é o que faz um backup tirado de uma réplica
ter um `.log` com horas diferentes do backup tirado do source.

### 2.4 Atomicidade de commit — **não atravessa o fio**

O `.log` **não tem id de transação**: o evento é carimbo, operação, rowid,
versão, usuário, origem, tamanho (`log.rs:134-158`). A marca `.tx` v3 — o
super-journal do ACID-C, com a cascata achatada na lista
(`transacao.rs:48-59`) — é **local e efêmera**: ela viaja dentro do backup e
quem a completa é a recuperação do arranque, e o `.tx` é removido ao fim
(`transacao.rs:1206`). Ela **não** é replicada e **não** tem correspondente no
diário.

Então, respondendo à pergunta 2 sem rodeio:

- **um commit com cascata chega à réplica como N eventos soltos**, um por
  escrita, e as escritas de tabelas diferentes vão para **diários diferentes**,
  com posições **independentes**;
- **uma réplica que cai no meio da aplicação de um commit fica com meio
  commit** — e pior, **não há como saber**. O lote é de 500
  (`replica.rs:52`), um commit maior que isso é cortado em dois pedidos; e a
  posição avança por lote (`servidor.rs:2823`). O estado intermediário é
  legível por qualquer cliente da réplica e é indistinguível de um estado
  completo.

Isto **não é defeito novo**: é o preço declarado de «a replicação anda por
tabela» (`INTEGRIDADE.md` §3). Mas o **A** e o **I** do ACID valem só dentro do
servidor que commitou. Uma réplica é `READ COMMITTED` sobre um fluxo **sem
fronteira de commit** — isto é mais fraco que `READ UNCOMMITTED` local, porque
nem existe a noção de «não confirmado» do outro lado. É o que não se pode
escrever em folha de produto, do mesmo modo que *ACID compliant* não se escreve.

### 2.5 Unicidade na réplica — a guarda que **para o par de servidores**

`julga_integridade` (`table.rs:1315-1317`) cala a FK (`3065`, `3356`, `3683`), a
cascata (`1844`, `3450`), o `conferir_filhas` do excluir (`3576`) e as regras de
esquema — DEFAULT/calculada/CHECK (`2853`). **Não cala a unicidade**, que é
conferida antes de qualquer gravação em `table.rs:3107-3115`.

**Medido** nesta frente, tabela com primária `porId` **e** um único secundário
`porEmail`; o nó B já tem `id=2, email=x`; chega o evento de A com
`id=1, email=x`:

```
inserir_replicado RECUSOU: [SP000020] chave duplicada: indice unico porEmail ja tem essa chave
aplicar_evento    RECUSOU: [SP000020] chave duplicada: indice unico porEmail ja tem essa chave
```

**O cenário que quebra:** no bidirecional, o `Err` sobe pelo `?` de
`aplicar_por_chave` → `aplicar_lote_bidi` (`servidor.rs:4017`) →
`alcancar_tabela_bidi`, e `desde = lote.ate` (`servidor.rs:4090`) **nunca
executa**. A posição não anda e **o mesmo lote volta para sempre**. É a doença
que o `INTEGRIDADE.md` §3.1 mediu, nomeou — *«não é uma linha perdida: é o par
de servidores parado»* — e curou **para a chave estrangeira**
(`inserir_replicado`, `table.rs:3957`). Para o **único secundário** ela continua
inteira. E não é um modelo exótico: chave primária + «e-mail único» + «CPF
único» é a modelagem mais comum que existe.

E aqui está o **tradeoff, que é real e não tem lado óbvio**: calar a unicidade
também seria errado. O único secundário é a **identidade que o bidirecional não
usa para casar** — calá-lo gravaria duas linhas com o mesmo e-mail, e o índice
único no disco passaria a mentir sobre si mesmo. As saídas honestas são três, e
todas custam: (a) recusar a tabela com único secundário no modo multi, como já
se recusa a tabela sem chave única (`bidirecional.rs:134-147`); (b) resolver o
conflito **por chave secundária também**, que é casar por N chaves e escolher um
vencedor por chave — desenho novo; (c) desviar a linha para uma área de
quarentena e seguir, que é inventar um arquivo. **Nenhuma entra sem o dono**:
(a) tira função de quem hoje replica, (b) muda a semântica do conflito e (c)
muda o formato.

### 2.6 A posição somada do cluster — onde ela mente

A posição de **uma tabela** é honesta: é a contagem de eventos do `.log` da
própria réplica (`servidor.rs:2710`), e é isso que faz a retomada não precisar
de estado extra (`REPLICACAO.md` §13 «A posição é o diário da própria réplica»).
A **soma** que o cluster publica (`posicao_do_diario`,
`servidor.rs:3343-3376`) mente em quatro cenários:

1. **Tabela apagada e recriada no source.** `excluir_tabela` leva **todos** os
   arquivos, `.log` incluído (`catalogo.rs:762-772`). O diário do source volta a
   zero; a réplica continua com N. Em `alcancar_tabela`,
   `if posicao >= no.eventos { return Ok(0) }` (`servidor.rs:2808-2810`): a
   réplica **não faz nada, para sempre, e não reclama** — fica com a tabela
   velha, com o esquema velho. **E o PITR pega este caso e a réplica não**:
   `diario_vivo_continua` compara o evento da posição e recusa com o texto *«a
   tabela foi apagada e recriada depois do backup»* (`servidor.rs:18640-18673`).
   É o padrão da casa em estado puro — **o conserto entrou no caminho que o
   motivou (o PITR, pedido 232) e o caminho IRMÃO ficou**. Na eleição é pior: o
   nó **atrasado** publica soma **maior** e `vencedor` promove ele
   (`cluster.rs:134-147`), sem `incompleta`, porque as duas tabelas abriram bem.
2. **A soma é um escalar de uma grandeza vetorial.** 1.000 eventos na tabela A
   e 0 na B soma igual a 0 em A e 1.000 em B. Um nó em dia na tabela grande e
   cego na pequena bate, na eleição, o nó que está no inverso.
3. **A soma conta tabela que não é replicada.** O comentário diz «somada sobre
   as tabelas **replicadas**» (`cluster.rs:165-166`); o código soma
   `db.todas_as_tabelas()` (`servidor.rs:3361`) dos databases configurados.
   Uma tabela local — criada só num nó, ou de trabalho — **infla a posição** e
   faz esse nó ganhar a eleição estando atrás no dado que importa.
   Comentário e código discordam; um dos dois está errado, e é decisão de quem
   desenhou o critério.
4. **Escrita local na réplica.** A posição **é** a contagem local, então uma
   escrita local incrementa a posição e a réplica passa a pedir `desde = N+1`:
   o evento N do source **nunca é aplicado**. Se o evento pulado for uma
   `alteracao`, o `aplicar_evento` **não** acusa (ele confere o rowid do que
   recebeu, não a existência do que não recebeu) — é **perda silenciosa de
   atualização**. A defesa hoje é um `eprintln!` de aviso
   (`servidor.rs:2315-2322`), não uma recusa.

O `incompleta` do pedido 211 (`cluster.rs:73-78,111-118`) está **certo** e cobre
o que promete — tabela que não abre ou não conta publica soma menor, e a eleição
prefere completa antes de olhar número. Ele **não** cobre nenhum dos quatro
acima, porque nos quatro a contagem **funciona**; é o significado dela que
mudou.

### 2.7 «Mais recente vence» — o que acontece com FK, UNIQUE e `Sequence`

- **FK.** Nada acontece: a réplica não julga (`table.rs:1315-1317`). Mãe aceita
  num lado e filha no outro convergem quando os dois eventos chegarem, em
  qualquer ordem — é a decisão do `INTEGRIDADE.md` §3, com os números. **Mas** o
  invariante pétreo «só existe filho se o pai existir primeiro» **não é
  verdadeiro na réplica em nenhum instante intermediário**, e o intervalo não
  tem teto: se a tabela da mãe estiver fora da lista de databases/tabelas da
  origem, ou for recusada no multi por não ter chave única
  (`bidirecional.rs:134-147`), a órfã é **permanente** e ninguém a conta. Não há
  hoje nenhum contador de «linhas órfãs na réplica».
- **UNIQUE.** §2.5: para o par de servidores.
- **`Sequence` na mesma faixa.** Dois masters na mesma faixa numeram a mesma
  chave, o casamento por chave com «mais recente vence» **apaga uma das
  linhas**. **O número do pedido 229, defeito (a): 4 inserções viraram 2
  linhas** (bloco 24 da sonda, `docs/PENDENCIAS.md:253`;
  `bidirecional.rs:101-125`). O que existe hoje é **detecção**, não conserto:
  `colisao_de_criacao` conta e grita no log (`servidor.rs:4196-4208`), e o
  conserto pleno — `inicio`/`passo` no `PSCH` — **muda formato e está parado**
  (`docs/AUTONUMBER.md` §B.2.2 e C.4).
- **O empate de carimbo é a regra, não a exceção.** **Medido** nesta frente:
  **12 eventos em 1 único milissegundo** — `carimbos DISTINTOS: 1 para 12
  eventos`. O carimbo é `i64` em ms (`log.rs:19-20,136-137`), e uma passada de
  commit grava dez linhas em muito menos que um milissegundo. Então «mais
  recente vence» cai no desempate por **origem numérica maior**
  (`bidirecional.rs:97-99`) com muito mais frequência do que o desenho sugere:
  dentro de um lote de escritas simultâneas nos dois nós, quem ganha é
  **arbitrário** (determinístico e igual dos dois lados, que é o que importa
  para convergir — mas arbitrário quanto a *qual trabalho sobrevive*).

### 2.8 Réplica sem cofre, `.reg` v5 — a única que grava **mentira sobre o dado**

Já medido e escrito no pedido 194, premissa 3 (`docs/SEGURANCA.md` §12.4). O
parecer do papel C é que **este é o pior item desta lista**, e digo por quê.

A faixa **inline** viaja em claro dentro da imagem: a réplica a aplica sem chave
nenhuma, e a versão do `.reg` (v4 ou v5) é **escolha local de cada servidor** —
não viaja e não precisa viajar. Um source v5 replica para uma réplica v4 sem
nenhum problema, nesta faixa.

A faixa **externa** (`Memo`/`Bin` de coluna marcada) viaja **cifrada**, e o sal é
sorteado **por arquivo, sempre** — então a condição «compartilhar a senha **e** o
sal» **nunca se satisfaz**. Os três casos medidos (`SEGURANCA.md:1975-1999`):

| origem | réplica | o que aconteceu |
|---|---|---|
| cifra ligada | a **mesma** senha | recusou («a etiqueta nao confere…») |
| cifra ligada | senha diferente | recusou, mesmo texto |
| cifra ligada | **cifra desligada** | **gravou 63 bytes de texto cifrado como se fossem o conteúdo, sem erro nenhum** |

O terceiro caso é o único sem erro, e numa coluna `Bin` **não há peneira** — o
`String::from_utf8` que salva o `Memo` por acidente (`table.rs:4069-4072`) não
existe para o `Bin` (`table.rs:4068`). A réplica passa a servir, como dado, um
texto cifrado. **É a lei do «Blumenau» que aparecia «BLUMENAU» escrita no
disco**: quem olha não tem como saber que não está gravado assim. E a bancada
**não pode** pegar: a tabela dela não tem coluna `Bin`, nem coluna marcada.

## 3. O que a bateria (papel F) deveria exercitar, e hoje não exercita

Não é conserto — é o que falta à carga para que o `iguais_no_fim` prove o que o
nome dele promete. Todos estes o portrait **pegaria**; nenhum está na carga:

| # | o que acrescentar à carga | qual garantia passa a ser provada |
|---|---|---|
| 1 | **uma inserção recusada** (chave duplicada) no meio da semeadura | o `rownum` (§2.1) — hoje o retrato passa por sorte |
| 2 | uma coluna **`Bin`** no esquema da tabela | o conteúdo do `.bin` (§1) |
| 3 | uma coluna **marcada** com a cifra ligada no source e **desligada** na réplica | §2.8 — o único caso sem erro |
| 4 | um **único secundário** e escrita conflitante nos dois nós | §2.5 — o par parado |
| 5 | `excluir_tabela` + `criar_tabela` no source no meio da corrida | §2.6.1 — a réplica muda para sempre |
| 6 | uma **escrita local** na réplica (sem `somente_leitura`) | §2.6.4 — o evento pulado |
| 7 | comparar o **`.log`** dos dois (carimbo e origem), não só o `.reg` | §2.3 |
| 8 | comparar a **lixeira** dos dois | §2.2 |

Os itens 1, 5 e 6 são de **uma linha** na carga cada. O 1 é o que eu pediria
primeiro: custa uma inserção repetida e derruba uma garantia que hoje se
anuncia.

## 4. As mudanças de FORMATO pendentes, e o custo de adiar

A lei é «mudança de formato entra **cedo**: enquanto não há dado em produção é
barata; depois vira migração». Estas são as três que a ordem pediu para nomear,
com o que cada dia de adiamento custa.

### 4.1 `inicio`/`passo` por nó para a `Sequence` — **`PSCH` v10**

- **O que é:** dois `u64` por coluna `Sequence` no bloco de esquema
  (`AUTONUMBER.md` §B.2.2 e a tabela de §B.4, item 6). Ausente = `1` e `1`,
  então o esquema em disco volta com o que foi gravado nele — **exatamente** o
  que o `PSCH` v7 fez com o byte `verificar` por chave (`FORMATO.md:295-308`).
- **Custo de adiar:** cada dia com bidirecional na mesma faixa é um dia em que
  **4 inserções podem virar 2 linhas** (pedido 229 (a)). Hoje o estrago é
  **visível** — `colisao_de_criacao` o conta — e continua sendo estrago.
- **O que vira migração:** o próprio documento marca a linha de corte
  (`AUTONUMBER.md:475-478`): enquanto **não houver `Sequence` com faixa
  declarada em produção**, é versão nova que lê a antiga com os padrões.
  Depois, é reescrever `PSCH` de toda tabela com `Sequence` — e, pior, **decidir
  retroativamente** em que faixa cada nó estava quando gerou os números que já
  estão gravados. Essa segunda parte não é migração: é adivinhação.
- **Parecer:** entra, e entra **junto** com a §4.2, num só bump de `PSCH`.
  Dois bumps para duas colunas de sistema é dois eventos de migração onde cabe
  um.

### 4.2 Carimbo de data/hora de sistema por linha — **`PSCH` v10**, e **medido agora**

Decisão do dono de 11/09/2026 (`~/.claude/CLAUDE.md`): *«Impossível o filho ter
a MESMA data do pai»* — nasce uma coluna de data/hora de sistema por linha, e no
commit o pai é carimbado com instante **estritamente anterior** ao do filho.
*Implementação pendente; formato se decide com o dono antes de gravar.*

**O papel C traz o número que faltava para essa decisão, e ele muda o desenho:**

> Medido em 17/09/2026: **12 eventos gravados num único milissegundo** — um só
> carimbo distinto para os 12. O `.log` carimba em **milissegundos**
> (`log.rs:19-20`), e uma passada de commit grava dez linhas em muito menos que
> isso.

Ou seja: **hoje o filho tem a mesma data do pai como norma, não como azar** — e
o diário, que é a única data que existe por escrita, **não consegue** ordenar mãe
e filha de um commit. A conclusão de projeto que sai daí é direta: **uma coluna
nova em milissegundos herdaria o empate inteiro e não cumpriria a ordem do
dono.** A coluna precisa de um desempate que não seja o relógio de parede —
resolução mais fina (µs/ns) **ou** um contador monotônico por commit gravado ao
lado. Isso é decisão do dono, e o número acima é o que ela precisava ter na
mesa.

- **Custo de adiar:** hoje não há como **provar no dado** que o pai veio antes.
  Cada dia de dado gravado é dado que, depois da mudança, terá a coluna nova
  **vazia ou retroativa** — e retroativa é inventada, porque a informação não
  existe em lugar nenhum (o `.log` só sabe o milissegundo, e ele empata).
- **O que vira migração:** `acrescentar_coluna` reescreve o `.reg` **inteiro**,
  slot a slot (`PENDENCIAS.md:292` já diz isso do irmão `Criptografar`). Para
  uma tabela de cem milhões de linhas isso é uma janela de parada. E há um
  segundo custo, replicação: coluna de sistema nova **muda o payload da imagem**,
  então source e réplica precisam subir **juntos** ou a imagem não cai no lugar
  certo — o `abrir_imagem` lê por offset (`table.rs:3867-3896`).
- **Parecer: é a mais urgente das três**, e não por ser a mais bonita — por ser
  a única cujo dado **não se reconstrói depois**.
- **E o aviso que o papel C precisa dar por escrito:** a coluna de sistema nova
  entra **no fim**, sempre (`FORMATO.md:476-478`), e a casa já pagou este preço
  três vezes: *«coluna de sistema nova quebra quem filtra pela primeira»* —
  procure quem usa `find(...)` onde devia usar `filter(...)`. O `rownum` fez
  isso e quebrou *todo salvar e todo incluir pela tela*.

### 4.3 Marca de índice suspenso — **NÃO, e recusado com número que já existe**

O briefing a lista como pendência de formato. **Ela não é pendência: é recusa
medida, e continua recusada.** `docs/PENDENCIAS.md:123` (pedido 114): adiar o
índice não único vale 1,59× só para tabela vazia, vira **prejuízo abaixo de
M≈N/3**, e «cobraria marcar **índice suspenso no formato**, cujo defeito é busca
respondendo errado em silêncio depois de uma queda».

**Parecer do papel C: não.** Uma marca de «este índice está suspenso» é uma
segunda verdade sobre o `.ndx`, e a primeira já existe e funciona — a marca de
«o índice ficou para trás», que faz **toda** operação de índice recusar até o
reparo, e que a recuperação do arranque sabe reconstruir
(`transacao.rs:1238-1250`). Uma suspensão que deixa a **busca responder** sobre
uma árvore incompleta é pior que a recusa: responde **a menos**, em silêncio, e
quem lê não sabe. Se o item voltar, volta pelo caminho que o próprio pedido 114
nomeia — **fundir** a série ordenada na árvore existente, sem marca nova no
formato.

## 5. O **não** do papel C

Cinco propostas boas que quebrariam uma garantia. Nenhuma foi feita nesta
rodada; estão aqui para **não voltarem sem o número**.

### NÃO 1 — «reaproveite o slot do `rownum` queimado» / «devolva o contador na recusa»

É a reação natural ao §2.1, e é a errada. Devolver o contador
(`proximo_rownum -= 1` no erro) reintroduz **reuso de número de ordem**: duas
escritas concorrentes que falham e reentram podem receber o **mesmo** `rownum`,
e a paginação por cursor passa a pular registro — que é exatamente o defeito que
o «nunca reaproveita número, nem depois de exclusão» (`FORMATO.md:480-484`)
existe para impedir. **A ordem de digitação é sagrada, e um número de ordem
devolvido é um slot reaproveitado com outro nome.**

O caminho que **não** quebra garantia é o inverso: consumir o `rownum` **depois**
das conferências que podem recusar, ou fazer a réplica **honrar o `rownum` da
imagem** em vez de gerar o dela. O segundo é o mais fiel à ideia de réplica — a
réplica não inventa, ela aplica — mas é decisão de projeto, porque muda o
significado de `numerar_linha` e toca o bidirecional, onde o `rownum` é
**local por desenho** (`servidor.rs:4222-4224`). **Não implemento nenhum dos
dois neste parecer; nomeio que o primeiro é proibido e os outros dois são
escolha, não conserto óbvio.**

### NÃO 2 — «ponha um id de transação no evento do `.log` e replique o commit inteiro»

Boa ideia, e é o que resolveria o §2.4. Ela **muda o formato do evento** — e o
evento é a única coisa da casa que não se reconstrói (`log.rs:538-541`: *«índice
perdido se reconstrói do `.reg`; evento perdido não se reconstrói»*). Três
consequências que precisam estar na mesa antes de qualquer byte:

1. **Não há reservado sobrando de graça.** Os 2 bytes reservados do cabeçalho já
   foram gastos pela `origem` (`log.rs:144-150`), e os 4 do `tempero` são do
   nonce da cifra (`log.rs:264-283`). Um id de transação é `u64`: o cabeçalho
   sai de 44 bytes.
2. **Aplicar em grupo obriga a réplica a segurar o lote inteiro em RAM antes de
   gravar**, e o teto de 500 eventos por lote (`replica.rs:52`) existe
   justamente porque *«um lote de dez mil linhas com anexo seria uma resposta de
   dezenas de megabytes montada de uma vez dos dois lados»*. Commit maior que o
   lote exige um protocolo de commit em duas fases na réplica — que é recurso
   novo, não ajuste.
3. **E não compra atomicidade entre tabelas**, que é onde o problema realmente
   dói: as posições são por tabela e independentes (`servidor.rs:2710`).
   Atomicidade entre tabelas pede uma **posição global**, e o `REPLICACAO.md`
   §4 tem o título «por que o PhxSql não precisa inventar um GTID». Inventar um
   agora é reabrir aquela decisão.

**Não.** Não sem o dono, e não como efeito colateral de uma frente de
replicação.

### NÃO 3 — «faça a réplica conferir a unicidade do único secundário como conferia a FK» (isto é, cale-a)

**Não**, e é o inverso do §2.5 — calar a unicidade grava duas linhas com a mesma
chave num índice **declarado único**, e o índice no disco passa a mentir sobre
si mesmo. A FK pôde ser calada porque a garantia é **da origem** e a órfã é
temporária e se cura quando a mãe chega (`INTEGRIDADE.md` §3 — a órfã é um estado,
não uma corrupção). Uma chave única violada **não se cura quando o próximo lote
chega**: as duas linhas ficam, e o próximo `buscar` pelo índice responde uma das
duas arbitrariamente. As três saídas honestas estão no §2.5, e **as três são
decisão do dono**.

### NÃO 4 — «a soma do cluster é frágil; some só as tabelas replicadas / normalize por tabela»

A metade fácil disto é conserto de comentário-versus-código (§2.6.3) e não pede
o papel C. A metade que **pede** é a que eu recuso hoje: trocar a soma por um
**vetor de posições por tabela** no pulso muda o **critério de eleição**, que é
código de consenso. `vencedor` (`cluster.rs:134-147`) é hoje uma função pura de
quatro campos, testável e igual em todo nó — «igual em todo nó» é a propriedade
que faz a eleição convergir. Comparar vetores exige uma **ordem total** sobre
eles, e ordem total sobre vetores de tabelas que os dois nós podem enxergar
**diferente** (um tem uma tabela que o outro não tem) é onde clusters de
verdade se partem em dois masters. **Não sem desenho próprio e sem o dono.**

O que **dá** para fazer sem tocar no consenso, e eu recomendo como o item de
maior retorno desta frente inteira: **dar à réplica a mesma conferência de
continuidade que o PITR já tem** (§2.6.1). `diario_vivo_continua`
(`servidor.rs:18640-18673`) já existe, já tem o texto certo — *«a tabela foi
apagada e recriada»* — e o irmão dela, `alcancar_tabela`, tem uma comparação de
`>=` onde deveria haver uma pergunta. Não é formato, não é consenso, não muda
cliente nenhum: é a guarda que já foi escrita alcançando o caminho irmão.

### NÃO 5 — «replique tabela com coluna externa marcada, e resolva a senha na configuração»

**Não**, e não é opinião: a condição **nunca se satisfaz**, porque o sal é
sorteado por arquivo (`SEGURANCA.md:1981-1987`). E o caso sem erro — réplica com
a cifra desligada gravando texto cifrado como conteúdo — é **dado errado sem
aviso**, que é a única categoria de defeito que este parecer trata como
inaceitável em qualquer prazo. Enquanto o envelope da §11.5 (chave de tabela
sorteada, envelopada pela chave mestra) não existir, a recomendação da
`SEGURANCA.md` §11.8 fica de pé e eu a endosso: **não replicar tabela com coluna
externa marcada**. E acrescento o que falta lá: isso deveria ser **recusa do
motor**, com o motivo escrito, e não recomendação em documento — do mesmo modo
que o multi recusa tabela sem chave única. Recusa na **declaração** custa um erro
lido; recusa nenhuma custa um `.bin` cheio de texto cifrado que ninguém sabe que
é texto cifrado.

## 6. O que é decisão do dono, em lista

Nada abaixo é conserto que uma frente possa tomar sozinha:

1. **Coluna de data/hora de sistema por linha** — o formato (`PSCH` v10) e,
   agora com o número medido, **a resolução**: ms empata (12 eventos, 1
   carimbo), e por isso a coluna precisa de resolução mais fina ou de um
   contador monotônico por commit. Sem isso ela não cumpre «impossível o filho
   ter a mesma data do pai».
2. **`inicio`/`passo` no `PSCH`** — o conserto pleno do pedido 229 (a).
   Recomendo **no mesmo bump** do item 1.
3. **Quem honra o `rownum` numa réplica** — a réplica gera o dela (hoje) ou
   aplica o da imagem? Muda o significado de `numerar_linha` e toca o
   bidirecional. **Devolver o contador na recusa está fora** (NÃO 1).
4. **O que fazer com único secundário no bidirecional** — recusar a tabela,
   casar por N chaves, ou quarentena. As três custam (§2.5).
5. **Replicar coluna externa marcada** — recusar no motor, ou esperar o
   envelope da §11.5. Hoje é recomendação em documento e o pior caso é silencioso.
6. **O critério de eleição do cluster** — se a soma continua sendo um escalar
   (§2.6, NÃO 4).

## 7. Como se refaz o que este parecer mediu

As três medições saíram de um binário isolado, com dependência de caminho para
as crates do repositório, **fora** do repositório e com `CARGO_TARGET_DIR`
próprio — para não disputar o `target` nem a trava com as frentes que rodavam em
paralelo. Roteiro, para não morrer com a sessão:

```bash
# um Cargo.toml com [workspace] vazio e as duas deps por caminho:
#   phxsql-core = { path = ".../crates/phxsql-core" }
#   phxsql-store = { path = ".../crates/phxsql-store" }
CARGO_TARGET_DIR=$PWD/target flock /tmp/phx-cargo.lock cargo run --offline -q
```

1. **`rownum` queimado:** criar tabela com primária única, inserir 3, inserir a
   **4ª repetida** (recusa), inserir 2 boas, replicar tudo por
   `diario_com_imagem` + `aplicar_evento`, comparar a coluna `rownum` linha por
   linha. Esperado: rowids iguais, `rownum` divergindo de 4 em diante.
2. **Carimbo em ms:** 10 inserções + 2 alterações seguidas, ler
   `diario_com_imagem(0,0)` e contar carimbos distintos. Esperado: 1.
3. **Unicidade na réplica:** tabela com primária `porId` e único secundário
   `porEmail`; inserir local, depois `inserir_replicado` com id diferente e
   **mesmo** e-mail. Esperado: `Duplicado`.

---

**Assinado:** papel C (DBA sênior), 17/09/2026 02:39 UTC.
Nenhum arquivo de código foi alterado; nenhum commit e nenhum `git add` partiu
desta frente.
