# A negação do shell passava por engano: o erro era de argumento, não de política

**Estado:** FRUTÍFERO

**Evidência:** `dc438b6`

## O que aconteceu

O E2E do desktop checava «`execute_shell` negado» pela presença de erro. Passava — com
`invalid args: missing field uuid`. A política nunca foi consultada.

## O que eu concluí primeiro, e estava errado

Que «volta erro» provava negação. Com a requisição válida, a negação volta como
**resultado** (`status: denied` + `evidence_uuid`), não como exceção: a checagem antiga
reprovaria a negação correta e aprovaria qualquer requisição malformada.

## O que a medição disse

Requisição válida: `status=denied`, `disabled by policy`, evidência gravada, cadeia
válida depois. 12/12, 8/8 corridas.

## A regra

Teste de recusa tem de montar a entrada **válida** e conferir o **motivo** da recusa.

## Como está guardado hoje

`tests/desktop/desktop_e2e.py` exige `status == denied` e ausência de `invalid args`.
