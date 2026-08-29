# DbLink — ligações para bancos de fora

Uma ligação guarda um apelido, um endereço e uma credencial. Depois disso o
banco de fora aparece no Centro de Controle como se fosse mais um: as tabelas
na lista, o conteúdo na mesma grade que serve as tabelas do PhxSql — com
agrupamento, busca, totais e paginação valendo igual.

O nome vem do Centro de Controle do HFSQL(R), e a ideia é a mesma.

## O que já fala, e o que ainda não

| Motor | Estado |
|---|---|
| MySQL(R) / MariaDB(R) | **cliente e dialeto**, testado contra MySQL(R) 8.0.46 |
| PostgreSQL(R) | **cliente e dialeto**, provados contra um servidor de protocolo no soquete — **ainda não contra um PostgreSQL(R) de verdade** (§ *O que ainda falta provar*) |

O cliente é escrito aqui, com a `std` do Rust e nada mais — a mesma regra do
resto do projeto. Um protocolo de rede é um formato de bytes; ler e escrever
bytes a `std` faz.

## As três travas

**1. Toda operação de DbLink exige `administrar`.** Uma ligação guarda UMA
credencial, e quem a usa fala com o outro banco como aquele usuário: as
permissões por base do PhxSql não atravessam para o outro lado. Deixar um
leitor navegar por ela seria emprestar o poder de quem a criou.

**2. Uma ligação nasce somente-leitura.** Recusa qualquer coisa que não seja
`SELECT`, `SHOW`, `DESCRIBE`, `EXPLAIN`, `WITH`, `TABLE` ou `VALUES`. Ligar a
escrita é uma decisão, e não um padrão herdado: a mesma tela que lista as
tabelas de um banco de produção apagaria uma se a escrita viesse ligada.

Duas coisas sustentam a trava, e as duas precisam existir:

- a primeira palavra tem de ser de consulta, olhada **depois** de tirar
  comentário — senão `/*x*/ SELECT 1` seria recusado sem motivo;
- `INTO OUTFILE` e `INTO DUMPFILE` estão fora, porque um `SELECT` que escreve
  arquivo no servidor do outro lado continua sendo escrita.

Emendar uma segunda instrução com `;` não entra na conta porque não é
possível: o cliente não pede `CLIENT_MULTI_STATEMENTS`, e o servidor recusa o
pacote com duas.

**3. Nome de objeto é conferido, e não escapado.** Escapar aspas exige saber em
que modo o outro servidor está — com `NO_BACKSLASH_ESCAPES` a contrabarra
deixa de escapar, e a mesma regra que protegia passa a não proteger. Nome de
tabela, coluna ou base não precisa de aspa, crase, contrabarra nem quebra de
linha, então nada disso passa. O que sobra ainda vai entre crases.

## O limite honesto: não há TLS

A `std` não traz TLS e o projeto não aceita dependência externa. A conversa com
o banco do outro lado é em **texto claro**.

- A **senha nunca viaja em texto**: o MySQL(R) a embaralha com um sal que muda
  a cada conexão.
- O **dado devolvido viaja em texto**. Use rede interna, VPN ou túnel.

Por isso também não há `mysql_clear_password`: mandar a senha em claro seria
entregá-la a quem estiver no caminho.

## As duas autenticações do MySQL(R)

**`mysql_native_password`** — funciona sempre. A conta é

```
SHA1(senha) XOR SHA1( sal || SHA1(SHA1(senha)) )
```

e é por causa dela que o SHA-1 existe no projeto. Ele não é usado em lugar
nenhum do formato do PhxSql: senha continua em PBKDF2-HMAC-SHA256, e
integridade em CRC-32 e SHA-256. Quem define o protocolo é o outro lado.

**`caching_sha2_password`** (o padrão do MySQL(R) 8) — só o **caminho rápido**,
que vale quando o servidor já tem a senha daquele usuário em cache. O caminho
completo exige mandar a senha cifrada com a chave pública RSA do servidor, ou
TLS, e nenhum dos dois cabe na `std`.

