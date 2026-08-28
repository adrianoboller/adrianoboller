# Formato de arquivo do PhxSql

Uma tabela de dados do PhxSql é composta por cinco arquivos físicos que
compartilham o mesmo nome-base:

```
cadastroClientes.reg  +  .ndx  +  .bin  +  .memo  +  .log  =  cadastroClientes
```

| Arquivo | Papel | Assinatura | Pagina? |
|---|---|---|---|
| `.reg` | Registros, na ordem de digitação | `PHXREG\0\0` | sim |
| `.ndx` | Índices (B+tree), todos no mesmo arquivo | `PHXNDX\0\0` | **não** |
| `.bin` | Binários (imagens, anexos) | `PHXBIN\0\0` | sim |
| `.memo` | Textos longos | `PHXMEMO\0` | sim |
| `.log` | Diário de inclusões, alterações e exclusões | `PHXLOG\0\0` | sim |

Uma tabela grande se parte em volumes numerados — `cadastroClientes_001.reg`,
`_002.reg`, … — segundo os parâmetros do `CREATE TABLE`. Ver a seção 5.

**Convenções gerais**

- Inteiros são **little-endian**, exceto dentro de chaves de índice, que usam
  big-endian por causa da ordenação.
- Todo arquivo começa com assinatura + versão de formato, e todo cabeçalho é
  protegido por CRC-32 (IEEE, polinômio refletido `0xEDB88320`).
- Offsets são absolutos, em bytes, a partir do início do arquivo.
- Campos marcados *reservado* são gravados como zero e ignorados na leitura.

---

## 1. `.reg` — a tabela física

O `.reg` é um *heap* de slots de largura fixa. O `rowid` é o número do slot,
começando em 1, e o endereço sai de uma conta, não de uma busca:

```
offset(rowid) = data_offset + (rowid - 1) * slot_size
```

### Cabeçalho (128 bytes)

| Off | Tam | Campo |
|----:|----:|---|
| 0 | 8 | assinatura `PHXREG\0\0` |
| 8 | 2 | versão do formato (2) |
| 10 | 2 | tamanho do cabeçalho (128) |
| 12 | 4 | número do volume |
| 16 | 4 | `slot_size` |
| 20 | 8 | `slot_count` — slots alocados, inclusive excluídos (só o volume 1) |
| 28 | 8 | `live_count` — registros ativos (só o volume 1) |
| 36 | 8 | `proxima_sequencia` — próximo valor da coluna `Sequence` (só o volume 1; 0 = nunca usada) |
| 44 | 8 | `data_offset` — onde começa o slot 1 |
| 52 | 4 | `schema_len` |
| 56 | 4 | CRC-32 do esquema |
| 60 | 8 | criado em (epoch, segundos) |
| 68 | 8 | alterado em |
| 76 | 8 | transação (reservado) |
| 84 | 40 | reservado |
| 124 | 4 | CRC-32 dos bytes 0..124 |

Logo após o cabeçalho vem o **esquema serializado** (`schema_len` bytes), e
`data_offset` é o próximo múltiplo de 64. A tabela é auto-descritiva: o
conjunto de arquivos basta para reabrir os dados, sem dicionário externo.

**Todo volume carrega o cabeçalho completo com o esquema**, então qualquer um
deles se descreve sozinho — se o volume 1 se perder, os outros ainda sabem
dizer o que são. Apenas o volume 1 tem contadores autoritativos da tabela
inteira.

### Slot

| Off | Tam | Campo |
|----:|----:|---|
| 0 | 1 | status: 0 = livre, 1 = ativo. **Nenhum outro valor é válido** |
| 1 | 1 | flags |
| 2 | 2 | reservado |
| 4 | 4 | CRC-32 do payload |
| 8 | 8 | versão do registro (incrementa a cada alteração) |
| 16 | 8 | reservado |
| 24 | N | payload |

`slot_size = 24 + payload_len`.

### Payload

```
[bitmap de nulos: ceil(n_colunas / 8) bytes][coluna 0][coluna 1]...
```

O bit `i` do bitmap ligado significa que a coluna `i` é NULL. Colunas NULL
gravam zeros no seu espaço.

### O byte de status só tem dois valores

Zero é livre, um é ativo, e **qualquer outra coisa é corrupção, não um estado**.

A distinção custou um defeito para ficar clara. Enquanto o código testava
`status != ativo` para decidir "este registro não existe", um único bit trocado
no cabeçalho do slot **apagava o registro em silêncio**: a leitura respondia
"não existe" sem erro, e o reparo dava o slot por bom e nunca ia buscar a cópia
no espelho — que estava lá, inteira.

