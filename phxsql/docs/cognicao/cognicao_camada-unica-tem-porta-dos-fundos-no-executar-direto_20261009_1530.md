# A camada "única" de proteção tinha porta dos fundos: quem chama o `executar` direto

**Estado:** PENDENTE

- **Quando:** 2026-10-09, 15:30
- **Onde:** `crates/phxsql-server/src/servidor/servico_sql_01.rs` (`sql_de_cadastro`),
  `crates/phxsql-server/src/servidor/servico_permissao_01.rs` (`executar_e_contar_escrita_local`)
- **Pedido:** 765/767, fatia P1

## O que aconteceu

O desenho pôs a camada de proteção no `executar_e_contar_escrita_local`, «por onde passam a
rede, a op `sql` e o job». Procurando quem chama `self.executar(` sem passar por ali, saíram
três pontos no `servico_sql_01.rs`: o das diretivas, o da transação e o `sql_de_cadastro`. Os
dois primeiros não executam nada da lista de perigo. O terceiro executa `CREATE/ALTER/DROP
USER` pela op `sql` direto no `executar`. Com a camada só no ponto do desenho, `DROP USER`
pelo SQL passaria sem a senha de execução, enquanto `usuario_excluir` pelo JSON seria recusado.

## O que eu concluí primeiro, e estava errado

Que «os três irmãos» cobriam todo comando, porque o comentário do `aplicar_direito_por_coluna`
diz que são três, e o desenho repete. A lista vale para quem chama `portoes_do_pedido` e depois
`executar`. O `sql_de_cadastro` não chama os portões: confia no `exigir_administrar` de dentro
da op. Para o direito por coluna isso não importa, porque cadastro não tem coluna. Para a
camada de proteção importa.

## O que a medição disse

`grep -rn "self\.executar(" crates/phxsql-server/src` fora do `servico_permissao_01.rs` deu
3 chamadas no `servico_sql_01.rs`, e 1 delas executa uma op da lista (`usuario_excluir` e
`usuario_alterar`). O teste `o_drop_e_recusado_pela_rede_pelo_sql_e_pelo_job` cobre o
`DROP USER` pelo SQL.

Há um segundo ponto do mesmo tipo: o plano largo. O `DELETE` por faixa chega à camada como mil
`excluir` de uma linha cada, e o tamanho só existe onde a lista de rowids fecha. A decisão
continua sendo uma só (`protecao::Veredito`), mas tem três pontos que perguntam.

## A regra

Antes de chamar uma camada de «única», procure quem chama a função de baixo (`executar`) sem
passar por ela. A lista de irmãos de outro portão não serve de inventário para este.

## Como está guardado hoje

O `sql_de_cadastro` passou a perguntar à camada (`protecao_do_pedido`), e o teste citado acima
recusa o `DROP USER`. Ainda não existe um conferidor que reprove um `self.executar(` novo fora
dos irmãos. Se alguém criar outro caminho direto amanhã, nenhum teste vai acusar.
