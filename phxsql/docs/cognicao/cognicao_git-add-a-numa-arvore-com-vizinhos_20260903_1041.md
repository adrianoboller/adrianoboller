# `git add -A` numa árvore com frentes paralelas commita o trabalho de outro

- **Quando:** 2026-09-03, 10:41
- **Onde:** o repositório inteiro, e não um arquivo
- **Custo:** um `reset --soft` e dois minutos; podia ter sido o trabalho de
  outra frente entrando num commit meu, com a mensagem errada, e sumindo da
  vista de quem o escreveu

## O que aconteceu

Fechei a primeira peça e commitei com o reflexo de sempre:

```bash
git add -A && git commit -F -
```

O `git status` do próprio comando mostrou o estrago: além dos meus quatro
arquivos, entraram `crates/phxsql-store/src/table.rs`,
`crates/phxsql-store/tests/chave-estrangeira.rs`, `bancada/guardas/catalogo.py`
e uma pasta inteira de base de prova com **vinte arquivos binários** de uma
frente vizinha — a da integridade referencial, que estava trabalhando na mesma
árvore e cujo `crates/phxsql-store/` eu tinha ordem explícita de **não tocar**.

## O que eu concluí primeiro, e estava errado

Que «não tocar em `crates/phxsql-store/`» era uma regra sobre **edição**.

Não é. É uma regra sobre o que sai com o meu nome, e **commitar é tocar** —
mais do que editar, porque editar o vizinho vê no `git diff` dele e commitar o
vizinho faz o `git diff` dele desaparecer. Ele vai procurar o trabalho onde o
deixou e não vai achar; vai achar dentro de um commit meu, descrito por uma
mensagem que fala de gatilho e de trava.

E havia um segundo erro embutido: a pasta `bancada/durabilidade/.base-da-prova/`
com binários de base de dados nunca deveria entrar em commit nenhum, de
ninguém. `add -A` não faz essa pergunta.

## O que a medição disse

Não é um número de desempenho, é uma contagem: **1 commit, 26 arquivos, dos
quais 22 não eram meus.** O desfazer foi
`git reset --soft HEAD~1` seguido de `git reset HEAD -- <os caminhos deles>`, e
o `git status` depois confirmou os arquivos deles de volta ao estado exato em
que estavam — não-preparados, com as mudanças intactas.

O que torna isto perigoso e não só chato: **o `add -A` não falha.** Ele sai com
código 0, e o commit fica bonito. Sem ler o `git status` que ele imprime, eu
teria seguido em frente.

## A regra

**Numa árvore com frentes paralelas, `git add` recebe caminhos — nunca `-A`,
nunca `.`, nunca `-u`.** E o `git status --short` se lê **depois** do commit,
não só antes: o que ele mostra é o que sobrou, e o que sobrou tem de ser
exatamente o que era de outro.

## Como está guardado hoje

Em lugar nenhum além deste arquivo, e isso fica escrito porque é o buraco:
**não há guarda técnica.** Um `pre-commit` que recusasse arquivos fora de uma
lista de território exigiria que o território estivesse escrito em algum lugar
que o hook lesse, e hoje ele vive na mensagem que abre a sessão.

Enquanto não existir, o que segura é ler o `git status` que o próprio comando
imprime — foi o que segurou hoje.
