# Formato de arquivo do PhxSql

Uma tabela de dados do PhxSql é composta por sete arquivos físicos que
compartilham o mesmo nome-base — mais o espelho e o descritor:

```
cadastroClientes.reg + .ndx + .bin + .memo + .log + .trash + .reason
                     ( +  .bkp, o espelho, quando ligado )
                     ( +  .pag, o descritor de partição )
```

| Arquivo | Papel | Assinatura | Pagina? | Quem lê |
|---|---|---|---|---|
| `.reg` | Registros, na ordem de digitação | `PHXREG\0\0` | sim | quem tem `ler` |
| `.ndx` | Índices (B+tree), todos no mesmo arquivo | `PHXNDX\0\0` | **não** | quem tem `ler` |
| `.bin` | Binários (imagens, anexos) | `PHXBIN\0\0` | sim | quem tem `ler` |
| `.memo` | Textos longos | `PHXMEMO\0` | sim | quem tem `ler` |
| `.log` | Diário de inclusões, alterações e exclusões | `PHXLOG\0\0` | sim | quem tem `diario` |
| `.trash` | Linhas que saíram do `.reg`, inteiras | `PHXTRH\0\0` | sim | **só `administrar`** |
| `.reason` | Por que cada linha foi excluída, e por quem | `PHXRSN\0\0` | sim | **só `administrar`** |
| `.pag` | Descritor de partição, em JSON | — (texto) | não | quem lê a tabela |

Os três últimos são **os arquivos do administrador**, e a razão está no que
cada um guarda. O `.trash` guarda o dado que alguém mandou apagar — quem só
tem `ler` perdeu o direito àquela linha no instante em que ela foi excluída, e
a lixeira devolveria o direito por outra porta. O `.reason` costuma ser ainda
mais revelador que o registro: *fraude*, *pedido de remoção do titular*,
*duplicidade com o contrato X*. O `.log` tem permissão própria (`diario`), que
só um administrador concede.

E um **oitavo arquivo opcional**, que só existe quando `espelho` está ligado no
`config.json`:

| Arquivo | Papel | Assinatura | Pagina? |
|---|---|---|---|
| `.bkp` | Espelho byte a byte do `.reg`, volume por volume | igual à do `.reg` | sim, junto |

O `.bkp` **não tem formato próprio**: ele é o `.reg`, escrito duas vezes. O
mesmo slot vai para os dois arquivos, no mesmo *offset*, no mesmo instante — e
por isso todo volume do `.reg` tem um volume irmão do `.bkp`.

Ele é lido **só quando o slot principal falha**: o CRC não bate, ou o byte de
status não é nem `0` (livre) nem `1` (ativo). Nesse caso a leitura busca o
mesmo *offset* no espelho, confere o CRC dele, e devolve a cópia boa. O
`reparar` faz a varredura completa nos dois sentidos: onde o principal quebrou
e o espelho está bom, o principal é reescrito; onde o principal está bom e o
espelho quebrou, o espelho é reescrito.

Custa uma escrita a mais por gravação e o dobro do espaço do `.reg`. Protege
contra o dado ficar **ruim** — bit trocado, escrita cortada, setor com
defeito. **Não** protege contra o disco morrer: os dois arquivos moram no mesmo
lugar.

Uma tabela grande se parte em volumes numerados — `cadastroClientes_001.reg`,
`_002.reg`, … — segundo os parâmetros do `CREATE TABLE`. Ver a seção 7.

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
| 8 | 2 | versão do formato (4) |
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
| 76 | 8 | `primeiro_rowid` — o primeiro rowid deste volume (só na partição por período) |
| 84 | 8 | `chave_periodo` — o período em que este volume abriu (só na partição por período) |
| 92 | 8 | `proximo_rownum` — próximo valor da coluna de sistema `rownum` (só o volume 1) |
| 100 | 8 | `slots_no_balde` — slots já usados **neste** volume (só na partição alfanumérica) |
| 108 | 8 | `marcadas` — linhas vivas marcadas como excluídas (só o volume 1) |
| 116 | 8 | reservado |
| 124 | 4 | CRC-32 dos bytes 0..124 |

