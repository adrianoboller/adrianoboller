# O teto decidido antes da espera vale pela espera inteira

Pedido 442 (achado M1 da revisão SEC de 434/435), 24/09/2026.

## 1. O que aconteceu

O laço da porta de dados (`servidor.rs`, `atender`) escolhia o teto da linha
com `teto_da_linha(&sessao, …)` e só então chamava `canal.ler_ate(…, teto)`. A
conexão passa a vida **parada dentro dessa leitura**. Medido pelo soquete, com o
defeito de pé:

- ana entra, a conexão dela responde e volta a esperar; root exclui ana por
  outra conexão; ana manda 1 MiB → **`ok:true`**, lida com o teto de 128 MiB;
- conexão anônima aberta num servidor sem cadastro; outra conexão cria o
  primeiro supervisor; a antiga manda 1 MiB → **`ok:true`**;
- e a metade da memória: VmRSS 13.640 kB → **80.252 kB** ocioso depois de um
  ping de 64 MiB, porque `let mut linha;` morava fora do laço e a `String`
  velha só morria quando a leitura seguinte devolvia.

## 2. O que eu concluí primeiro, e estava errado

Que bastava **refrescar a ficha antes de perguntar o teto** — um
`refrescar_a_sessao` no topo do laço, uma linha. O defeito tinha cara de «a
ficha não se refresca», e o `refrescar_a_sessao` só rodava dentro do
`despachar`.

Estava errado porque o refresco no topo do laço acontece **antes** da espera, e
a exclusão acontece **durante** ela. A conexão da ana já está parada na leitura,
armada com 128 MiB, quando root a exclui. Refrescar antes de armar fecha só a
exclusão que cai entre duas linhas — o caso raro. O caso comum é a conexão
ociosa, e a ociosa é justamente a que passa a vida ali.

## 3. O que a medição disse

A hipótese virou guarda (`teto-refrescado-antes-do-bloqueio`) e foi **medida**:
com o refresco antes de armar a leitura, os **dois** testes de soquete caem
(2/2) — o excluído e a conexão aberta antes do primeiro cadastro.

O conserto que se sustentou decide o teto **quando a linha passa do teto de
todos** (`Canal::ler_decidindo`, no motor): até 64 KiB ninguém pergunta nada;
passou disso, a pergunta é feita com a ficha refrescada naquele instante. As
três variantes medidas contra os mesmos testes:

| variante | excluído | aberta antes do cadastro |
|---|---|---|
| teto antes da leitura, sem refresco (o defeito) | cai | cai |
| refresco + teto antes da leitura (o plausível) | cai | cai |
| pergunta na hora certa, sem refresco | cai | **passa** (o cadastro vazio é lido vivo) |
| pergunta na hora certa, com refresco (o conserto) | passa | passa |

E a memória, com a linha declarada dentro da volta: 13.592 kB → **14.648 kB**
ocioso depois do mesmo ping de 64 MiB.

Uma armadilha que a leitura em duas partes cria, e que o teste unitário do
motor trava: a divisa entre as duas leituras pode cair no meio de um caractere
de dois bytes, e o `read_line` de cada metade recusaria a linha legítima como
UTF-8 inválido. A leitura passou a ser por byte (`read_until`), e o texto se
confere uma vez, no fim.

## 4. A regra

**Decisão tomada antes de uma espera usa o dado de antes da espera: pergunte na
hora em que o byte chega, e não na hora em que a leitura foi armada — e deixe o
caso barato (a linha que cabe em todos) sem pergunta nenhuma.**

## 5. Como está guardado hoje

- `crates/phxsql-core/src/fio.rs`, `Canal::ler_decidindo`, e o teste
  `o_teto_decidido_so_pergunta_quando_a_linha_passa_do_pequeno` (conta QUANTO
  foi lido e QUANTAS vezes se perguntou);
- `crates/phxsql-server/tests/teto-da-linha-anonima.rs` (os dois casos e o
  comportamento velho) e `tests/linha-grande-nao-fica-residente.rs` (o VmRSS,
  num binário só dele para o vizinho não entrar na conta);
- guardas `teto-decidido-antes-do-bloqueio`, `teto-decidido-sem-refrescar-a-ficha`,
  `teto-refrescado-antes-do-bloqueio` e `linha-residente-depois-da-resposta`,
  PROVADAS;
- `docs/SEGURANCA.md` §22.2.

**Onde o buraco ficou:** o mesmo padrão — decidir antes de esperar — pode
existir em qualquer laço que guarda estado de sessão e bloqueia. Nesta
varredura o `teto_da_linha` é o único chamador, e os outros leitores do motor
usam teto fixo (não dependem de quem fala). Não há catraca que ache o padrão
por texto.