Hoje um status inválido cai na mesma segunda chance da falha de CRC. Fica de
fora um caso: se o bit trocado deixar o status exatamente em **0**, o slot fica
indistinguível de uma exclusão legítima. Desempatar isso exige o `.log`, que
registra toda exclusão com data e hora.

### Ordem de digitação

Registros são **sempre anexados no fim**. Excluir marca o slot como livre, mas
o slot **não é reaproveitado**. Essa é uma escolha deliberada: reaproveitar
manteria o arquivo compacto, mas quebraria a garantia de que percorrer o `.reg`
do início ao fim devolve os registros na ordem em que foram digitados. O espaço
de slots excluídos só volta com uma compactação explícita, que renumera os
rowids e reconstrói os índices.

Com paginação a garantia continua valendo, porque o volume N+1 vem sempre
depois do N e dentro de cada volume os slots seguem em ordem de inserção.

---

## 2. `.ndx` — os índices

Todos os índices da tabela moram no mesmo arquivo, dividido em páginas de
tamanho fixo (padrão 4096 bytes). A página 0 guarda o cabeçalho e o diretório
de índices; as demais são nós da B+tree.

### Cabeçalho (128 bytes, dentro da página 0)

| Off | Tam | Campo |
|----:|----:|---|
| 0 | 8 | assinatura `PHXNDX\0\0` |
| 8 | 2 | versão do formato (1) |
| 10 | 2 | tamanho do cabeçalho (128) |
| 12 | 4 | `page_size` |
| 16 | 4 | quantidade de índices |
| 20 | 8 | quantidade de páginas |
| 28 | 8 | primeira página livre (0 = nenhuma) |
| 36 | 4 | tamanho do diretório |
| 40 | 4 | CRC-32 do diretório |
| 44 | 8 | alterado em |
| 52 | 72 | reservado |
| 124 | 4 | CRC-32 dos bytes 0..124 |

### Diretório de índices (a partir do offset 128)

Uma entrada por índice, em sequência:

```
u16 tamanho_do_nome | nome | u8 único | u32 key_len | u64 página_raiz | u64 qtd_chaves
```

### Página da árvore

| Off | Tam | Campo |
|----:|----:|---|
| 0 | 1 | tipo: 1 = folha, 2 = interno |
| 1 | 1 | flags |
| 2 | 2 | quantidade de entradas |
| 4 | 8 | próxima folha (0 = fim) |
| 12 | 8 | folha anterior |
| 20 | 8 | filho da direita (só em nó interno) |
| 28 | 4 | CRC-32 da página, calculado sobre 0..28 e 32..fim |
| 32 | … | entradas |

### Chave completa

Cada entrada de folha guarda a **chave completa**:

```
[chave codificada: key_len bytes][rowid: 8 bytes big-endian]
```

Como o rowid entra no fim e em big-endian, **toda chave completa é única** e a
comparação byte a byte também desempata por rowid. Isso dá três coisas de
graça: índices duplicados funcionam sem tratamento especial, o resultado de uma
busca já sai em ordem de digitação, e a árvore nunca precisa lidar com chaves
iguais.

- Entrada de folha: `ck_len` bytes = `key_len + 8`
- Entrada de nó interno: `ck_len + 8` bytes (chave completa + página filha)

Num nó interno, o filho da entrada `i` guarda as chaves **menores** que a chave
da entrada `i`; `filho_direita` guarda as maiores ou iguais à última chave.

### Codificação de chave que preserva ordem

A regra de ouro: comparar as chaves com `memcmp` tem de dar exatamente a mesma
ordem que comparar os valores lógicos. Com isso a B+tree é totalmente agnóstica
de tipo — o mesmo código serve para inteiro, data, decimal, texto, ASC, DESC e
NOCASE.

Cada componente ocupa `1 + largura` bytes:

```
[presença: 0x00 = NULL, 0x01 = preenchido][bytes ordenáveis do valor]
```

| Tipo | Codificação |
|---|---|
| `Bool` | 1 byte, 0 ou 1 |
| `IntN` | N bytes big-endian, com o bit de sinal invertido |
| `UIntN` | N bytes big-endian |
| `Real4` / `Real8` | bits IEEE-754: se negativo, inverte todos os bits; se positivo, liga o bit mais alto |
| `Decimal` | 16 bytes big-endian, bit de sinal invertido |
| `Date` / `Time` | como `Int4` |
| `DateTime` | como `Int8` |
| `Str(n)` | n bytes UTF-8, completados com `0x00` |
| `Uuid` | 16 bytes crus, big-endian — já são a chave |
| `Uuid256` | 32 bytes crus, big-endian — já são a chave |
| `Sequence` | 8 bytes big-endian, como `UInt8` |

