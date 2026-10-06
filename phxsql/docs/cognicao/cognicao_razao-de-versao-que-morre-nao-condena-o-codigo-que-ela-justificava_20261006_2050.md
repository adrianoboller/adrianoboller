# Razão de versão que morre não condena o código que ela justificava

**Estado:** PENDENTE

## O que aconteceu

Pedido 653: cinco comentários diziam «a casa promete 1.75», e o
`rust-version` já era 1.89 (pedido 635). Dois deles justificavam AUSÊNCIA de
código pela versão: a trava de dados (`servidor.rs`, campo
`panicos_na_trava`) e a `TravaDaGuarda` (`pulso.rs`) não chamavam
`clear_poison` «porque é de 1.77». O pedido os marcou como candidatos a
conserto de código, não só de texto.

## O que eu concluí primeiro, e estava errado

Que, morta a razão, o código devia mudar: limpar o veneno no fim do reparo
(`reparar_a_trava`), e a trava voltaria a ficar limpa depois de um pânico
reparado. Lendo a ordem do desenrolar, a hipótese morre: o reparo roda no
`Drop` da `TravaMedida`, que AINDA segura a guarda; o `RwLock` só se envenena
quando a guarda cai, depois do corpo do `Drop`. Limpar ali é limpar um veneno
que ainda não foi posto. O único outro lugar é a tomada seguinte — que já
lê os dois contadores atômicos para decidir, e limpar ali compraria uma
escrita na trava sem mudar decisão nenhuma.

## O que a medição disse

- `o_veneno_permanente_continua_recuperando_tomada_apos_tomada`: depois de UM
  pânico reparado, o `RwLock` continua envenenado e 40 pedidos seguidos (20
  `inserir` + 20 `varrer`) atendem; contadores em (1, 1).
- Defeito reposto (veneno = recusa, guarda `veneno-permanente-recusa`): o
  primeiro pedido depois do pânico recusa, e a prova cai (medido em
  06/10/2026, 1 de 1).
- O comentário do `op_cluster_estado` dizia «`map_or(true, ..)` e não
  `is_none_or`: o MSRV é 1.75» em cima de um `is_none_or` — o código já tinha
  mudado e o comentário ficou desmentindo-o.

## A regra

Quando a razão escrita de uma ausência morre, reabra a pergunta e responda de
novo pelo código — a resposta pode continuar sendo «não», com outro porquê;
o que não pode é o porquê velho continuar escrito.

## Como está guardado hoje

- A razão nova está nos dois comentários (`servidor.rs`, `pulso.rs`) e o
  teste acima trava o comportamento (`s.dados.is_poisoned()` depois de 40
  tomadas).
- **O buraco:** a `TravaDaGuarda` do `pulso.rs` ficou com a marca própria
  sem prova nova; a equivalência com o `clear_poison` foi lida, não medida.
