# Ensinar o crivo a ver a crase não bastou — o apagador rodava antes dele

- **Quando:** 2026-09-03, 03:48
- **Onde:** `crates/phxsql-server/src/conferidor.rs` — `literal()`,
  `sem_interpolacao()`, `RECEITAS`, `via_rotulo()`
- **Custo:** uma segunda rodada de medição no meio do próprio conserto — dei
  o crivo por fechado uma vez, medi de novo por desconfiança e achei que
  ainda faltava a parte maior das 63 chamadas de `ficha(` que o pedido 165
  mandava resolver.

## O que aconteceu

O pedido 165 (`docs/PENDENCIAS.md`) apontava um falso negativo do conferidor
de idiomas: `literal()` só reconhecia `"` e `'`, então `avisar(`, `confirm(`
e `folha(` chamados com o texto entre **crase** (`` avisar(`Tabela criada`) ``)
ficavam invisíveis à conta, junto com os ajudantes `ficha(` (63 chamadas) e
`carta(` (18).

Corrigi `literal()` para aceitar crase como aspa, e acrescentei `carta(` e
`ficha(` ao `RECEITAS` — `ficha(valor, rotulo, unidade)` precisou de um
tratamento à parte, porque o rótulo é o SEGUNDO argumento, e o primeiro
(`valor`) é dado. Medi: **1.549 → 1.685**. Um ganho real, mas pequeno demais
para o tamanho do problema que o pedido descrevia.

## O que eu concluí primeiro, e estava errado

Concluí que o trabalho estava feito: a lista de receitas cobria `carta(` e
`ficha(`, os testes existentes continuavam verdes, e o número tinha subido
136 — coerente com "um número grande", como o pedido antecipava. Ia seguir
direto para aposentar a catraca em 1.685.

**Estava errado.** Fui conferir *onde* as novas 136 ocorrências apareciam
antes de aposentar o teto — não por dúvida do resultado, mas porque a
instrução mandava "remeça todos, e não confie nos números registrados". E aí
o diff por conteúdo mostrou zero ocorrências de `ficha(` na faixa de linhas
onde ele é chamado dezenas de vezes (`ficha(linhas.length, "tabelas")` e
companhia). As 62 chamadas de `ficha(` desta base vivem **todas** dentro de
um `${…}` — `` ${ficha(r.database, "database")} `` —, e `sem_interpolacao`
já tinha apagado a linha inteira, receita e tudo, antes de `via_rotulo` ter
a chance de procurar `ficha(` nela. Ensinar `literal()` a ver a crase e
ensinar `RECEITAS` a conhecer `ficha(` foram os dois consertos certos, e os
dois eram **inertes** para o caso mais comum, porque o defeito real estava
uma etapa antes: o apagador de interpolação não sabia que havia algo para
não apagar.

## O que a medição disse

| etapa | medido | diferença |
|---|---:|---:|
| antes de qualquer conserto | 1.549 | — |
| `literal()` aceita crase + `carta(`/`ficha(` no `RECEITAS` | 1.685 | +136 |
| `sem_interpolacao` preserva `${chamada_conhecida(...)}` | **1.744** | +59 |
| depois de traduzir o lote coerente do Painel (32 chaves) | **1.720** | −24 |

A segunda etapa sozinha (+59) é menor que a primeira (+136) em contagem
bruta, mas é ela que resolve o problema descrito: sem ela, nenhuma das 62
chamadas de `ficha(` desta base — a imensa maioria do "63" citado no pedido
— chegava a ser vista. Confirmado por amostra: as linhas 5252-5254
(`${ficha(linhas.length,"tabelas")}` e vizinhas) só aparecem no relatório
`--tudo` depois da terceira etapa.

## A regra

**Quando um crivo aprende uma forma nova, pergunte primeiro que ETAPA
apaga essa forma antes de o crivo chegar a ela.** Ensinar o reconhecedor
(`literal()`, `RECEITAS`) sem auditar o que roda ANTES dele (aqui,
`sem_interpolacao`) mede um progresso real mas parcial, e a diferença entre
"parcial" e "resolvido" só aparece comparando a lista de achados por
CONTEÚDO — não pelo placar total, que sobe de qualquer forma e parece
suficiente.

## Como está guardado hoje

- `sem_interpolacao()` ganhou `abre_com_receita()`: quando um `${…}` abre
  com uma chamada que o `RECEITAS` reconhece, só o marcador `${` vira
  `BURACO` — o resto da chamada segue intacto para `via_rotulo` continuar
  enxergando, e o crivo de `normalizar()` continua barrando qualquer código
  que escape pela fresta.
- A catraca antiga (`TETO`, 1.549) foi **aposentada**, e não subida — molde
  de `conferidor_grades::TETO_TABELA_NA_MAO`. Nasceu `TETO_ROTULOS_E_CRASE`
  em 1.744, baixada a 1.720 no mesmo commit que traduziu o Painel inteiro
  (32 chaves, `tela.pa_*`).
- Prova real dos dois lados: `cargo test -p phxsql-server conferidor::` — os
  testes já existentes (`ve_o_rotulo_fora_da_marcacao`,
  `reprova_o_rotulo_cravado_e_aprova_o_da_fabrica`,
  `dado_interpolado_nunca_conta_como_rotulo`) continuam verdes com o crivo
  novo, e a catraca (`a_catraca_dos_textos_fora_da_fabrica`) trava o número
  medido de hoje nos dois sentidos.
- O que ainda escapa, documentado e não escondido: o primeiro argumento de
  `linha(` (telemetria) e texto escrito com `\uXXXX` — nenhum dos dois
  mudou nesta rodada, e `docs/MENSAGENS.md` diz por quê.
