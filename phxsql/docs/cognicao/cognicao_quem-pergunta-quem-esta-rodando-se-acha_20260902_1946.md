# Quem pergunta «quem está rodando?» pela linha de comando se acha

- **Quando:** 2026-09-02, 19:46
- **Onde:** `phxsql/comunicacao.sh`, o agente que existe para relatar
- **Custo:** duas versões erradas seguidas, e a segunda mais fundo que a
  primeira — no único programa cujo trabalho é não mentir

## O que aconteceu

O agente de comunicação precisa dizer o que está rodando. Escrevi:

```sh
VIVOS=$(ps -eo cmd | grep -cE '^(cargo|rustc)|node .*(bateria)|phxsqld')
```

O cabeçalho passou a dizer **«3 processos»** com a lista **vazia** logo abaixo
— o aviso se contradizendo dentro de si mesmo. A contagem era a mentirosa: o
padrão contém a palavra `phxsqld`, e o próprio `grep` aparece no `ps`
carregando o padrão na linha de comando. Ele casava a si mesmo.

Consertei tirando a âncora e usando o mesmo crivo dos dois lados. **Errou de
novo, e pior:** passou a casar o meu próprio *shell*, porque a linha de comando
dele carrega o comando inteiro que eu tinha digitado — `cargo build` incluído.

## O que eu concluí primeiro, e estava errado

Duas vezes. Primeiro que o problema era a âncora `^`. Depois que era o
`grep -v grep`. Nenhum dos dois: o problema é **medir identidade de processo
por texto**. Todo texto que descreve o alvo também descreve quem o procura.

## O que a medição disse

Com o crivo pelo nome do executável (`ps -eo comm=`), o mesmo instante que
antes reportava «3 processos» com lista vazia passa a reportar **«nada
compilando»** — e, com uma compilação de verdade em curso, reporta `cargo` e
três `rustc`, com o tempo de cada um.

## A regra

**Identidade de processo se pergunta pelo NOME DO EXECUTÁVEL, nunca pela linha
de comando.** O *shell* chama-se `bash` e nunca `cargo`, então ele sai por
construção — e não por uma exceção que alguém lembrou de escrever.

E o corolário, que é estrutural: **a lista se mede uma vez, e a contagem sai
dela.** Duas medições da mesma coisa é uma a mais do que se consegue manter em
acordo.

## Como está guardado hoje

Corrigido no `comunicacao.sh`, com a história escrita ao lado e prova nos dois
sentidos: silencioso sem nada rodando, e nomeando `cargo` + três `rustc` com
uma compilação real em curso.

**É a terceira vez que esta armadilha aparece nesta base.** Já pegou um
`pgrep -f cacar2` e um `pgrep -f video-demonstracao`, e nas duas vezes o
relato disse «está rodando» enquanto nada rodava. As duas primeiras foram
consertadas com arquivo de PID — remendo local. Esta é a primeira vez que a
causa comum foi nomeada. **Não há guarda** que impeça a quarta: seria uma
varredura recusando `pgrep -f` e `ps | grep` de linha de comando nos scripts
da casa, e ela ainda não existe.
