# Parecer do papel C — pedido 355 (ledger, SHA-256 sem sal)

**Data:** 23/09/2026 · **Papel:** C (DBA sênior) · **Escopo:** 0.19, grupo B
(«vaza segredo») · **Modo:** só leitura. Não consertei, não editei código, não
comitei. Nenhum conteúdo real de linha de ledger nem hash de dado verdadeiro
aparece aqui.

---

## 0. Veredito, em quatro linhas

1. **O conserto que o pedido pede já entrou** — commit `a51f1a3`, 18/09/2026,
   pela decisão do dono do mesmo dia. A recusa está em **três portas de
   declaração** e cobre os **dois funis** por onde o protocolo passa. Medido no
   fonte, não na memória.
2. **O `.md` de pendência e o dossiê ainda dizem «Planejado».** O pedido 355
   está `☐` em `PENDENCIAS.md:379` e «Planejado» em
   `docs/dossie/pedidos-351-410.html:250`, com o código, os testes e o
   `FORMATO.md` todos dentro. É o **mesmo achado de processo** que o board da
   0.19 já nomeou em outros quatro pedidos.
3. **A `SEGURANCA.md` está VELHA na linha deste vazamento** — `:1785` ainda o
   lista como aberto, cita `ledger.rs:199` e `:218` (as funções andaram para
   `:206` e `:231`) e não menciona a guarda. Na **mesma tabela**, três linhas
   vizinhas trazem «FECHADO em …». Esta não traz.
4. **O que sobra é real e não está escrito em lugar nenhum:** a guarda é por
   **declaração**, e **replicação não é declaração**. A combinação proibida
   **nasce em nó novo** pelo caminho da réplica. Ver §5.3.

**Não muda o `PSCH`. Não quebra cadeia gravada.** Detalhe em §6.

---

## 1. Para que serve esse hash hoje — medido no código

A pergunta do enunciado («integridade, encadeamento, deduplicação, ou as
três?») tem resposta medida: **integridade e encadeamento ao mesmo tempo, pela
MESMA função; deduplicação não, nem por acidente.**

O motivo é uma linha de `preparar_bloco`:

`crates/phxsql-store/src/ledger.rs:322-328`

```rust
valores[i_anterior] = Value::Uuid256(anterior);
valores[i_altura]   = Value::UInt(altura);
valores[i_hash]     = Value::Null;
let h = hash_do_bloco(&esquema, &valores);
```

O `anterior` (= hash do bloco de baixo) e a `altura` são **postos na linha
antes** do hash ser calculado, e `conteudo_canonico` **não os exclui**
(`ledger.rs:206-224` só pula `hash`, `assinatura` e coluna de sistema). Logo o
hash de um bloco **é** o hash do conteúdo **e** o elo da corrente: não há dois
hashes, há um só fazendo os dois papéis.

`verificar_cadeia` (`ledger.rs:380-442`) confere **três** provas, nesta ordem,
e o enum `Prova` (`:336-351`) diz qual caiu:

| prova | linha | o que ela pega | usa o hash? |
|---|---:|---|---|
| **(iii) Altura** | `:400-409` | buraco ou bloco fora de ordem | não |
| **(ii) Ligação** | `:410-419` | elo quebrado: `anterior` ≠ hash de baixo | sim, o gravado |
| **(i) Conteúdo** | `:420-430` | adulteração da linha depois de gravada | sim, recalculado |

**Deduplicação: não.** O predicado do modo (`schema.rs:227-242`) exige as três
colunas **e o índice único `porAltura`** — único sobre a **altura**, não sobre o
hash. O índice `porHash` existe no esquema de exemplo (`ledger.rs:465`) e **não
é peça do modo**. E mesmo se fosse, dedup de conteúdo não existiria: dois blocos
de conteúdo idêntico em alturas diferentes têm `anterior` e `altura` diferentes,
logo hashes diferentes.

**Consequência que decide o pedido:** a saída «tirar o conteúdo do hash e deixar
só o encadeamento» está **morta por construção** — ela apaga a prova (i), que é
a única que pega adulteração de linha. Sobrariam duas provas que um adversário
com acesso de escrita satisfaz reescrevendo a cadeia inteira. O dono já disse
isso na decisão de 18/09 («nem se tira a coluna marcada do canônico»); o código
mostra **por quê**.

---

## 2. O que exatamente entra no hash — medido

**Regra única, uma fonte só:** `phxsql_core::schema::coluna_no_hash_do_ledger`
(`crates/phxsql-core/src/schema.rs:250-252`):

