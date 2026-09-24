# O alcance de um pânico é a trava que o chamador segura — e a cópia não herda o nome do pedido

*24/09/2026, 01:15 — pedidos 446 e 447.*

## 1. O que aconteceu

O pedido 446 chegou assim: `phxsql_core::hash::de_hex` fatia `&t[i..i + 2]`
por **byte**, e `"a€"` (quatro bytes, par) derruba a thread. O chamador
nomeado era o `pulso::conferir`, que lê a `prova` do `cluster_pulso` — texto do
fio, mas atrás da credencial do cluster e de um id da lista.

Medido pelo soquete (`tests/hexadecimal-do-fio.rs`), esse caminho custava
**a conexão e nada mais**: o `conferir_identidade` não segura trava nenhuma
quando chama o `de_hex`, o nó seguia atendendo, o `cluster_estado` e o
`criar_database` da conexão seguinte passavam.

O dano grande estava na **cópia**. A varredura `grep "\[i\.\.i + 2\]"` achou
`carga::hex_para_bytes` — o `de_hex` copiado, com o mesmo corte —, e é ela que o
`json_para_valor` chama para a coluna `Bin`. O `op_inserir` converte a linha
**depois** de tomar a trava global de dados (precisa do esquema). O pânico
desenrola com o `RwLock` de escrita na mão, e veneno de `RwLock` é permanente:

```text
a trava de dados ficou ENVENENADA: o inserir seguinte, por OUTRA conexao,
recebeu {"ok":false,"op":"inserir","erro":"[SP000010] arquivo corrompido:
uma operacao anterior entrou em panico e deixou a trava suja", ...}
```

Qualquer usuário com direito de inserir numa tabela com coluna binária parava
a base de **todos**, até o reinício. A mesma varredura achou o `%XX` da porta
web (`http::desescapar`, `bruto[i + 1..i + 3]`), servido **sem credencial** no
`GET /idiomas`: conexão fechada sem resposta, vaga devolvida pelo pedido 248,
porta de pé. E a **resposta** do pulso passa pelo mesmo `de_hex`: ali a thread
de pulso morre e nunca mais sobe, porque só se desmarca do `pulsando` pelo
caminho normal.

## 2. O que eu concluí primeiro, e estava errado

- **«A gravidade do 446 se mede no chamador que o pedido nomeia.»** O chamador
  nomeado era o mais protegido dos quatro (credencial do cluster, id da lista,
  chave estática) e o mais barato (só a conexão). O caro era uma cópia que
  **não aparecia no pedido** e que nenhum `grep de_hex` acharia: ela não chama
  o motor, repete o motor.
- **«O mapa vazio do 447 abre eleição ou promove indevidamente.»** Era a
  hipótese do próprio pedido. Medido, é o contrário: 1 de 3 nunca é maioria,
  então o nó envenenado **nunca** promove. O dano é paralisia — o master perde a
  maioria e recusa toda escrita a cada tique; a réplica envenenada que a outra
  elege (2 de 3, vencedora ela) não se promove (1 de 3, nenhum vencedor), e o
  cluster fica sem master, sem prazo. Falhar «fechado» aqui não é prudência: é
  o failover que não acontece.
- **«`%+1` deve ficar como veio.»** Escrevi a expectativa assim no teste da web
  e ela caiu: o `+` é o espaço do formulário, e `% 1` é a leitura certa. O
  defeito de antes era o `from_str_radix` aceitar `+1` como dígito e devolver o
  byte 1.

## 3. O que a medição disse

| caminho | credencial | trava na mão no pânico | dano medido |
|---|---|---|---|
| `cluster_pulso`, pedido | cluster + id da lista | nenhuma | a conexão |
| `cluster_pulso`, resposta | o par na lista | nenhuma | o laço de pulso daquele par: 1 pulso em 4,5 s com `pulso_s = 1`, e o supervisor não sobe outro (o id segue marcado) |
| `inserir` em coluna `Bin` | direito de inserir | **escrita global de dados** | **a base inteira, até reiniciar** |
| `GET /idiomas?idioma=%€` | **nenhuma** | nenhuma | a conexão (a vaga volta) |

E o 447, com a trava do mapa envenenada num cluster de três: `vivos()` **1 de
3** (era 3 de 3), `vencedor` = nenhum, `registrar` seguinte perdido (mapa com
0 nós). As outras cinco travas do `EstadoCluster`, uma por uma:
`master_atual()` = `None` (a réplica parava de redirecionar), `degradacao()` =
`[]` com o cluster degradado, `marcar_pulso` = `false` para nó novo,
`acrescentar`/`remover` = `false` e a `lista()` de volta ao `config.nos`, aviso
de promoção = `None`. **Gatilho conhecido para envenenar qualquer uma delas:
nenhum** — o que se faz com elas na mão é `insert`, `remove` e `clone` de
contêiner do `std`.

## 4. A regra

**Para medir o alcance de um pânico, pergunte que trava o chamador segura
quando ele acontece — e procure a CÓPIA da função, porque a cópia não tem o
nome que o pedido procura.**

## 5. Como está guardado hoje

- O `de_hex` lê byte a byte pelo `hash::digito_hex`, e a cópia do `carga.rs`, o
  `%XX` da web, o `\uXXXX` do JSON e o dígito do `uuid` passaram a chamar o
  motor. Testes de unidade em `hash`, `carga`, `json` e `http`; o dano, pelo
  soquete, em `tests/hexadecimal-do-fio.rs`.
- As seis travas do `EstadoCluster` são `TravaDaGuarda` — o motor do 436, não
  uma segunda cópia dele. Testes `cluster::testes::o_mapa_com_a_trava_envenenada_continua_vendo_os_vivos`
  e `a_familia_das_travas_do_cluster_recupera_o_veneno`.
- Guardas 46–51 do catálogo (`de-hex-fatia-texto-por-byte`,
  `prova-do-pulso-derruba-a-conexao`, `copia-do-de-hex-envenena-a-trava-de-dados`,
  `percent-da-web-fatia-texto-por-byte`, `mapa-do-cluster-envenenado-vira-vazio`,
  `lista-do-cluster-envenenada-volta-ao-arranque`).
- **O buraco que ficou, e não é deste pedido:** a trava global de dados continua
  falhando fechado para sempre diante de **qualquer** pânico dentro dela — o
  `Bin` era um gatilho, não o único possível. Recuperá-la é decisão do DBA,
  porque ali, ao contrário do cluster, o desenrolar pode deixar uma escrita pela
  metade. E a thread de pulso que morre por pânico continua sem se desmarcar do
  `pulsando`: o gatilho conhecido fechou, a fragilidade não.
