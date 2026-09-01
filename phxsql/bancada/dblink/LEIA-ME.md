# A bancada do DbLink

Três provas, contra os dois motores **de verdade** — não contra servidores de
protocolo, que respondem o que mandarem eles responder.

| Arquivo | Contra o quê | O que prova |
|---|---|---|
| `prova-sincronia.py` | MySQL® 8.0.46 | a sincronia de tabelas primas: sentido, dono por linha, reentrância, e o limite documentado de a exclusão não viajar |
| `prova-postgres.py` | PostgreSQL® 16.13 | o cliente e o dialeto: SCRAM, catálogo, tipos, chave, comentários e o dado — **19 conferências, cada uma contra o `psql`** |
| `prova-mysql.py` | MySQL® 8.0.46 | o cliente e o dialeto do outro lado: identidade, catálogo, forma da resposta, acento, NULO, estimativa e paginação — **47 conferências, cada uma contra o `mysql`** |

## Por que contra o cliente oficial, e não contra o que o script espera

Conferir contra o que o script espera prova que o script e o servidor
concordam. Conferir contra o cliente **oficial** do outro motor, pelo mesmo
transporte, é o que prova que a resposta está certa: dois códigos sem uma linha
em comum têm de dizer a mesma coisa.

## O que a prova do PostgreSQL® achou no primeiro minuto

A premissa que a abria envelheceu: o `docs/DBLINK.md` dizia *«não há
PostgreSQL® instalado na máquina onde este código foi escrito»*, e há — o
16.13, com `scram-sha-256` já no `pg_hba.conf`. **A lista do que falta também é
palpite até alguém medir**, inclusive quando o palpite é nosso.

E rodá-la achou **um defeito com três sintomas**, todos invisíveis ao servidor
falso:

| sintoma | o que se via |
|---|---|
| `dblink_tabelas` | lista **vazia**, sem erro nenhum |
| `dblink_estrutura` | colunas **vazias** |
| `dblink_ler` | `relation "bancada_phx.clientes" does not exist` |

Uma causa só: `base` quer dizer coisas diferentes nos dois motores. No MySQL®
é o database — que lá **é** o esquema. No PostgreSQL® a conexão já está dentro
do database, e o qualificador é o **esquema**. Mandar o database como esquema
procura um esquema que não existe, e os dois primeiros efeitos são **mudos**.

O pior caso era o da tela: ela lista os bancos com `dblink_bancos` e devolve o
escolhido em `database` — o que fazia a grade do DbLink com PostgreSQL® mostrar
**nenhuma tabela**, calada.

## O que a prova do MySQL® achou: o mesmo defeito, do outro lado

O sintoma mudo tinha um irmão gêmeo no MySQL®, e ele estava lá desde sempre:
**uma ligação salva sem base padrão listava zero tabelas, sem erro nenhum.**

| sintoma | o que se via |
|---|---|
| `dblink_tabelas` | lista **vazia**, sem erro nenhum |
| `dblink_estrutura` | `MySQL 1046: No database selected` |
| `dblink_ler` | `MySQL 1046: No database selected` |

Uma causa só, e desta vez **no SQL, não no chamador**: com base vazia o dialeto
perguntava `TABLE_SCHEMA = DATABASE()`, e sem base padrão `DATABASE()` é
**NULO**. Em SQL `x = NULL` nunca é verdadeiro — então aquela cláusula não casa
com nada, nunca. E o ramo só é alcançado quando não há base padrão, ou seja:
**ele estava sempre vazio.**

`dblink_salvar` aceita `database` vazio, e num servidor de MySQL® uma conexão
enxerga todas as bases — então a ligação sem base padrão é a forma **natural**
de navegar por várias. A tela escapava por sorte (`DBL.database = … ||
bases[0]` escolhe a primeira que o `dblink_bancos` devolveu); quem fala pela
porta de dados, pelo MCP ou por script, não.

O conserto é o simétrico do que o ramo do PostgreSQL® já fazia com base vazia:
em vez de nomear um esquema, **tirar os de sistema** e listar o que aquele
usuário enxerga. Cada linha já traz o campo `schema`, então a resposta carrega
o que a pergunta seguinte precisa — e a prova confere justamente isso, pedindo
a estrutura de volta com o `schema` que a linha trouxe.