Logo após o cabeçalho vem o **esquema serializado** (`schema_len` bytes), e
`data_offset` é o próximo múltiplo de 64. A tabela é auto-descritiva: o
conjunto de arquivos basta para reabrir os dados, sem dicionário externo.

### O bloco de esquema (`PSCH`, versão 5)

O bloco começa com `PSCH` e a versão. A **3** acrescentou os metadados de
coluna, o marcador de chave primária e o modo de partição. A **4** acrescentou
a coluna de sistema `softdeleted` e um byte no fim, com o sinal de *motivo
obrigatório*. A **5** acrescentou a coluna de sistema `rownum`. A leitura ainda
aceita a 2: tabela gravada antes abre normalmente, ganha um `id` v7 sorteado na
hora e os textos vazios. **Escrever, só na 5.**

Por coluna, nesta ordem:

| Campo | Tam | O que é |
|---|---:|---|
| `nome` | 2 + n | o nome no disco |
| tipo | 4 | tag + dois parâmetros (largura do `Str`, precisão/escala do `Decimal`) |
| `nullable` | 1 | aceita nulo |
| `id` | 16 | **UUID v7 da coluna**, sorteado na criação e nunca reaproveitado |
| `caption` | 2 + n | rótulo de tela; vazio significa "use o nome" |
| `descricao` | 2 + n | para que a coluna serve |
| `mascara` | 2 + n | PICTURE do Clarion(R): `@N-11.2`, `@D6`, `@P###-####P` |

O `id` existe para que **renomear a coluna não quebre nada**: uma tela, um
relatório ou um mapeamento apontam para ele, e renomear troca só o `nome`. É a
mesma razão de o esquema morar no `.reg` — um dicionário externo se perde, se
desatualiza, e obriga quem copia os arquivos da tabela a copiar mais um.

Por índice, os sinalizadores viraram um byte com dois bits: **único** no bit 0
e **primário** no bit 1.

### A coluna de sistema `softdeleted`

Toda tabela criada a partir da v4 ganha, **no fim da lista**, uma coluna `Bool`
não nula chamada `softdeleted`. Ela marca a linha como excluída sem apagar
nada: a linha some das listas e continua inteira no `.reg`, e `restaurar`
desfaz.

No fim, e não no começo, por uma razão de formato: assim os *offsets* das
colunas do usuário não mudam de lugar quando ela entra, e quem monta a linha
posicionalmente pode continuar mandando só as colunas que declarou — `inserir`
com N−1 valores preenche `false`, e `atualizar` com N−1 **mantém o que a linha
já tinha**.

A coluna entra na **criação** da tabela. Ler o esquema do disco não acrescenta
nada: a lista de colunas gravada é a verdade inteira. Se a leitura
acrescentasse a coluna, cada linha de uma tabela v3 passaria a ser lida com os
*offsets* deslocados — **silenciosamente**, porque o CRC do slot continuaria
batendo: os bytes seriam os mesmos, só a interpretação mudaria.

Uma tabela anterior à v4 continua legível exatamente como está. Ela só não tem
exclusão suave, e a mensagem de erro diz isso em vez de ler lixo.

Declarar `softdeleted` à mão é permitido — quem recria uma tabela precisa —,
mas só como `Bool` não nula. Com outro tipo, o esquema é recusado: seria uma
coluna comum com nome reservado, e o motor passaria a marcar exclusão num
campo que o usuário lê como texto. Nulo também é recusado: seria um terceiro
estado entre excluída e não excluída.

### A coluna de sistema `rownum`

`UInt8` não nula, e ela entra **depois** da `softdeleted` — coluna de sistema
nova sempre no fim, senão uma tabela gravada na versão anterior teria os
*offsets* deslocados ao ser relida.

É o **número de ordem de chegada** da linha. O motor preenche; não se escreve à
mão e não se ajusta — um valor escolhido seria uma ordem inventada. Nunca
reaproveita número, nem depois de exclusão: se reaproveitasse, uma linha nova
apareceria **atrás** de um cursor parado numa página, e a paginação passaria a
pular registro sem avisar. Alterar a linha não renumera.

O contador vive nos bytes 92..100 do cabeçalho do volume 1 e vai ao disco no
`sincronizar`, como os outros.

