# Trava dupla esconde a trava que se quer provar: prove pela porta que só tem ela

**Estado:** PENDENTE

**Evidência (para quem for validar):** `crates/phxclaw-agent/tests/ide_credencial_do_terminal.rs::o_terminal_do_usuario_nao_leva_o_token_mestre_e_morre_com_ele`.
Sem a conferência da rota em `rbac::acesso_da_sessao` (`// REPOSTO`), as asserções HTTP
(`GET /v1/tasks`, `GET /v1/config`, `POST /v1/tasks` com a credencial do terminal → 401)
continuaram passando; a asserção pelo websocket do terminal (que confere só pelo portão,
`conferir_rota`) caiu com `{"ev":"pronto"}` — a credencial abriu outro terminal.

## O que aconteceu

A credencial do terminal do IDE (`phxs_`) só vale em `POST /v1/ide/completar`. O teste conferia o
escopo pedindo outras rotas HTTP com ela. O RED que tirava a conferência da rota do portão não o
derrubou.

## O que eu concluí primeiro, e estava errado

Que a conferência da rota estava no lugar errado e não era chamada. Era chamada, e o portão deixava
passar — mas cada rota HTTP chama de novo o `api::auth`, que não aceita a credencial do terminal: uma
SEGUNDA trava, que respondia 401 pelo motivo errado. A única porta que confia só no portão é o
websocket do terminal (o token chega na primeira mensagem, e não há `auth` de rota atrás dele).

## A regra

Quando há duas travas em série, o teste do escopo de uma delas tem de entrar pela porta onde ela
está SOZINHA; pela porta com as duas, o RED de cada uma passa calado pela outra. Na matriz do
`rbac.rs` essa porta é a de `Escopo::TokenNaMensagem` (e a de `Escopo::Convite`, pelo mesmo motivo).

## Como está guardado hoje

O teste acima abre o websocket do terminal com a credencial e exige o erro, com o motivo no
comentário dele; as asserções HTTP ficaram como a segunda trava, ditas como tal no cabeçalho do
arquivo de teste.