Os testes de unidade são o par de sempre, em `dblink/dialeto.rs`:
`sem_base_padrao_o_mysql_nao_compara_com_o_database_nulo` (o novo) e
`com_base_escolhida_nada_muda_e_o_postgres_nao_se_mexeu` — que é **o que mais
importa numa mudança destas**: com base escolhida, que é o caso comum e o que a
tela cadastra, a consulta é byte por byte a de antes.

## A armadilha do oráculo, que custou uma hipótese inteira

A primeira leitura trouxe `ItajaÃ­` e `ChapecÃ³` no lugar de `Itajaí` e
`Chapecó`, e a hipótese pronta era *«o cliente pede o conjunto errado no aperto
de mão»*. Medida, ela morreu: `HEX(cidade)` no próprio MySQL® devolvia
`4974616A61C383C2AD` — **o dado já estava duplo-codificado no disco**, e quem o
gravou assim foi o `mysql` desta máquina, que abre em `latin1`.

Ou seja: o defeito estava no **oráculo**, e conferir contra ele teria acusado o
DbLink de um erro que era do lado de cá. Por isso a prova hoje **começa**
conferindo o oráculo (`@@character_set_client` = `utf8mb4`) antes de fazer
qualquer pergunta, e todo `mysql` que ela chama leva
`--default-character-set=utf8mb4`. Sem isso a comparação é entre **transportes**
e não entre motores.

## Como refazer

```bash
cargo build --release                 # a regra do binário velho
service mysql start
python3 bancada/dblink/prova-mysql.py
```

A prova sobe um `phxsqld` próprio (porta 7490), mata **pelo PID** e apaga o que
criou. O `mysqld` ela **não** sobe nem derruba — derrubar o banco de outra
frente na mesma máquina custa mais do que a prova vale: se não estiver no ar,
ela diz o comando e para. A base ela monta sozinha se faltar, pelo soquete
local do root.

### A base da prova, se ela não existir

O script a monta; o SQL está aqui porque **receita que só existe dentro do
script morre com ele**:

```bash
mysql --default-character-set=utf8mb4 <<'SQL'
CREATE DATABASE IF NOT EXISTS bancada_phx
  CHARACTER SET utf8mb4 COLLATE utf8mb4_general_ci;
CREATE USER IF NOT EXISTS 'phxprova'@'%'
  IDENTIFIED WITH mysql_native_password BY 'prova-1234';
GRANT ALL ON bancada_phx.* TO 'phxprova'@'%';
FLUSH PRIVILEGES;
SQL
mysql --default-character-set=utf8mb4 bancada_phx <<'SQL'
CREATE TABLE clientes (
  id       INT NOT NULL,
  nome     VARCHAR(40) NOT NULL,
  cidade   VARCHAR(20) COMMENT 'Cidade do cliente',
  saldo    DECIMAL(15,2),
  ativo    TINYINT(1),
  cadastro DATE,
  apelido  VARCHAR(20) DEFAULT NULL,
  PRIMARY KEY (id),
  KEY por_cidade (cidade)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4
  COMMENT='Cadastro de clientes da bancada';
INSERT INTO clientes VALUES
  (1,'Ana Prado','Blumenau',1500.50,1,'2024-10-04','ana'),
  (2,'Bruno Reis','Joinville',2750.00,0,'2024-11-15',NULL),
  (3,'Carla Lima','Itajaí',980.25,1,'2025-01-20',''),
  (4,'Diego Souza','Curitiba',12000.75,1,'2025-03-08',NULL),
  (5,'Elisa Nunes','Chapecó',430.00,0,'2025-06-30','lisa');
CREATE TABLE sem_analise (id INT PRIMARY KEY, texto VARCHAR(10))
  ENGINE=InnoDB STATS_AUTO_RECALC=0;
INSERT INTO sem_analise VALUES (1,'um'),(2,'dois'),(3,'tres');
SET SESSION cte_max_recursion_depth = 5000;
INSERT INTO sem_analise
  SELECT n+3, CONCAT('t',n) FROM (
    WITH RECURSIVE s(n) AS (SELECT 1 UNION ALL SELECT n+1 FROM s WHERE n<2000)
    SELECT n FROM s) x;
SQL
```

