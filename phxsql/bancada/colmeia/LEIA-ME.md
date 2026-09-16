# A bancada da colmeia (hive)

A pergunta que abriu a pasta foi a premissa do segundo tipo de banco
(`docs/propostas/colmeia.md` §1): *a nossa colmeia lê config-shaped mais rápido
que o Padrão?* — respondida em 12/09/2026 pelo H2. A segunda veio do dono em
16/09/2026, literal: *«Compare o tipo colmeia com o sqlite e phxsql — Insert,
update, delete e select.»* — e é o CRUD desta pasta.

Tudo aqui é para ser **refeito**. O protótipo é **de bancada**: sem
`ColumnType::Hive`, sem `TipoDatabase::Hive` despachando nada, sem aval de
formato do dono. Ele grava em `std::env::temp_dir()` e apaga o que gravou.

| Arquivo | O que é |
|---|---|
| `resultados.json` | **H2**: só a leitura (busca de ponto), colmeia × Padrão, três N — gerado pelo `--example custo-da-colmeia` sem argumentos |
| `medir-crud.py` | o **maestro do CRUD**: chama o exemplo para os lados colmeia e Padrão, roda o SQLite(R) em Python, junta, julga e grava |
| `resultados-crud.json` | a última medição **completa** do CRUD, crua: data, máquina, carga, versões, os três lados por (regime × operação × N), a linha de base do Python e as syscalls por operação |

O lado Rust dos dois mora em `crates/phxsql-store/examples/custo-da-colmeia.rs`
— um arquivo só, porque o modo `crud` reaproveita o formato PSHV, o leitor
(`Colmeia::ler`), o gerador de dados e o Padrão do H2. Um exemplo irmão teria
de copiar tudo isso, e duas cópias do mesmo formato é onde elas começam a
divergir.

## Como refazer

```bash
cargo build --release --example custo-da-colmeia -p phxsql-store   # binário velho mede o passado
bancada/esta-medindo.sh && echo "há medição em curso — espere"
python3 bancada/colmeia/medir-crud.py                    # 2 regimes × 3 N × 4 operações
python3 bancada/colmeia/medir-crud.py --rapido           # fumaça do próprio script (300 ops, 3 reps)
python3 bancada/colmeia/medir-crud.py --regimes sistema --tamanhos 10000 --sem-syscalls
```

O SQLite(R) vem na biblioteca padrão do Python (`sqlite3`, extensão em C) —
nada a instalar, e a pétrea de zero dependências continua inteira: o PhxSql
não puxa crate nenhuma para isso.

O regime `por_operacao` é o demorado, porque manda o disco: um `fsync` custa
~0,16 ms nesta máquina e o Padrão paga 8–9 por operação; `duracao_total_s` no
JSON diz quanto levou a última corrida (com as esperas). Quatro opções para
uma corrida interrompida ou disputada: `--continuar` retoma do
`resultados-crud.parcial.json` sem remedir o que já está completo;
`--rust-pronto <dir>` reaproveita um lado Rust já medido; `--espera-max <min>`
alarga o limite de espera por seção; `--sem-portao` é para a continuação de
uma corrida que já passou pelo portão (quem chegou depois está esperando por
ela — consultar o portão de novo seria esperar por quem espera, o impasse pago
em 16/09/2026).

O script **espera a máquina parar** antes de cada seção cronometrada (carga de
1 min abaixo de 1,0 e nenhum `cargo`/`rustc` vivo, até 20 min, sem matar
processo nenhum), consulta o `esta-medindo.sh` **na primeira seção** (protocolo
da casa: quem chegou primeiro mede, quem chega depois espera) e grava a carga
que encontrou. Seção que nunca achou o slot
sai como `NAO MEDIDA`, com o motivo, em vez de estimada. Durante a corrida o
progresso vai para `resultados-crud.parcial.json`; só no fim ele vira
`resultados-crud.json`, por `os.replace` — nunca meia medição no arquivo
versionado. A corrida é detectável pelo `bancada/esta-medindo.sh` pelos dois
crivos dele: o script está em `bancada/*.py` e o exemplo em
`target/release/examples/`.

## O que se compara — e o que cada lado FAZ por operação

Quatro operações de **ponto** sobre o mesmo par `caminho → valor` config-shaped
(`gN/sN/vN` → 8–64 bytes de texto), N = 1.000 / 10.000 / 100.000, 4.000
operações por repetição, 15 repetições, mediana e faixa min–máx, vencedor
**só quando a faixa dele fica inteira abaixo das outras duas**.

Os três lados recebem **os mesmos dados**: o exemplo Rust grava o conjunto num
JSON e o Python o lê — igualdade por construção, não por dois geradores que
alguém teria de provar iguais. E os três seguem o **mesmo contrato de
repetição**: `reps + 1` passadas, a primeira é aquecimento e não entra; a
leitura mede sobre a mesma base; inserir, atualizar e excluir **refazem a base
de N antes de cada passada**, para toda passada partir do mesmo estado.
Atualizar e excluir usam `min(4.000, N)` chaves **distintas** — excluir a mesma
chave duas vezes não é operação —, e o número real vai no JSON.

A regra 4 da bancada («mesma quantidade de trabalho») já foi quebrada duas
vezes nesta casa, e aqui ela **não pode ser cumprida por igualdade**, porque os
três lados não fazem o mesmo trabalho por desenho. Então o número não viaja
sozinho — viaja com esta tabela ao lado:

| por operação | colmeia (protótipo PSHV) | PhxSql Padrão (`Table`) | SQLite(R) 3.45.1 |
|---|---|---|---|
| **ler** | desce a árvore de células por busca binária no buffer em memória; devolve `&[u8]` sem alocar | `buscar` no `.ndx` (índice único) + `ler` no `.reg`, decodificando a linha em `Vec<Value>` | `SELECT valor WHERE chave=?`: b-tree do índice automático da chave + b-tree da tabela; materializa a linha |
| **inserir** | anexa a célula VALOR + **cópia de caminho** (folha, grupo e raiz nascem de novo no fim) + reescreve o bloco base de 128 B no lugar | `.reg` (append, ordem de digitação) + `.ndx` + diário `.log` + descritor `.pag` | `INSERT`: cria o `-journal`, grava página da tabela e do índice, apaga o `-journal` |
| **atualizar** | anexa VALOR novo + cópia de caminho; o valor velho vira lixo inalcançável | `buscar` + regrava a linha **no lugar** + índice + diário | `UPDATE … WHERE chave=?`: journal, regrava a página da tabela |
| **excluir** | anexa a folha **sem** a entrada + cópia de caminho; a célula fica (o «undelete» projetado) | `buscar` + conferir filhas + copiar a linha para a lixeira `.trash` + índice + diário | `DELETE … WHERE chave=?`: journal, remove da tabela e do índice |
| **o que NÃO tem** | índice separado, diário, transação, lixeira, tipos, conferência de unicidade fora da própria árvore | — | lixeira, trilha, colunas de sistema |
| **durabilidade `por_operacao`** | `fsync` após as células, `fsync` após o bloco base (a raiz nova só vale com as células no disco — a ordem do `.ndx` da casa) | `sincronizar()` após cada escrita: `fsync` do que a operação **escreveu** (mais os dois do `.ndx`, sempre); até 16/09/2026 era de cada arquivo aberto, sem pular os limpos — pedido 258 | `synchronous=FULL` + autocommit: `fsync` do journal e do banco em cada transação |
| **durabilidade `sistema`** | só `write` | nunca sincroniza; lixeira na janela | `synchronous=OFF` + autocommit (uma transação por operação, sem `fsync`) |

Os `fsync`/`write`/`openat`/`unlink` **por operação** de cada lado não são
contados lendo código: saem de `strace -c`, por diferença entre uma corrida de
1.000 e uma de 200 operações (o que a montagem da base custa cancela), e vão
para `syscalls_por_op` no JSON.

O regime de **cache** é declarado, não escondido: os três lados quentes — a
colmeia com o arquivo inteiro em memória, o Padrão com
`definir_cache_paginas(1_000_000)`, o SQLite(R) com `cache_size` de 256 MiB
(o padrão dele, 2 MiB, não cabe a base de 100.000 e mediria o cache, não o
motor).

## A cópia de caminho, e por que ela é o preço da pétrea

As células da colmeia são endereçadas por offset, e a pétrea manda que célula
**nunca se reescreva nem se reuse** (`docs/propostas/colmeia-estrutura.md`
§4). Trocar uma entrada de um NÓ exige um NÓ novo — e o pai aponta para o
velho, então o pai nasce de novo também, até a raiz. Cada escrita anexa a
cadeia inteira (três nós neste conjunto), e o maior deles é a **raiz**, que
cresce com o número de grupos: o JSON publica `bytes_anexados_por_op` para que
isso apareça no número, e não numa frase.

O que se compra com isso: cada escrita é **atômica por construção**. Até o
bloco base apontar para a raiz nova, quem lê vê a árvore antiga inteira — sem
diário e sem marca `.tx`. O único lugar do arquivo escrito no lugar é o bloco
base de 128 bytes (como o REGF faz com os números de sequência dele).

Célula velha vira lixo **inalcançável** a partir da raiz nova e fica até um
`compactar` explícito. O protótipo não marca o status dela no lugar: seriam
três ou quatro `pwrite` a mais por operação para um campo que a leitura nunca
consulta; o compactador acha o lixo pelo que não alcança da raiz.

## A linha de base do Python

O tempo do SQLite(R) inclui a chamada Python → C. Não se subtrai nada: o JSON
publica, ao lado, o custo do **mesmo laço** com `con.execute("SELECT ?",
(k,)).fetchone()` (nenhuma tabela no meio) e o do laço vazio, para o leitor
saber quanto do número é Python e quanto é motor.

## O que esta bancada NÃO decide

- **Formato.** Nada aqui autoriza `TipoDatabase::Hive` a sair de
  `motor_pronto() == false`; o formato PSHV se fecha com o dono.
- **Transação, integridade, tipos.** O protótipo não os tem, e é por isso que
  a frase «a colmeia é N× mais rápida» só vale com «fazendo menos» ao lado.
- **Cache frio.** Os três lados são medidos quentes; o custo de aquecer fica
  fora, como no H2.
- **A variante `WITHOUT ROWID` do SQLite(R)** (uma b-tree só, agrupada pela
  chave) não foi medida; a tabela rowid + índice automático é a que tem a
  mesma forma do Padrão (`.reg` + `.ndx`).
