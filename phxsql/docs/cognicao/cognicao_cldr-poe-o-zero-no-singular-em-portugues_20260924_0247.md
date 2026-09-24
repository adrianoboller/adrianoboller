# O CLDR põe o zero no singular em português: «0 pasta»

- **Quando:** 2026-09-24, 02:47
- **Onde:** `crates/phxzip-web/ui/phxzip.js`, `plural()`; captura
  `testes-web/phxzip/capturas/19-nomes-hostis.png`

## O que aconteceu

O resumo do pacote saiu «6 arquivos, **0 pasta**». O `plural()` pergunta a
categoria ao `Intl.PluralRules`, e para `pt-BR` o `select(0)` devolve `"one"`.

## O que eu concluí primeiro, e estava errado

Que a regra de plural estava errada no meu código. Não estava: é o CLDR, que
para o português põe 0 e 1 na mesma categoria — certo em francês («0 dossier»),
estranho em português do Brasil, onde se diz «0 pastas» ou «nenhuma pasta».

## O que a medição disse

`new Intl.PluralRules("pt-BR").select(0)` → `"one"`; `"fr"` → `"one"`;
`"en"`, `"de"`, `"it"`, `"es"` → `"other"`.

## A regra

Zero ganha chave própria («nenhuma pasta», «keine Ordner»), em vez de depender
da categoria do CLDR — e em vez de uma regra de plural escrita à mão, que seria
uma segunda fábrica de plural.

## Como está guardado hoje

`zip.arquivos_zero` e `zip.pastas_zero` no `textos.json`, nos seis idiomas. A
tela do PhxSql não tem `plural()` e não é alcançada.
