# Update README and plan docs
# 27/08 18:34

p='README.md'
s=open(p).read()

s=s.replace('''Motor de dados em Rust no modelo de quatro arquivos do HFSQL: cada tabela
lógica é a soma de quatro arquivos físicos.

```
cadastroClientes.reg    registros, na ordem de digitação
cadastroClientes.ndx    índices (B+tree)
cadastroClientes.bin    binários
cadastroClientes.memo   textos longos

.reg + .ndx + .bin + .memo  =  cadastroClientes
```''','''Motor de dados em Rust no modelo de arquivos separados do HFSQL: cada tabela
lógica é a soma de cinco arquivos físicos.

```
cadastroClientes.reg    registros, na ordem de digitação
cadastroClientes.ndx    índices (B+tree)
cadastroClientes.bin    binários
cadastroClientes.memo   textos longos
cadastroClientes.log    diário de inclusões, alterações e exclusões

.reg + .ndx + .bin + .memo + .log  =  cadastroClientes
```

Tabelas grandes se partem em volumes numerados, e os databases se organizam em
schemas:

```
base/
└── Z/                          database Z
    ├── cadastroClientes_001.reg  ┐
    ├── cadastroClientes_002.reg  ├ tabela paginada, na raiz
    ├── cadastroClientes.ndx      ┘
    ├── X/  pedidos.reg ...       schema X
    └── Y/  notas.reg ...         schema Y
```''')

s=s.replace('''- **`bin`** e **`.memo`** tiram o conteúdo de tamanho livre de dentro do
  registro. Uma foto de 5 MB não infla o slot: o `.reg` guarda só um ponteiro
  de 16 bytes, e uma varredura da tabela não arrasta os blobs junto.''','''- **`.bin`** e **`.memo`** tiram o conteúdo de tamanho livre de dentro do
  registro. Uma foto de 5 MB não infla o slot: o `.reg` guarda só um ponteiro
  de 16 bytes, e uma varredura da tabela não arrasta os blobs junto.
- O **`.log`** é append-only e sem índice, então registrar uma operação custa
  36 bytes no fim de um arquivo — não atrapalha a escrita.''')

s=s.replace('''- **`.bin`** e **`.memo`** tiram o conteúdo de tamanho livre de dentro do
  registro. Uma foto de 5 MB não infla o slot: o `.reg` guarda só um ponteiro
  de 16 bytes, e uma varredura da tabela não arrasta os blobs junto.

E o `.reg` guarda a ordem''','''- **`.bin`** e **`.memo`** tiram o conteúdo de tamanho livre de dentro do
  registro. Uma foto de 5 MB não infla o slot: o `.reg` guarda só um ponteiro
  de 16 bytes, e uma varredura da tabela não arrasta os blobs junto.
- O **`.log`** é append-only e sem índice, então registrar uma operação custa
  36 bytes no fim de um arquivo — não atrapalha a escrita.

E o `.reg` guarda a ordem''')

