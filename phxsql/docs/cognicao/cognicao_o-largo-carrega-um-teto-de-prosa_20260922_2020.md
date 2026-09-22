# Cognição: o `largo` que existe para alargar carrega um teto de PROSA junto

**Descoberto em** 22/09/2026, 20:20 UTC, exercitando a tela do pedido 379
(o bloco do fio no assistente de replicação).

## 1. O que aconteceu

O passo 2 do assistente ganhou um campo para o **pino** — a chave pública
esperada do outro servidor, **64 dígitos hexadecimais**
(`replicacao.origens[].chave_do_fio`). Ele nasceu como `label.cmp` dentro de
um `.form-dbl`, que é uma grade de trilhas de 230 px: o valor não cabia e o
campo rolava.

O conserto óbvio já existia na casa e foi o que usei: `label.cmp.largo`, que
faz `grid-column:1/-1`. A captura passou de 265 px para **453 px** — melhorou,
e continuou rolando.

O que faltava ler está trinta linhas abaixo da regra do `largo`, em
`ui/index.html`:

```css
.ficha-edit label.largo,.form-dbl .cmp.largo{max-width:var(--medida)}
```

O `largo` **atravessa a grade e para na medida de leitura**. A intenção está
escrita no comentário dele, e é boa: «o campo largo (memo, SQL, descrição) é
uma caixa de TEXTO, então o teto dela é o do texto e não o do campo».

## 2. O que eu concluí primeiro, e estava errado

Duas vezes.

Primeiro: **«o campo está estreito porque a grade tem três colunas; `largo`
resolve»**. Resolveu metade, e eu quase parei aí — a captura «melhorou»
visivelmente, e melhora é o disfarce mais barato de um conserto incompleto.

Segundo, e pior: escrevi a asserção como **`largura > 480 px`**. Era número
inventado, e ele reprovou 453 px sem me dizer nada sobre o que importa. A
pergunta certa não tem pixel nenhum dentro: **o valor cabe?** —
`scrollWidth > clientWidth`. Foi ela que mediu, e foi ela que ficou no roteiro.

## 3. O que a medição disse

Medido no navegador, a 1280 px de viewport, com o diálogo `.caixa.larga`:

| | largura do campo |
|---|---|
| `label.cmp` (padrão, teto `--teto-campo`) | 265 px |
| `label.cmp.largo` (teto `--medida`) | **453 px** |
| a grade inteira, que é o que `1/-1` deveria dar | **834 px** |

O `grid-column` computado era `1 / -1` **o tempo todo** — a grade dizia 834 e
o item media 453. Quem cortava era o `max-width`, e não a grade. Ler o HTML
mostrava `class="cmp largo"` e nada mais; o corte só aparece com o elemento
pintado e medido.

Os 64 dígitos cabem em 834 px na fonte monoespaçada de 12,5 px.

## 4. A regra

**Classe de ajuda também morde — e a que morde pior é a que parece resolver.**
Antes de usar um `largo`, um `mini`, um `compacto`, leia o que ele carrega
JUNTO do que você quer: o nome promete uma propriedade e a regra costuma
entregar duas. E **meça o que importa, não pixels**: campo cabe quando não
rola, e esse é o número que não envelhece quando a fonte ou o diálogo mudar.

O corolário do caso: **chave não é prosa.** A medida de leitura existe para a
linha de texto não ficar longa demais para o olho voltar ao começo da
seguinte. Uma chave não se lê — se **confere**, dígito a dígito, contra o que
o terminal do outro servidor imprimiu. Cortada na medida, ela esconde
justamente o COMEÇO do valor, que é por onde a conferência começa. É a mesma
razão pela qual o `<pre>` já estava isento, escrita quatro parágrafos acima na
mesma folha.

## 5. Como está guardado hoje

- **A isenção**, ao lado da do `<pre>` e com o número medido no comentário:
  `.form-dbl .cmp.largo.chave-hex{max-width:none}`, em
  `crates/phxsql-server/ui/index.html`. Classe explícita em vez de `:has()`,
  para não depender de suporte de seletor.
- **A fonte**: nasceu junto `input.campo.mono`, o irmão de uma linha do
  `textarea.campo.mono` que já existia — em fonte proporcional o `1` e o `l`
  se confundem, e conferir chave é o que o pino existe para que se faça.
- **A guarda**, exercitando e não lendo:
  `testes-web/exercitar-fio-e-rele.mjs` mede `scrollWidth > clientWidth` no
  `#rzPino` com um pino de verdade dentro, e mede também que a fonte é
  monoespaçada e que o valor **não** chegou maiúsculo à tela (rótulo se
  estiliza; dado, nunca). Reponha o `max-width` e a linha falha.
- **Onde o buraco ficou:** a guarda cobre ESTE campo. Não há régua que
  pergunte «quantos campos desta página rolam com o valor que eles esperam»,
  e ela é que pegaria o próximo. Quem escrever um campo de valor longo
  continua dependendo de abrir a tela e olhar.
