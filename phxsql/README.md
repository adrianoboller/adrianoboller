<img src="marca/derivados/phxsql-logo-560.png" alt="PhxSql" width="260">

# PhxSql — Phoenix Database Engine

> Built to store. Engineered to scale.

Motor de dados em Rust no modelo de arquivos separados do HFSQL(R): cada tabela
lógica é a soma de sete arquivos físicos — mais um oitavo, o espelho `.bkp`,
quando ele está ligado.

```
cadastroClientes.reg    registros, na ordem de digitação
cadastroClientes.ndx    índices (B+tree)
cadastroClientes.bin    binários
cadastroClientes.memo   textos longos
cadastroClientes.log    diário de inclusões, alterações e exclusões
cadastroClientes.trash  as linhas que saíram do .reg, inteiras
cadastroClientes.reason por que cada linha foi excluída, e por quem

.reg + .ndx + .bin + .memo + .log + .trash + .reason = cadastroClientes
```

Mais o `.pag` ao lado — um JSON que descreve como a tabela está partida, para
quem está de fora ler sem abrir o `.reg`. É gerado, e o motor nunca o lê.

Os três últimos são **os arquivos do administrador**: o `.trash` guarda o dado
que alguém mandou apagar, e o `.reason` costuma ser mais revelador que o
registro que foi excluído.

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

## Por que arquivos separados

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
- O **`.trash`** e o **`.reason`** também são append-only, e existem porque uma
  exclusão precisa deixar rastro: a linha inteira num, o porquê no outro. O
  `.trash` é gravado e **sincronizado antes** de o slot do `.reg` ser liberado —
  entre perder o dado e duplicá-lo, o motor duplica.

## Paginação: anda por cursor, salta por posição

Num motor relacional, pular para o meio de uma tabela grande exige um índice: a
ordem lógica não tem nada a ver com a posição física. Aqui tem —
`offset = data_offset + (rowid−1) × slot_size`. Continuar depois do rowid
500.000 **não é procurar: é uma conta.**

```json
{"op":"varrer","database":"loja","tabela":"clientes","max":200,"depois":4000}
{"op":"varrer","database":"loja","tabela":"clientes","max":200,"pular":100000}
```

Toda tabela tem a coluna de sistema **`rownum`** — a ordem de chegada da linha,
que o motor preenche e nunca reaproveita. Se ninguém apagou de vez e ninguém
marcou como excluída, a **posição** de uma linha na lista *é* o `rownum` dela
menos um — e aí «ir para a página 500» é uma bissecção de vinte leituras, e não
meio milhão de passos. O motor confere as duas condições no cabeçalho, em tempo
constante, e diz na resposta qual caminho pagou.

Medido numa tabela de 200.000 linhas, pelo protocolo, pedindo 200 linhas:

| `pular` | bissecção | passo |
|---:|---:|---:|
| 200 | 7 ms | 6 ms |
| 20.000 | 7 ms | 18 ms |
| 100.000 | 6 ms | 72 ms |
| 199.800 | 6 ms | **131 ms** |

A bissecção é **plana** — e os 6 ms dela são decodificar e serializar as 200
linhas, não achar o começo.

## Profiler: o que chega, antes de virar dado

O ponto de captura é uma linha depois do `read_line` e uma antes do despacho —
**nada foi gravado ainda**. Por isso o pedido que *trava* aparece na lista como
«em curso», que é justamente o que se quer achar.

```json
{"op":"profiler_ligar","database":"Comercial","so_escrita":true,
 "arquivo":"/var/log/phxsql-monitor.txt"}
```

Filtra por banco, usuário, operação e só-escrita; grava num `.txt` no caminho
escolhido. **A senha não passa por aqui**: o pedido é *analisado* e os campos
sensíveis viram `"***"` antes de encostar na memória ou no arquivo — nunca
recortado, porque recortar depende de o pedido estar escrito de um jeito.

## Rodar em contêiner

```bash
docker build -t phxsql .
docker compose up -d     # um master e duas réplicas, em portas diferentes
```

