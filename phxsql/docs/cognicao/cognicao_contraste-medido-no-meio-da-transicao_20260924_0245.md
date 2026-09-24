# Contraste medido no meio da transição reprova o tema certo

- **Quando:** 2026-09-24, 02:45
- **Onde:** `testes-web/phxzip/exercitar.mjs`, caso `tema-claro`

## O que aconteceu

O caso troca para o tema claro e mede o contraste de cada botão de ação.
Reprovou: «Escolher pacote» **2,38:1** e «Fechar» **2,18:1** sobre o papel.

## O que eu concluí primeiro, e estava errado

Que as cores da ação do tema claro não passavam no papel, e que eu teria de
escurecê-las. Mas as do claro são as do PhxSql, já medidas acima de 4,5:1. Os
números batiam com outra coisa: 2,18:1 é o `#a8b0c0` **do tema escuro** sobre
branco. O `.acao` tem `transition: color .12s`, e a medida saiu no primeiro
quadro depois do clique — a cor do tema anterior pintada sobre o fundo do novo.

## O que a medição disse

Esperando `document.getAnimations().length === 0` antes de medir, com a lista
aberta (14 botões de ação e 7 textos): menor contraste **5,45:1** no claro e
**5,30:1** no escuro, nenhum abaixo de 4,5 (`resultado.json` de 24/09/2026,
03:12). E um segundo engano no mesmo caso, achado depois: a medida rotulada
«com erro na tela» media a lista que o caso anterior deixara aberta. Número
certo com rótulo errado também é número errado.

## A regra

Cor computada se mede depois que as animações acabam. Um número tirado no meio
de uma transição não é da cor nem do tema: é do caminho.

## Como está guardado hoje

O `coresNoTema` do `exercitar.mjs` espera as animações. A prova das guardas
repõe a cor do escuro no papel (`--acao-consultar: #5fa6e8` no claro) e confere
que o caso reprova — prova de que a espera não afrouxou a medida.
