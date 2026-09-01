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
| PostgreSQL(R) | **cliente e dialeto**, provados contra um servidor de protocolo no soquete **e contra um PostgreSQL(R) 16.13 de verdade** — 19 conferências, cada uma contra o `psql` (`bancada/dblink/prova-postgres.py`) |

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

## A prova contra um PostgreSQL(R) de verdade

**A premissa desta seção caducou, e isso é resultado.** Ela dizia «não há
PostgreSQL(R) instalado na máquina onde este código foi escrito». Há: o
**16.13**, com `scram-sha-256` já configurado no `pg_hba.conf` para 127.0.0.1.
*A lista do que falta também é palpite até alguém medir* — inclusive quando o
palpite é nosso.

`bancada/dblink/prova-postgres.py` fecha o que faltava: as cinco operações
contra o servidor real, com **cada resposta conferida contra o `psql`**, que é
o oráculo independente. Dezenove conferências.

### O que a prova achou: um defeito com três sintomas

O servidor falso não tinha como achar isto, e a razão é a que já estava escrita
aqui — **servidor falso responde o que mandarem ele responder**.

| sintoma | o que se via |
|---|---|
| `dblink_tabelas` | lista **vazia**, sem erro nenhum |
| `dblink_estrutura` | colunas **vazias** |
| `dblink_ler` | `relation "bancada_phx.clientes" does not exist` |

**Uma causa só, e ela estava no chamador, não no dialeto.** O `base` que as
consultas de catálogo recebem quer dizer coisas diferentes nos dois motores —
e o dialeto sabia disso desde sempre, está na tabela do §*Onde os dois
divergem*. No MySQL(R) `base` é o database, que lá **é** o esquema. No
PostgreSQL(R) a conexão já está dentro do database, e o qualificador é o
**esquema**.

`base_escolhida` entregava o database da ligação nos dois casos. Do lado do
PostgreSQL(R) isso vira «procure o esquema `bancada_phx`», que não existe — e
os dois primeiros efeitos são **mudos**.

**O pior caso era o da tela.** Ela lista os bancos com `dblink_bancos` — que no
PostgreSQL(R) devolve **bancos** — e manda o escolhido de volta no campo
`database`. Resultado: a grade do DbLink com PostgreSQL(R) mostrava **nenhuma
tabela**, sem uma linha de erro.

### O que `database` quer dizer, por motor

| | MySQL(R) | PostgreSQL(R) |
|---|---|---|
| ausente | a base da ligação | **nada** — todos os esquemas de usuário |
| igual à base da ligação | a base da ligação | **todos os esquemas** dela (é o que a tela quer dizer) |
| outro nome | outro database | aquele **esquema** |

Cada tabela da resposta traz o campo `schema`, então quem precisa da distinção
não a perde.

### O que a prova conferiu

| | contra o quê |
|---|---|
| o SCRAM-SHA-256 | um servidor que **confere de verdade** a prova do cliente |
| `current_user` e `current_database` | o que o `psql` responde na mesma conexão |
| a lista de bancos | `pg_database` |
| a lista de tabelas, **pelos dois caminhos** | `pg_tables` |
| comentário da tabela e da coluna | `obj_description` e `col_description` |
| colunas, na ordem, e os tipos | `format_type`, como o próprio PostgreSQL os escreve |
| a chave primária | `pg_index` |
| o dado, e a **soma** | `sum(saldo)` |
| o booleano | ele manda `t`/`f`, e **não** `1`/`0` — a armadilha que o §*Onde os dois divergem* já nomeava, agora vista acontecer |
| o `reltuples` | **`-1`**, que é o que a 14+ devolve para tabela nunca analisada — e o DbLink publica `0`, não `-1` |

### Prova real, nos dois sentidos

Repondo o defeito, a prova **reprova em 14** das 19 e nomeia cada uma. E o
teste de unidade `no_postgres_a_base_da_ligacao_nao_vira_esquema` cai junto —
com o par `no_mysql_nada_muda` ao lado, que é o teste que mais importa numa
mudança destas: **nada do lado do MySQL(R) mudou**.

