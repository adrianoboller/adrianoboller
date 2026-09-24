# `bind("…:0")` não é livre de corrida se o ouvinte cai — pedido 401

**Estado:** PENDENTE

Evidência candidata, para quem for promover: o teste existe e passa de forma
estável em corridas repetidas —
`crates/phxsql-server/tests/corrida-da-porta-por-fora.rs::o_padrao_novo_nunca_colide_enquanto_segura_o_que_pediu`.
A promoção para FRUTÍFERO fica para o dono/integrador, por decisão explícita
(`docs/cognicao/LEIA-ME.md`: nenhum script, agente ou integrador promove
sozinho).

## O que aconteceu

Escrevendo a prova real do pedido 401 (fechar a corrida do `porta_livre()`
copiado em ~26 arquivos de teste — reservar, ler, soltar, e só bem depois
ligar de verdade), a primeira versão do teste "verde" tentou provar algo mais
forte do que o conserto realmente garante: que `TcpListener::bind("127.0.0.1:0")`
nunca devolve o mesmo número de porta duas vezes, mesmo sob disputa real entre
muitas threads.

## O que eu concluí primeiro, e estava errado

Escrevi (e quase deixei como comentário permanente do teste): *"o sistema
operacional nunca devolve, para dois `bind` CONCORRENTES na mesma máquina, o
mesmo número de porta — é a mesma garantia com que o kernel aloca file
descriptors, ele serializa a entrega de números efêmeros."* Parecia óbvio:
se é assim que o SO evita reusar um fd ainda aberto, deveria ser assim que
evita reusar uma porta ainda aberta.

## O que a medição disse

O teste, como escrito, fazia cada thread `bind`, ler a porta com
`local_addr()` e devolver só o `u16` — deixando o `TcpListener` cair de
escopo (fechando o soquete) antes de a próxima thread tentar. Com cem threads
soltas ao mesmo tempo por um `Barrier`, **duas delas receberam o MESMO
número de porta**, reproduzido de forma estável (não uma vez isolada). A
causa: soltar o ouvinte devolve o número ao sistema NA HORA, e outra thread,
ainda disputando um `bind(0)`, podia recebê-lo de volta um instante depois —
exatamente a mesma história que `servico.rs` já tinha registrado num
comentário de CI de 03/09/2026 (`porta_livre()` velho daquele arquivo: "um
`bind(:0)` de outra thread do MESMO binário podia receber a porta recém-
solta"). Reescrevendo o teste para cada thread **manter o ouvinte vivo** até
o fim (a mesma coisa que `Servidor::escutar()` faz — o `TcpListener` que
aceita conexão é o MESMO que fez o `bind`, nunca é solto e reaberto), as
mesmas cem threads, sob a mesma disputa, devolveram cem números distintos,
de forma estável em corridas repetidas.

## A regra

**A garantia de `bind(0)` é "nunca dou o mesmo número para dois donos ao
mesmo tempo", não "nunca reuso um número depois que o dono soltou".** Um
helper de teste (ou de produção) só herda essa garantia enquanto o
`TcpListener` que recebeu o número continua vivo até o uso — ler a porta e
depois LARGAR o ouvinte reabre exatamente a janela que o pedido 401 fechou,
só que num ponto diferente da corrente.

## Como está guardado hoje

Nos quatro testes de `corrida-da-porta-por-fora.rs`: os dois primeiros
(`a_janela_do_padrao_antigo_deixa_qualquer_um_tomar_a_porta` e
`o_padrao_novo_nunca_colide_enquanto_segura_o_que_pediu`) provam o mecanismo
com sockets crus; os dois últimos
(`vermelho_com_o_servidor_de_verdade_a_janela_derruba_o_arranque` e
`verde_com_o_servidor_de_verdade_o_mesmo_ataque_nao_impede_o_arranque`) provam
o mesmo contra o `Servidor` de produção. O comentário do teste verde registra
a hipótese errada e o número, para a mesma suposição não voltar sem medição.
Todo o conserto do pedido 401 (`Servidor::escutar()`/`subir_web`/`subir_rest`/
`subir_swagger` gravando o endereço REAL logo após o `bind`, nunca soltando o
ouvinte antes de usá-lo) já seguia a regra certa por acidente de desenho; o
que faltava era o teste que a nomeia.