- **NULL** ordena antes de qualquer valor (byte de presença 0x00).
- **DESC** inverte todos os bytes do componente, o que inverte a ordem e joga
  NULL para o fim.
- **NOCASE** aplica *fold* ASCII para maiúsculas antes de comparar, preservando
  o comprimento em bytes (mesma semântica do atributo NOCASE do Clarion(R)).

### Remoção

Remover tira a entrada da folha **sem rebalancear** a árvore. A busca continua
correta (folhas vazias apenas não produzem resultado), mas páginas podem ficar
subocupadas depois de muitas exclusões. Reconstruir o índice devolve a árvore
ao formato compacto.

---

## 3. `.bin` e `.memo` — os arquivos externos

Os dois usam a mesma estrutura — uma pilha de blocos *append-only* — e diferem
apenas na assinatura e na semântica do conteúdo: o `.memo` guarda UTF-8 e o
`.bin` guarda bytes crus.

### Cabeçalho (64 bytes)

| Off | Tam | Campo |
|----:|----:|---|
| 0 | 8 | assinatura |
| 8 | 2 | versão do formato (2) |
| 10 | 2 | tamanho do cabeçalho (64) |
| 12 | 4 | número do volume |
| 16 | 8 | `fim` — ponto de anexação |
| 24 | 8 | bytes vivos |
| 32 | 8 | bytes mortos |
| 40 | 8 | quantidade de blocos |
| 48 | 8 | alterado em |
| 56 | 4 | CRC-32 dos bytes 0..56 |
| 60 | 4 | reservado |

### Bloco

| Off | Tam | Campo |
|----:|----:|---|
| 0 | 1 | status: 1 = vivo, 0 = morto |
| 1 | 3 | reservado |
| 4 | 4 | tamanho do conteúdo |
| 8 | 4 | CRC-32 do conteúdo |
| 12 | 4 | reservado |
| 16 | N | conteúdo |

Atualizar um conteúdo **não** reescreve o bloco antigo: grava um bloco novo no
fim e marca o antigo como morto. O espaço morto só volta com a compactação — o
cabeçalho mantém `bytes_mortos` justamente para decidir quando compensa
compactar.

### Ponteiro gravado no `.reg`

Colunas `Bin` e `Memo` ocupam **16 bytes fixos** dentro do slot do `.reg`:

| Off | Tam | Campo |
|----:|----:|---|
| 0 | 6 | offset do bloco dentro do volume (u48 — 256 TB por volume) |
| 6 | 2 | número do volume (u16 — 65.535 volumes) |
| 8 | 4 | tamanho do conteúdo |
| 12 | 4 | CRC-32 do conteúdo |

O offset ocupa 48 bits em vez de 64 justamente para liberar os dois bytes do
volume sem crescer o ponteiro. 256 TB por volume é folga de sobra.

Conteúdo vazio não consome bloco: o ponteiro fica zerado. O CRC aparece nos
dois lugares (ponteiro e bloco) de propósito — a leitura confere os dois, então
um ponteiro apontando para o bloco errado é detectado, não só um bloco
corrompido.

---

## 4. `.log` — o diário da tabela

Toda inclusão, alteração e exclusão é registrada com data e hora. O arquivo é
append-only e sem índice: é um diário, não uma tabela.

### Cabeçalho (64 bytes)

| Off | Tam | Campo |
|----:|----:|---|
| 0 | 8 | assinatura `PHXLOG\0\0` |
| 8 | 2 | versão do formato (1) |
| 10 | 2 | tamanho do cabeçalho (64) |
| 12 | 4 | número do volume |
| 16 | 8 | eventos neste volume |
| 24 | 8 | `fim` — ponto de anexação |
| 32 | 8 | alterado em |
| 56 | 4 | CRC-32 dos bytes 0..56 |

### Evento (36 bytes)

| Off | Tam | Campo |
|----:|----:|---|
| 0 | 8 | carimbo — milissegundos desde 1970-01-01T00:00:00Z |
| 8 | 1 | operação: 1 = inclusão, 2 = alteração, 3 = exclusão |
| 9 | 1 | flags |
| 10 | 2 | reservado |
| 12 | 8 | rowid afetado |
| 20 | 8 | versão do registro depois da operação |
| 28 | 4 | usuário (0 = não informado) |
| 32 | 4 | CRC-32 dos bytes 0..32 |