A imagem final é **`scratch`** — sem shell, sem gerenciador de pacotes, só o
binário. Só é possível porque não há dependência externa nenhuma: com o alvo
musl o servidor sai `static-pie` com **3,4 MB**.

## Replicação: Master e espelhos

A réplica **procura** o master; o master não empurra nada. É o desenho do
MySQL(R), e existe por causa do firewall: o master abre uma porta de entrada e
não precisa alcançar ninguém de volta.

```json
"replicacao": { "papel": "source", "imagem_da_linha": true }
```

O `.log` sempre foi o binlog; o que faltava era a **imagem da linha** dentro do
evento — o payload cru do `.reg` mais o *conteúdo* dos anexos, porque os
ponteiros são offsets desta máquina.

Medido com quatro servidores (`bancada/replicacao/`):

| | |
|---|---|
| Master, com a imagem no diário | 28.914 linhas/s |
| Aplicação, por réplica (as três em paralelo) | 4.357 eventos/s |
| Atraso de uma escrita até as três | 1,3 s a 2,1 s |
| Réplica derrubada: voltar a atender e alcançar 4.000 eventos | 343 ms + 1,0 s |
| Retrato SHA-256 das quatro tabelas, no fim | idênticos |

O `rowid` **não é transmitido**: o `.reg` nunca reaproveita slot, então uma
réplica que aplicou tudo na ordem chega ao mesmo número sozinha. Se não chegar,
divergiu — e a replicação para ali em vez de espalhar.

## Carga em lote

Gravar mil linhas com mil pedidos custa mil aberturas de tabela, mil travas e
mil `fsync`. `inserir_lote` faz tudo uma vez só — **2.659 → 39.287 linhas/s
(14,8×)**, medido com 20.000 linhas pela rede por
[`bancada/carga/medir.py`](bancada/carga/medir.py).

O mesmo pedido aceita texto colado em **JSON, CSV, TXT, XML ou HTML**, e
adivinha o formato pelo conteúdo. A primeira linha manda: as colunas casam pelo
**nome**, não pela posição.

## Partição alfanumérica

Um arquivo por letra inicial de uma coluna:

```
cadastroClientes_A.reg  …  cadastroClientes_Z.reg
cadastroClientes_0.reg  …  cadastroClientes_9.reg
cadastroClientes_Outros.reg
```

São 37 volumes fixos, e o rowid é atribuído como
`(balde − 1) × registros_por_arquivo + slot` — a inversa exata da conta de
sempre, então **nenhum caminho de leitura mudou**. O teto passa a ser por letra,
e a ordem de digitação sai do `rowid` e vai para o `rownum`.

E o `.reg` guarda a ordem em que os dados foram digitados — algo que um heap com
reaproveitamento de espaço perde.

## Conflito de escrita: o segundo a salvar escolhe

Duas pessoas com a mesma ficha aberta terminavam com a segunda gravação
apagando o trabalho da primeira — sem erro, sem registro, sem ninguém perceber
até faltar o dado.

Cada slot do `.reg` guarda uma **versão**, que sobe a cada regravação. Quem lê
com `"com_versao": true` recebe a versão junto e a manda de volta no
`atualizar`; se ela não for mais a atual, o servidor recusa com o erro **3004
`CONFLITO`**. Conferir custa 24 bytes — o cabeçalho do slot, não a linha.

Na tela, o conflito abre as **três colunas** do HFSQL(R) — «valor anterior», «o
outro escreveu», «você escreve» — e vai um passo além: **já vem marcado quem
mexeu em cada coluna**. Dois que editaram campos diferentes da mesma linha saem
dali com os dois trabalhos preservados, sem escolher nada.

Não é trava: travar na leitura prenderia a linha toda vez que alguém fechasse o
navegador com a ficha aberta. E a conferência é **pedida, não imposta** — quem
não manda a versão continua com a última gravação vencendo, como antes.

## Direito até a tabela, e não só até a base

A folha de pagamento e a tabela de clientes moram no mesmo banco porque o
negócio é um só, e o direito de ler as duas não é o mesmo:

```json
"bases": {
  "Z": { "ler": true, "inserir": true,
         "tabelas": { "folha": {} } }
}
```