```rust
nome != LEDGER_COL_HASH && nome != LEDGER_COL_ASSINATURA && !e_coluna_de_sistema(nome)
```

Entra: **toda coluna do esquema, na ordem do esquema**, menos três famílias —
`hash` (um hash não cobre a si mesmo), `assinatura` (assina o hash, vem depois)
e as colunas de sistema (`softdeleted`, `rownum`, e os carimbos do `PSCH` v10).
Cada campo é `<byte de tipo><valor big-endian>`, com tamanho prefixado nos
variáveis — o leiaute está publicado em `ledger.rs:19-43` **de propósito**.

**A afirmação nova, e é ela que fecha a discussão do sal:** o valor da coluna
marcada entra **em claro** no hash, e todos os **outros** campos do mesmo slot
estão em claro **no disco** — a cifra sela só as faixas marcadas. O atacante
com o `.reg` e sem a chave tem *tudo menos um campo*, e tem um SHA-256 que
depende de tudo. **O oráculo é exato, por linha, offline, sem limite de
tentativas.** O pedido acertou isto e eu confirmo no fonte.

**Uma correção ao modelo de ameaça, a favor nosso, medida agora:** como o
`anterior` entra no conteúdo canônico, o hash de todo bloco de altura ≥ 2
depende do **prefixo inteiro da cadeia**. Isso dá separação de domínio de graça:
o mesmo CPF em duas bases diferentes **não** produz o mesmo hash. A
enumerabilidade **cruzada entre bases** existe só no bloco **gênese**
(`anterior = NULO`, `altura = 1`, `ledger.rs:319`). Não muda o veredito — o
oráculo por linha continua exato dentro da base —, mas apaga o risco de
correlação entre bases, que ninguém tinha nomeado.

---

## 3. Quantas linhas de ledger existem hoje — o censo, parcialmente medido

O DBA de 18/09 declarou que **não mediu**. Medi a parte que se mede daqui, por
assinatura de esquema: o bloco `PSCH` grava os nomes, então uma tabela em forma
de cadeia carrega a cadeia de caracteres `porAltura` dentro do próprio `.reg`.

| população | `.reg` | em forma de cadeia (`porAltura`) | com coluna marcada |
|---|---:|---:|---:|
| **repositório** (fora de `target/`) | **65** | **0** | **0** |
| **disco inteiro** (fora de `/proc`) | **53.144** | **4** | **0** *(ver ressalva)* |

Os **quatro** achados no disco são os mesmos dois bancos de demonstração do
`--example identificadores`, de **28/08/2026** (`Cadeia/blocos` em três cópias e
`Comercial/blocos`), e nenhum está no repositório — vivem no *scratchpad* de
sessão. O esquema dos quatro, lido do cabeçalho: `id`, `hash`, `anterior`,
`altura`, `autor`, `carimbo`. **Nenhuma coluna candidata a dado pessoal.**

**Ressalva honesta sobre a terceira coluna:** eu confirmei a **forma** (a cadeia
de caracteres `porAltura` dentro do bloco `PSCH`) e li os **nomes** das colunas;
eu **não li o byte de marca** de nenhum `.reg` — ele é um byte por coluna no fim
do bloco `PSCH` desde a v6 e não se acha por texto. O zero da terceira coluna é
**inferência pelo esquema**, não leitura do byte. O que fecha isso é o censo
`--example` da receita abaixo.

**Leitura de DBA:** **zero cadeias versionadas.** «Mudança de formato entra
cedo» continua **verdade hoje**, e o custo de qualquer decisão sobre este hash é
o de escrever código — não o de migrar dado. Mas a janela não é nossa para
sempre: ela fecha no dia em que a primeira cadeia com coluna marcada existir num
nó de cliente, e a partir daí **não tem conserto**, porque recolher o oráculo é
regravar a cadeia, que é exatamente o que uma cadeia não permite.

**A receita do censo completo, que continua faltando** (e que eu **não escrevi**
porque não conserto): um `--example` que percorra `base/`, abra cada tabela por
`Table::abrir` e imprima `(nome, e_tabela_ledger, tem_dado_pessoal, blocos)`.
Meu `grep` por `porAltura` acha a **forma**; ele **não** lê a marca de dado
pessoal, que é um byte por coluna no fim do bloco do `PSCH` e não se acha por
texto. O censo que decide é o do par, não o da forma — e **ele tem de rodar em
todo nó**, não só no primário, pela razão da §5.3.

