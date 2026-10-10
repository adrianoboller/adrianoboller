# A política de Trusted Types confere o texto que ENTREGA, e não o que recebeu

**Estado:** PENDENTE

## O que aconteceu

Pedido 771, terceira etapa: `require-trusted-types-for 'script'` e
`trusted-types phx` na CSP da página. Os 221 sinks de HTML da `ui/` passaram
pelo `phxHTML` (`ui/funil.js`), a única porta para a política `phx`. A
primeira bateria inteira com a diretiva deu 99/105: as seis quedas eram
**do próprio teste** (`07-responsivo`, `15-transacoes`, `46-botoes-de-telas-avulsas`
fazem `innerHTML` dentro do `page.evaluate`), e nenhuma da tela.

## O que eu concluí primeiro, e estava errado

1. Que o `createHTML` podia conferir a entrada e devolvê-la como veio, com os
   comentários `<!-- … -->` dos modelos recusados. Os modelos da telemetria e
   da Claude têm **8** comentários dentro de HTML montado; recusar quebraria
   telas. Pular os comentários na conferência também não servia: o navegador
   fecha comentário em `--!>`, e um leitor que pula até `-->` vê um atributo
   entre aspas onde o navegador vê `<img onerror>`.
2. Que `textarea` e `title` eram texto puro e bastava saltar até o fecho. No
   HTML são RCDATA; dentro de um `<svg>` são elementos comuns e o que está
   dentro vira etiqueta. A mesma string é lida de dois jeitos conforme o lugar
   em que o `innerHTML` cai.
3. Que a medida dos sinks da tela bastava. A bateria mostrou que os casos de
   teste também escrevem `innerHTML`, e eles quebram primeiro.

## O que a medição disse

- O funil TIRA os comentários e confere o texto já sem eles, que é o mesmo
  texto que o navegador recebe: o truque do `--!>` some junto (vetor no
  `tt-vetores`: a saída é `""`, não um `<img>`).
- `textarea`/`title` só entram sem `<` por dentro: aí os dois jeitos de ler dão
  o mesmo texto. Os 4 `<title>` do SVG e os 2 `<textarea>` da tela têm dado
  escapado e passam.
- Vetores: 19 benignos passam (inclusive `&amp;` em consulta de URL, que a
  primeira versão recusava), 24 venenos recusados.
- RED: sem a diretiva, «innerHTML com texto cru PASSOU»; com o funil reduzido
  a `s => s`, «deixou passar um manipulador onerror», e o `script-src-attr`
  ainda segurou o `onerror` (xss = 0): as camadas são independentes.

## A regra

Política que transforma tem de conferir o texto DEPOIS de transformar — o que
ela confere tem de ser byte a byte o que ela entrega. E o leitor da conferência
usa o espaço em branco do analisador (`\t\n\f\r` e espaço), não o `\s` do JS.

## Como está guardado hoje

`testes-web/casos/52-csp.mjs` (cru barrado e relatado, veneno pelo funil
recusado, segunda política morta), `http.rs::todo_innerhtml_da_interface_passa_pelo_funil`
(sink sem o funil, em qualquer arquivo servido, reprova) e o vigia da bateria,
que passou a reprovar o evento `phxhtmlrecusado`. **Buraco:** o funil é um
leitor próprio; um estado do analisador do navegador que ele não imite
(conteúdo estrangeiro além de SVG, `select` com regras novas do Chromium) é
onde uma divergência voltaria. Não há vetor para `<select>` ainda.
