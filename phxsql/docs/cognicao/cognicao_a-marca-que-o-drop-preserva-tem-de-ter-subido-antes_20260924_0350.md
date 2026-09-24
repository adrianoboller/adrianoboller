# A marca que o `Drop` preserva tem de ter subido antes — e o conserto por `panicking()` passa em cinco de seis provas

*24/09/2026, 03:50 — pedidos 456 e 457.*

## 1. O que aconteceu

O `impl Drop for NdxFile` chamava `fechar()` sem perguntar nada. Num pânico no
meio de uma escrita, isso levava ao disco as páginas no estado em que o pânico
as deixou e gravava o byte 52 em 0: a árvore rasgada voltava **limpa**. O
conserto (camada 0 do parecer do DBA) pôs no `NdxFile` o estado «escrita em
voo», ligado antes de o `.reg` andar e desligado quando a última chave entra;
com ele ligado, `fechar`, `sincronizar` e `Drop` não levam nada ao disco. A
camada 1 sobe o byte 52 **antes** do slot do `.reg`, no `inserir` e no
`atualizar` que troca chave.

A prova (`crates/phxsql-store/tests/panico-no-meio-da-escrita.rs`) arma um
pânico de verdade em quatro pontos, reabre a tabela e pergunta a **garantia**
antes da marca.

## 2. O que eu concluí primeiro, e estava errado

- **«A camada 1 é para o `SIGKILL`; o pânico se resolve só com a camada 0.»** É
  como o parecer a enquadra («fecha a mesma janela para o `SIGKILL`»), e eu a
  li assim. Medido, é falso: no pânico **depois do contador do `.reg`** nenhuma
  página do `.ndx` sujou nesta sessão, então **nada tinha subido o byte 52**. O
  `Drop` que não baixa a marca preserva uma marca que não existe, e a tabela
  volta limpa e com o id repetido aceito — com a camada 0 inteira.
- **«O conserto por `thread::panicking()` só falha no FFI, que é um caso à
  parte.»** O caso à parte é o único que o pega: com o `Drop` decidindo pela
  thread, **cinco das seis** provas de pânico passam, porque nelas a tabela
  morre no desenrolar. Uma suíte sem a prova do «capturado e solto depois»
  aprovaria o conserto errado sem um único vermelho.

## 3. O que a medição disse

Cada defeito reposto contra as nove provas do arquivo (e a da ABI):

| defeito reposto | caem | vermelho |
|---|---|---|
| `Drop` de antes (sem camada 0) | 6 de 9 | «chave duplicada aceita no índice único», «mãe apagada com filha viva», «linha viva fora do índice (CRC inválido na página 2)» |
| sem camada 1 | 3 de 9 | «chave duplicada aceita no índice único» (contador, atualizar, capturado) |
| `Drop` por `panicking()` | 1 de 9, e a da ABI | «chave duplicada aceita… entrou de novo pela ABI» |
| `sincronizar` sem a porta (457) | 1 de 9 | «chave duplicada aceita… o sincronizar do outro descritor limpou a marca» |
| toda recusa do `.reg` interrompe | 1 de 9 | «a recusa não pode deixar o índice recusando» (tabela cheia) |
| `reindexar` sem a janela | 1 de 9 | «linha viva fora do índice: a linha 1 … devolve []» |

Custo: `strace` na sonda do fecho, **3 escritas no `.ndx` por
abrir+inserir+fechar, antes e depois**; `onde-doi` com dois índices, cinco
pares intercalados sob carga das outras frentes, **6,0–6,4 µs antes contra
5,8–6,7 µs depois** — faixas cruzadas, sem diferença mensurável.

## 4. A regra

Quando um conserto **preserva** um estado no disco, prove que o estado foi
**posto** antes do ponto da falha — e prove o `Drop` pelos dois chamadores
dele, o desenrolar e o «capturado e solto depois».

## 5. Como está guardado hoje

Sete guardas no catálogo (`drop-grava-o-ndx-rasgado`,
`marca-do-ndx-sobe-depois-do-reg`, `drop-do-ndx-decide-por-panicking` e a
mesma `-pela-abi`, `sincronizar-limpa-o-ndx-aberto-sujo`,
`reindexar-sem-janela-grava-o-ndx-vazio`,
`janela-do-ndx-interrompe-em-toda-recusa`). **O buraco:** o `.fts` no nível da
`Table` não entrou — a árvore dele não rasga (a janela interna o cobre), mas um
pânico entre o `.reg` e o `indexar_texto` o deixa atrás do `.reg` marcado
limpo, e a busca de texto perde a linha calada. É o mesmo desenho, e não foi
feito nesta frente.