Quando o servidor pede o caminho completo, o erro diz isso e as duas saídas:

```
ALTER USER 'fulano'@'%' IDENTIFIED WITH mysql_native_password BY '...'
```

ou conectar uma vez com o cliente oficial, o que deixa a senha em cache e
libera o caminho rápido até o servidor reiniciar.

## Onde as ligações ficam

Num arquivo próprio, apontado por `dblink` no `config.json` (padrão
`dblink.json`). Separado do `config.json` de propósito: o cadastro muda pela
tela, e reescrever o `config.json` inteiro a cada ligação nova arriscaria os
comentários e o resto da configuração.

```json
{
  "dblink": [
    {
      "nome": "matriz",
      "motor": "mysql",
      "host": "10.0.0.20",
      "porta": 3306,
      "usuario": "leitor",
      "senha_env": "PHXSQL_DBLINK_MATRIZ",
      "database": "erp",
      "descricao": "ERP da matriz",
      "somente_leitura": true,
      "timeout_s": 10,
      "max_linhas": 1000
    }
  ]
}
```

O arquivo é gravado com permissão só do dono (`0600`) e trocado de forma
atômica — um corte de energia no meio deixa o arquivo antigo inteiro, e não um
cadastro pela metade.

**A senha fica nele em texto**, porque precisa ser apresentada ao outro banco:
não dá para guardar só o hash, como se faz com a senha de usuário do PhxSql.
Quem preferir não tê-la em arquivo usa `senha_env` e deixa o valor numa
variável de ambiente — que é o caminho recomendado, porque `config.json` e
`dblink.json` costumam ir para o controle de versão e variável de ambiente
não. Em nenhum dos dois casos a senha aparece na resposta do protocolo, na
tela ou no log.

## As operações

Todas exigem `administrar`.

| Operação | O que faz |
|---|---|
| `dblink` | lista as ligações cadastradas, sem as senhas |
| `dblink_salvar` | cria ou substitui uma ligação |
| `dblink_excluir` | apaga uma ligação (o banco do outro lado não é tocado) |
| `dblink_testar` | conecta, dá `ping` e diz versão, usuário efetivo e base |
| `dblink_bancos` | as bases do outro servidor |
| `dblink_tabelas` | as tabelas de uma base, com tamanho e comentário |
| `dblink_estrutura` | colunas e índices de uma tabela |
| `dblink_ler` | o conteúdo de uma tabela, paginado |
| `dblink_consultar` | uma instrução escrita à mão |

`dblink_salvar` sem o campo `senha` **mantém** a que já estava. É o que faz a
tela de edição funcionar: ela nunca recebe a senha, então não teria como
devolvê-la, e sem essa regra mudar a porta apagaria a credencial.

`dblink_consultar` pede que **as duas** travas cedam para escrever: a ligação
não pode ser somente-leitura E este servidor também não. Um espelho não vira
caminho de escrita para o banco do outro só porque a ligação permitia.

## O que o cliente não faz

- Sem TLS (acima).
- Sem protocolo binário nem instrução preparada — só `COM_QUERY`.
- Sem `LOCAL INFILE`: aceitar seria deixar o servidor do outro lado pedir
  arquivo **desta** máquina.
- Sem `CLIENT_MULTI_STATEMENTS`.
- Carga acima de 16 MB chega partida em vários quadros; a leitura junta, a
  escrita não parte (nenhuma consulta que este cliente manda chega perto).

## O cliente PostgreSQL(R)

Está em `crates/phxsql-server/src/pg/`, e nasceu de um pedido que parecia
grande e não era: **os tijolos já existiam**. O `scram-sha-256` que o
PostgreSQL(R) 10+ usa por padrão é feito de SHA-256, HMAC e PBKDF2 — os três
escritos aqui, anos antes, para o hash de senha do `config.json`.

