# DbLink — ligações para bancos de fora

Uma ligação guarda um apelido, um endereço e uma credencial. Depois disso o
banco de fora aparece no Centro de Controle como se fosse mais um: as tabelas
na lista, o conteúdo na mesma grade que serve as tabelas do PhxSql — com
agrupamento, busca, totais e paginação valendo igual.

O nome vem do Centro de Controle do HFSQL(R), e a ideia é a mesma.

## O que já fala, e o que ainda não

| Motor | Estado |
|---|---|
| MySQL(R) / MariaDB(R) | **cliente escrito**, testado contra MySQL(R) 8.0.46 |
| PostgreSQL(R) | a definição já pode ser guardada; o cliente ainda não existe |

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

---

MySQL, MariaDB, PostgreSQL, HFSQL e Clarion são marcas dos seus respectivos
donos, citadas aqui por referência técnica.
