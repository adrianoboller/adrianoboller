# PhxSql

Motor de dados em Rust no modelo de quatro arquivos do HFSQL: cada tabela
lógica é a soma de quatro arquivos físicos.

```
cadastroClientes.reg    registros, na ordem de digitação
cadastroClientes.ndx    índices (B+tree)
cadastroClientes.bin    binários
cadastroClientes.memo   textos longos

.reg + .ndx + .bin + .memo  =  cadastroClientes
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

E o `.reg` guarda a ordem em que os dados foram digitados — algo que um heap com
reaproveitamento de espaço perde.

## Estado atual

O motor de armazenamento está completo e testado: **57 testes**, sem nenhuma
dependência externa (só a `std`), o que faz o projeto compilar offline.

| Peça | Situação |
|---|---|
| `.reg` — heap, CRC por registro, esquema embutido | pronto |
| `.ndx` — B+tree com divisão de páginas, chave composta, ASC/DESC/NOCASE/único | pronto |
| `.bin` / `.memo` — blocos com CRC, contabilidade de espaço morto | pronto |
| `Table` — valores, nulos, índices sincronizados, verificação de integridade | pronto |
| CLI `phxsql` | pronto |
| Compactação, transações, concorrência, camada SQL | pendente |

## Uso

```bash
cargo build --release

./target/release/phxsql demo /tmp/dados
./target/release/phxsql info /tmp/dados cadastroClientes
./target/release/phxsql listar /tmp/dados cadastroClientes --indice porCidadeLimite
./target/release/phxsql verificar /tmp/dados cadastroClientes
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
  phxsql-core/     tipos, valores, esquema, codificação de chaves, CRC, calendário
  phxsql-store/    os quatro arquivos e a tabela que os costura
  phxsql-cli/      a ferramenta de linha de comando
  phxsql-store/examples/basico.rs   exemplo executável
docs/
  FORMATO.md       especificação byte a byte dos quatro arquivos
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

**CRC em toda parte.** Cabeçalho, registro, página de índice e bloco externo têm
CRC-32 próprio. `phxsql verificar` percorre os quatro arquivos e confere tudo,
inclusive se a contagem de chaves de cada índice bate com a de registros vivos.

## Licença

MIT OR Apache-2.0
