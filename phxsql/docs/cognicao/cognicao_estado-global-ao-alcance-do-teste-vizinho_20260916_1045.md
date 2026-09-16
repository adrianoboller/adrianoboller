# Estado global ao alcance de um teste é estado que o teste vizinho escreve — e a suíte roda em paralelo

Descoberto em 16/09/2026, 10:45:27 UTC, pelo papel F na caçada do pedido 247,
com a primeira mensagem de falha que trazia os dois ids.

## 1. O que aconteceu

`v7_nunca_repete_nem_anda_para_tras` (`crates/phxsql-core/src/uuid.rs`) caiu
**uma vez** em 13 corridas da rodada da leitura repetível, sob carga, e o log
guardou só a linha do pânico: `u > anterior`. Sem os dois ids não havia como
dizer se era o teste, o relógio do contêiner ou o gerador.

A asserção passou a imprimir os dois ids em hexa, o milissegundo e o contador
de cada um, o relógio da máquina e o estado do gerador no instante. Com ela,
o laço sob carga (quatro processos rodando `cargo test -p phxsql-core --lib
uuid`, com quatro busy loops e as compilações das outras frentes, load
average 5,7 a 18,4) reproduziu de imediato:

```
id 1208 nao cresceu: anterior 01a0a9d2-5d69-78ae-… (ms 1789555531113, contador 0x8ae)
depois 01a0a9d2-5d69-7409-… (ms 1789555531113, contador 0x409);
relogio da maquina agora 1789555531113 ms; RELOGIO = (ms 1789555531113, contador 0x40a)
```

**Mesmo milissegundo, contador de `0x8ae` para `0x409`.** O contador só
nasce na metade de baixo da faixa (`& 0x07FF`) no ramo «milissegundo novo»
(`agora > ultimo_ms`) — e o milissegundo não era novo. Logo alguém tinha
posto o `ultimo_ms` do gerador num valor MENOR que o real entre as duas
chamadas. E havia exatamente um candidato: o teste vizinho
`contador_estourado_empresta_do_futuro` fazia
`*RELOGIO.lock().unwrap() = (5_000, CONTADOR_MASCARA)` para montar o cenário
do estouro. O `libtest` despacha os testes em ordem alfabética com um fio por
CPU; `contador_estourado…` é o 2º do módulo e `v7_nunca…` o 11º, mas o fio
do 2º só roda quando o sistema operacional o escalona — e sob carga ele
rodou no meio do laço do 11º.

O outro lado da mesma corrida apareceu no mesmo laço: `contador_estourado…`
caiu **39 vezes** com `left: (1789555531113, 1034) right: (5001, 0)` — o
vizinho tinha avançado o estado entre a escrita dele e a leitura dele.

## 2. O que eu concluí primeiro, e estava errado

Os três suspeitos do pedido eram o teste (comparação de 128 bits montada
errada), o relógio do contêiner (`SystemTime` é CLOCK_REALTIME e o NTP o puxa
para trás) e o gerador sob concorrência (janela entre ler o relógio e
incrementar o contador). Li o fonte e conclui que os três estavam limpos —
e estavam: a comparação é o `Ord` derivado sobre `[u8; 16]` big-endian; o
relógio da máquina pode retroceder, mas o `agora > ultimo_ms` do passo absorve
isso; e o passo inteiro acontece sob um `Mutex` global, sem janela.

O erro foi de **alcance da pergunta**: «o gerador é monotônico sob o
`RELOGIO`» é verdade, e não responde nada, porque o `RELOGIO` era um `static`
solto no módulo que qualquer item do módulo — inclusive o `mod tests`, filho
dele — podia escrever. A leitura anterior (a do pedido) deu o gerador por bom
pelo mesmo motivo e chegou ao mesmo lugar: «pela leitura, `proximo_passo` é
monotônico». Nenhuma das duas leituras perguntou **quem mais escreve o
estado**, porque a lista de suspeitos era de código de produção e o defeito
morava num teste.