O carimbo é em **milissegundos**, não segundos, para que operações no mesmo
segundo continuem ordenáveis. Uma operação recusada — chave duplicada, tabela
cheia, coluna obrigatória em branco — **não** gera evento: o diário registra o
que aconteceu, não o que foi tentado.

---

## 5. Paginação de tabelas grandes

Definida no `CREATE TABLE` e gravada no esquema:

| Parâmetro | Significado |
|---|---|
| `registros_por_arquivo` | quantos registros cabem em cada volume do `.reg` |
| `max_arquivos` | quantos volumes a tabela pode ter |
| `digitos` | largura do sufixo, padrão 3 (`_001`) |
| `bytes_por_arquivo` | tamanho de cada volume dos arquivos externos |

Capacidade da tabela = `registros_por_arquivo × max_arquivos`. Passar disso
devolve erro explícito "tabela cheia", em vez do estouro silencioso de 2 GB
que o TopSpeed(R) dava.

### O endereçamento continua sendo uma conta

```
volume = (rowid - 1) / registros_por_arquivo + 1
slot   = (rowid - 1) % registros_por_arquivo + 1
offset = data_offset + (slot - 1) * slot_size
```

Três garantias sobrevivem intactas:

- **Ordem de digitação:** o volume N+1 vem sempre depois do N.
- **O rowid é global e nunca muda.** Ele não é "posição no volume", é posição
  na tabela; o volume sai dele por divisão.
- **O `.ndx` não muda em nada.** Ele já guarda rowid, e nenhuma linha do código
  de índice precisa saber que existe volume.

### Arquivos externos

`.bin`, `.memo` e `.log` rolam por bytes, não por contagem: quando o bloco novo
não cabe no volume atual, ele vai **inteiro** para o próximo — um bloco nunca é
partido entre volumes.

A exceção: um bloco maior que `bytes_por_arquivo` fica sozinho no seu volume em
vez de ser recusado. Sem isso, uma foto de 2 MB seria impossível de gravar num
volume de 1 MB, e trocar de volume não resolveria nada.

### Abertura preguiçosa

Uma tabela de 999 volumes não pode manter 999 descritores de arquivo abertos.
Os volumes são abertos sob demanda e mantidos num cache LRU de 64 posições — o
mesmo *lazy open* que o `FileManager` do Clarion(R) faz.

### Como reabrir sem saber a geometria

A paginação mora dentro do esquema, que mora dentro do primeiro volume — e a
largura do sufixo faz parte dela. Abrir uma tabela, então, começa varrendo o
diretório atrás de `nome.reg` ou do menor `nome_<dígitos>.reg`, e só depois de
ler o esquema é que o conjunto de volumes é montado.

---

## 6. Hierarquia: database, schema e tabela

```
base/
└── Z/                        database Z
    ├── cadastroClientes.reg  ┐
    ├── cadastroClientes.ndx  ├ tabelas da raiz (sem schema)
    ├── ...                   ┘
    ├── X/                    schema X
    │   └── pedidos.reg ...   tabelas do schema X
    └── Y/                    schema Y
        └── notas.reg ...     tabelas do schema Y
```

A regra é estrutural, sem arquivo de marcação: um diretório dentro da base é um
database; um diretório dentro de um database é um schema; um arquivo `.reg` é
uma tabela. Tabelas soltas na raiz do database são as "tabelas raiz" —
equivalentes ao `public` do Postgres ou ao `dbo` do SQL Server.

O nome qualificado é `schema.tabela`, ou só `tabela` na raiz — o mesmo formato
que o catálogo do FraseSQL espera. Duas tabelas de mesmo nome em schemas
diferentes não colidem.

Nomes de database, schema e tabela são validados: nada de `..`, barra,
contrabarra, dois-pontos, curinga ou caractere de controle.

---

## 7. Reindex

Recriar o `.ndx` inteiro a partir do `.reg`: varre os registros ativos na ordem
de digitação, recodifica as chaves e reconstrói cada B+tree do zero. Resolve
três coisas de uma vez:

- `.ndx` corrompido, apagado ou perdido numa cópia incompleta;
- árvore subocupada depois de muitas exclusões (a remoção não rebalanceia);
- índice novo acrescentado a uma tabela que já tem dados.

Como a varredura é na ordem de digitação, a árvore sai com os rowids em ordem
crescente dentro de cada chave.

---

## 8. Identificadores: `Uuid`, `Uuid256` e `Sequence`

Três tipos de largura fixa que cabem inteiros no slot — nada vai para o `.bin`.