**Por que ela existe, se já há o `rowid`.** O `rowid` é a *posição física*.
Enquanto o volume sai de divisão, posição e ordem de chegada são a mesma coisa
e o rowid serve de cursor sozinho. Na **partição alfanumérica** não são: a
linha vai para o volume da letra dela, e duas linhas digitadas em seguida caem
em arquivos diferentes com rowids que não se comparam. O `rownum` é o que
sobra de monotônico.

**Ela não é `Sequence`.** Uma tabela só pode ter uma coluna `Sequence` — o
contador do `.reg` é único —, e reservar essa única vaga para o motor tiraria
do usuário um tipo que é dele. O `rownum` tem contador próprio.

**Como ela pagina sem índice.** O `rownum` cresce com o `rowid`, porque o
`.reg` guarda as linhas na ordem de chegada. Uma sequência crescente num
arquivo de acesso aleatório se procura por **bissecção**: achar a linha de
número 500.000 num milhão custa vinte leituras, sem índice nenhum a manter. É
o mesmo motivo de o endereço sair de uma conta — a ordem lógica é a ordem
física.

**A exceção que a partição alfanumérica cria.** Ali o `rownum` **não** cresce
com o rowid: a Silva digitada primeiro mora no `_S`, com rowid alto, e a Alves
digitada depois mora no `_A`, com rowid 1 — número de ordem 1 num rowid maior
que o do número 2. Bissetar uma sequência que não está ordenada devolveria a
linha errada *em silêncio*, que é pior que devolver devagar; nesse modo o motor
varre. É a razão de `Table::posicao_e_rownum` recusar a partição por letra.

### `marcadas`, e a pergunta que ela responde em tempo constante

O cabeçalho do volume 1 guarda quantas linhas vivas estão **marcadas** como
excluídas. É um contador em cache, como o `live_count` ao lado dele, e existe
para duas contas que sem ele custariam a tabela inteira:

1. **Quantas linhas esta visão enxerga.** `registros − marcadas` são as ativas,
   `marcadas` são as excluídas, `registros` são todas. Era por não existir esse
   número que a resposta do `varrer` tinha deixado de trazer o total — mostrar
   «página 3 de 40» custava percorrer tudo.
2. **A posição de uma linha na lista é o `rownum` dela?** Se ninguém apagou de
   vez (`proximo_rownum − 1 == live_count`) e ninguém marcou (`marcadas == 0`),
   sim — e aí pular para a posição 500.000 é uma bissecção de vinte leituras em
   vez de meio milhão de passos.

Contador em cache diverge se algum caminho esquecer de mexer nele, e aqui a
divergência mandaria a tela para a linha errada sem avisar. Duas defesas: o
contador vai ao disco **na mesma operação** que o muda (128 bytes a mais, e não
no `sincronizar`, senão uma queda o faria voltar atrás), e `verificar` o
**reconta varrendo** em vez de acreditar nele — é o mesmo caminho que o reparo
chama.

### Chave primária, chave estrangeira, chave composta

Só um índice pode ser primário, ele é sempre único, e nenhuma coluna dele pode
aceitar nulo — uma identidade nula não identifica. As três conferências
acontecem no `Schema::new`.

O papel de uma coluna nas chaves **não é gravado na coluna**: sai dos índices e
das chaves estrangeiras, que são a verdade.

| Marca | De onde sai |
|---|---|
| primária | a coluna aparece no índice marcado como primário |
| estrangeira | a coluna aparece em alguma chave estrangeira |
| composta | a chave de que ela participa tem mais de uma coluna |

Guardar "é primária" no próprio campo criaria uma segunda verdade ao lado do
índice, e as duas divergiriam no primeiro `ALTER`.

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
| 8 | 2 | versão do formato (2) |
| 10 | 2 | tamanho do cabeçalho (64) |
| 12 | 4 | número do volume |
| 16 | 8 | eventos neste volume |
| 24 | 8 | `fim` — ponto de anexação |
| 32 | 8 | alterado em |
| 56 | 4 | CRC-32 dos bytes 0..56 |

### Evento: 44 bytes de cabeçalho, e talvez um corpo