Cada peça está lá por um motivo, e sem ela uma conferência passaria por
vacuidade:

- **comentário na tabela e numa coluna**, para que `TABLE_COMMENT` e
  `COLUMN_COMMENT` devolvam texto — sem eles, comparar `""` com `""` não prova
  nada;
- **`apelido` com um NULO e uma cadeia vazia** na mesma coluna, que é o que
  separa `None` de `""` na leitura do protocolo;
- **acento em `Itajaí` e `Chapecó`**, que é o utf8mb4 no fio;
- **`DECIMAL(15,2)`**, porque sem o campo `decimais` a grade arredondava
  15000,50 para 15.001;
- **`sem_analise` com `STATS_AUTO_RECALC=0` e 2.000 linhas inseridas depois de
  nascer com três**, que é a tabela **nunca analisada**: `TABLE_ROWS` fica em 3
  enquanto `COUNT(*)` é 2.003 — **668× de desvio**. É o análogo MySQL® do `-1`
  do PostgreSQL®, e é o que prova que `registros_estimados` publica a
  **estimativa do servidor** e não uma contagem.

## Prova real, nos dois sentidos

Quatro defeitos repostos, cada um compilado e rodado contra o MySQL® real:

| defeito reposto | onde | conferências que caem |
|---|---|---|
| `TABLE_SCHEMA = DATABASE()` de volta (o defeito de verdade) | `dialeto.rs` | **2**, + 1 pulada dizendo que pulou |
| `LIMIT {limite}, {salto}` — os dois números trocados | `dialeto.rs` | **11** |
| `SHOW COLUMNS` no lugar de `SHOW FULL COLUMNS` | `dialeto.rs` | **3** |
| `NULL` do protocolo virando cadeia vazia | `mysql.rs` | **3** |

Com os quatro consertados, **47 verdes**.

E o que a prova real ensinou por não pegar: repor `LIMIT {salto}, {limite}` —
a forma «só do MySQL®» que o comentário do dialeto avisa — **passou nas 47**.
E está certo que passe: `LIMIT m, n` e `LIMIT n OFFSET m` querem dizer a mesma
coisa, e o risco daquela forma não é de correção no MySQL®, é de
**portabilidade** para o PostgreSQL®. Quem guarda isso é o teste de unidade
`a_paginacao_sai_na_forma_que_os_dois_entendem`, não esta prova. **Uma prova
contra um motor não pode provar o que só aparece contra o outro** — e saber
qual guarda cobre o quê vale mais do que somar as duas contagens.

## O que esta prova NÃO prova

- **A forma da resposta de `dblink_estrutura` não é a mesma nos dois motores.**
  Do lado do MySQL® são as nove colunas do `SHOW FULL COLUMNS` (`Field`,
  `Type`, `Collation`, `Null`, `Key`, `Default`, `Extra`, `Privileges`,
  `Comment`); do lado do PostgreSQL® são seis, montadas para casar com uma
  ordem que o `SHOW FULL COLUMNS` **não** tem. A prova confere a forma contra o
  cabeçalho que o próprio `mysql` imprime, e por isso lê tudo **por nome** e
  nunca por posição — mas ela não conserta a divergência, só a documenta.
- **`caching_sha2_password` pelo caminho completo** continua fora: a prova usa
  `mysql_native_password`, que é o que a `docs/DBLINK.md` recomenda para a
  ponte. O caminho completo exige TLS ou a chave RSA, e nenhum dos dois cabe
  sem dependência externa.
- **Truncamento**: `dblink_tabelas` corta em 5.000 tabelas e a resposta **não**
  carrega o `truncado` que o resultado interno já tem. A prova não chega perto
  do teto, então não o exercita.
- **Queda de conexão** no meio de uma leitura: é do sistema operacional, e
  *o que depende do sistema operacional se prova contra o sistema operacional*
  — como a prova do `BULKINSERT` fez pelo soquete, e não por teste unitário.
