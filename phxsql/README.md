<img src="marca/derivados/phxsql-logo-560.png" alt="PhxSql" width="260">

# PhxSql — Phoenix Database Engine

> Built to store. Engineered to scale.

Motor de dados em Rust no modelo de arquivos separados do HFSQL(R): cada tabela
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

## Por que cinco arquivos

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

O motor de armazenamento está completo e testado: **339 testes**, sem nenhuma
dependência externa (só a `std`), o que faz o projeto compilar offline.

| Peça | Situação |
|---|---|
| `.reg` — heap, CRC por registro, esquema embutido | pronto |
| `.ndx` — B+tree com divisão de páginas, chave composta, ASC/DESC/NOCASE/único | pronto |
| `.bin` / `.memo` — blocos com CRC, contabilidade de espaço morto | pronto |
| `.log` — diário datado de inclusão, alteração e exclusão | pronto |
| Paginação em volumes `_001`, `_002`, … com abertura preguiçosa | pronto |
| Hierarquia database → schema → tabela | pronto |
| `Uuid` v4/v7 (RFC 9562), `Uuid256` de 256 bits e `Sequence` | pronto |
| CRC-32 slice-by-8 — inserção 3,1× mais rápida, medida | pronto |
| Varredura em memória dividida entre núcleos — 1,8× em 4 núcleos | pronto |
| Chave estrangeira **declarada** no esquema (CASCADE / RESTRICT / SET NULL) | parcial — guardada e reportada, **não aplicada** na gravação |
| Reindex — recria o `.ndx` do zero a partir do `.reg` | pronto |
| `Table` — valores, nulos, índices sincronizados, verificação de integridade | pronto |
| CLI `phxsql` | pronto |
| `config.json` e servidor TCP na porta 5000 | pronto |
| Log de acessos por IP, com data e hora | pronto |
| Cadastro de usuários, senha em hash, permissão por base | pronto |
| Login por desafio-resposta (a senha não trafega) e Base64 | pronto |
| Blacklist com bloqueio automático e gancho de firewall | pronto |
| Centro de Controle — interface web embutida no `phxsqld` | pronto |
| View Database — grade de tabelas, ficha de edição, incluir/salvar/excluir | pronto |
| Gestão de tabelas — criar, duplicar, reparar, ver partições e excluir | pronto |
| Tabela em memória e `SelectMemory` — 87× mais rápido, medido | pronto |
| Chave assimétrica Ed25519 (RFC 8032) como segundo fator | pronto |
| Backup com manifesto SHA-256, ZIP e agendamento | pronto |
| Nível de usuário: nenhum, leitor, operador, dono, admin | pronto |
| Tema claro e escuro, console para mais de um servidor | pronto |
| Barra de menu tradicional — sete menus, atalhos e navegação por teclado | pronto |
| Painel com sete gráficos, agregado numa chamada | pronto |
| phx-grid na aba Conteúdo: agrupamento por arrastar | pronto |
| Espelho `.bkp`: segunda chance do `.reg` | pronto |
| Bancada medida contra o MySQL(R), 10 milhões de registros | pronto |
| Replicação — `.log` v2 com imagem da linha (as portas já entram no `config.json`) | desenhada |
| Jobs de execução — operação nomeada no relógio | pendente |
| Triggers nas três operações | pendente |
| Stored procedures | pendente |
| Trava por tabela no lugar da trava única global | pendente |
| Parar e subir a porta de dados pela interface | pendente |
| Servidor MCP | pendente |
| Camada SQL (tabela virtual via rusqlite, atrás de *feature*) | pendente |
| Driver ODBC de saída, depois cliente ODBC/OLE DB | pendente |
| Integração no FraseSQL como `engine = "phxsql"` | pendente |
| Compactação, transações, concorrência | pendente |

O roteiro completo, com as decisões tomadas e o que cada peça depende, está em
[`docs/PLANO.md`](docs/PLANO.md); a revisão do que ainda falta, com o porquê de
cada ausência, em [`docs/PENDENCIAS.md`](docs/PENDENCIAS.md).

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

### O servidor e o Centro de Controle

```bash
./target/release/phxsqld --exemplo 1 > config.json
$EDITOR config.json          # troque o token e ligue "web"
./target/release/phxsqld --config config.json
```

Com `"web": { "ligado": true }` o próprio `phxsqld` passa a servir o Centro
de Controle em `http://127.0.0.1:5001` — a página está embutida no binário,
não há servidor web para instalar. A árvore mostra bancos, schemas e tabelas;
cada tabela abre em cinco abas (Estrutura, Conteúdo, Índices, Diário,
Integridade) e há três telas de administração (Usuários, Acessos, Bloqueios).

Em `127.0.0.1` e em `https://` o login usa desafio-resposta e a senha **não
sai da máquina de quem entra**. Fora de contexto seguro o navegador não
oferece a cifra: a página cai em Base64 e diz isso na tela. Detalhes na
seção 9 do [`MANUAL.txt`](MANUAL.txt).

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
  phxsql-server/   config, usuários, blacklist, servidor TCP e o HTTP da
                   interface; ui/index.html é o Centro de Controle
  phxsql-store/examples/basico.rs   exemplo executável
docs/
  FORMATO.md       especificação byte a byte dos arquivos
  USUARIOS.md      cadastro, senha em hash e as dez permissões
  SEGURANCA.md     política, blacklist, firewall e as formas de login
  REPLICACAO.md    o desenho da replicação Source → Réplica
  PLANO.md         leitura do rusqlite e do FraseSQL, e o roteiro do projeto
exemplos/
  Config_exemplo_0N.json   isolado, source e réplica
MANUAL.txt         manual do operador
CHANGELOG.md       o que mudou em cada versão, defeitos primeiro
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

**A senha nunca fica em texto puro.** O `config.json` guarda
PBKDF2-HMAC-SHA256 com 210.000 iterações — SHA-256, HMAC e PBKDF2 escritos aqui
para não quebrar a regra de zero dependências, e conferidos contra os vetores
oficiais (FIPS 180-4, RFC 4231). Gere o hash com
`echo -n 'a senha' | phxsqld --senha`.

**Base64 não é criptografia, e o código diz isso.** O `login` aceita
`senha_b64`, mas há um teste chamado
`base64_nao_esconde_nada_de_quem_captura` que decodifica a credencial para
provar. O que protege de verdade é o desafio-resposta, onde a senha nunca sai
da máquina do cliente.

**Firewall quebrado não vira porta aberta.** O bloqueio vale sempre dentro do
servidor; a regra de `iptables` é um extra desligado por padrão, roda sem
shell e valida o IP como endereço antes de usá-lo.

**Cadastrar usuários só aperta a segurança.** Sem cadastro, o token dá poder
total, como antes. Com cadastro, o token vira só a chave da porta da rede e o
login passa a ser exigido — nunca o contrário.

## Marca

Os arquivos oficiais estão em [`marca/`](marca/): manual de marca, logotipo,
tela de abertura e os derivados usados na documentação.

| | |
|---|---|
| Tipografia | Exo 2 — SemiBold / Medium / Regular |
| Fundo | `#010418` |
| Paleta | `#FFC43D` `#FF8A1C` `#FF4D10` `#D71A1A` `#8B0D0D` `#DDE2EB` |

## Licença

MIT OR Apache-2.0