O protocolo em si tem uma pegadinha que vale escrever: **o `int32` de tamanho
inclui os próprios 4 bytes**, e não inclui o byte de tipo. Errar por um aqui
produz um cliente que "quase" funciona.

A outra é o `ReadyForQuery`. Uma consulta responde `T`, `D`…`D`, `C` e só então
`Z`. Parar de ler no `C` — que é onde a resposta *parece* acabar — deixa o `Z`
na fila, e a **próxima** consulta lê a resposta da anterior. Tudo continua
"funcionando", com um desencontro constante de uma mensagem. Por isso o laço
lê sempre até o `Z`, e por isso um erro do servidor é **guardado** e não
devolvido na hora: sair no `E` deixaria o `Z` para trás.

### As três autenticações, e por que só uma entra

| método do `pg_hba.conf` | o que este cliente faz |
|---|---|
| `scram-sha-256` | **faz**, conferido contra o vetor da §3 do RFC 7677 |
| `md5` | recusa: exige MD5, que está quebrado e não entra neste projeto |
| `password` | **recusa de propósito**: sem TLS a senha iria legível no fio |

O `password` é o que merece explicação, porque tecnicamente daria para fazer em
três linhas. A regra do projeto é que senha não viaja em claro, e um cliente
que aceitasse isso calado tiraria de quem configurou o servidor uma decisão que
é dele. Os dois casos recusam dizendo **qual linha mudar** no `pg_hba.conf`.

E a autenticação é **mútua**: o cliente confere a assinatura que o servidor
manda no fim. Sem essa conferência, qualquer um no meio do caminho poderia
dizer "pode entrar" sem conhecer a senha — e receberia a consulta seguinte.

## O dialeto

O cliente conectava, autenticava e consultava, e `Motor::conecta()` continuava
`false`. Não era esquecimento: `dblink_tabelas`, `dblink_estrutura` e o teste de
ligação montavam SQL de MySQL(R) — crase em volta do nome, `SHOW INDEX`,
`current_user()` —, e acender o sinal teria ligado um botão que falha na
primeira consulta.

O que faltava está em `dblink/dialeto.rs`, e as operações passaram de
`servidor.rs` para `dblink/operacoes.rs` para ficarem ao lado dele.

### Por que duas vezes, e não um SQL «portátil»

Porque as perguntas não têm resposta portátil. O MySQL(R) responde «quais são
os índices desta tabela» com `SHOW INDEX`; o PostgreSQL(R) responde com um
`JOIN` de `pg_index`, `pg_class` e `pg_attribute`. Um SQL que servisse aos dois
seria o que **nenhum** dos dois faz bem — e a diferença reapareceria no formato
do resultado de qualquer jeito.

O que o dialeto garante é que as **colunas saem na mesma ordem** nos dois, e
isso está no SQL, onde dá para ler as duas versões lado a lado. Não há camada
que reordene nada depois.

### As diferenças que aparecem em todo lugar

| | MySQL(R) | PostgreSQL(R) |
|---|---|---|
| identificador | crase, dobrada para escapar | aspas duplas, dobradas |
| paginação | `LIMIT n OFFSET m` (e também `LIMIT m, n`) | só `LIMIT n OFFSET m` |
| bases | `SHOW DATABASES` | `SELECT datname FROM pg_database` |
| colunas | `SHOW FULL COLUMNS` | `pg_attribute` + `format_type` |
| índices | `SHOW INDEX` | `pg_index` + `unnest(indkey) WITH ORDINALITY` |
| quem sou | `current_user()` | `current_user` — **sem parênteses**, é palavra reservada |
| booleano lido | `1` / `0` | `t` / `f` |
| data literal | `'2026-08-29'` | `DATE '2026-08-29'` |
| inteiro sem sinal | `BIGINT UNSIGNED` | **não existe**: sobe um tamanho, e `u64` vira `numeric(20,0)` |

