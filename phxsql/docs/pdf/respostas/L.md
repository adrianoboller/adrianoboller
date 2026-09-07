# L) exemplo de uso DBLINK MULTILINK DATABASE

> Corrida em 2026-09-07T16:36:01Z UTC · commit `a56a165` ·
> `target/release/phxsqld` · reproduzido por
> `python3 bancada/dblink/prova-phxsql.py` e
> `python3 bancada/conexoes/multilink.py`

## Resposta curta

**DBLINK existe** e é uma ligação de UM PhxSql para OUTRO banco, cadastrada
por `dblink_salvar` e usada por `dblink_tabelas`/`dblink_ler`/`dblink_consultar`
— exercitado abaixo entre **dois `phxsqld` de verdade**, com **44
conferências, cada uma contra a mesma pergunta feita direto ao servidor de
origem**. **"MULTILINK"** — o nome do pacote proprietário analisado em
`docs/MULTILINK.md`, e do recurso do HFSQL(R) de onde ele saiu — não existe
como *link* (a análise está lá: 582 crates externas, licença por máquina, e
está fechada por decisão): o que existe e faz o mesmo destino é um servidor
**hub** com **duas ligações de DbLink ao mesmo tempo**, de onde uma única
sessão de cliente lê e combina dado de **dois servidores de origem
diferentes** — exercitado abaixo com três `phxsqld`. O limite real, também
medido: não há `SELECT` que **atravesse** dois DbLink dentro de um `FROM` —
quem combina os dois lados é o cliente.

## Exemplo exercitado

### 1. DBLINK — um PhxSql (`phx-a`) lendo OUTRO PhxSql (`phx-b`) de verdade

Dois `phxsqld` próprios, portas 7491/7492 (script de referência), com o dado
só em `phx-b` (25 linhas em `rh.funcionarios`). Cadastro da ligação em
`phx-a`:

```json
{"op":"dblink_salvar","nome":"b","motor":"phxsql","host":"127.0.0.1","porta":7492,
 "token_remoto":"prova-phx-b","usuario":"root","senha":"prova-b-5432",
 "database":"rh","somente_leitura":true}
```

```
ok   dblink_salvar aceita motor phxsql: {"gravado": true, "ligacao": {..., "conecta": true, ...}}
ok   a senha nao volta na ficha: (oculta)
ok   o token nao volta na ficha: (oculto)
ok   o dblink.json fica so do dono (0600): 0o600
```

`dblink_testar`, `dblink_bancos`, `dblink_tabelas` e `dblink_estrutura`, cada
um conferido contra a MESMA pergunta feita direto a `phx-b` (o oráculo
independente):

```
-- 1. dblink_testar: os DOIS portoes em serie, token e login
ok   a versao do outro lado           dblink: 0.18.0   direto: 0.18.0
ok   com quem o outro lado acha que fala   dblink: root   direto: root

-- 3. dblink_tabelas contra o `sistabelas` do phx-b
ok   a contagem de registros          dblink: 25   direto: 25
ok   a chave primaria                 dblink: pk_id   direto: pk_id
ok   os bytes do .reg                 dblink: 2625   direto: 2625

-- 4. dblink_estrutura: os nomes do SHOW, contra o `esquema` do phx-b
ok   os tipos     dblink: ['Int8','Str(40)','Str(30)','Decimal { precisao: 12, escala: 2 }','Bool',...]
                  direto: ['Int8','Str(40)','Str(30)','Decimal { precisao: 12, escala: 2 }','Bool',...]
ok   Non_unique tem a polaridade do NOME: 0 e unico: {'pk_id': '0', 'porCidade': '1'}
```

`dblink_ler` paginado e `dblink_consultar` — o SQL rodando **lá**, no motor
de `phx-b`:

