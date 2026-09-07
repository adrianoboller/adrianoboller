# O `#[ignore]` que deveria lembrar funciona como silenciador

**07/09/2026, 15:20** — revisão completa, papel A.

## 1. O que aconteceu

A revisão rodou a bateria inteira: **1.669 testes, zero falhas, 4 ignorados**.
Fui olhar os quatro, porque *quem não roda tem de aparecer* — e dois deles não
eram ignorados por custo:

```
tabela_que_nao_abre_nao_pode_encolher_a_posicao_em_silencio
    ... ignored, VERMELHA de proposito: prova um erro engolido que ainda nao foi consertado
coluna_externa_marcada_sozinha_nao_pode_ir_em_claro
    ... ignored, VERMELHA de proposito: prova um vazamento que ainda nao foi consertado
```

São duas **guardas vermelhas**: testes escritos para falhar com o defeito de pé,
com o estrago medido no comentário, esperando a decisão do dono. Boa prática
desta casa.

`grep` no `docs/PENDENCIAS.md`: **zero ocorrências**. Nenhuma das duas estava na
lista. A do vazamento da cifra estava assim desde **05/09**.

## 2. O que eu concluí primeiro, e estava errado

Concluí, ao ver «1.669 passaram, 0 falharam», que **a árvore estava sã**. É o
que o portão diz, é o que o `cargo test` imprime, e é o que qualquer relatório
de revisão teria escrito.

O erro está em confundir *«a bateria está verde»* com *«não há defeito
conhecido»*. Um teste desligado sai da conta dos que falham **e** da conta dos
que passam — ele não aparece em nenhum dos dois números que alguém lê. A prática
de entregar a guarda vermelha resolve o problema de **provar** o defeito e cria
um segundo, silencioso: ela o tira do único lugar onde ele seria notado.

E o mecanismo é perverso na direção certa: quanto mais disciplinada a casa é em
escrever a guarda antes do conserto, **mais defeito fica invisível**.

## 3. O que a medição disse

| | |
|---|---:|
| testes na bateria | **1.669** |
| falharam | **0** |
| ignorados | **4** |
| ignorados por CUSTO (X25519 de 1.000.000 de iterações; corpo traçado da sonda) | **2** |
| ignorados por DEFEITO — guardas vermelhas | **2** |
| dessas, no `PENDENCIAS.md` | **0** |
| a mais antiga, sem pedido desde | **05/09/2026** |

E o estrago que a mais séria delas prova, medido em 05/09 com dez linhas pelo
soquete: só colunas externas marcadas → `.memo` de **6.264 B, texto em claro**;
com uma inline junto → **6.664 B, cifrado**. Os 400 B são os 40 por valor
(nonce 24 + etiqueta 16) das dez linhas. O `{"op":"config"}` responde
`cifra.ligada: true` nos dois casos.

## 4. A regra

**Prova desligada tem de estar escrita onde o dono lê — o `#[ignore]` conta
para quem roda o teste, e ninguém lê a saída do `cargo test` procurando o que
NÃO rodou.**

E o corolário, que é o que torna isto uma regra e não um lembrete: **a lista dos
zeros é a que ninguém confere.** «0 falharam» é lido como boa notícia; «4
ignorados» não é lido. Todo número que a casa publica como zero precisa dizer
também **o que ele não conta** — foi a mesma falha do conferidor de grades
(contava `<table>` cru e não o ajudante) e do de idiomas (media cinco sextos da
tela e anunciava o número inteiro), aqui numa terceira forma.

## 5. Como está guardado hoje

- Os dois defeitos viraram os pedidos **210** (vazamento da cifra em coluna
  externa marcada sozinha) e **211** (a posição do diário encolhendo em
  silêncio), com a cadeia no fonte e o número medido em cada um.
- A guarda contra a repetição é a `TETO_VERMELHA_SEM_PEDIDO = 0`, em
  `crates/phxsql-server/src/conferidor_vermelhas.rs`: varre a árvore atrás de
  `#[ignore = "VERMELHA de proposito…`, pega o nome da função abaixo, e exige
  que ele esteja no `PENDENCIAS.md`. Relatório em
  `cargo run --example vermelhas-sem-pedido -p phxsql-server`.
- **Prova real nos dois sentidos**: apagando um dos nomes do `PENDENCIAS.md` a
  catraca fica vermelha nomeando arquivo, linha e função; com os dois, verde.
- **O buraco que fica, declarado**: a catraca exige que o nome **apareça** no
  `PENDENCIAS.md`, e não que o pedido esteja **aberto**. Um pedido fechado por
  engano com a guarda ainda vermelha passaria. Fechar isso exigiria ler o
  estado (☑️/◐/☐) da linha em que o nome aparece, e a linha pode ser longa —
  ficou de fora desta rodada, medido e escrito, em vez de suposto resolvido.
