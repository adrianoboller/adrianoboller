# BULKINSERT: adiar o `.ndx` compra quanto?

A dívida escrita em `crates/phxsql-server/src/carga.rs`: o `BULKINSERT` ainda
paga o índice linha a linha; adiar (carregar só `.reg`/`.log` e reconstruir
com `reindexar()`) é a terceira parte, a maior, e não estava medida. Este
diretório mede os **três regimes** que existem hoje ou foram propostos:

| | o que é | script |
|---|---|---|
| (a) inline | os dois índices mantidos durante a inserção -- o de hoje | `medir.py` (via `--example bulkinsert-adiar-ndx`) |
| (b) adiado, síncrono, sob a reserva | carrega sem índice, reconstrói ANTES de soltar a tabela | idem |
| (c) adiado, em thread, reserva solta | carga devolve "ok" cedo; o índice reconstrói depois, com o servidor atendendo | `reconstrucao-em-thread.py` |

## (a) e (b): `medir.py`

```bash
flock /tmp/phx-cargo.lock cargo build --release --examples -p phxsql-store
python3 bancada/carga/adiar-ndx/medir.py --repeticoes 5 10000 100000 1000000
```

Chama `target/release/examples/bulkinsert-adiar-ndx` (instrumento em
`crates/phxsql-store/examples/`), que faz duas coisas por chamada de
processo: mede os dois regimes com N linhas numa tabela **vazia** (a forma
do BULKINSERT: importar, migrar, semear) e **confere** -- antes de apagar as
tabelas -- que os dois terminam no mesmo estado: mesma contagem, índice
único e não único respondendo idêntico nos dois. Guarda em `resultados.json`.

Este caso é **M=N, N_antes=0** -- carga total num alvo vazio. NÃO é o caso
geral (carregar M numa tabela que já tem N), que o pedido 114 (29/08/2026)
já mediu e fechou com `--example adiar-vale-quando` (continua existindo, em
`crates/phxsql-store/examples/`, sem mudança): o `reindexar` refaz a tabela
INTEIRA, e abaixo de M≈N/3 adiar custa tempo. Ver `docs/DESEMPENHO.md` §4.4 e
a seção "contra o 114" do relatório desta rodada.

## (c): `reconstrucao-em-thread.py`

```bash
flock /tmp/phx-cargo.lock cargo build --release --examples -p phxsql-store --bins
flock /tmp/phx-cargo.lock cargo build --release --bins -p phxsql-server
python3 bancada/carga/adiar-ndx/reconstrucao-em-thread.py 1000000 3
```

O motor não tem (ainda) um modo de índice suspenso nem uma reconstrução de
fato em thread. Mas a RPC `"op":"reindexar"` já chama `Table::reindexar()`
sob a MESMA trava global (`self.dados`, um `RwLock`) que toda leitura e
escrita do servidor usa -- e o servidor não distingue "RPC de um cliente" de
"chamada interna de uma thread de fundo": as duas tomam a trava do mesmo
jeito. Rodar essa RPC numa conexão separada enquanto outra martela uma
segunda tabela (`operacao`) reproduz fielmente o que uma thread de fundo
causaria.

A tabela `carga` chega no estado "carregada, índice vazio" via
`--example preparar-carga-adiada`, rodado ANTES de o servidor subir -- fora
da janela medida. A técnica (apagar o `.ndx` e recriar vazio casando o
esquema) é a mesma que `crates/phxsql-store/tests/paginacao_log_reindex.rs`
já prova correta.

Mede, por corrida: o tempo do `reindexar` (a trava fica presa por esse
tempo inteiro), o op/s do operador antes/durante/depois, e a PIOR PAUSA que
um único pedido do operador sofre durante a reconstrução -- o número que
decide se "de graça" é verdade. Guarda em `resultados-regime-c-<n>.json`.

## As três, no mesmo lugar

Nenhum dos três scripts mede sozinho a pergunta "vale a pena": (a)/(b) dizem
quanto tempo TOTAL cada regime consome; (c) diz o que a OPERAÇÃO sente
enquanto (b) faria isso escondido dentro da janela reservada. As duas
metades juntas são a resposta.