| Off | Tam | Campo |
|----:|----:|---|
| 0 | 8 | carimbo — milissegundos desde 1970-01-01T00:00:00Z |
| 8 | 1 | operação: 1 = inclusão, 2 = alteração, 3 = exclusão |
| 9 | 1 | flags — bit 0: tem imagem |
| 10 | 2 | reservado |
| 12 | 8 | rowid afetado |
| 20 | 8 | versão do registro depois da operação |
| 28 | 4 | usuário (0 = não informado) |
| 32 | 4 | tamanho da imagem (0 = sem imagem) |
| 36 | 4 | CRC-32 dos bytes 0..36 **e da imagem** |
| 40 | 4 | reservado |
| 44 | N | imagem da linha |

O carimbo é em **milissegundos**, não segundos, para que operações no mesmo
segundo continuem ordenáveis. Uma operação recusada — chave duplicada, tabela
cheia, coluna obrigatória em branco — **não** gera evento: o diário registra o
que aconteceu, não o que foi tentado.

### A imagem da linha

Sem ela o evento diz que o rowid 42 mudou; não diz **para quê**. Isso basta para
auditoria e não basta para replicar. Fica atrás de um interruptor no
`config.json` (`replicacao.imagem_da_linha`), e vem ligada num servidor com
`papel: source`.

```
imagem = [tam_payload u32][payload]
         [qtd_externos u16]
         [ (coluna u16, tamanho u32, conteúdo) ... ]
```

O payload vai **cru**, do jeito que está no `.reg` — sem reencodar, sem passar
por texto, sem perder precisão de decimal nem de data. E o **conteúdo** dos
externos vai junto, não os ponteiros: os offsets do `.bin` e do `.memo` são
desta máquina e apontariam para qualquer coisa na outra. É a mesma razão pela
qual o `.trash` guarda conteúdo.

Exclusão não leva imagem: o rowid basta.

O CRC cobrir a imagem, e não só o cabeçalho, é o detalhe que importa: a imagem é
o que a réplica grava **como dado**. Um byte trocado ali entraria na réplica sem
ninguém notar.

Medido, mesma tabela e mesmas 100.000 linhas:

| `imagem_da_linha` | linhas/s | bytes por evento |
|---|---:|---:|
| desligada | 21.740 | 44 |
| ligada | 19.531 | 223 |

### O que a largura variável custa

Até a versão 1 o evento N morava no offset `64 + N × 36`, e pular era uma conta.
Agora não é: chegar ao evento N é caminhar pelos anteriores lendo o tamanho de
cada um. O que salva a leitura é o `eventos neste volume` do cabeçalho — um
volume inteiro se pula sem abrir o arquivo.

---

## 5. `.trash` — a linha inteira, antes de sumir

Só quem tem `administrar` lê este arquivo.

### A ordem é o recurso

A linha é gravada aqui e **o arquivo é sincronizado** antes de o slot do `.reg`
ser liberado. Se a máquina cair no meio, o pior caso é a linha aparecer nos
dois lugares — o que se resolve olhando —, e **nunca em nenhum**. A ordem
inversa (liberar e depois guardar) tem uma janela em que o registro não existe
em lugar nenhum, e essa janela não tem conserto depois.

Entre perder e duplicar, o motor duplica.

### Por que não é um `.reg` paralelo

Um `.reg` guarda *payload* de largura fixa, e as colunas `Bin`/`Memo` moram
nele como **ponteiro** para o `.bin`/`.memo`. Copiar só o *payload* para um
`.reg` paralelo guardaria os ponteiros — que apontam para blocos que a própria
exclusão acabou de liberar, e que a próxima inserção pode reaproveitar. A foto
voltaria sendo a foto de outra linha.

Por isso o registro daqui é de **tamanho variável**: o *payload* byte a byte,
mais o **conteúdo** de cada coluna externa logo em seguida.

### Cabeçalho do arquivo (64 bytes)

Mesmo desenho do `.log`: assinatura, versão, volume, quantidade, fim e CRC-32.

### Registro (56 bytes de cabeçalho + payload + externos)