Três delas merecem nota, porque erram calado:

**`LIMIT m, n` é a forma que os exemplos de MySQL(R) usam**, e o PostgreSQL(R)
não a entende. O dialeto emite sempre `LIMIT n OFFSET m`, que os dois aceitam.

**`current_user()` com parênteses é erro de sintaxe no PostgreSQL(R)** — lá é
palavra reservada, não função.

**Booleano lido volta `t`/`f` de um e `1`/`0` do outro.** Uma comparação
ingênua (`== "1"`) trata todo booleano do PostgreSQL(R) como falso, sem erro
nenhum — que é o pior jeito de estar errado. `dialeto::booleano_lido` entende
as duas formas.

### E «database» quer dizer coisas diferentes

No MySQL(R) uma conexão enxerga todas as bases do servidor e troca entre elas.
No PostgreSQL(R) uma conexão enxerga **uma** base e os esquemas dela; trocar de
base exige reconectar. Por isso o campo `database` de `dblink_tabelas` filtra a
base num motor e o **esquema** no outro — e é por isso que a ligação para
PostgreSQL(R) precisa do `database` certo no cadastro, e não só do host.

## A prova, e o que ainda falta provar

**Não há PostgreSQL(R) instalado na máquina onde este código foi escrito.** Isso
está dito aqui porque muda o valor do que foi provado.

O que **foi** provado, em `crates/phxsql-server/tests/dblink-postgres-no-fio.rs`:
um servidor que fala o protocolo de fio sobe num soquete e confere, byte a
byte, o que o cliente manda.

| o que se prova | como |
|---|---|
| a mensagem de abertura leva `user`, `database`, `client_encoding`, `application_name` | o servidor lê os pares e compara |
| o `int32` de tamanho **inclui a si mesmo** em toda mensagem | o servidor confere tamanho declarado × lido |
| o SCRAM-SHA-256 | o servidor **refaz a conta do RFC 5802 e confere a prova do cliente** |
| a senha nunca viaja | o servidor procura a senha nos bytes de toda mensagem recebida |
| o SQL é o do dialeto certo | compara o `Q` recebido com a cadeia esperada, e recusa crase |
| erro do servidor não desencontra o ciclo | manda `E` e depois responde a consulta **seguinte** |
| `md5` e `password` são recusados | o servidor os oferece e o cliente sai com o erro que diz o que mudar |

E, além disso, a implementação do SCRAM continua conferida contra o **vetor da
§3 do RFC 7677**, no teste de `pg/scram.rs`.

### O que o servidor falso **não** prova

Que o SQL do dialeto é aceito por um PostgreSQL(R) de verdade. Um servidor
falso responde o que mandarem ele responder: não valida sintaxe, não tem
`pg_class` e não sabe se `unnest(...) WITH ORDINALITY` existe na versão do
outro lado.

**Essa prova continua pendente.** O que ela exige:

1. um PostgreSQL(R) 10 ou mais novo alcançável pela rede, com
   `scram-sha-256` na linha do usuário no `pg_hba.conf`;
2. uma base com ao menos uma tabela, um índice e um comentário — o suficiente
   para as três consultas de catálogo devolverem linha;
3. rodar `dblink_testar`, `dblink_bancos`, `dblink_tabelas`,
   `dblink_estrutura` e `dblink_ler` contra ela e comparar com o que o
   `psql` mostra;
4. repetir contra as versões que interessam: `unnest(...) WITH ORDINALITY`
   pede 9.4+, e `reltuples` devolve `-1` em vez de `0` para tabela nunca
   analisada a partir da 14 — o código já trata os dois, e o que falta é
   **ver acontecer**.

Até lá, a linha honesta é a da tabela do começo: cliente e dialeto escritos e
provados no fio, **não** provados contra o servidor real.

---

MySQL, MariaDB, PostgreSQL, HFSQL e Clarion são marcas dos seus respectivos
donos, citadas aqui por referência técnica.
