# Update format specification
# 27/08 18:33

p='docs/FORMATO.md'
s=open(p).read()

s=s.replace('''Uma tabela de dados do PhxSql é composta por quatro arquivos físicos que
compartilham o mesmo nome-base:

```
cadastroClientes.reg   +  .ndx  +  .bin  +  .memo   =   cadastroClientes
```

| Arquivo | Papel | Assinatura |
|---|---|---|
| `.reg` | Registros, na ordem de digitação | `PHXREG\\0\\0` |
| `.ndx` | Índices (B+tree), todos no mesmo arquivo | `PHXNDX\\0\\0` |
| `.bin` | Binários (imagens, anexos) | `PHXBIN\\0\\0` |
| `.memo` | Textos longos | `PHXMEMO\\0` |''','''Uma tabela de dados do PhxSql é composta por cinco arquivos físicos que
compartilham o mesmo nome-base:

```
cadastroClientes.reg  +  .ndx  +  .bin  +  .memo  +  .log  =  cadastroClientes
```

| Arquivo | Papel | Assinatura | Pagina? |
|---|---|---|---|
| `.reg` | Registros, na ordem de digitação | `PHXREG\\0\\0` | sim |
| `.ndx` | Índices (B+tree), todos no mesmo arquivo | `PHXNDX\\0\\0` | **não** |
| `.bin` | Binários (imagens, anexos) | `PHXBIN\\0\\0` | sim |
| `.memo` | Textos longos | `PHXMEMO\\0` | sim |
| `.log` | Diário de inclusões, alterações e exclusões | `PHXLOG\\0\\0` | sim |

Uma tabela grande se parte em volumes numerados — `cadastroClientes_001.reg`,
`_002.reg`, … — segundo os parâmetros do `CREATE TABLE`. Ver a seção 5.''')

s=s.replace('''| 0 | 8 | assinatura `PHXREG\\0\\0` |
| 8 | 2 | versão do formato (1) |
| 10 | 2 | tamanho do cabeçalho (128) |
| 12 | 4 | flags (reservado) |''','''| 0 | 8 | assinatura `PHXREG\\0\\0` |
| 8 | 2 | versão do formato (2) |
| 10 | 2 | tamanho do cabeçalho (128) |
| 12 | 4 | número do volume |''')

s=s.replace('''| 20 | 8 | `slot_count` — slots alocados, inclusive excluídos |
| 28 | 8 | `live_count` — registros ativos |''','''| 20 | 8 | `slot_count` — slots alocados, inclusive excluídos (só o volume 1) |
| 28 | 8 | `live_count` — registros ativos (só o volume 1) |''')

s=s.replace('''Logo após o cabeçalho vem o **esquema serializado** (`schema_len` bytes), e
`data_offset` é o próximo múltiplo de 64. A tabela é auto-descritiva: o
quarteto de arquivos basta para reabrir os dados, sem dicionário externo.''','''Logo após o cabeçalho vem o **esquema serializado** (`schema_len` bytes), e
`data_offset` é o próximo múltiplo de 64. A tabela é auto-descritiva: o
conjunto de arquivos basta para reabrir os dados, sem dicionário externo.

**Todo volume carrega o cabeçalho completo com o esquema**, então qualquer um
deles se descreve sozinho — se o volume 1 se perder, os outros ainda sabem
dizer o que são. Apenas o volume 1 tem contadores autoritativos da tabela
inteira.''')

s=s.replace('''### Ordem de digitação

Registros são **sempre anexados no fim**. Excluir marca o slot como livre, mas
o slot **não é reaproveitado**. Essa é uma escolha deliberada: reaproveitar
manteria o arquivo compacto, mas quebraria a garantia de que percorrer o `.reg`
do início ao fim devolve os registros na ordem em que foram digitados. O espaço
de slots excluídos só volta com uma compactação explícita, que renumera os
rowids e reconstrói os índices.''','''### Ordem de digitação

Registros são **sempre anexados no fim**. Excluir marca o slot como livre, mas
o slot **não é reaproveitado**. Essa é uma escolha deliberada: reaproveitar
manteria o arquivo compacto, mas quebraria a garantia de que percorrer o `.reg`
do início ao fim devolve os registros na ordem em que foram digitados. O espaço
de slots excluídos só volta com uma compactação explícita, que renumera os
rowids e reconstrói os índices.

Com paginação a garantia continua valendo, porque o volume N+1 vem sempre
depois do N e dentro de cada volume os slots seguem em ordem de inserção.''')

# Ponteiro novo
s=s.replace('''Colunas `Bin` e `Memo` ocupam **16 bytes fixos** dentro do slot do `.reg`:

| Off | Tam | Campo |
|----:|----:|---|
| 0 | 8 | offset do bloco no arquivo externo |
| 8 | 4 | tamanho do conteúdo |
| 12 | 4 | CRC-32 do conteúdo |''','''Colunas `Bin` e `Memo` ocupam **16 bytes fixos** dentro do slot do `.reg`:

| Off | Tam | Campo |
|----:|----:|---|
| 0 | 6 | offset do bloco dentro do volume (u48 — 256 TB por volume) |
| 6 | 2 | número do volume (u16 — 65.535 volumes) |
| 8 | 4 | tamanho do conteúdo |
| 12 | 4 | CRC-32 do conteúdo |

O offset ocupa 48 bits em vez de 64 justamente para liberar os dois bytes do
volume sem crescer o ponteiro. 256 TB por volume é folga de sobra.''')

s=s.replace('''| 0 | 8 | assinatura |
| 8 | 2 | versão do formato (1) |
| 10 | 2 | tamanho do cabeçalho (64) |
| 12 | 4 | flags |
| 16 | 8 | `fim` — ponto de anexação |''','''| 0 | 8 | assinatura |
| 8 | 2 | versão do formato (2) |
| 10 | 2 | tamanho do cabeçalho (64) |
| 12 | 4 | número do volume |
| 16 | 8 | `fim` — ponto de anexação |''')

s=s.replace('''---

## 4. Limites''','''---

## 4. `.log` — o diário da tabela

Toda inclusão, alteração e exclusão é registrada com data e hora. O arquivo é
append-only e sem índice: é um diário, não uma tabela.

### Cabeçalho (64 bytes)

| Off | Tam | Campo |
|----:|----:|---|
| 0 | 8 | assinatura `PHXLOG\\0\\0` |
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
que o TopSpeed dava.

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
mesmo *lazy open* que o `FileManager` do Clarion faz.

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

## 8. Limites''')

s=s.replace('''| Registros por tabela | 2⁶⁴ − 1 rowids |''','''| Registros por tabela | `registros_por_arquivo × max_arquivos`, ou 2⁶⁴ − 1 sem paginação |
| Volumes por arquivo | 65.535 (limite do ponteiro externo) |
| Offset dentro de um volume externo | 256 TB (48 bits) |''')

s=s.replace('''## 5. O que este formato ainda não faz''','''## 9. O que este formato ainda não faz''')
s=s.replace('''- **Sem compactação implementada.** O formato prevê e mede o espaço morto, e a
  API de leitura de blocos vivos existe, mas o comando ainda não foi escrito.''','''- **Sem compactação implementada.** O formato prevê e mede o espaço morto, mas
  o comando ainda não foi escrito. O reindex já existe e cobre a parte do
  índice.
- **O `.log` não guarda o conteúdo anterior**, só o evento. Serve de auditoria,
  ainda não de journal para desfazer.''')
open(p,'w').write(s)
print("FORMATO.md atualizado")