s=s.replace('''O motor de armazenamento está completo e testado: **57 testes**, sem nenhuma
dependência externa (só a `std`), o que faz o projeto compilar offline.

| Peça | Situação |
|---|---|
| `.reg` — heap, CRC por registro, esquema embutido | pronto |
| `.ndx` — B+tree com divisão de páginas, chave composta, ASC/DESC/NOCASE/único | pronto |
| `.bin` / `.memo` — blocos com CRC, contabilidade de espaço morto | pronto |
| `Table` — valores, nulos, índices sincronizados, verificação de integridade | pronto |
| CLI `phxsql` | pronto |
| Compactação, transações, concorrência, camada SQL | pendente |''','''O motor de armazenamento está completo e testado: **104 testes**, sem nenhuma
dependência externa (só a `std`), o que faz o projeto compilar offline.

| Peça | Situação |
|---|---|
| `.reg` — heap, CRC por registro, esquema embutido | pronto |
| `.ndx` — B+tree com divisão de páginas, chave composta, ASC/DESC/NOCASE/único | pronto |
| `.bin` / `.memo` — blocos com CRC, contabilidade de espaço morto | pronto |
| `.log` — diário datado de inclusão, alteração e exclusão | pronto |
| Paginação em volumes `_001`, `_002`, … com abertura preguiçosa | pronto |
| Hierarquia database → schema → tabela | pronto |
| Chave estrangeira no esquema (CASCADE / RESTRICT / SET NULL) | pronto |
| Reindex — recria o `.ndx` do zero a partir do `.reg` | pronto |
| `Table` — valores, nulos, índices sincronizados, verificação de integridade | pronto |
| CLI `phxsql` | pronto |
| `config.json` e servidor TCP na porta 5000 | pendente |
| Servidor MCP | pendente |
| Camada SQL (tabela virtual via rusqlite, atrás de *feature*) | pendente |
| Driver ODBC de saída, depois cliente ODBC/OLE DB | pendente |
| Integração no FraseSQL como `engine = "phxsql"` | pendente |
| Compactação, transações, concorrência | pendente |

O roteiro completo, com as decisões tomadas e o que cada peça depende, está em
[`docs/PLANO.md`](docs/PLANO.md).''')

s=s.replace('''./target/release/phxsql demo /tmp/dados
./target/release/phxsql info /tmp/dados cadastroClientes
./target/release/phxsql listar /tmp/dados cadastroClientes --indice porCidadeLimite
./target/release/phxsql verificar /tmp/dados cadastroClientes
```''','''./target/release/phxsql demo /tmp/dados/Z --paginado
./target/release/phxsql info /tmp/dados/Z cadastroClientes
./target/release/phxsql listar /tmp/dados/Z cadastroClientes --indice porCidadeLimite
./target/release/phxsql log /tmp/dados/Z cadastroClientes
./target/release/phxsql reindex /tmp/dados/Z cadastroClientes
./target/release/phxsql verificar /tmp/dados/Z cadastroClientes
./target/release/phxsql bancos /tmp/dados
./target/release/phxsql tabelas /tmp/dados Z
```''')

s=s.replace('''```
crates/
  phxsql-core/     tipos, valores, esquema, codificação de chaves, CRC, calendário
  phxsql-store/    os quatro arquivos e a tabela que os costura
  phxsql-cli/      a ferramenta de linha de comando
  phxsql-store/examples/basico.rs   exemplo executável
docs/
  FORMATO.md       especificação byte a byte dos quatro arquivos
```''','''```
crates/
  phxsql-core/     tipos, valores, esquema, chaves estrangeiras, paginação,
                   codificação de chaves, CRC, calendário
  phxsql-store/    os cinco arquivos, os volumes, a hierarquia e a tabela
  phxsql-cli/      a ferramenta de linha de comando
  phxsql-store/examples/basico.rs   exemplo executável
docs/
  FORMATO.md       especificação byte a byte dos arquivos
  PLANO.md         leitura do rusqlite e do FraseSQL, e o roteiro do projeto
```''')

s=s.replace('''**CRC em toda parte.** Cabeçalho, registro, página de índice e bloco externo têm
CRC-32 próprio. `phxsql verificar` percorre os quatro arquivos e confere tudo,
inclusive se a contagem de chaves de cada índice bate com a de registros vivos.''','''**CRC em toda parte.** Cabeçalho, registro, página de índice, bloco externo e
evento do diário têm CRC-32 próprio. `phxsql verificar` percorre os cinco
arquivos e confere tudo, inclusive se a contagem de chaves de cada índice bate
com a de registros vivos.

**A paginação não custa nada ao índice.** O volume sai do rowid por divisão, e
o rowid continua global e imutável — então o `.ndx` nem sabe que existe volume.

**Operação recusada não vira evento.** O `.log` registra o que aconteceu, não o
que foi tentado: chave duplicada, tabela cheia ou coluna obrigatória em branco
falham sem sujar o diário.''')
open(p,'w').write(s)
print("README.md atualizado")
