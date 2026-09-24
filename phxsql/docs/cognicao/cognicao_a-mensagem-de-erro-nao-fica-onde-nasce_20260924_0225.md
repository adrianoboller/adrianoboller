# A mensagem de erro não fica onde nasce

Pedido 453, 24/09/2026.

## 1. O que aconteceu

`carga::hex_para_bytes` recusava com `format!("hexadecimal invalido: {hex:?}")`.
Procurando os irmãos — erros de conversão que interpolam o valor recebido com
`{:?}` —, a família era bem maior que o pedido: sete no `phxsql-core` (decimal
duas vezes, data, a conversão da carga, três do UUID), o literal da expressão,
a hora da agenda do job, e do lado do protocolo os dez caminhos do
`json_para_valor` e dos dois leitores de inteiro, por onde passa todo
`inserir`, `alterar` e filtro.

## 2. O que eu concluí primeiro, e estava errado

Que o dano era de **memória**: a mensagem copia o valor, e um megabyte recebido
vira mais um megabyte na mão. É verdade, e é o menor dos danos — a cópia morre
com a resposta.

O que fica é o **disco**. A mensagem não mora onde nasce: volta ao cliente e é
escrita inteira no `acessos.log`, que nasce **sem rodízio** de fábrica. E a
regra de redação desta casa — «o que não se analisa vira o tamanho» — já valia
para o Profiler e para o grito do conflito de replicação, e **não alcançava a
mensagem de erro de conversão**, que é o caminho mais comum de todos.

## 3. O que a medição disse

Pelo soquete, um `inserir` com 1 MiB de `z`:

| coluna | resposta antes | `acessos.log` antes | resposta depois | `acessos.log` depois |
|---|---|---|---|---|
| `Bin` | 1.048.766 bytes | +1.048.856 | 230 | +320 |
| `Int8` | 1.048.778 bytes | +1.048.868 | 213 | +303 |

Quem tem direito de inserir escrevia um megabyte no disco por pedido
**recusado** — o pedido que não grava nada na tabela era o que mais gravava no
servidor.

## 4. A regra

**Quando uma mensagem cita o que veio de fora, siga a mensagem até onde ela
mora — resposta, log, Profiler — e meça o dano lá; e a citação passa por um
motor só, com teto: curto sai citado, longo sai como tamanho, nunca um pedaço.**

## 5. Como está guardado hoje

- `phxsql_core::error::citar` e `TETO_DA_CITACAO = 48`, o mesmo número que o
  `bidirecional::valor_redigido` passou a ler dali;
- `hex_para_bytes` cita tamanho e posição, nunca o valor (o valor de `Bin` é
  conteúdo);
- testes: `carga::…::os_irmaos_de_conversao_nao_ecoam_o_valor_grande`,
  `valores::testes_eco_do_valor`, `tests/hexadecimal-do-fio.rs`
  (`o_valor_torto_grande_nao_volta_inteiro_nem_vai_ao_log`), e as guardas
  `hexadecimal-ecoa-o-valor`, `citar-sem-teto` e `json-recebido-ecoa-o-valor`,
  PROVADAS;
- `docs/SEGURANCA.md` §22.3.

**Onde o buraco ficou:** o texto da expressão (`onde` do `varrer`) ainda volta
inteiro no erro de sintaxe; o dado pessoal curto ainda é citado, porque a
conversão não recebe a marca da coluna; e não há catraca que ache um `{:?}` novo
de valor recebido — o conferidor de erro cru já foi recusado com número (8
interpolações, 2 defeitos), e a mesma conta vale aqui.
