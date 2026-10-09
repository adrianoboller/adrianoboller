# A régua que separa produção de teste precisa ver todo `cfg(test)`, não só o literal

**Estado:** FRUTÍFERO

**Evidência:** `ba65032e`

**Validação (08/10/2026):** o caso «a cópia dentro de `#[cfg(all(test, unix))]` não conta» do `bancada/guardas/trecho-vivo.py --autoteste` passa (a execução de hoje termina em «todos passaram»), e o número 137 → 131 é reproduzível; o conserto está no commit `ba65032e`. Promovido em 08/10/2026 pelo papel H.

**Alcance da pétrea, não lei nova:** é o alcance de «régua que passa a medir mais aposenta a catraca» (G) e de «a receita de um número também envelhece» (H). **Cruzamento:** é o segundo defeito do mesmo `producao()` no mesmo dia do #12 (`cognicao_regua-por-arquivo-muda-de-numero-quando-o-arquivo-se-divide_20261008_1300.md`); os dois são o alcance da mesma régua de corte por texto, e ler um sem o outro repete o erro.

## O que aconteceu

Pedido 657, 08/10/2026. Ao fortalecer os testes dos `gancho-programa-*` com a
frase que só o ponto guardado escreve (`"e gravavel pelo grupo"`, `"por grupo
ou outros e sem o bit sticky"`), a oitava régua do
`bancada/guardas/trecho-vivo.py` (`mensagem_ambigua`) **subiu** a contagem
dessas frases em vez de reconhecê-las como únicas: `x2`, `x3`. A frase existia
uma vez no código de produção e mais uma vez em cada teste que a conferia.

## O que eu concluí primeiro, e estava errado

Que a frase escolhida não era única de verdade — que algum comentário do
`gancho.rs` a repetia — e que o conserto era procurar uma frase mais longa.
Teria trocado frases boas por piores e o número continuaria errado.

## O que a medição disse

O `gancho.rs` abre os testes com `#[cfg(all(test, unix))] mod testes {`. O
corte da produção (`MODULO_DE_TESTE_COM_CORPO`) só reconhecia o literal
`#[cfg(test)]`, então o arquivo inteiro contava como produção, testes
inclusive. Com o corte aceitando `cfg(all(test, …))`: **137 → 131** entradas
ambíguas, **6 saíram** (os quatro `gancho-programa-*`, `gancho-sem-kill-no-prazo`
e `gancho-zumbi`) e **nenhuma entrou**. Nos dois sentidos, num arquivo
sintético: com o corte antigo a frase repetida no módulo de teste acusa
(`{'perde precisao': 2}`); com o novo, `[]`.

## A regra

Régua que separa produção de teste por texto tem de reconhecer **toda** forma
de `cfg` que liga o teste — `cfg(test)`, `cfg(all(test, …))` —, e quando ela
acusa uma frase que você acabou de escrever num teste, desconfie primeiro do
corte, depois da frase.

## Como está guardado hoje

`trecho-vivo.py --autoteste`, caso «a cópia dentro de
`#[cfg(all(test, unix))]` não conta». O buraco que fica: `cfg(any(test, …))`
e `cfg(test)` aplicado a item solto (não a `mod`) continuam fora do corte, e
as duas existem: `cfg(any(test` aparece 1 vez em `crates/`, e há funções e
constantes de teste soltas (`irmas.rs`, `crc.rs`, `qr.rs`, `hash.rs`) que a
régua lê como produção. O efeito delas no número não foi medido.
