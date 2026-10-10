# O `atualizar` grava a linha INTEIRA, e o catálogo diz «só as que mudam»

**Estado:** PENDENTE

## O que aconteceu

Na tela de Proteção (pedidos 765/766/767, fatia P15), mudar o modo de uma
linha de `phxsys.protecao` mandava `{"op":"atualizar", …, "valores":{"modo":"observar"}}`
— exatamente o que o catálogo ensina: `valores` é «coluna: valor; **só as que
mudam**» (`catalogo.rs`, op `atualizar`, que também é ferramenta MCP). A
bateria (`testes-web/casos/56-protecao.mjs`) reprovou nos dois temas com
`[SP000018] tipo invalido: coluna op e obrigatoria e recebeu NULL`.

## O que eu concluí primeiro, e estava errado

Que a guarda da P13 (`toque_na_guarda`) estava lendo o pedido e recusando por
um caminho torto, porque a escrita era na própria tabela de proteção. Não era:
o erro vem do `json_para_linha` (`valores.rs`), cujo comentário diz com todas
as letras «Colunas ausentes no objeto entram como NULL». O `op_atualizar`
monta a linha nova só do pedido; só a marca de excluída é herdada.

## O que a medição disse

- Coluna obrigatória ausente: recusa (o caso da tela).
- Coluna **anulável** ausente: pela leitura do `json_para_linha`, vira NULL
  **calada** — não medido por teste nesta rodada; é o próximo passo.
- A ficha da tela nunca caiu nisso porque o `valores()` dela manda **todas**
  as colunas editáveis. A tela de Proteção passou a fazer igual (relê o
  esquema e manda todas as colunas que se gravam, com só o modo trocado).

## A regra

Quem chama `atualizar` manda a linha inteira; e o texto do catálogo que diz o
contrário é defeito de documentação com cara de dado perdido — quem segue o
catálogo (um cliente, ou uma IA pela ferramenta MCP) apaga as colunas que não
mandou.

## Como está guardado hoje

Na tela: o `atualizar` da Proteção manda a linha inteira, e o caso 56 cai se
voltar a mandar só o modo (o primeiro erro da bateria). **O buraco continua
aberto no motor e no catálogo**: nem o texto «só as que mudam» foi corrigido
nem o `atualizar` passou a herdar o que não veio — a decisão entre as duas é
do papel C (garantia de dado) e vai à mesa do integrador como pedido novo.
