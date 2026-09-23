# O teto do fio para onde o `Canal` começa — e o aperto fica do lado de fora

*23/09/2026, 19:55 UTC · pedidos 312 e 278, frente B*

## 1. O que aconteceu

O `TETO_DO_REGISTRO` (128 MiB) mora em `Canal::ler`
(`crates/phxsql-core/src/fio.rs`), e o comentário ao lado dele conta, com todas
as letras, por que ele desceu para lá: *«a frente da trava pôs um limite no
`read_line` da réplica, a frente da cifra trocou aquele `read_line` por este
canal, e juntar as duas sem cuidado devolveria o limite ilimitado»*. A guarda
existe, está escrita, e tem a história do defeito ao lado.

E ainda assim havia duas leituras **fora** dela: `replica::Cliente::cifrar`
(`replica.rs`) e `servidor::Remoto::cifrar` (`servidor.rs`). As duas leem a
resposta do aperto de mão — e o `Canal` só existe **depois** do aperto.

## 2. O que eu concluí primeiro, e estava errado

Li o pedido 312 como *«o servidor lê sem teto antes de autenticar, então uma
conexão que nunca termina de falar consome o que quiser»* e fui procurar o
buraco no laço `atender`. **Não está lá**: o laço de conexão do servidor já lê
pelo `Canal` desde a primeira linha, com o teto valendo, e a linha acima do
teto ainda vira resposta, violação e registro no `acessos.log`.

A direção do ataque é a **inversa** da que eu presumi: quem lê sem teto é o
**cliente** — a réplica falando com o source, o nó pulsando o outro, a
interface falando com outro PhxSql — e quem escolhe o tamanho é o **servidor**
do outro lado, ou quem estiver no meio dele. Um source malicioso, um DNS
sequestrado ou um endereço trocado no `config.json` bastam. Procurar o defeito
do lado errado teria custado a rodada e «consertado» um caminho que já estava
certo.

## 3. O que a medição disse

Um source falso que aceita a conexão e nunca manda o fim de linha:

| medida | antes | depois |
|---|---|---|
| bytes que a réplica guardou numa linha só | **201.326.592** (192 MiB) | teto do aperto, **65.537** |
| tempo até engolir tudo | **294,8 ms** | recusa imediata |
| erro que voltava | `Connection reset by peer` (irmão) ou prazo | `LIMITE_EXCEDIDO` |

Os 192 MiB são **1,5× o `TETO_DO_REGISTRO`**, e esse número é a prova: se o
teto do registro alcançasse aquela leitura, ela teria parado em 128 MiB.

## 4. A regra

**Guarda que mora no objeto não alcança o que acontece antes de o objeto
nascer.** Quando uma proteção descer para dentro de uma abstração, procure as
leituras do *aperto* — a fase em que a abstração ainda está sendo construída é
exatamente a fase em que ninguém provou ser ninguém.

E a segunda, que saiu da mesma tarde: **mexer no catálogo depois de rodar a
suíte é rodar a suíte à toa.** Acrescentar quatro parâmetros à ficha do
`cluster_pulso` em `catalogo.rs` derrubou
`segredos::testes::todo_parametro_com_cara_de_segredo_esta_na_lista` — um teste
de outra área, que varre o catálogo procurando nome com cara de segredo e achou
`cluster_pulso.nonce`. A corrida verde de vinte minutos antes não sabia disso.
Quem pegou foi o provador de guardas, que roda a suíte inteira por conta
própria.

## 5. Como está guardado hoje

* `TETO_DO_APERTO` (64 KiB) em `crates/phxsql-core/src/fio.rs`, com o porquê do
  número escrito ali: não são os 200 bytes do caso feliz porque a resposta de
  **erro** do aperto carrega texto traduzido.
* As duas leituras passam pelo `Canal` (ainda `Claro` naquele ponto), com
  `ler_ate`.
* Provas nos dois sentidos, e o irmão tem a **sua**:
  `replica::testes_do_teto_do_aperto::{o_aperto_de_mao_recusa_a_linha_sem_fim,
  resposta_curta_do_source_continua_passando}` e
  `tests/teto-do-aperto.rs` para o `Remoto`.
* Guarda `aperto-de-mao-sem-teto` no catálogo (`bancada/guardas/catalogo.py`),
  com o defeito que a motivou e os testes que têm de cair.
* **Onde o buraco ficou**: `phxsql-odbc` (`conexao.rs:395`) faz a mesma leitura
  crua no aperto do cliente ODBC, e `http::ler_pedido` (`http.rs:152`) lê a
  **primeira linha** do pedido HTTP sem teto — o `MAX_CABECALHO` só começa a
  contar a partir da segunda. Os dois foram achados **lendo o código, e não
  medidos** — dizer isso é a diferença entre um achado e um número —, e **não
  foram consertados nesta frente**: o ODBC tem frente viva e o HTTP é outro
  caminho. Ficam nomeados aqui para não virarem descoberta de novo.