A regra da tabela **substitui** a da base ali — o que permite tanto tirar
`folha` de quem lê o banco inteiro quanto dar `clientes` a quem não lê o banco
nenhum. A árvore e o catálogo passam a listar só o que dá para abrir.

## Estado atual

O motor de armazenamento está completo e testado: **390 testes** só nele
(`phxsql-core` 163 + `phxsql-store` 227), **619 no projeto inteiro**, sem
nenhuma dependência externa (só a `std`) — o que faz o projeto compilar
offline.

| Peça | Situação |
|---|---|
| `.reg` — heap, CRC por registro, esquema embutido | pronto |
| `.ndx` — B+tree com divisão de páginas, chave composta, ASC/DESC/NOCASE/único | pronto |
| `.ndx` — cache de páginas de leitura, despejo por segunda chance — inserção 2,4× mais rápida, medida | pronto |
| `.bin` / `.memo` — blocos com CRC, contabilidade de espaço morto | pronto |
| `.log` — diário datado de inclusão, alteração e exclusão, com a imagem da linha para replicar | pronto |
| Paginação em volumes `_001`, `_002`, … com abertura preguiçosa | pronto |
| Partição por período — mensal, bimestral, semestral, anual | pronto |
| Metadados de campo: id estável, caption, descrição e máscara PICTURE | pronto |
| Chave primária declarada, com marca de composta derivada dos índices | pronto |
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
| Copiar e colar tabela entre bancos e schemas | pronto |
| SysTables e SysColumns — catálogo e dicionário de dados | pronto |
| Gerir banco: configurações, diretivas de acesso, conexões, backup | pronto |
| Editor de menu — troca o nome exibido de cada item | pronto |
| Tabela em memória e `SelectMemory` — 87× mais rápido, medido | pronto |
| Chave assimétrica Ed25519 (RFC 8032) como segundo fator | pronto |
| Backup com manifesto SHA-256, ZIP e agendamento | pronto |
| Nível de usuário: nenhum, leitor, operador, dono, admin | pronto |
| Tema claro e escuro, console para mais de um servidor | pronto |
| Barra de menu tradicional — nove menus, atalhos e navegação por teclado | pronto |
| Painel com sete gráficos, agregado numa chamada | pronto |
| phx-grid: agrupamento por arrastar, ordem por nível, rodapé e total geral | pronto |
| Tabela dinâmica com assistente — cruzamento somado no servidor | pronto |
| Durabilidade configurável — gravação 20× mais rápida, medida | pronto |
| Seção `recursos`: memória, CPU, threads, conexões e usuários | pronto |
| `sequencias` — o contador de cada tabela, ajustável pelo admin | pronto |
| Espelho `.bkp`: segunda chance do `.reg` | pronto |
| Bancada medida contra o MySQL(R), 10 milhões de registros | pronto |
| Replicação — `.log` v2 com imagem da linha, quatro modos e agendamento por origem | pronto |
| Cluster com eleição e promoção automática, e `REDIRECIONA` na escrita | pronto |
| Jobs de execução — operação nomeada no relógio, com o poder do usuário do job | pronto |
| Triggers nas três operações, e stored procedures — um interpretador só | pronto |
| Parar e subir a porta de dados pela interface, trocando a porta | pronto |
| Servidor MCP — `phxsqld --mcp`, com o `tools/list` lendo o catálogo | pronto |
| Camada SQL — crate `phxsql-sql`, **escrita aqui** e não sobre o rusqlite | pronto |
| Driver ODBC de saída — `cdylib` de ABI C, provada por 73 conferências e pelo `isql` | pronto |
| Telemetria ao vivo, marca de dado pessoal (LGPD), DbLink com MySQL(R) e PostgreSQL(R) | pronto |
| Cliente OLE DB nativo | recusado com motivo — a ponte `MSDASQL` cobre ([`docs/ODBC.md`](docs/ODBC.md) §6) |
| Restaurar backup — a metade que falta de «Backup e restauração» | pendente |
| Trava por tabela no lugar da trava única global | pendente |
| Integração no FraseSQL como `engine = "phxsql"` | pendente |
| Compactação, transações, modo exclusivo, TLS | pendente |

