# `caem` sem o módulo é QUEBRADA — e a régua barata não vê

**Descoberto em 17/09/2026, ~00:40 UTC.** Frente F (prova real) com o chapéu
de G, na segunda leva da pétrea «senha nunca em texto puro».

## 1. O que aconteceu

A entrada `debug-da-ligacao-mostra-a-senha`, escrita pela frente do `Debug`
derivado em 17/09 (commit `74de67e`), nomeava os três testes **sem o caminho
do módulo**: `o_debug_da_ligacao_nunca_mostra_a_senha_nem_o_token` em vez de
`dblink::testes::o_debug_da_ligacao_nunca_mostra_a_senha_nem_o_token`. O
`trecho-vivo.py --catraca` dizia `ok 0` nas quatro réguas. O provador
oficial (`provar-guardas.py --so debug-da-ligacao`, 1m30s) devolveu:

```
debug-da-ligacao-mostra-a-senha QUEBRADA
    teste que o catalogo nomeia e o binario nao tem: o_debug_da_ligacao_…
```

O `julgar` do provador compara o nome do `caem` com o que o `cargo test`
imprime (`test dblink::testes::… ... ok`), e o nome curto não está lá.

## 2. O que eu concluí primeiro, e estava errado

Li o briefing («a frente do `Debug` catalogou 1 guarda a mais») e contei a
entrada como cobertura — o `--catraca` verde reforçou. Só desconfiei ao ler o
`julgar` para escrever a sonda do raio: `vereditos.get(n)` com `n` vindo do
catálogo. A régua barata nunca teria acusado, porque `TETO_TESTE_MORTO` e
`TETO_TESTE_FORA_DO_BINARIO` procuram `fn <nome>` no arquivo — o nome curto
**existe**, então as duas passam.

## 3. O que a medição disse

- `--catraca` antes do conserto: `ok 0` nas quatro réguas; piso 170.
- provador antes: `QUEBRADA`, três nomes «que o binário não tem».
- provador depois (nomes com `dblink::testes::`): `PROVADA 1/1`, 32,9 s; raio
  medido pela sonda: **1 dos 1.107** do `phxsql-server --lib`.

## 4. A regra

**Nome de teste no catálogo leva o módulo, e a régua que confere teste vivo
tem de exigir `::` no nome — teste que existe com outro caminho é QUEBRADA
que se vê sem compilar.**

## 5. Como está guardado hoje — e onde o buraco ficou

Consertado no `bancada/guardas/catalogo.py` (os três nomes), provado, e
escrito na §15.7.7 do `docs/CATRACAS.md` como a **sexta forma de QUEBRADA**
para a tabela da §12. **O buraco:** a régua ainda não a vê. É uma pergunta de
texto puro (`"::" in nome`) que cabe no `trecho-vivo.py` nascendo em 0 hoje;
não entrou nesta frente porque uma catraca nova é decisão do papel G com o
orquestrador, não do papel F de passagem.
