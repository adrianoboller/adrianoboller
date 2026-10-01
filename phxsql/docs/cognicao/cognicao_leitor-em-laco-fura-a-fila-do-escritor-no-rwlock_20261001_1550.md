# O leitor em laço fura a fila do escritor no `RwLock` — e o floco era isso, não o disco

**Estado:** PENDENTE

## O que aconteceu

Pedido 623. A prova pelo soquete do pedido 330
(`crates/phxsql-server/tests/absorcao-do-bidi-no-arranque.rs`) caiu em 2 de 2
corridas dos portões com «Some(0) inserir durante a absorcao, maior espera
2,14 s, sob a exclusiva 22», e passava sozinha. A absorção do bidirecional é o
único leitor da casa que toma a ficha COMPARTILHADA em laço: fatia de 10 ms,
solta, toma de novo.

Medido com a máquina parada, sem carga: as fatias corriam coladas (11 ms cada,
sem buraco entre elas), o escritor esperava **115–360 ms** por `inserir` e até
**2,1 s**, e só 2–3 `inserir` terminavam durante uma absorção de ~1,8 s. O
`read_unlock` do futex limpa o bit de escritor esperando e acorda o escritor; o
leitor, que não dormiu, pede a leitura de novo antes de o acordado ser escalado
e a leva. Sob a suíte, com os núcleos tomados, o escritor perdia todas.

Reproduzindo sob carga (4 processos de `fsync` de 8 MiB + 6 de CPU, 4
núcleos) apareceu o irmão: **211.060 eventos sob a exclusiva e 3,3 s de
escritor parado**, numa fatia de 13 ms que não absorveu nada. O prazo era
conferido no TOPO do laço — a fatia que gastava os 10 ms abrindo a tabela ou
esperando o `toques_bidi` saía com zero, a falta não encurtava, e a
pré-absorção desistia para a exclusiva. O comentário da função dizia «um lote
inteiro sempre entra»; o código não fazia isso.

E o terceiro, na prova do pânico do relógio da janela
(`panico_no_relogio_da_janela_derruba_o_processo_em_vez_de_parar_a_janela`):
sob a mesma carga caiu **15 de 16** com o diagnóstico do filho VAZIO — o relógio
nunca entrou em pânico. A janela fecha na própria gravação quando ela chega
150 ms depois do último fecho, e dois commits seguidos com os núcleos tomados
chegavam depois. A prova acusava o defeito sem o pânico ter acontecido.

## O que eu concluí primeiro, e estava errado

A hipótese que veio no pedido, e a que estava escrita no próprio teste desde a
versão anterior: «o pico era o disco» — o `fsync` lento de cada `inserir` sob a
suíte. Era plausível e não medido. Sem carga nenhuma o escritor já esperava
2,1 s; o disco do contêiner fazia `fsync` de 8 MiB em 30 ms. A troca de
«cronometrar» por «contar quem terminou» tinha consertado o sintoma errado, e o
comentário que a justificava fez ninguém olhar a trava.

## O que a medição disse

| absorção, 300.000 eventos, build de teste | maior espera | `inserir` no meio | furaram a fila |
|---|---|---|---|
| antes, sem carga | 1,57–2,19 s | 2–3 | (o contador não existia) |
| sem o `ceder` (defeito reposto), sob carga | 455–787 ms | 166–225 | **93–165**; 3 de 3 vermelhas |
| antes, sob carga | até 3,42 s | 43–155; 1 de 5 caiu com 211.060 sob a exclusiva | — |
| cedendo a vez, sem carga | 123–131 ms | 83–85 (2 escritores) | 0 |
| cedendo a vez, sob carga | 234 ms – 1,73 s | 251–321; entraram entre fatias 268–339; 6 de 6 verdes | 0 |

Relógio da janela sob a mesma carga, 4 cópias em paralelo: 1 de 16 verde antes,
16 de 16 depois (gravando até o filho cair ou o diagnóstico trazer o reparo), e
com o defeito reposto (a família que nunca casa) 8 de 8 vermelhas pelo motivo
certo, em 1,2–2,2 s cada.

## A regra

Leitor que toma a ficha compartilhada em laço CEDE a vez entre as voltas, e a
prova conta a ENTRADA do escritor, não o fim dele — o fim carrega o disco.

## Como está guardado hoje

`PortaoDoRetrato::fila`/`ceder`/`depois_de_ceder` (`crates/phxsql-server/src/retrato.rs`)
e o contador `entradas`, um `fetch_add` por tomada exclusiva. O
`replicacao_estado` publica `escritores_entre_fatias` e
`fatias_que_furaram_a_fila`. Guardas: `pre-absorcao-fura-a-fila-do-escritor`,
`leitor-que-cede-volta-na-hora`, `fatia-com-o-prazo-vencido-nao-anda`,
`bidi-absorve-o-diario-sob-a-exclusiva-pelo-soquete`; a do pânico
(`panico-em-thread-de-servico-morre-calado`) reprovada com a prova nova.

O buraco que fica: a ficha de leitura de qualquer OUTRO laço que alguém
escrever amanhã não passa por aqui — o `ceder` é chamado, não imposto.
