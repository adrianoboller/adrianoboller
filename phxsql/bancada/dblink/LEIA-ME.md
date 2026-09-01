# A bancada do DbLink

Duas provas, contra os dois motores **de verdade** — não contra servidores de
protocolo, que respondem o que mandarem eles responder.

| Arquivo | Contra o quê | O que prova |
|---|---|---|
| `prova-sincronia.py` | MySQL® 8.0.46 | a sincronia de tabelas primas: sentido, dono por linha, reentrância, e o limite documentado de a exclusão não viajar |
| `prova-postgres.py` | PostgreSQL® 16.13 | o cliente e o dialeto: SCRAM, catálogo, tipos, chave, comentários e o dado — **19 conferências, cada uma contra o `psql`** |

## Por que contra o `psql`, e não contra o que o script espera

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

## Como refazer

```bash
cargo build --release                 # a regra do binário velho
service postgresql start
python3 bancada/dblink/prova-postgres.py
```

A prova sobe um `phxsqld` próprio (porta 7480), mata **pelo PID** e apaga o que
criou. O PostgreSQL ela **não** sobe nem derruba: se não estiver no ar, diz o
comando e para.

### A base da prova, se ela não existir

```bash
su postgres -c psql <<'SQL'
CREATE ROLE phxprova LOGIN PASSWORD 'prova-1234';
CREATE DATABASE bancada_phx OWNER phxprova;
SQL
su postgres -c "psql -d bancada_phx" <<'SQL'
CREATE TABLE clientes (
  id integer PRIMARY KEY, nome varchar(40) NOT NULL, cidade varchar(20),
  saldo numeric(15,2), ativo boolean, cadastro date);
CREATE INDEX por_cidade ON clientes (cidade);
COMMENT ON TABLE  clientes        IS 'Cadastro de clientes da bancada';
COMMENT ON COLUMN clientes.cidade IS 'Cidade do cliente';
INSERT INTO clientes VALUES
  (1,'Ana Prado','Blumenau',1500.50,true,'2024-10-04'),
  (2,'Bruno Reis','Joinville',2750.00,false,'2024-11-15'),
  (3,'Carla Lima','Itajai',980.25,true,'2025-01-20'),
  (4,'Diego Souza','Curitiba',12000.75,true,'2025-03-08'),
  (5,'Elisa Nunes','Chapeco',430.00,false,'2025-06-30');
GRANT ALL ON clientes TO phxprova;
SQL
```

A tabela tem **comentário na tabela e numa coluna** de propósito: é o que faz
`obj_description` e `col_description` devolverem linha, e sem eles duas
conferências passariam por vacuidade. E ela **não é analisada**, também de
propósito — é assim que o `reltuples` devolve o `-1` da 14+.

## Prova real, nos dois sentidos

Repondo o defeito (o `base` voltando a ser o database no PostgreSQL®), a prova
**reprova em 14** das 19 conferências e nomeia cada uma. Com o conserto, 19
verdes. Há também o teste de unidade `no_postgres_a_base_da_ligacao_nao_vira_esquema`,
que cai se alguém repuser o defeito — e o par dele,
`no_mysql_nada_muda`, que é o teste que mais importa numa mudança destas.
