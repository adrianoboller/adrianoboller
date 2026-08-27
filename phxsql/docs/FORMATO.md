# Formato de arquivo do PhxSql

Uma tabela de dados do PhxSql é composta por quatro arquivos físicos que
compartilham o mesmo nome-base:

```
cadastroClientes.reg   +  .ndx  +  .bin  +  .memo   =   cadastroClientes
```

| Arquivo | Papel | Assinatura |
|---|---|---|
| `.reg` | Registros, na ordem de digitação | `PHXREG\0\0` |
| `.ndx` | Índices (B+tree), todos no mesmo arquivo | `PHXNDX\0\0` |
| `.bin` | Binários (imagens, anexos) | `PHXBIN\0\0` |
| `.memo` | Textos longos | `PHXMEMO\0` |

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
| 8 | 2 | versão do formato (1) |
| 10 | 2 | tamanho do cabeçalho (128) |
| 12 | 4 | flags (reservado) |
| 16 | 4 | `slot_size` |
| 20 | 8 | `slot_count` — slots alocados, inclusive excluídos |
| 28 | 8 | `live_count` — registros ativos |
| 36 | 8 | reservado (cabeça da lista de livres, versão futura) |
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
quarteto de arquivos basta para reabrir os dados, sem dicionário externo.

### Slot

| Off | Tam | Campo |
|----:|----:|---|
| 0 | 1 | status: 0 = livre, 1 = ativo |
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

### Ordem de digitação

Registros são **sempre anexados no fim**. Excluir marca o slot como livre, mas
o slot **não é reaproveitado**. Essa é uma escolha deliberada: reaproveitar
manteria o arquivo compacto, mas quebraria a garantia de que percorrer o `.reg`
do início ao fim devolve os registros na ordem em que foram digitados. O espaço
de slots excluídos só volta com uma compactação explícita, que renumera os
rowids e reconstrói os índices.

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

- **NULL** ordena antes de qualquer valor (byte de presença 0x00).
- **DESC** inverte todos os bytes do componente, o que inverte a ordem e joga
  NULL para o fim.
- **NOCASE** aplica *fold* ASCII para maiúsculas antes de comparar, preservando
  o comprimento em bytes (mesma semântica do atributo NOCASE do Clarion).

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
| 8 | 2 | versão do formato (1) |
| 10 | 2 | tamanho do cabeçalho (64) |
| 12 | 4 | flags |
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
| 0 | 8 | offset do bloco no arquivo externo |
| 8 | 4 | tamanho do conteúdo |
| 12 | 4 | CRC-32 do conteúdo |

Conteúdo vazio não consome bloco: o ponteiro fica zerado. O CRC aparece nos
dois lugares (ponteiro e bloco) de propósito — a leitura confere os dois, então
um ponteiro apontando para o bloco errado é detectado, não só um bloco
corrompido.

---

## 4. Limites

| Limite | Valor |
|---|---|
| Colunas por tabela | 65 535 |
| Tamanho de `Str(n)` | 65 535 bytes |
| Precisão de `Decimal` | 38 dígitos |
| Conteúdo de um bloco `.bin` / `.memo` | 4 GiB |
| Chave de índice | `page_size / 4 - 8` bytes (1016 numa página de 4096) |
| Índices por tabela | o diretório precisa caber na página 0 |
| Registros por tabela | 2⁶⁴ − 1 rowids |

## 5. O que este formato ainda não faz

Documentado aqui para não haver surpresa:

- **Sem transações.** `inserir` desfaz o que gravou se um índice falhar, mas
  não há *journal* nem `commit`/`rollback` de várias operações.
- **Sem concorrência.** Um processo por tabela; não há travas de arquivo nem de
  registro.
- **Sem compactação implementada.** O formato prevê e mede o espaço morto, e a
  API de leitura de blocos vivos existe, mas o comando ainda não foi escrito.
- **Sem camada SQL.** Esta é a camada de armazenamento; o parser e o executor
  entram por cima.