---

## 4. O que os quatro motores fazem — a matriz, com fonte

Busquei no help dos quatro, em 23/09/2026. O resultado é **incomum e precisa
ser dito com todas as letras**: a matriz **não decide, porque o mecanismo não
existe em nenhum dos quatro.**

| motor | tem hash criptográfico **por linha**, gravado **ao lado da linha**? | o que tem em lugar disso | fonte |
|---|---|---|---|
| **PostgreSQL** (4) | **não** | *data checksums* por **página**, não criptográficos, ligados por padrão, no nível do *cluster* inteiro — «Only data pages are protected by checksums». Auditoria é `pgaudit`, **extensão**, e escreve no **log do servidor**, fora da tabela. `pgcrypto` dá `digest()`/`hmac()` ao **usuário**, que escolhe | `postgresql.org/docs/current/checksums.html` §28.2 |
| **MariaDB** (3) | **não** | `CHECKSUM TABLE` — «Report a checksum for table contents… useful for verifying replica», calculado **sob demanda**, não gravado. Auditoria é o `server_audit`, **plugin**, e escreve em **arquivo** | `mariadb.com/docs/…/checksum-table` |
| **MySQL** (2) | **não** | `CHECKSUM TABLE [QUICK\|EXTENDED]` — «The checksum value depends on the table row format», sob demanda. Auditoria é o `audit_log` **Enterprise**, em arquivo, e a resposta deles ao «o artefato tem texto claro» é **cifrar o artefato**: `audit_log_encryption`, **AES-256-CBC** com chave derivada de senha | `dev.mysql.com/doc/refman/8.4/en/checksum-table.html`, `…/audit-log-logging-configuration.html` |
| **SQLite** (1) | **não** | `dbhash` — «computes the SHA1 hash of the schema and content», **utilitário externo**, do banco inteiro, não gravado. `cksumvfs` — *shim* de VFS com «an 8-byte checksum» **por página**, «intended to help detect database corruption» | `sqlite.org/dbhash.html`, `sqlite.org/cksumvfs.html` |

**Placar: 0 de 4.** Não há convergência a aceitar nem divergência a ponderar —
a régua PG 4 / MariaDB 3 / MySQL 2 / SQLite 1 **não se aplica**, porque os
quatro lados somam **zero** dos dois lados da questão. Dizer «ganhou 10 a 0» aqui
seria inventar um voto.

**O que os quatro CONVERGEM, e aí são 4 de 4, e vale como aceite:**

1. **Artefato de integridade é não criptográfico e mora FORA da linha** —
   página (PG, SQLite) ou tabela sob demanda (MySQL, MariaDB). Nenhum grava um
   *digest* de valor ao lado do valor.
2. **Trilha de auditoria vai para ARQUIVO, não para coluna.** `pgaudit`,
   `server_audit` e `audit_log` os três escrevem fora do dado.
3. **Quando o artefato de auditoria carrega texto claro, a resposta é CIFRAR o
   artefato inteiro** (MySQL `audit_log_encryption`, AES-256-CBC). **Nenhum dos
   quatro salga um hash de conteúdo** — porque nenhum dos quatro tem um.

**O choque com pétrea nossa, e ele aparece em vez de ficar calado:** o item 1
diria, se levado ao pé da letra, «tire o hash de dentro da linha». **Não passa**,
e não é gosto: o hash **na** linha é o que faz a cadeia ser cadeia — tirá-lo de
lá exige um segundo arquivo com uma árvore de Merkle e um relógio de selagem, o
que é produto novo e não conserto de vazamento. O item 3, sim, **converge com a
nossa saída (e)**: cifrar/chavear o artefato, em vez de salgá-lo. Registro a
convergência e ela **reforça** o que o dono decidiu, não o contrário.

**O que a pesquisa NÃO alcança, e digo que não alcança:** o motor que **tem**
exatamente este mecanismo é o SQL Server 2022 / Azure SQL (*ledger tables*, hash
de linha + árvore de Merkle com o *digest* publicado **fora do banco**, em
armazenamento imutável). Ele **não está nos quatro** e eu **não o medi** nesta
rodada. Se alguém quiser a receita de como eles conciliam ledger com coluna
cifrada, é pesquisa do papel J — e é a única fonte que responderia.

---

## 5. O caminho recomendado, e o que ele custa

### 5.1 Recomendo MANTER a decisão do dono de 18/09 — recusa na declaração