| Offset | Bytes | Campo |
|---:|---:|---|
| 0 | 8 | carimbo em milissegundos desde 1970-01-01T00:00:00Z |
| 8 | 1 | *flags* (reservado) |
| 9 | 1 | quantas colunas externas a linha tem |
| 10 | 2 | *reservado* |
| 12 | 8 | rowid que a linha tinha |
| 20 | 4 | usuário que excluiu (0 = não informado) |
| 24 | 4 | tamanho do *payload* |
| 28 | 16 | UUID **v7** deste descarte |
| 44 | 4 | tamanho total do registro |
| 48 | 4 | *reservado* |
| 52 | 4 | CRC-32 de tudo, menos estes 4 bytes |
| 56 | n | o *payload* do slot, byte a byte como estava no `.reg` |
| … | … | por externo: `(coluna u16)(tamanho u32)(bytes)` |

O **tamanho total** está no cabeçalho de propósito: quem percorre o arquivo
avança por ele sem somar os externos um a um, e um registro que se declara
maior que o volume é recusado em vez de arrastar a leitura para dentro do
registro seguinte.

O CRC cobre o *payload* e os anexos, e não só o cabeçalho: o `.trash` só vale
como prova de que a linha era assim se adulterar o conteúdo for detectado.

O rowid guardado é **memória de onde a linha estava**, não promessa de para
onde ela volta: o `.reg` não reaproveita slot, nem por restauração.

### Esvaziar

`esvaziar_lixeira` apaga os volumes e recomeça do volume 1. Daqui não volta —
e por isso o expurgo é registrado no `.reason` **antes** de o dado sair, e a
operação exige motivo escrito mesmo numa tabela que não exige motivo para
excluir.

---

## 6. `.reason` — por que cada linha foi excluída

Só quem tem `administrar` lê este arquivo.

O `.log` já diz que houve uma exclusão no rowid tal, no instante tal. O que ele
não diz — e não tem onde dizer, porque o evento dele tem 36 bytes fixos — é
**por quê**. Este arquivo guarda a frase, a identidade do registro e o usuário,
e **sobrevive ao registro**: a linha pode sumir do `.reg` e do `.trash`, e o
motivo continua aqui.

### Cabeçalho do arquivo (64 bytes)

Mesmo desenho do `.log`.

### Registro (48 bytes de cabeçalho + dois textos)

| Offset | Bytes | Campo |
|---:|---:|---|
| 0 | 8 | carimbo em milissegundos |
| 8 | 1 | tipo: `1` suave, `2` física, `3` restauração, `4` expurgo |
| 9 | 1 | *flags* (reservado) |
| 10 | 2 | tamanho do texto do motivo |
| 12 | 8 | rowid |
| 20 | 4 | usuário (0 = não informado) |
| 24 | 16 | UUID **v7** deste evento |
| 40 | 2 | tamanho do texto da identidade |
| 42 | 2 | *reservado* |
| 44 | 4 | CRC-32 do cabeçalho (menos estes 4 bytes) e dos dois textos |
| 48 | n | motivo, UTF-8 |
| … | m | identidade, UTF-8 |

O **UUID é v7 do próprio evento**: ele identifica *esta* exclusão, e como o v7
leva o relógio nos primeiros 48 bits, ordenar por ele é ordenar por quando
aconteceu.

A **identidade** é o valor que identifica a linha na tabela — a chave primária,
senão a primeira coluna `Uuid` ou `Sequence` —, já em texto. Está aqui porque
quem lê o motivo seis meses depois não tem mais o esquema daquela linha na
cabeça, e "rowid 4173" não diz nada.

O CRC cobre os dois textos. Se cobrisse só o cabeçalho, trocar *fraude* por
*engano* passaria sem ser notado — e o arquivo existe justamente para isso não
poder acontecer.

Tetos: **2000 bytes** de motivo e **512** de identidade, cortados no limite de
caractere para nunca gravar UTF-8 inválido.

### O motivo obrigatório

É uma escolha da tabela, gravada no esquema (v4) e feita na criação. Marcada,
o motor **recusa** qualquer exclusão sem uma frase escrita — antes de qualquer
gravação. Vale para tabela cujo apagamento alguém vai ter de justificar
depois; numa tabela de rascunho, obrigar só ensina todo mundo a digitar um
ponto.