| Tipo | Bytes | O que é |
|---|---:|---|
| `Uuid` | 16 | UUID de 128 bits do RFC 9562, guardado em big-endian (a mesma ordem em que se escreve) |
| `Uuid256` | 32 | identificador de 256 bits. **Não é um UUID** — o RFC só define 128. Existe porque um SHA-256 cabe exatamente |
| `Sequence` | 8 | contador crescente da tabela, atribuído na inserção quando o valor chega nulo |

### Por que o v7 e não o v4

O `.ndx` compara chaves byte a byte, e os bytes de um UUID estão em big-endian:
comparar bytes é comparar o número. Nos primeiros 48 bits de um **v7** está o
relógio em milissegundos — então **a ordem do índice é a ordem de criação**.

Isso não é detalhe estético. Chave aleatória (v4) manda cada inserção para uma
folha diferente da B+tree: toda gravação suja uma página nova, e quanto maior a
tabela, mais longe uma da outra. Chave crescente cai sempre na folha mais à
direita, que já está na memória. É a diferença entre semear a árvore inteira e
anexar no fim dela — e a bancada mostra que é exatamente aí que a inserção do
motor sofre.

Use `v4` quando o id **não pode** revelar quando foi criado. Nos outros casos,
`v7`.

### Monotonia dentro do mesmo milissegundo

Dois v7 gerados no mesmo milissegundo sairiam fora de ordem se dependessem só
do relógio. Por isso os 12 bits de `rand_a` viram um contador (método 1 da
seção 6.2 do RFC 9562): nasce sorteado na metade de baixo da faixa a cada
milissegundo novo e soma 1 a cada id seguinte. Estourou, o relógio anda 1 ms
para frente em vez de repetir. O gerador nunca devolve valor menor ou igual ao
anterior, nem entre threads.

### A sequência

Diferente do rowid em uma coisa que importa: **o rowid é a posição física** do
registro e não se escolhe; a sequência é um valor de dado — pode nascer onde se
quiser, ser gravada à mão e continuar de onde parou.

- Valor nulo na inserção → recebe o próximo número.
- Valor escrito à mão → **empurra o contador** para depois dele, senão a
  próxima numeração automática passaria por cima do que já existe.
- Valor nulo numa **alteração** → mantém o número que a linha já tinha. A
  sequência identifica a linha; renumerar trocaria a identidade dela.
- Excluir **não devolve** o número, pela mesma razão que o `.reg` não
  reaproveita slot.

O contador mora nos bytes 36..44 do cabeçalho do volume 1 e vai ao disco no
`sincronizar`, junto com os demais contadores. **Se a máquina cair antes disso,
o contador volta atrás e números já gravados podem repetir** — por isso a
sequência sozinha não é chave única. Quem precisa de unicidade declara um
índice `unico` sobre ela, e aí o próprio índice recusa a repetição.

Uma sequência por tabela: duas dividiriam o mesmo contador, o que só pareceria
defeito. O esquema recusa na criação.

---

## 9. Limites

| Limite | Valor |
|---|---|
| Colunas por tabela | 65 535 |
| Tamanho de `Str(n)` | 65 535 bytes |
| Sequência por tabela | 1 (o contador do cabeçalho é único) |
| Valor máximo de `Sequence` | 2⁶⁴ − 1 |
| Precisão de `Decimal` | 38 dígitos |
| Conteúdo de um bloco `.bin` / `.memo` | 4 GiB |
| Chave de índice | `page_size / 4 - 8` bytes (1016 numa página de 4096) |
| Índices por tabela | o diretório precisa caber na página 0 |
| Registros por tabela | `registros_por_arquivo × max_arquivos`, ou 2⁶⁴ − 1 sem paginação |
| Volumes por arquivo | 65.535 (limite do ponteiro externo) |
| Offset dentro de um volume externo | 256 TB (48 bits) |

## 10. O que este formato ainda não faz

Documentado aqui para não haver surpresa:

- **Sem transações.** `inserir` desfaz o que gravou se um índice falhar, mas
  não há *journal* nem `commit`/`rollback` de várias operações.
- **Sem concorrência.** Um processo por tabela; não há travas de arquivo nem de
  registro.
- **Sem compactação implementada.** O formato prevê e mede o espaço morto, mas
  o comando ainda não foi escrito. O reindex já existe e cobre a parte do
  índice.
- **O `.log` não guarda o conteúdo anterior**, só o evento. Serve de auditoria,
  ainda não de journal para desfazer.
- **Sem camada SQL.** Esta é a camada de armazenamento; o parser e o executor
  entram por cima.