E recomendo por número, não por deferência: a saída (a) **custa zero byte de
formato**, é **reversível** (recusa se afrouxa quando o dono quiser; hash em
claro já gravado **não se recolhe**) e **não fecha o caso de uso** — quem quer
cadeia tira a marca da coluna; quem quer a marca usa tabela comum. As outras
duas continuam onde o parecer de 18/09 as deixou:

- **sal por linha / sal do arquivo: morto, e a razão é de formato.** O sal do
  `cofre.rs` é gravado **em claro** no cabeçalho — o próprio doc do
  `MATERIAL_LEN` diz «sorteado por arquivo; não é segredo». **Sal que mora na
  linha ou no arquivo não é sal**, é exatamente a frase do enunciado. Paga o
  preço todo (mata «reproduzir de fora», `ledger.rs:19-43`) e não fecha o
  oráculo por um bit.
- **HMAC-SHA256 com a chave do cofre (saída «e»): viva, guardada, e continua
  cabendo.** `phxsql_core::hash::hmac_sha256` já existe (`hash.rs:172`),
  conferido contra a RFC 4231 — **zero dependência nova**, é reuso, não
  acréscimo. Sai `[u8; 32]`, **cabe no mesmo `Uuid256`**: **zero mudança de
  formato**. Onde mora o segredo: na **chave do cofre**, que é derivada por
  PBKDF2 da senha e **não está no disco** — é essa a diferença inteira para o
  sal. O que ela custa é a **redação da promessa**: «reproduzível por qualquer
  ferramenta» vira «reproduzível por quem tem a senha», e **só** nas tabelas com
  coluna marcada.

### 5.2 O que ainda falta no 355, e é pouco — mas não é nada

| # | o que falta | por quê importa | mede-se como |
|---|---|---|---|
| **F1** | **Prova real do ORÁCULO** (não da guarda) | Os cinco testes provam que a **recusa** funciona. Nenhum prova que o **oráculo existia** — e a lei da casa é «o teste tem de FALHAR com o defeito reposto». Hoje o defeito **não se consegue repor pela declaração**, que é justamente o conserto. | Pela porta legada, que já está aberta e testada: `esquema_legado_com_cpf_marcado` (`ledger.rs:875-882`) monta a combinação por `Schema::do_disco`. Fixar as outras colunas, iterar N candidatos **sintéticos** (nunca CPF real), exigir **exatamente 1** acerto. É o teste que documenta o preço da população legada. |
| **F2** | **Censo do par** `(e_tabela_ledger, tem_dado_pessoal)`, em **todo nó** | Diz se «entra cedo» ainda é verdade. Meu `grep` mede a **forma**, não a **marca** (§3). | `--example` sobre `base/`, por `Table::abrir` |
| **F3** | `PENDENCIAS.md:379` e `pedidos-351-410.html:250` **dizem «Planejado»** com o código dentro | O board da 0.19 já nomeou esta classe: «pedido que carrega a decisão do dono no corpo continua marcado Planejado e some da vista» | leitura |
| **F4** | `SEGURANCA.md:1785` **velha nas duas pontas** | Cita `ledger.rs:199`/`:218` (hoje `:206`/`:231`) e lista o vazamento como **aberto**, sem a guarda. Três linhas vizinhas da mesma tabela sabem dizer «FECHADO em …» | leitura |

**F3 e F4 não são cosmética.** Um documento de segurança que lista como aberto o
que está fechado gasta a atenção de quem lê no lugar errado — e, pior, ensina a
desconfiar das outras linhas da mesma tabela, que estão certas.

### 5.3 O achado que este parecer acrescenta: replicação não é declaração

A guarda mora nos **verbos de declarar** do `Schema` — `new` (`schema.rs:959`),
`marcar_dado_pessoal` (`:1329-1331`) e `com_coluna` (`:1528-1530`). Foi decisão
consciente e **certa**: pô-la em `Table::criar` teria recusado **semear uma
réplica** de uma cadeia legítima que já existe (está escrito no corpo do commit
`a51f1a3`).

**A consequência que não está escrita em lugar nenhum:** a réplica recebe o
esquema **serializado pelo fio** e o remonta por `Schema::desserializar`
(`crates/phxsql-server/src/replica.rs:331` → `schema.rs:1976`), que é o caminho
de **leitura** e **não julga** — e o próprio `valores.rs:620` confirma o desenho:
«uma tabela nascida por replicação é criada do MESMO bloco de esquema do source,
byte a byte».