### O que continua de fora

Repetir contra **outras versões**. O `unnest(...) WITH ORDINALITY` pede 9.4+ e
o `reltuples` mudou de `0` para `-1` na 14; o código trata os dois, e o que
está provado é a 16.13. Uma 12 ou uma 13 na mesa fechariam a outra ponta.

---

MySQL, MariaDB, PostgreSQL, HFSQL e Clarion são marcas dos seus respectivos
donos, citadas aqui por referência técnica.


## A prova contra um MySQL(R) de verdade (0.18.0)

Feita a pedido, na máquina da bancada, contra o **MySQL(R) 8.0.46** real — não
o servidor falso do teste de fio. O roteiro e o resultado, para quem refizer:

1. No MySQL(R): base `crm`, tabela `clientes` (BIGINT, VARCHAR, DECIMAL(12,2),
   DATE, chave primária e índice `porCidade`), 5 linhas, usuário `phx` com
   `caching_sha2_password` — **o padrão do 8.x, de propósito**, porque é o
   caminho difícil.
2. `dblink_salvar` + `dblink_testar` + `dblink_tabelas` + `dblink_ler` +
   `dblink_estrutura` pela porta de dados, e a mesma coisa pela tela.

**O que a prova achou, nos dois sentidos:**

- **Primeira tentativa recusada, e a recusa é a documentada**: o servidor pediu
  a autenticação *completa* do `caching_sha2_password`, que exige TLS ou a
  chave RSA — nenhum dos dois cabe sem dependência externa. O erro nomeia as
  duas saídas, e as duas foram provadas:
  - **caminho rápido**: uma conexão de qualquer cliente oficial aquece o cache
    do servidor e o nosso handshake passa (testado: versão, `current_user()` =
    `phx@127.0.0.1`, 0 ms);
  - **caminho durável**: `ALTER USER ... IDENTIFIED WITH mysql_native_password`
    — sobrevive ao reinício do mysqld, e é o que o manual recomenda para a
    ponte.
- `dblink_tabelas` viu `clientes` (InnoDB, 5 registros estimados);
  `dblink_estrutura` trouxe os tipos com precisão do DECIMAL e os dois índices;
  `dblink_ler` trouxe as 5 linhas ordenadas — e a grade da tela somou o limite
  (37.851,25) sobre dados que nunca estiveram num arquivo nosso.

**A distinção que importa**: isto é o **DbLink nativo**, escrito aqui, zero
dependências — o caminho «por protocolo» que `docs/MULTILINK.md` recomenda. O
pacote MULTILINK proprietário continua fora pelas 582 crates que ele arrasta;
esta prova mostra que o destino dele já é alcançável sem ele.

## A sincronia de tabelas primas (0.18.0)

Uma tabela do PhxSql e a prima dela no outro banco, **gravando entre si**: a
linha que só existe de um lado é copiada para o outro, e a mesma linha
diferente nos dois é resolvida por quem for o **dono**. É convergência de
ESTADO pela chave primária — não é replicação de eventos, porque o diário do
outro banco não é lido. Três limites saem desse desenho, e são de desenho,
não de preguiça:

- **Exclusão não viaja.** Linha apagada de um lado REAPARECE na próxima
  rodada, vinda do outro. Propagar exclusão exigiria distinguir «apagada lá»
  de «nova aqui», e sem diário dos dois lados isso é adivinhação. Para apagar,
  apague nos dois antes da próxima rodada. (A prova do estágio 5 confere que
  este limite é verdade, não intenção.)
- **A chave primária é a identidade** — e tem de ser de UMA coluna. Chave
  composta recusa na ligação, com o motivo no texto.
- **O teto é o `max_linhas` da ligação.** Tabela maior recusa com erro claro;
  sincronizar metade e fingir que acabou seria pior que não sincronizar.