```
-- 5. dblink_ler contra o `varrer` do phx-b
ok   a contagem do outro lado veio junta: 25
ok   e a resposta diz que ha mais pagina: True

-- 6. dblink_consultar: o SQL roda LA, no motor do phx-b
{"op":"dblink_consultar","dblink":"b","sql":"SELECT nome, cidade FROM funcionarios WHERE cidade = 'Blumenau'"}
ok   as linhas   dblink: ['Pessoa 001','Pessoa 003', ...]   direto: ['Pessoa 001','Pessoa 003', ...]
ok   COUNT(*) devolve a contagem e NENHUMA linha de dado
```

E o que ele recusa, dizendo por que — a ligação é somente-leitura de
verdade:

```
-- 7. o que ele RECUSA, e recusa dizendo por que
ok   `ordem` recusa em vez de devolver a ordem errada calada
ok   a sincronia recusa e manda para a REPLICACAO
ok   a ligacao somente-leitura recusa a escrita: [SP000025] acesso negado: a ligacao "b" esta em somente leitura e a instrucao nao e consulta
ok   e o phx-b continua com as 25 linhas: 25
```

**44 conferências, 0 falhas** (`grep -c '^  ok'` = 44 na saída completa) —
mesmo número já documentado em `docs/DBLINK.md`, e remedido nesta rodada,
não citado do documento.

### 2. MULTILINK — um HUB com DUAS ligações, uma sessão atravessando dois servidores

Três `phxsqld` próprios (faixa 6400–6419 desta frente): o **hub** (6412) e
dois servidores de origem, `phx-vendas` (6413, com `com.pedidos`) e
`phx-estoque` (6414, com `log.itens`). O cliente fala **só** com o hub.

```
hub: phxsqld pid 13287 na porta 6412
phx-vendas: phxsqld pid 13292 na porta 6413
phx-estoque: phxsqld pid 13297 na porta 6414

-- 1. o hub cadastra DUAS ligacoes -- e' isto que faz um DBLINK virar MULTILINK
ok   dblink_salvar 'vendas' -> phx-vendas
ok   dblink_salvar 'estoque' -> phx-estoque
ok   o hub lista as duas ligacoes  -- ['estoque', 'vendas']

-- 2. dblink_tabelas dos DOIS lados, na MESMA sessao do cliente com o hub
ok   tabelas vistas atraves de 'vendas'    pelo hub: ['pedidos']   direto: ['pedidos']
ok   tabelas vistas atraves de 'estoque'   pelo hub: ['itens']     direto: ['itens']

-- 3. UMA sessao de cliente com o hub, que ATRAVESSA os dois servidores
      relatorio combinado, pelo HUB, numa unica sessao de cliente:
        3 pedidos  (via dblink 'vendas',  phx-vendas :6413) somam  R$ 5051.40
        3 itens    (via dblink 'estoque', phx-estoque:6414) somam  167 unidades
ok   os pedidos, pelo hub contra o phx-vendas direto
ok   os itens, pelo hub contra o phx-estoque direto
ok   achado real desta rodada: SUM() nao existe na op sql -- so COUNT(*)
     [SP000018] esquema invalido: SQL, coluna 8: SUM() nao tem quem calcule
     embaixo. So COUNT(*) passa, porque a contagem sai do cabecalho da
     tabela em O(1)

-- 4. o hub continua SEM base propria de dado -- ele so LE de fora
ok   o hub nao tem nenhum database dele  -- []

-- 5. o limite real do MULTILINK aqui: nao ha SQL que atravesse os DOIS
ok   SQL nao enderega um DBLINK no FROM -- recusa (o hub nem tem database local)
     [SP000018] nao encontrado: database qualquer nao existe em dados
```

O achado do passo 3 é real e não estava suposto antes de rodar: a primeira
tentativa usou `SELECT SUM(total) FROM pedidos` pelo `dblink_consultar`, e
o próprio `phx-vendas` **recusou** — a op `sql` do PhxSql só sabe `COUNT(*)`
como agregado (sai do cabeçalho da tabela em O(1); qualquer outro agregado
exigiria varrer, e a op `sql` hoje não faz). A prova ficou de pé lendo as
linhas por `dblink_consultar` e somando do lado do cliente — que é
exatamente o que a tela do Centro de Controle faria numa grade combinada.

