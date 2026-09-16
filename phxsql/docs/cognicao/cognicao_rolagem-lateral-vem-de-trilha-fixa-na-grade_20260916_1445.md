# Os 3 px de rolagem lateral vinham de uma trilha FIXA na grade, não de tabela

Descoberto em 16/09/2026, 14:45, exercitando a sétima página
(`docs/status/status-do-projeto.html`) a 390 px.

## 1. O que aconteceu

A página tem tabelas largas, e todas vivem dentro de um `.rolo` com
`overflow-x:auto` — a receita de sempre para tabela em telefone. Mesmo assim,
a 390 px a **página inteira** rolava 3 px para o lado.

A primeira sonda apontou o suspeito óbvio:

```
"estouram": ["TABLE. @538", "THEAD. @538", "TR. @538", "TH.num @401", …]
```

Doze elementos de tabela passando da janela. Só que **nenhum deles era a
causa**: todos estavam dentro do `.rolo`, que os clipa — quem rola é o
contêiner, não o documento.

## 2. O que eu concluí primeiro, e estava errado

Concluí que o `.rolo` não estava pegando todas as tabelas, e comecei a
procurar a tabela que tinha escapado do embrulho. Não havia nenhuma: as sete
estavam embrulhadas.

O diagnóstico plausível sobreviveu porque a sonda media a coisa errada —
`getBoundingClientRect().right > innerWidth` acha todo elemento que *passa* da
janela, inclusive os que um ancestral rolante já resolveu. **Uma sonda que
acusa o inocente junto com o culpado não é sonda, é lista.**

A segunda sonda ignorou quem tem ancestral com `overflow-x: auto|scroll` e
devolveu `[]` — nenhum elemento passava da janela. E a página continuava
rolando 3 px. Foi aí que a terceira sonda fez a pergunta certa: **quem tem
`scrollWidth` maior que o próprio `clientWidth` sem ser rolante?**

## 3. O que a medição disse

```
{"t":"SPAN","c":"n","cw":62,"sw":83,"txt":"244 · 91,4%"}
```

A coluna do número da barra era `5.4em` no desktop e `4.6em` no telefone —
**62 px de trilha para 83 px de texto `nowrap`**. O texto vazava 21 px para
fora da grade, e o pior pedaço desse vazamento chegava 3 px além do envelope:

| medida | antes | depois |
|---|---:|---:|
| `documentElement.scrollWidth` a 390 px | 393 | 390 |
| rolagem lateral real (`window.scrollX` após `scrollTo(9999,0)`) | 3 | 0 |
| trilha do número × conteúdo | 62 px × 83 px | `auto` |

Conferido contra três páginas irmãs da casa no mesmo harness
(`docs/dossie/status.html`, `docs/pmo/status-do-projeto.html`,
`docs/dossie/graficos.html`): as três dão `scrollWidth` 390 e rolagem 0 — então
os 3 px eram meus, e não do medidor.

## 4. A regra

**Coluna de grade que recebe número não leva medida fixa: leva `auto`.** E:
**sonda de estouro lateral tem de descontar quem já está dentro de um contêiner
rolante** — senão ela entrega doze inocentes e esconde o culpado, que é pior
que não sondar.

## 5. Como está guardado hoje

- A grade da barra é `minmax(96px,13em) minmax(0,1fr) auto` no desktop e
  `minmax(0,1fr) auto` até 640 px, com o comentário do porquê ao lado — e o
  número continua vindo **depois** da barra, nunca por cima dela.
- A sonda **não morreu com a sessão**: virou
  `docs/dossie/sonda-de-estouro.mjs`, irmã do `olhar.mjs` — aquele tira a
  foto, esta mede. Ela faz as três perguntas separadas (a página rola? quem
  estoura a caixa? quem estoura o conteúdo?) e **sai != 0** quando a página
  rola, para caber num portão.
  **Prova real nos dois sentidos, medida:** com o defeito reposto numa cópia da
  página (trilha `5.4em`/`4.6em` de volta) ela sai **1** dizendo «rola 3 px»;
  com o conserto, sai **0**.
- **Onde o buraco ficou:** ela é um comando que alguém tem de lembrar de dar —
  não está em portão nenhum nem na bateria de `testes-web/`. *Catraca que só
  roda quando alguém lembra não segura nada*, e esta linha diz isso em vez de
  parecer resolvido.