---

## 7. Paginação de tabelas grandes

Definida no `CREATE TABLE` e gravada no esquema:

| Parâmetro | Significado |
|---|---|
| `registros_por_arquivo` | quantos registros cabem em cada volume do `.reg` |
| `max_arquivos` | quantos volumes a tabela pode ter |
| `digitos` | largura do sufixo, padrão 3 (`_001`) |
| `bytes_por_arquivo` | tamanho de cada volume dos arquivos externos |
| `modo` | **o que faz o volume cortar**: a contagem ou o calendário |

Capacidade da tabela = `registros_por_arquivo × max_arquivos`. Passar disso
devolve erro explícito "tabela cheia", em vez do estouro silencioso de 2 GB
que o TopSpeed(R) dava.

**Não existe "sem teto".** O sufixo tem largura fixa: com três dígitos o volume
1000 simplesmente não teria nome de arquivo. Teto omitido vira o maior que cabe
no sufixo — 999 com três dígitos.

### Três regras de corte

| `modo` | quando o volume corta | sufixo |
|---|---|---|
| `PorQuantidade` | a cada `registros_por_arquivo` linhas | `_001` |
| `PorPeriodo { coluna, periodo }` | quando o período da coluna de data vira — **ou** quando o volume enche | `_001` |
| `PorLetra { coluna }` | **não corta**: são 37 volumes fixos, e a linha vai para o da letra dela | `_A`, `_0`, `_Outros` |

O período é `Mensal`, `Bimestral`, `Semestral` ou `Anual`, e os blocos sempre
começam em janeiro: bimestre é jan-fev, mar-abr, …; semestre é jan-jun e
jul-dez. Não há bimestre a começar em fevereiro.

A coluna do período tem de ser `Date` ou `DateTime` **e obrigatória** — sem
data não há período em que a linha caiba. As duas conferências acontecem na
criação do esquema, não na primeira gravação: um esquema que só quebra ao
inserir já nasceu quebrado.

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

### A partição alfanumérica

```
cadastroClientes_A.reg   cadastroClientes_0.reg
cadastroClientes_B.reg   cadastroClientes_1.reg      cadastroClientes_Outros.reg
…                        …
cadastroClientes_Z.reg   cadastroClientes_9.reg
```

São **37 volumes**, sempre os mesmos, nesta ordem: `A`..`Z` (1..26), `0`..`9`
(27..36) e `Outros` (37). A ordem é o formato — mudar a lista mudaria o
endereço de toda linha já gravada.

O volume sai da **primeira letra** de uma coluna de referência, e o valor dela
vira texto pela mesma função que o `.reason` usa — então número também
particiona, e `12345` cai no `_1`. Três decisões:

- **Acento cai na letra sem acento.** «Ávila» vai para o `_A`. Um balde `_Á`
  separado faria «Avila» e «Ávila» — a mesma pessoa digitada por duas pessoas —
  pararem em arquivos diferentes. A tabela de dobra é escrita à mão e cobre o
  português, o espanhol e o alemão; o que não cobrir cai em `Outros`, que é um
  lugar visível e não um erro escondido.
- **Vazio vai para `Outros`,** e não para `A`. Nome em branco não começa com A;
  juntá-lo com os Andrades esconderia o problema no maior balde.
- **Maiúscula e minúscula são o mesmo balde.** O contrário faria a mesma
  consulta achar ou não achar conforme como foi digitada.

A coluna de referência tem de ser **obrigatória** e **não externa**: o valor de
um `Bin`/`Memo` mora fora do slot, e o balde precisa ser decidido *antes* de a
linha ser gravada — ler o `.memo` para saber em que arquivo gravar seria a
ordem invertida.

#### O endereço continua sendo a mesma conta

O rowid é **atribuído** assim:

```
rowid = (balde - 1) × registros_por_arquivo + slot_no_balde
```

que é a inversa exata da conta de `localizar`. Por isso **nenhum caminho de
leitura mudou**: `localizar` continua devolvendo (volume, offset) por divisão,
o `.ndx` continua guardando rowid sem saber que balde existe, e o espelho
`.bkp` também não muda.