## O que NÃO existe, e é dispensa registrada

- **O pacote MultiLink proprietário, integrado.** `docs/MULTILINK.md`
  documenta as duas análises: a primeira (só binários) recusada por
  incompatibilidade de `rustc` e por ser dependência binária; a segunda (com
  os fontes) recusada porque o `Cargo.lock` resolve **582 crates externas**
  e exige um runtime assíncrono inteiro (`tokio`) dentro do `phxsqld`. Os
  números de crates não foram remedidos nesta rodada: o pacote
  (`PHOENIX_FONTES_MULTILINK_V10_S11_RECONCILIADO`) não está presente nesta
  máquina — procurado e não encontrado (`find / -iname '*multilink*'` e
  `*mldbx*`, fora dos próprios `docs/` do projeto). O que esta rodada prova é
  o **destino** que o MultiLink prometia — ver a tabela do outro banco pelo
  DbLink — alcançado sem ele, pelo hub acima.
- **`FROM` de SQL que atravesse um DbLink** — exercitado no passo 5: a
  sintaxe `FROM vendas.pedidos` não existe para endereçar uma ligação
  (`docs/SQL.md` só resolve `banco.schema.tabela` **local**). Quem combina
  dado de dois DbLinks é sempre o cliente (ou a tela), com uma chamada por
  ligação — nunca uma junção rodada dentro do servidor.
- **Agregado além de `COUNT(*)` na op `sql`** — achado nesta rodada (passo
  3), não documentado antes com este exemplo: `SUM()`, `AVG()`, `MIN()`,
  `MAX()` não têm tradução na op `sql` hoje. Isto não é limite do DbLink: é
  limite do tradutor SQL do PhxSql (`docs/SQL.md`), que o DbLink apenas
  repassa — `dblink_consultar` manda o texto inteiro para o motor SQL do
  servidor de origem, e o de origem tem a mesma limitação.
- **DbLink para MySQL(R)/PostgreSQL(R) real** — o código existe
  (`crates/phxsql-server/src/dblink/mysql.rs`, `.../pg/`, os testes
  `dblink-postgres-no-fio.rs`) e já foi provado em rodadas anteriores contra
  um MySQL(R) 8.0.46 e um PostgreSQL(R) 16.13 reais (`docs/DBLINK.md`,
  47 e 23 conferências respectivamente). **Não foi reexercitado nesta
  rodada**: há um PostgreSQL(R) 16.13 e um MySQL(R) 8.0.46 instalados nesta
  máquina, mas os dois serviços estão **parados**
  (`pg_lsclusters` mostra `down`; `mysqladmin ping` recusa a conexão), e
  subir/derrubar um serviço de sistema compartilhado por outras frentes
  desta mesma rodada está fora do escopo desta frente — dispensa registrada,
  não esquecimento. O caminho para refazer, se outra frente subir os dois
  serviços, é `python3 bancada/dblink/prova-mysql.py` e
  `python3 bancada/dblink/prova-postgres.py`.
- **Um terceiro hop** (hub → servidor B → servidor C dentro da MESMA
  consulta) — não existe e não foi tentado: `dblink_consultar` roda a
  instrução inteira dentro de UM servidor de origem só; ele não sabe que o
  destino tem os próprios DbLinks, e não haveria como saber sem entrar em
  laço. Multi-hop, se um dia for pedido, é trabalho novo.

## Como se refaz

```bash
python3 bancada/dblink/prova-phxsql.py       # DBLINK: dois phxsqld, 44 conferencias
python3 bancada/conexoes/multilink.py        # MULTILINK: hub + duas origens, tres phxsqld
```