Logo: **a população em combinação proibida não é só «a que já existe»; ela
CRESCE por replicação, para nós que nunca a tiveram.** Três coisas saem disso, e
nenhuma é «ponha a guarda ali» — a guarda ali quebraria a réplica legítima:

1. O **censo (F2) tem de rodar em todo nó**, não no primário.
2. A frase «NÃO desfaz cadeia que já existe» do `ledger.rs:63-66` e do
   `FORMATO.md:675-681` está **incompleta**: o alcance real é «não desfaz, **e
   acompanha a réplica**». Quem ler o alcance atual conclui que a população é
   fechada. Ela não é.
3. Se o dono um dia reabrir e escolher HMAC, a **réplica é o caso difícil** — a
   chave do cofre é por arquivo/senha, e uma cadeia com HMAC só se verifica onde
   a senha chega. Isso é decisão de produto, e vai à mesa, **não** se inventa
   aqui.

---

## 6. Formato: não muda o `PSCH`, e não quebra cadeia gravada

**Não muda.** Escrito, medido e confirmado nas três camadas:

- O que deixou de existir é a **declaração**, não o byte. A marca de dado
  pessoal continua sendo **um byte por coluna no fim do bloco** desde a
  **`PSCH` v6**, e `Schema::do_disco` **devolve o que foi gravado**
  (`FORMATO.md:675-681`).
- O `PSCH` em vigor para escrita é a **v10** (`FORMATO.md:362-376`); a leitura
  aceita da v2. **Este pedido não toca nenhuma das duas.**
- **Cadeia gravada antes da guarda abre, lê e grava bloco novo** — e há teste
  que cai se alguém descer a guarda para `do_disco` ou `Table::abrir`:
  `cadeia_marcada_gravada_antes_da_guarda_abre_le_e_grava`
  (`ledger.rs:931-978`). É o teste do **comportamento velho**, que é o que mais
  importa numa guarda nova.

**E o que mudaria o formato, para ficar registrado antes de alguém propor:**
misturar duas funções de hash **dentro da mesma cadeia** exigiria um
discriminador por bloco — `PSCH` novo **e** slot novo. Hoje isso não acontece
por consequência, não por regra: como a combinação não se declara mais, nenhuma
cadeia pode nascer meio SHA-256 e meio HMAC. **A saída (e) custa zero formato
exatamente enquanto o censo der zero.**

**Ligação com o 314 (☑️):** tabela em modo ledger **com cadeia** fica em `PSCH`
v9 para sempre, porque `Table::acrescentar_coluna` recusa coluna nova em cadeia
— acrescentar muda o conteúdo canônico de **toda** linha antiga e faria
`verificar_cadeia` gritar adulteração numa cadeia intacta. Uma cadeia **nascida**
em v10 funciona: `coluna_no_hash_do_ledger` já exclui coluna de sistema, e os
carimbos do v10 são de sistema. Não há conflito entre os dois pedidos; há uma
dependência de ordem, e ela já está resolvida do lado certo.

---

## 7. O que eu NÃO medi

1. **O oráculo.** Não o exercitei; li o código que o produz. F1 continua aberto.
2. **A marca, byte a byte.** A varredura terminou: **53.144** `.reg` no disco,
   **4** em forma de cadeia, **0** no repositório. Mas eu medi a **forma** e os
   **nomes de coluna**; **não li o byte de `dado_pessoal`** de nenhum `.reg` —
   ele é byte no `PSCH` e não se acha por texto. O «0 com coluna marcada» da §3
   é inferência pelo esquema, e só o `--example` do censo o transforma em
   leitura.
3. **A via indireta pelo protocolo**, que o pedido já declarava não medida: o
   `ledger` continua com **zero** referências de código em
   `phxsql-server/src` (a única ocorrência, `valores.rs:2233`, é comentário).
   Não medi se `varrer` devolve a coluna `hash` a quem tem a coluna marcada
   negada — a peneira de coluna tira por **nome**, e nome nenhum lhe diz que
   `hash` fala do `cpf`.
4. **Os testes.** Não rodei `cargo` (limite de disco, quatro frentes vivas).
   Os «887 testes verdes em core+store» são do corpo do commit `a51f1a3`,
   **citados**, não remedidos por mim — e número citado é número que não se
   mede.
5. **SQL Server / Azure SQL ledger**, o único motor que tem o mecanismo. Fora
   dos quatro, não medido, nomeado em §4 como pesquisa do papel J.
