# Campo novo de pedido colide com campo do protocolo

*Descoberto em 03/09/2026, 17:12, escrevendo o DbLink de PhxSql para PhxSql
(pedido 166).*

## 1. O que aconteceu

O motor `phxsql` do DbLink precisa de **duas** credenciais para o outro
servidor, porque o PhxSql tem dois portões em série: o **token de serviço** (a
chave da porta da rede, conferido antes de tudo) e o **login**. Guardei o token
na `Definicao` e li do pedido o campo natural:

```rust
let token = j.texto_ou("token", "").to_string();   // dblink/mod.rs
```

A primeira execução da prova por soquete recusou o **cadastro**:

```
{"op":"dblink_salvar","nome":"b","motor":"phxsql","token":"prova-phx-b",…}
→ [SP000025] acesso negado: token invalido
```

O erro não era sobre o token do outro servidor. Era sobre o **deste**: `token`
já existe em todo pedido deste protocolo, e o portão 1 do `despachar` o lê
**antes** de qualquer operação (`servidor.rs`, «Portao 1 -- o token»). O campo
novo foi comido pelo portão, que comparou o token do `phx-b` com o do `phx-a` e
disse não.

O campo virou `token_remoto` / `token_remoto_env`.

## 2. O que eu concluí primeiro, e estava errado

Que o `dblink_salvar` estava recusando por **permissão**, e fui procurar se
`administrar` chegava ao root da prova — a operação exige `administrar`, e a
mensagem é `acesso negado`. Era plausível e era falso: o root da prova é
supervisor, e todas as outras operações passavam na mesma conexão.

O que denunciou foi o **contraste**: `dblink_testar` funcionava e
`dblink_salvar` não, na mesma sessão, com o mesmo usuário. Duas operações com a
mesma exigência de permissão não podem discordar sobre permissão — a diferença
tinha de estar no **corpo do pedido**, e a única diferença era o campo novo.

## 3. O que a medição disse

Da prova, com os dois servidores no ar:

| pedido | resposta |
|---|---|
| `dblink_salvar` com `"token": <token do phx-b>` | `acesso negado: token invalido` |
| o mesmo com `"token_remoto": <token do phx-b>` | `{"gravado": true}` |
| `dblink_testar` na mesma conexão, antes e depois | passa nos dois |

E o alcance: **nenhum teste de unidade acharia isto.** O `Definicao::de_json`
lê o campo certo em memória; o portão que o rouba só existe quando há um
servidor e um soquete. Foi a primeira execução da prova por soquete que pisou
nele — antes de qualquer conferência de dado.

Na mesma execução caiu um segundo, do mesmo naipe «pergunta feita ao valor
errado»: `dblink_bancos` filtrava com `b.texto_ou("", "")`, que procura um
**campo** de nome vazio num valor **escalar** e devolve sempre o padrão. Toda
base caía fora do filtro e a lista saía **vazia, sem erro nenhum** — o mesmo
sintoma mudo que a prova contra um PostgreSQL(R) de verdade já tinha achado na
lista de tabelas.

## 4. A regra

**Campo novo de pedido procura primeiro quem já usa aquele nome.** O protocolo
tem campos que o despachante lê antes da operação — `token`, `op`, `database`,
`tabela` —, e um cadastro que reaproveite um deles é comido pelo portão, com um
erro que fala do lugar errado.

É a irmã da pétrea que já existe («quando o portão passar a olhar um campo
novo, procure quem não tem esse campo») pelo lado oposto: aquela cobre o portão
que ganha um campo; esta, o **pedido** que ganha um campo que o portão já tem.

## 5. Como está guardado hoje

- O campo é `token_remoto`, com o motivo escrito no comentário do campo
  `token` da `Definicao` (`crates/phxsql-server/src/dblink/mod.rs`).
- A prova por soquete `bancada/dblink/prova-phxsql.py` cadastra pelo nome novo
  e confere que o token **não sai** na resposta do protocolo — 44 conferências,
  zero falhas.
- O `docs/DBLINK.md` traz a tabela dos dois defeitos que a prova achou, na
  seção *O terceiro motor*.
- **Onde o buraco ficou:** não há guarda automática que reprove um campo de
  cadastro homônimo de um campo do protocolo. Hoje a lista dos campos que o
  despachante lê está só no `despachar`, e conferi-la a mão. Uma catraca que
  cruzasse os nomes lidos por `Definicao::de_json` (e pelos outros cadastros:
  job, origem, rotina) com os campos do portão pegaria a próxima — e não
  existe.