E um limite herdado do módulo, agora visível aqui: **texto com aspa simples
recusa o empurrão** (`Sant'Ana` não sobe). É a mesma decisão do `nome_seguro`
— escapar depende do modo do outro servidor (`NO_BACKSLASH_ESCAPES` muda o
que a contrabarra faz), então o que emendaria SQL é recusado com erro, nunca
emendado. Se um dia o escape entrar, entra por decisão medida (aspa dobrada
não depende do modo; a contrabarra sim), não por acidente.

### As duas operações

- **`dblink_ligar`** cria (ou confere) a tabela local espelhando a prima:
  tipos convertidos com as duas contas que não são óbvias — texto chega em
  BYTES do utf8mb4 (VARCHAR(60) viaja como 240; divide-se por 4), e o
  `DECIMAL(p,s)` chega como p+2 com casas e p+1 sem (sinal e ponto). A chave
  primária vira índice único local `porChave`, que é o que permite o upsert
  sem varrer. Cada tabela ligada guarda `sentido` (puxar / empurrar / dois) e
  `dono` (aqui / lá).
- **`dblink_sincronizar`** roda uma rodada e devolve o relatório: puxadas
  novas e alteradas, empurradas, iguais, conflitos. O empurrão usa
  `INSERT ... ON DUPLICATE KEY UPDATE` em lotes — cair no meio e recomeçar
  grava a mesma linha de novo e nada dobra (estágio 6 da prova: rodada
  repetida dá 0/0/0).

O conflito é **por linha**, nunca «marca tudo para um lado»: é a mesma lição
da janela de conflito da tela. E o casamento das colunas é **por nome**,
nunca por posição — pela posição, uma coluna acrescentada de um lado
deslocaria as seguintes e a sincronia gravaria cidade dentro de telefone,
com o CRC batendo.

Os portões de permissão são conferidos **dentro** da operação, contra o alvo
local (`local_database`.`local_tabela`): são operações sem o campo `tabela`
do pedido, exatamente o furo que o `juntar`/`unir` já ensinou.

### O assistente da tela

O botão **Assistente…** da tela DbLink monta tudo isso em cinco passos:
conexão → teste → base → tabelas (com sentido e dono por linha) → job. Cada
passo só avança com o anterior PROVADO — o teste tem de passar, a ligação tem
de gravar — porque um assistente que deixa pular o teste é um cadastro com
etapas. O último passo cria o job (`sincronia-<ligação>`) que roda a
convergência sozinho no intervalo escolhido, e dispara a primeira rodada na
hora, mostrando o relatório.

### A prova real, e o que ela ensinou

`bancada/dblink/prova-sincronia.py` roda os sete estágios contra o MySQL(R)
8.0.46 de verdade: ligar detecta a chave; a primeira rodada puxa tudo; linha
nova de cada lado atravessa; o dono vence o conflito; a exclusão local
reaparece (o limite é real); a rodada repetida é 0/0/0; e o job puxa sozinho.
O assistente foi exercitado no navegador de ponta a ponta (Playwright), com
captura de cada passo.

O que as provas acharam — e teria passado sem elas:

- **A ordem da puxada não é a ordem dos ids** (HashMap não garante ordem). O
  defeito era da própria prova, que supunha rowid=1 para o menor id; o
  conserto é achar o rowid pela chave (`buscar` no índice único). Teste que
  passa por engano é pior que teste que falta — de novo.
- **`buscar` espera a chave como LISTA** (`"chave": [1]`), não como objeto.
- **A árvore da tela não se remonta sozinha**: a sincronia criou o database
  local, o disco tinha, a tela não mostrava. É a lição da coluna de sistema
  por outro caminho — quando uma peça nova nasce no fim de um fluxo, procure
  quem monta a lista uma vez e nunca mais. O conserto (remontar a árvore no
  Fechar) foi provado no navegador nos dois sentidos: captura sem o database
  antes, com ele depois.
- **Decimal negativo menor que um perde o sinal na divisão inteira**: -0,50
  escalado é -50, o inteiro da divisão é 0, e 0 não carrega sinal. Sem o
  empréstimo do sinal, o outro banco gravaria crédito onde era dívida. O
  teste unitário falha com o defeito reposto e passa com o conserto.
