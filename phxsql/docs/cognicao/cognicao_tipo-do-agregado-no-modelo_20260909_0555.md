# O tipo de um agregado no modelo do `consultar` nao e o tipo da coluna

- **Descoberta:** 09/09/2026, ~05:55, lendo `pivot.rs::fechar_valor` para
  montar o cabecalho `colunas` do `agrupar` (pedido 237).
- **Frente:** C20-CONSULTA.

## 1. O que aconteceu

O modelo tipado do `consultar` precisa do tipo de cada coluna de um sub-pedido
`agrupar`. O contrato (`docs/propostas/comparativo-19.md`) dizia: «`soma`/
`media`/`minimo`/`maximo` → o tipo da coluna agregada». Fui montar o cabecalho
a partir do tipo da coluna e, ao ler `fechar_valor`, o `Value` que ele devolve
para uma coluna `Int4` nao e `Int`: e `Value::Real`, e o `ColumnType` que sai
junto e `Real8`.

## 2. O que eu conclui primeiro, e estava errado

Que `soma(valor)` sobre uma coluna `Int4` sairia como inteiro — `Int8`, digamos
—, porque «somar inteiros da inteiro» e porque o contrato dizia «o tipo da
coluna agregada». Ia escrever o cabecalho copiando `esquema.colunas()[c].ty`
para a soma e o minimo. Teria dito `Int4` num campo que a resposta entrega como
numero fracionario, e um `consultar` de fora que comparasse `soma > 10` acabaria
convertendo `12.0` para `Int4` — funciona por sorte, quebra no primeiro
`media`.

## 3. O que a medicao disse

`Acumulador` guarda dois dominios: `soma_i: i128` so para `Decimal` (inteiro
escalado, exato) e `soma_f: f64` para todo o resto. `fechar_valor` devolve:

- `contagem`/`distintos` → `Value::UInt`, `UInt8`.
- coluna `Decimal` → `Value::Decimal`, `Decimal { 38, escala }` (a media divide
  o inteiro escalado uma vez, truncando na escala da coluna).
- **qualquer outra coluna** (`Int4`, `Real4`, `Date`…) → `Value::Real`,
  **`Real8`** — inclusive a soma e o minimo, nao so a media.

Medido no teste `a_contagem_por_coluna_ignora_o_nulo_e_o_cabecalho_diz_o_tipo`:
`soma_valor` e `media_valor` sobre `Int4` saem com tipo `Real8`; `soma_preco` e
`media_preco` sobre `Decimal(10,2)` saem `Decimal { precisao: 38, escala: 2 }`.
A media de inteiro e uma fracao (`(5+7)/2 = 6`, mas `12/... ` em `f64`), e por
isso `Real8` e a resposta certa, nao o `Int` que eu esperava.

## 4. A regra

O tipo com que um agregado SAI mora num lugar so —
`pivot.rs::tipo_do_agregado` —, e ele decide pelo par `(funcao, decimal)`, nao
pelo tipo da coluna: contagem e `UInt8`, `Decimal` preserva o Decimal, e todo o
resto (soma, media, minimo, maximo) e `Real8`. O cabecalho `colunas` e o valor
de cada grupo saem do MESMO lugar, para nao poderem divergir.

## 5. Como esta guardado hoje

`pivot.rs::tipo_do_agregado` e `decimal_e_escala`, chamados por `fechar_valor`
(o valor) e por `op_agrupar` (o cabecalho `colunas`). O contrato do
comparativo-19 esta corrigido de fato na implementacao e provado no teste; a
frase do contrato («o tipo da coluna agregada») era a premissa medida errada —
resultado, nao falha, como a lei do projeto pede que se registre.