Cada volume guarda no próprio cabeçalho (bytes 100..108) quantos slots já usou.
Fica no volume, e não num arquivo separado, pela mesma razão da fronteira do
período: um arquivo separado seria uma segunda verdade.

O `slot_count` do volume 1 deixa de ser "quantos slots" e passa a ser a **marca
d'água** — o maior rowid que já existiu. Entre o fim do `_A` e o começo do `_B`
há `registros_por_arquivo` menos os usados de puro vazio, então a varredura anda
**por balde**: dentro do balde vai até `usados`, e no fim salta direto para o
início do próximo.

#### A ordem de digitação muda de campo

O que se perde é o rowid ser crescente na ordem de chegada: com os baldes, o
rowid diz em que **arquivo** a linha está, e não quando ela chegou. Dentro de
cada volume a ordem continua sendo a de digitação, e slot excluído continua sem
ser reaproveitado.

A ordem global fica na coluna de sistema `rownum`. **Sem ela este modo seria uma
quebra da regra da casa; com ela, é uma troca de campo.** A leitura sai em ordem
alfabética de balde — que é a ordem do arquivo.

#### O teto passa a ser por letra

`registros_por_arquivo` é o teto **de cada balde**, e não da tabela. Num
cadastro brasileiro o `_S` costuma ter dez vezes o `_K`: quem enche primeiro
derruba a inserção daquela letra com as outras 36 ainda com espaço, e o erro
diz **qual** balde encheu — «tabela cheia» com 3% de ocupação seria uma
mensagem que não ajuda ninguém.

#### O que é recusado

**Alterar a coluna de referência.** Mudar «Silva» para «Andrade» mudaria o
arquivo em que a linha mora, e com ele o rowid — que é a identidade dela em
todo índice. Mover não é opção; deixar a linha no balde errado também não,
porque aí o `_S` deixa de conter os S. Então a alteração é recusada, com o
caminho escrito na mensagem: exclua e insira de novo, e a linha nova nasce no
balde certo com outro rowid.

#### Só o `.reg` leva a letra

O `.bin`, o `.memo`, o `.log`, o `.trash` e o `.reason` rolam por **tamanho**, e
continuam com o sufixo numérico: um `Clientes_B.log` se leria como «o diário do
balde B», e o diário é da tabela inteira.

### Na partição por período, o endereço sai de uma busca binária

O volume não pode sair de divisão quando o corte depende do calendário: dois
meses rendem quantidades diferentes. Então cada volume grava no **próprio
cabeçalho** o rowid em que começou (offset 76) e o período em que abriu
(offset 84), e a tabela de fronteiras é remontada lendo esses cabeçalhos na
abertura — poucos bytes por volume, uma vez.

```
volume = a última fronteira com primeiro_rowid <= rowid   (busca binária)
slot   = rowid - primeiro_rowid[volume] + 1
offset = data_offset + (slot - 1) * slot_size
```

Volume é coisa que se conta em dezenas, não em milhares — cada um guarda
`registros_por_arquivo` linhas —, então a busca binária custa três ou quatro
comparações num vetor que já está na memória.

**Sem arquivo extra e sem bloco que cresce.** A alternativa seria guardar a
tabela de fronteiras num sexto arquivo, ou dentro do bloco de esquema — e o
bloco de esquema é seguido pelos dados, então crescer significaria empurrar a
tabela inteira. O cabeçalho de cada volume já existe e tem lugar sobrando.

### A linha atrasada não volta

Esta é a regra que define o desenho. Um lançamento de **janeiro digitado em
março** entra no volume de março, não no de janeiro.

Voltar significaria escrever no meio de um arquivo já fechado, quebrando ao
mesmo tempo as duas garantias que sustentam o formato: a ordem de digitação e o
endereço contíguo. Por isso o período de um volume é **o período em que ele
abriu**, e um volume pode conter linhas de períodos anteriores que chegaram
depois.

Quem quiser todos os lançamentos de janeiro usa o índice pela data — que é
exatamente para isso que ele existe. A partição por período é uma decisão de
*como o arquivo cresce*, não de *como o dado se consulta*.