E a segunda conclusão errada, mais curta: pensei que a suíte inteira
reproduziria como o módulo. Não reproduz na mesma taxa — 0 em 40 — porque
entre o despacho do 2º teste e o do 11º há 349 testes de outros módulos
competindo pelos quatro fios, e o fio do 2º quase sempre roda antes.

## 3. O que a medição disse

| O quê | Corridas | Falhas |
|---|---|---|
| Código antigo, `--lib uuid`, sob carga (4 processos × 250) | 1.000 | **54** (5,4 %) — `v7_nunca…` 38×, `contador_estourado…` 39× |
| Código antigo, suíte inteira do crate, sob carga | 40 | 0 |
| Defeito reposto pelas duas trocas da guarda, só `a_geracao_entre_fios…` | 20 | **20** |
| Defeito reposto, módulo inteiro | 20 | `a_geracao…` 20×, `v7_nunca…` 10×, `contador_estourado…` 3× |
| Conserto, `--lib uuid`, sob carga (4 × 250) | 1.000 | **0** |
| Conserto, suíte inteira do crate, sob carga | 40 | **0** |
| Executor do catálogo, `relogio-ao-alcance-do-teste` | 1 | PROVADA, 1/1 caíram, 4,7 s |

Os três suspeitos, cada um com o seu número:

- **O teste.** A mensagem medida mostra `0x8ae` seguido de `0x409` no mesmo
  ms: a comparação estava certa, o valor é que andou para trás.
- **O relógio do contêiner.** Na mensagem, o relógio da máquina é o mesmo ms
  dos dois ids — não retrocedeu. E o novo `relogio_da_maquina_para_tras_nao_
  leva_o_id_junto` prova na função pura que `agora` menor ou igual ao último
  emitido cai no contador: `(1_000, 7)` com `agora = 0` dá `(1_000, 8)`. O
  estado é um só por processo, não por fio.
- **O gerador sob concorrência.** `a_geracao_entre_fios_nunca_anda_para_tras`
  põe quatro fios gerando (≥ 2.000 cada) e confere sequência por fio e
  unicidade do conjunto: 1.040 corridas sob carga, 0 falhas.

## 4. A regra

**Teste de lógica que precisa de um estado monta o estado LOCAL — nunca
escreve o `static` do módulo — e o `static` fica onde o `mod tests` não
alcança.** O `libtest` roda os testes em paralelo, e um `static` ao alcance de
um teste é um `static` que outro teste vê mudar no meio do laço, quando o
escalonador quiser.

E o corolário para o papel F: **quando a lista de suspeitos é só de código de
produção, o suspeito que falta é o teste vizinho.** A primeira pergunta de uma
falha intermitente é «quem mais escreve este estado?», e a resposta inclui o
`#[cfg(test)]`.

## 5. Como está guardado hoje

- A lógica do passo virou função pura, `avancar(estado, agora)`, e o estado
  foi para `mod relogio` com o `static ESTADO` **privado** — o `mod tests` é
  filho de `uuid`, não de `relogio`, e não o enxerga. Não há como repor o
  defeito sem antes reabrir o módulo.
- A guarda `relogio-ao-alcance-do-teste` em `bancada/guardas/catalogo.py`
  repõe os dois pontos (reabre o `static` com `pub(super)` e volta a
  escrevê-lo no cenário) e exige que `a_geracao_entre_fios_nunca_anda_para_
  tras` caia. Ele cai sempre porque monta o cenário do vizinho **64 vezes**
  enquanto os fios geram: a escrita única do teste antigo derrubava o vizinho
  quando o escalonador queria (10 em 20), e guarda que depende do escalonador
  mente na metade das rodadas.
- A asserção de `v7_nunca_repete_nem_anda_para_tras` imprime os dois ids, ms
  e contador de cada um, o relógio da máquina e o estado do gerador — a
  próxima falha desse naipe não custa a caçada de novo.
- **O buraco que ficou:** a regra vale para qualquer `static` de módulo com
  teste que o escreve, e só o do `uuid.rs` foi fechado. Não há conferidor que
  procure `*ALGO.lock()… =` dentro de `#[cfg(test)]` no repositório; é
  varredura para outra rodada, com o número medido antes de virar catraca.
