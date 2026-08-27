# PhxSql

Motor de dados em Rust no modelo de arquivos separados do HFSQL: cada tabela
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
```

## Por que quatro arquivos

Cada arquivo tem um padrão de acesso diferente, e separá-los deixa cada um
otimizado para o seu:

- O **`.reg`** tem slots de largura fixa, então o endereço de um registro sai
  de uma multiplicação, não de uma busca. Ler o registro 5.000 é um `seek`.
- O **`.ndx`** é paginado e só ele sofre a reescrita aleatória da B+tree. O
  arquivo de dados nunca é reordenado.
- **`.bin`** e **`.memo`** tiram o conteúdo de tamanho livre de dentro do
  registro. Uma foto de 5 MB não infla o slot: o `.reg` guarda só um ponteiro
  de 16 bytes, e uma varredura da tabela não arrasta os blobs junto.
- O **`.log`** é append-only e sem índice, então registrar uma operação custa
  36 bytes no fim de um arquivo — não atrapalha a escrita.

E o `.reg` guarda a ordem em que os dados foram digitados — algo que um heap com
reaproveitamento de espaço perde.

## Estado atual

O motor de armazenamento está completo e testado: **104 testes**, sem nenhuma
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
[`docs/PLANO.md`](docs/PLANO.md).

## Uso

```bash
cargo build --release

./target/release/phxsql demo /tmp/dados/Z --paginado
./target/release/phxsql info /tmp/dados/Z cadastroClientes
./target/release/phxsql listar /tmp/dados/Z cadastroClientes --indice porCidadeLimite
./target/release/phxsql log /tmp/dados/Z cadastroClientes
./target/release/phxsql reindex /tmp/dados/Z cadastroClientes
./target/release/phxsql verificar /tmp/dados/Z cadastroClientes
./target/release/phxsql bancos /tmp/dados
./target/release/phxsql tabelas /tmp/dados Z
```

Na biblioteca (este trecho e o `crates/phxsql-store/examples/basico.rs`,
que compila e roda com `cargo run --example basico`):

```rust
use phxsql_core::{Column, ColumnType, IndexColumn, IndexDef, Schema, Value};
use phxsql_store::Table;

let esquema = Schema::new(
    "cadastroClientes",
    vec![
        Column::new("id", ColumnType::Int8).obrigatoria(),
        Column::new("nome", ColumnType::Str(60)).obrigatoria(),
        Column::new("cidade", ColumnType::Str(40)),
        Column::new("ficha", ColumnType::Memo),
    ],
    vec![
        IndexDef::new("porId", vec![IndexColumn::asc(0)]).unico(),
        IndexDef::new("porNome", vec![IndexColumn::asc(1).sem_caixa()]),
    ],
)?;

let mut t = Table::criar("/tmp/dados", esquema)?;

let rowid = t.inserir(&[
    Value::Int(1),
    Value::Str("Adriano Boller".into()),
    Value::Str("Blumenau".into()),
    Value::Memo("Cliente desde 1998.".into()),
])?;

// Busca pelo índice, sem distinguir maiúsculas.
let achados = t.buscar("porNome", &[Value::Str("adriano boller".into())])?;
assert_eq!(achados, vec![rowid]);

// Varredura na ordem de digitação, direto do .reg.
for (rowid, linha) in t.varrer()? {
    println!("{rowid}: {:?}", linha[1]);
}

t.verificar()?;
```

## Estrutura

```
crates/
  phxsql-core/     tipos, valores, esquema, chaves estrangeiras, paginação,
                   codificação de chaves, CRC, calendário
  phxsql-store/    os cinco arquivos, os volumes, a hierarquia e a tabela
  phxsql-cli/      a ferramenta de linha de comando
  phxsql-store/examples/basico.rs   exemplo executável
docs/
  FORMATO.md       especificação byte a byte dos arquivos
  PLANO.md         leitura do rusqlite e do FraseSQL, e o roteiro do projeto
```

## Decisões de projeto que valem explicação

**Ordem de digitação é uma garantia, não um acaso.** Slots excluídos nunca são
reaproveitados. Reaproveitar manteria o arquivo compacto, mas percorrer o `.reg`
deixaria de devolver os registros na ordem em que foram digitados. O espaço volta
com compactação explícita.

**A B+tree não conhece tipos.** As chaves chegam já codificadas de forma que
comparar bytes dá a mesma ordem que comparar valores. Um único código de árvore
atende inteiro, data, decimal, texto, ASC, DESC, NOCASE e chave composta.

**O rowid faz parte da chave.** Ele vai no fim, em big-endian. Assim toda chave
é única, índices duplicados funcionam sem caso especial, e o resultado de uma
busca já sai em ordem de digitação.

**CRC em toda parte.** Cabeçalho, registro, página de índice, bloco externo e
evento do diário têm CRC-32 próprio. `phxsql verificar` percorre os cinco
arquivos e confere tudo, inclusive se a contagem de chaves de cada índice bate
com a de registros vivos.

**A paginação não custa nada ao índice.** O volume sai do rowid por divisão, e
o rowid continua global e imutável — então o `.ndx` nem sabe que existe volume.

**Operação recusada não vira evento.** O `.log` registra o que aconteceu, não o
que foi tentado: chave duplicada, tabela cheia ou coluna obrigatória em branco
falham sem sujar o diário.

## Licença

MIT OR Apache-2.0