Consequência prática: um volume recém-criado e ainda vazio não tem período. O
`.reg` grava `i64::MIN` como sentinela, e a primeira linha **adota** o volume
em vez de cortar um novo — senão a tabela nasceria com um arquivo vazio.

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

## 8. `.pag` — o descritor de partição

JSON indentado, ao lado dos outros arquivos da tabela. Diz **como a tabela está
partida**, que arquivo guarda o quê, e quanto tem em cada um:

```json
{
  "tabela": "clientes",
  "modo": "letra",
  "coluna_referencia": "nome",
  "registros_por_arquivo": 1000,
  "max_arquivos": 37,
  "endereco": "volume = (rowid - 1) / registros_por_arquivo + 1; …",
  "baldes": [
    { "balde": 1, "letra": "A", "arquivo": "clientes_A.reg",
      "existe": true, "registros": 2, "primeiro_rowid": 1 },
    …
  ]
}
```

Existe para quem está do **lado de fora** — uma camada SQL, um ETL, um
relatório, um `ls` — descobrir isso sem abrir o `.reg` e sem saber ler o bloco
de esquema. A conta do endereço vai escrita por extenso, porque é exatamente o
que quem lê precisa saber para não ter de adivinhar.

**Ele não é fonte de verdade**, e isso é o desenho e não um detalhe. O modo e a
coluna de referência estão no bloco de esquema dentro do `.reg`; quantas linhas
cada balde tem está no cabeçalho de cada volume. O `.pag` é **gerado** a partir
dos dois, na criação e a cada `sincronizar`.

A razão é a mesma que impede gravar «é chave primária» na coluna, e a mesma que
impede um arquivo `sequences` com uma segunda cópia dos contadores: uma segunda
cópia é uma segunda verdade, e as duas divergem no primeiro caminho que
esquecer de atualizar uma delas. Aqui a divergência seria pior que o normal —
o `.pag` diz em que **arquivo** a linha está.

Por isso o motor nunca **lê** este arquivo para decidir nada. Apagar o `.pag`
não quebra a tabela; regravar resolve.

---

## 9. Hierarquia: database, schema e tabela

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

## 10. Reindex

Recriar o `.ndx` inteiro a partir do `.reg`: varre os registros ativos na ordem
de digitação, recodifica as chaves e reconstrói cada B+tree do zero. Resolve
três coisas de uma vez:

- `.ndx` corrompido, apagado ou perdido numa cópia incompleta;
- árvore subocupada depois de muitas exclusões (a remoção não rebalanceia);
- índice novo acrescentado a uma tabela que já tem dados.

Como a varredura é na ordem de digitação, a árvore sai com os rowids em ordem
crescente dentro de cada chave.

---

## 11. Identificadores: `Uuid`, `Uuid256` e `Sequence`

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

## 12. Limites

| Limite | Valor |
|---|---|
| Colunas por tabela | 65 535 |
| Tamanho de `Str(n)` | 65 535 bytes |
| Sequência por tabela | 1 (o contador do cabeçalho é único) |
| Valor máximo de `Sequence` | 2⁶⁴ − 1 |
| Precisão de `Decimal` | 38 dígitos |
| Texto do motivo no `.reason` | 2 000 bytes |
| Identidade no `.reason` | 512 bytes |
| Colunas externas numa linha do `.trash` | 255 |
| Tamanho de um registro do `.trash` | 4 GiB |
| Volumes na partição alfanumérica | 37 (A-Z, 0-9, Outros) — fixo |
| Registros por balde | `registros_por_arquivo`, e é o teto **por letra** |
| Valor máximo de `rownum` | 2⁶⁴ − 1 |
| Conteúdo de um bloco `.bin` / `.memo` | 4 GiB |
| Chave de índice | `page_size / 4 - 8` bytes (1016 numa página de 4096) |
| Índices por tabela | o diretório precisa caber na página 0 |
| Registros por tabela | `registros_por_arquivo × max_arquivos`, ou 2⁶⁴ − 1 sem paginação |
| Volumes por arquivo | 65.535 (limite do ponteiro externo) |
| Offset dentro de um volume externo | 256 TB (48 bits) |

## 13. O que este formato ainda não faz

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
