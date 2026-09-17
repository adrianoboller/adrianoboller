# A guarda do teto provava a máquina, e não a constante

Frente 303-F (papel F, prova real), 17/09/2026 06:05 UTC.

## 1. O que aconteceu

O pedido 303 pedia o teste do `TETO_DA_RESPOSTA` — o teto de 128 MiB que a
réplica aplica em cada linha lida do source (`docs/REPLICACAO.md` §18). A QA
tinha escrito, no inventário de 17/09 02:34, que **não achou teste que sirva de
`caem`** para ele, e por isso a guarda não entrou no catálogo.

Ao medir, o quadro era outro e mais interessante: **havia teste, havia guarda no
catálogo, e mesmo assim o teto de fábrica estava sem prova.**

- O teste existe desde 30/08/2026 (`292686e`):
  `phxsql_core::fio::testes::o_teto_do_registro_para_a_leitura_e_nao_so_recusa_depois`.
- A guarda existe no catálogo: `fio-sem-teto-de-registro`
  (`bancada/guardas/catalogo.py`), que repõe o defeito «leitura ilimitada»
  tirando o `take`.
- Mas os dois exercitam `Canal::ler_ate(leitor, 64)` — o teto **passado na
  mão**. Ninguém exercitava `Canal::ler`, que é a linha que amarra a máquina à
  constante (`crates/phxsql-core/src/fio.rs:523`) — e é a única que os dois
  lados que recebem chamam: a réplica em `replica.rs:183` e o laço de conexão
  do servidor em `servidor.rs:8881`.

## 2. O que eu concluí primeiro, e estava errado

Ao achar o teste de 30/08 conferindo **quanto foi lido** (e não só o veredito),
concluí que o item 5 do pedido valia: *«o teto já está provado por um teste que
a QA não achou; o que falta é catalogar»*. Ia parar ali e devolver o nome do
teste.

Errado, e por um motivo que a leitura não mostra: a prova cobria a **máquina**
(o `take` vem antes da leitura) e não a **constante**. Um teto que se prova com
o teto que o próprio teste escolheu prova que o mecanismo funciona para
*algum* número — nunca que o número de fábrica chegou até lá.

## 3. O que a medição disse

Defeito reposto em `fio.rs:523`, `self.ler_ate(leitor, TETO_DO_REGISTRO)` →
`self.ler_ate(leitor, u64::MAX - 1)`:

| o que rodou | com o defeito reposto |
| --- | --- |
| `cargo test -p phxsql-core` (antes do teste novo) | **350 passando, 0 falhando** |
| `--test teto-da-resposta` (antes de existir) | não existia |
| bateria inteira | verde |

Ou seja: o fio inteiro — réplica e servidor — podia ficar **sem teto nenhum**
sem uma única prova cair.

Com o teste novo
(`fio::testes::a_leitura_padrao_para_no_teto_do_registro_e_nao_no_que_o_outro_lado_mandar`),
o mesmo defeito reprova dizendo o número:
`registro de 134221824 bytes atravessou em vez de ser recusado pelo teto de 134217728`.

E a divisão de trabalho entre unidade e soquete **também foi medida**, em vez de
suposta. Repondo o outro defeito (o `take` removido, que é o do
`fio-sem-teto-de-registro`):

| prova | com o `take` removido |
| --- | --- |
| `fio::testes::a_leitura_padrao_…` | **cai** (consumiu 134.221.824 de uma oferta de 134.221.824) |
| `fio::testes::o_teto_do_registro_para_a_leitura_…` | cai (leu 10.001 com teto de 64) |
| `a_resposta_acima_do_teto_…` (soquete, réplica) | **PASSA** — a recusa continua chegando pelo nome certo; o dano é só de memória, e memória não aparece no fio |
| `o_pedido_acima_do_teto_…` (soquete, servidor) | cai, mas **pendurando**: o servidor lê a linha inteira, não sobra quebra para drenar, e os dois lados esperam um pelo outro (15,4 s de prazo) |

A terceira linha é a que importa para quem for cortar teste achando duplicado:
**a prova de soquete não substitui a unitária**, porque o veredito do fio é o
mesmo nos dois casos e o que muda é a memória gasta.

Dois números de forma, medidos de passagem:

- `expect_err` **imprime o `Ok` que recebeu**. Com o defeito reposto o `Ok`
  carrega os 128 MiB, e a primeira versão da prova reprovava com um pânico de
  **134 MB** de `x`. Desembrulhar o veredito à mão custa quatro linhas e devolve
  uma mensagem legível — que ainda por cima diz o tamanho.
- A primeira versão da prova do servidor mandava um `{"op":"ping",…}` de 129 MiB
  **válido**. Com o defeito reposto ela reprovava por **prazo, em 31 s**, com
  `WouldBlock: Resource temporarily unavailable` — o servidor analisando o que
  ia recusar do mesmo jeito. Com a linha inalisável, a mesma reprovação sai em
  **0,37 s** e nomeia a garantia (`nome: "ESQUEMA_INVALIDO"`, esperado
  `"LIMITE_EXCEDIDO"`).
- Custo do teto real numa bateria: as quatro provas de soquete, que movem
  ~129 MiB cada uma pelo laço local, rodam em **0,22 s** somadas; o módulo
  `fio` inteiro, com a leitura de 128 MiB dentro, em **0,11 s**.

## 4. A regra

**Teto provado com o teto passado na mão prova o mecanismo, nunca a
constante — a prova tem de entrar pela porta que os chamadores usam.**

E a irmã, de forma: **reprovação que não nomeia a garantia quebrada não ensina
nada** — nem a que sai por prazo, nem a que imprime 134 MB de payload.

## 5. Como está guardado hoje

- `crates/phxsql-core/src/fio.rs` —
  `fio::testes::a_leitura_padrao_para_no_teto_do_registro_e_nao_no_que_o_outro_lado_mandar`,
  com igualdade **exata** sobre quantos bytes a fonte entregou (`teto + 1`), e o
  comportamento velho (a linha que cabe) no mesmo teste.
- `crates/phxsql-server/tests/teto-da-resposta.rs` — quatro provas de soquete:
  a réplica recusando a resposta gigante pelo nome e a rodada seguinte abrindo
  outra conexão; o servidor **respondendo** a recusa antes de fechar e anotando
  no `acessos.log` quantos bytes leu (número exato conferido contra o que se
  mandou); e as duas do comportamento velho.
- **O buraco que fica:** o catálogo de guardas ainda não tem entrada para o
  defeito da **constante** nem para o caminho de soquete — a entrada
  `fio-sem-teto-de-registro` cobre só o `take`, e não tem `seguem`. As duas
  entradas sugeridas, com `caem`/`seguem` medidos, foram entregues à QA pela
  frente 303-F. Enquanto elas não entram, o que existe é prova real feita à mão
  (duas vezes, nos dois sentidos) e nenhuma corrida periódica contra o defeito.
