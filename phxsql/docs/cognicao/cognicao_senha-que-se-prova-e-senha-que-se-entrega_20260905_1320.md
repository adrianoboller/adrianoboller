# Senha que se PROVA e senha que se ENTREGA são coisas diferentes

**05/09/2026, 13:20** — descoberto medindo o `op_login` para instruir a frente
da cifra por tabela, depois de o dono pedir que o login do ambiente de gestão
receba a senha de acesso **e** a senha do banco.

## 1. O que aconteceu

O desenho de três níveis fechou com o dono: uma senha do SGBD, uma senha
**obrigatória por banco**, e uma lista **opcional** de tabelas cifradas com a
senha do banco. Aí veio o pedido que parecia só ergonomia:

> «No login do ambiente de gestão do sgdb posso passar a senha de acesso e a
> senha do banco e já abrir a conexão com o banco diretamente ligado.»

Duas senhas no mesmo formulário, dois campos no mesmo JSON. A leitura óbvia é
que a segunda entra no molde da primeira — o `op_login` já recebe senha, já
tem desafio-resposta, já tem segundo fator por Ed25519. Bastaria um campo.

Fui medir onde o campo encaixa, e o caminho mais bem-feito que existe ali é
justamente o único que **não serve**.

## 2. O que eu concluí primeiro, e estava errado

**Duas conclusões, e a segunda é a que ensina.**

**A primeira, mais antiga:** «a senha do banco mora no cofre». O módulo se
chama `cofre`, guarda senha, deriva chave — parece o lugar. Medido, é o único
lugar onde ela **não pode** morar: `COFRE` é um
`static Mutex<Option<Segredo>>` (`crates/phxsql-store/src/cofre.rs`), UM
`Segredo { senha, iteracoes, modo, ajuste }` para o processo inteiro. Com
senha por banco, dois logins simultâneos em bancos diferentes se
sobrescreveriam; e o `definir_com` ainda esvazia o cache de derivadas inteiro
a cada troca, de propósito — e o motivo escrito ali está **certo** para o
desenho de UMA senha. O nome do módulo apontava para o lugar errado.

**A segunda, e o aprendizado de verdade:** «a senha do banco entra no login
como um segundo campo, no mesmo molde da senha da conta».

Errado, e errado pelo motivo oposto do que eu esperava. O melhor caminho da
senha da conta — o desafio-resposta — existe exatamente para que a senha
**nunca chegue ao servidor**. O servidor guarda só o hash, e prova o
conhecimento sem nunca ter o segredo. Uma senha de cifra que nunca chega é uma
senha com a qual não se deriva chave nenhuma.

Ou seja: o caminho que parecia reúso é o caminho estruturalmente incapaz. Não
por falta de código — por projeto, e por um projeto que está **certo**.

## 3. O que a medição disse

`fn op_login`, `crates/phxsql-server/src/servidor.rs:6061`. Três caminhos de
senha da conta, medidos lendo o corpo inteiro:

| caminho | campo | a senha chega ao servidor? |
|---|---|---|
| 1 — desafio-resposta | `"prova"` + `"nonce_cliente"` | **não** |
| 2 — base64 | `"senha_b64"` | sim (dentro do túnel) |
| 3 — texto puro | `"senha"` | sim (dentro do túnel) |

O caminho 1 chama `phxsql_core::senha::derivado_do_hash(&u.senha_hash)` e
depois `desafio::conferir_prova(&dk, &nonce, nonce_cliente, &login, prova)`. E
o `derivado_do_hash` é uma linha só (`crates/phxsql-core/src/senha.rs:108`):
`destrinchar(guardado).map(|(_, _, hash)| hash)` — o servidor tem **o hash**, e
o hash é o que ele usa. A senha não está ali e não se reconstrói dali. Essa é a
prova de que o caminho 1 não entrega.

E onde a chave do banco pode morar, medido nos dois lados, porque eles são
diferentes:

| caminho | struct | onde | vive quanto |
|---|---|---|---|
| TCP | `Sessao` | `servidor.rs:253` | a conexão |
| web | `http::Sessao` | `http.rs:421` | até `expira_ms`, renovado a cada uso |

O da web é o que atravessa pedidos — o comentário da `http.rs:414` diz isso
com todas as letras, porque é o que faz o desafio-resposta por HTTP funcionar:
*«o nonce precisa sobreviver de um pedido para o outro, e a sessão é o único
lugar que atravessa os dois»*. A chave derivada do banco mora **ali**, e herda
de graça as três disciplinas que a sessão já tem: expira sozinha (`limpar`),
dá para derrubar (`encerrar_sessao`, `servidor.rs:12726`), e a listagem já
corta credencial de propósito — `Sessoes::listar` corta o id em oito letras,
com o motivo escrito ao lado.

Uma armadilha medida junto: `http::Sessao` deriva `Debug`
(`#[derive(Debug, Clone)]`, `http.rs:420`). Chave de cifra guardada ali sai
inteira no primeiro `{:?}` que alguém escrever.

## 4. A regra

**Antes de reusar um caminho de credencial, pergunte se ele ENTREGA o segredo
ou apenas PROVA que o outro lado o conhece — e só reúse quando o uso novo
precisa do que aquele caminho realmente dá.**

O corolário, que é a parte que muda documentação: quando as duas coisas
convivem no mesmo formulário, a mesma pétrea — *senha nunca em texto puro* —
se cumpre por caminhos **opostos**. A da conta se cumpre **não mandando** a
senha. A da cifra se cumpre mandando-a dentro do túnel e redigindo-a no log,
porque não mandar não é opção. Uma lei só, dois mecanismos; quem confundir os
dois ou enfraquece a primeira ou impossibilita a segunda.

## 5. Como está guardado hoje

**Não está.** A frente da cifra por tabela está de pé com esta medição nas
mãos (quarta correção enviada em 05/09 às 13:20), e nenhuma linha foi escrita
ainda. O que existe hoje continua sendo a senha única do processo, vinda do
`config.json`.

Dois buracos ficam nomeados, e o segundo é maior que o primeiro:

- **O `Debug` derivado da `http::Sessao`.** Se a chave entrar ali sem um tipo
  com `Debug` manual, o vazamento nasce junto com a funcionalidade. Vai como
  guarda pedida ao catálogo de QA, e não como comentário — comentário que se
  declara resolvido é o motivo de ninguém olhar de novo.

- **O preço de tirar a senha do `config.json` ainda não foi contado.** Se a
  senha vem só do login, a fronteira que o cabeçalho do `cofre.rs` documenta
  melhora de verdade: deixa de valer que *«quem lê o `config.json` desta
  máquina tem a senha»*. Mas ninguém abre tabela cifrada sem uma sessão viva
  que a tenha fornecido, e há quem abra tabela **sem sessão nenhuma** — o
  `reindexar` do arranque, a replicação, os jobs agendados, a recuperação de
  janela. Enquanto essa lista não sair medida, «senha só no login» é escolha
  plausível e não escolha medida; e a diferença entre as duas é exatamente o
  que esta casa cobra de qualquer receita de fora.