O roteiro completo, com as decisões tomadas e o que cada peça depende, está em
[`docs/PLANO.md`](docs/PLANO.md); a revisão do que ainda falta, com o porquê de
cada ausência, em [`docs/PENDENCIAS.md`](docs/PENDENCIAS.md) — que é a fonte
desta tabela, e onde o estado é medido contra o código. As **propostas** de
trabalho, lidas dos manuais de quatro motores e ainda esperando aprovação,
estão em [`docs/SPRINTS.md`](docs/SPRINTS.md); elas não são pendências.

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

## Os pacotes de download

```bash
./empacotar.sh            # três zips em pacotes/: fontes, Linux e Windows
./empacotar.sh conferir   # desempacota o que está lá e confere
```

Cada zip traz um `MANIFESTO.sha256` com o hash de todos os seus arquivos, e o
conferidor viaja dentro do próprio pacote:

```bash
./phxsql conferir-pacote        # responde INTEGRO, ou o que difere, falta e veio a mais
sha256sum -c MANIFESTO.sha256   # o segundo caminho, que não depende de rodar nada do zip
```

O que cada pacote leva, o que ele deliberadamente não leva e as quatro travas
que rodam antes de qualquer zip sair estão em
[`docs/EMPACOTAMENTO.md`](docs/EMPACOTAMENTO.md).

## Estrutura

```
crates/
  phxsql-core/     tipos, valores, esquema, chaves estrangeiras, paginação,
                   codificação de chaves, CRC, SHA-256, calendário
  phxsql-store/    os sete arquivos, os volumes, a hierarquia e a tabela
  phxsql-sql/      analisador SQL e tradutor para as operações do protocolo
  phxsql-cli/      a ferramenta de linha de comando (phxsql)
  phxsql-cmd/      o console interativo (phxsqlcmd)
  phxsql-server/   config, usuários, blacklist, servidor TCP e o HTTP da
                   interface; ui/index.html é o Centro de Controle
  phxsql-odbc/     o driver ODBC 3.x, uma cdylib de ABI C
  phxsql-ffi/      o PhxSql EMBUTIDO: o motor como biblioteca (cdylib para o
                   Android, staticlib para o iOS), com o cabeçalho phxsql.h e
                   um programa em C que o exercita
  phxsql-store/examples/basico.rs   exemplo executável
docs/
  FORMATO.md       especificação byte a byte dos arquivos
  USUARIOS.md      cadastro, senha em hash e as dez permissões
  SEGURANCA.md     política, blacklist, firewall e as formas de login
  REPLICACAO.md    o desenho da replicação Source → Réplica
  ODBC.md          registro do driver e connection string
  EMBUTIDO.md      o motor como biblioteca no aparelho: a ABI de C, as seis
                   decisões dela, e por que não é um "mini servidor"
  EMPACOTAMENTO.md como os três zips de download são montados e conferidos
  MOBILE.md        medido contra o SQLite(R), e a forma que cabe num aparelho
  PLANO.md         leitura do rusqlite e do FraseSQL, e o roteiro do projeto
exemplos/
  Config_exemplo_0N.json   isolado, source e réplica
bancada/           a medição: carga, comparação, profiler, telemetria
testes-web/        as baterias que exercitam a tela num navegador
marca/             a marca oficial e seus derivados
empacotar.sh       monta os três zips de download
MANUAL.txt         manual do operador
CHANGELOG.md       o que mudou em cada versão, defeitos primeiro
provar.py          a bateria inteira num comando só
bancada/guardas/   o catálogo dos defeitos repostos, e o executor que os repõe
```

### Como se prova

```bash
python3 provar.py --construir     # compila e roda as dezesseis partes
python3 provar.py --listar        # o que existe, e o que cada parte prova
```

Ele imprime o que passou, o que falhou, quanto cada parte demorou e **o que foi
pulado, com o motivo** — bateria que esconde o que não rodou mente por omissão.
Recusa rodar com binário velho, porque a interface está embutida no `phxsqld`.
O desenho está em `docs/TESTES.md` §7.

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
