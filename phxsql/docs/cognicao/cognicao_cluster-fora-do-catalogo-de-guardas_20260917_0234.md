# Cognição: `cluster.rs` inteiro fora do catálogo de guardas

## 1. O que aconteceu

Inventariando as entradas de `bancada/guardas/catalogo.py` que tocam
replicação, cluster e quórum (ordem do dono, 17/09/2026 02:27 UTC), medi
quantas das 180 entradas vivas referenciam cada arquivo de replicação:

```
grep -n '"arquivo": "crates/phxsql-server/src/cluster.rs"' bancada/guardas/catalogo.py   -> 0
grep -n '"arquivo": "crates/phxsql-server/src/replica.rs"'  bancada/guardas/catalogo.py   -> 1
```

Treze entradas tocam replicação/cluster no total, mas doze moram em
`servidor.rs` (o despacho) ou em `table.rs`/`bidirecional.rs` (o store); uma
em `replica.rs`. **`cluster.rs` — o arquivo da eleição, da época, do
escalonamento a quente e dos papéis `spare`/`read_replica` — tem zero.**

Isso apesar de `cluster.rs` e o código de papéis em `servidor.rs` terem, hoje,
pelo menos sete testes reais e documentados como "prova real" que nunca
entraram no catálogo:

- `eleicao_prefere_completa_a_incompleta` (`cluster.rs:677`, pedido 211);
- `no_acrescentado_a_quente_passa_a_ser_aceito_no_pulso` (`servidor.rs:31756`,
  pedido 217 — prova as duas pontas: pulso de nó desconhecido recusado, e
  aceito depois do acréscimo a quente, sem reiniciar);
- `spare_nao_atende_cliente_nem_de_leitura` e
  `spare_promover_vira_primario_e_abre_a_escrita` (modo C, pedido 214);
- `read_replica_recusa_escrita_apontando_o_primario_e_serve_leitura` (modo D,
  pedido 214);
- `aplicar_num_source_trancado_por_administracao_e_recusado` +
  `replica_trancada_continua_aceitando_o_diario_do_source` (portão 2b-bis,
  pedido 214(c));
- `sem_replicas_autorizadas_nada_muda` + três irmãos (portão 2a-bis, pedido
  214/§7);
- `tabela_que_nao_abre_nao_pode_encolher_a_posicao_em_silencio`
  (`servidor.rs`, pedido 211 — e este é o mais grave dos sete: o
  `CATRACAS.md` §7 já o documentou como "guarda vermelha que virou verde", ou
  seja, JÁ foi tratado como guarda formal desta casa, e mesmo assim nunca
  entrou no catálogo que o `provar-guardas.py` reprova periodicamente).

## 2. O que eu concluí primeiro, e estava errado

Antes de rodar o terceiro grep (`"arquivo": ".../cluster.rs"`), eu tinha
concluído que a cobertura de replicação estava "razoável, com alguns buracos
pontuais" — porque a busca por palavras-chave (`replica`, `cluster`, `pulso`,
`bidi`, `colisao`, `credencial`) já tinha achado treze entradas, um número que
pareceu suficiente à primeira vista. Só quando fui procurar, uma a uma, a
guarda de cada pétrea citada na ordem do dono (eleição, escalonamento a
quente, spare, read replica, `replicas_autorizadas`) é que percebi que
**nenhuma delas estava nas treze** — as treze cobrem só a metade mais antiga
da replicação (modo A/B, `INTEGRIDADE.md` §3), e nada dos pedidos 211/214/217,
que são mais recentes e vivem majoritariamente em `cluster.rs`. O erro foi
medir a cobertura pelo **número de entradas achadas**, não pelo **arquivo que
elas cobrem** — a mesma armadilha que o `CLAUDE.md` já nomeia para a permissão
por tabela: "sete das 116 operações nomeando tabela onde o portão não olha".

## 3. O que a medição disse

- 180 entradas vivas no catálogo (`PISO_DAS_ENTRADAS`, medido 17/09 02:34).
- 13 tocam replicação/cluster/quórum por palavra-chave.
- Dessas 13: **12 em `servidor.rs`/`table.rs`/`bidirecional.rs`, 1 em
  `replica.rs`, 0 em `cluster.rs`.**
- 7 testes reais e documentados, cobrindo 5 pedidos diferentes (203, 211, 214,
  217, e a metade mais nova de 214(c)), sem entrada correspondente no
  catálogo — confirmado por grep de `spare`, `read_replica`,
  `SPARE_EM_ESPERA`, `ESCRITA_NA_REPLICA`, `replicas_autorizadas`,
  `cluster_no_acrescentar`, `incompleta`: zero ocorrências em
  `bancada/guardas/catalogo.py` para qualquer um desses termos.

## 4. A regra

**Quando uma pétrea nasce de um pedido novo, confira se o arquivo inteiro
onde ela mora tem alguma entrada no catálogo de guardas — não só se a
palavra-chave do pedido aparece em alguma entrada.** Contar entradas por
palavra-chave dá falso conforto: um arquivo pode ficar 100% fora do catálogo
mesmo com "resultados" de busca não-vazios, se as palavras-chave da busca
combinam com entradas de um arquivo vizinho.

## 5. Como está guardado hoje, e onde o buraco fica

Não está guardado. `bancada/guardas/catalogo.py` continua com zero entradas
para `crates/phxsql-server/src/cluster.rs`. O buraco é nomeado em
`docs/propostas/inventario-qa-replicacao-2026-09-17.md` §2.2, com os sete
testes reais e o `--so` que A pode escolher rodar depois de escrever as
entradas correspondentes — mas escrever as entradas em si é trabalho de
código (papel B) ou da próxima frente de QA que tocar `cluster.rs`, não desta
frente estática.
