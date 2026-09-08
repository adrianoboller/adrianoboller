# A perda de precisão da `Sequence` acontece no analisador, não no `numerar`

Descoberta em 2026-09-07, durante a frente G2 (pedido 229, defeito b).

## 1. O que aconteceu

Um id `Sequence` acima de 2⁵³ mandado como **número cru** pelo protocolo é
gravado trocado, calado (bloco 19 da sonda: `9007199254740993` → gravado
`9007199254740992`). O `Json` desta casa só tem `Numero(f64)`
(`crates/phxsql-core/src/json.rs`), e acima de 2⁵³ o passo do `f64` vira 2.

O `docs/AUTONUMBER.md`, §B.2.6 item 2, propunha o conserto assim: *«`numerar` e
`ajustar_sequencia` recusam valor acima de 2⁵³»*.

## 2. O que eu concluí primeiro, e estava errado

Segui a proposta ao pé da letra: ia pôr a recusa no `Table::numerar`
(`table.rs`), que é onde o valor escolhido pelo cliente vira contador
(`anotar_sequencia`). Parecia o lugar certo — é a função que recebe o valor da
`Sequence` na inserção.

Estava errado, e o erro teria **compilado e passado nos meus próprios testes se
eu os escrevesse sobre `Value`**. Porque quando o valor chega ao `numerar`, ele
**já é** um `Value::UInt(9007199254740992)` — a perda aconteceu antes, no
`Json::analisar`, que converteu o literal cru para `f64` e arredondou. O
`numerar` nunca vê o `9007199254740993`; vê o `...992` já corrompido, que é
`≤ 2⁵³` e passaria por qualquer teto posto ali.

## 3. O que a medição disse

O caminho do valor, traçado no fonte:

```
cliente manda  {"id": 9007199254740993}   (numero cru)
  Json::analisar  → Json::Numero(9007199254740992.0)   ← PERDA AQUI
  json_para_valor → Value::UInt(9007199254740992)
  Table::numerar  → anotar_sequencia(9007199254740992)  ← tarde demais
```

O teste `f64_perde_precisao_acima_de_dois_elevado_a_cinquenta_e_tres`
(`json.rs`) confirma: `Json::analisar("9007199254740993").inteiro()` devolve
`9007199254740992`. E o mesmo inteiro **como texto**
(`Json::Texto("9007199254740993")`) não passa por `f64` e sobrevive — é a saída
que a recusa aponta.

## 4. A regra

**Onde um número perde precisão é onde o texto vira `f64`, não onde o valor é
usado. O crivo mora na fronteira `Json → Value`, e o irmão que grava certo é o
mesmo valor como texto.**

## 5. Como está guardado hoje

- O crivo `Json::inteiro_impreciso` (`json.rs`) e a constante
  `INTEIRO_EXATO_MAX = 2⁵³`.
- A recusa na **entrada**: `json_para_valor`, ramo `Sequence`
  (`crates/phxsql-server/src/valores.rs`), e no campo `proxima` do
  `op_ajustar_sequencia` (`servidor.rs`).
- A saída honesta: `valor_para_json` emite `Sequence` acima do teto como texto.
- Testes: `sequencia_recusa_numero_cru_acima_do_teto_do_f64`,
  `sequencia_grande_como_texto_atravessa_intacta`,
  `sequencia_grande_sai_como_texto_no_json` (`valores.rs`).
- Guardas provadas: `sequencia-numero-cru-perde-precisao` e
  `sequencia-grande-sai-numero-mentiroso` (`bancada/guardas/catalogo.py`).

**Onde o buraco ficou, nomeado:** `Int8`/`UInt8` partilham o mesmo teto e
**não** ganharam o crivo — alargá-lo muda o comportamento de todo cliente que
hoje manda `Int8` grande como número, e isso é decisão de projeto (papel C). O
`Json::inteiro_impreciso` já está pronto para alcançá-los; falta a decisão.
