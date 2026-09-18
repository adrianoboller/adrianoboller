# Falar sobre a armadilha casa com a busca pela armadilha — e o conteúdo publicado também

18/09/2026, 19:20. Papel A, republicando as sete páginas depois do fecho da leva.

## 1. O que aconteceu

Publiquei a página dos pedidos e o dossiê. A plataforma devolveu, nas duas, um
aviso dizendo que a página oferece um arquivo por download que o visualizador
nunca entrega.

Li como achado novo e **abri o pedido 380**. Depois medi, e o pedido estava
errado de três maneiras.

## 2. O que eu concluí primeiro, e estava errado

**O pedido inteiro.** As três coisas que eu errei numa linha só:

1. **Duplicata.** O pedido **327** já existia, com a mesma causa nomeada pelo
   próprio serviço e medida em 17/09.

2. **Fundamentação errada, pelo caminho que esta casa já tinha mapeado.** No
   dossiê, os dois `<a download>` que o `grep` conta são os **comentários** que
   dizem «nada de `<a download>`, que o visualizador bloqueia». O botão é
   `window.print()` — que é do navegador e **funciona**.

   E o agravante: o `docs/dossie/LEIA-ME.md` já registrava esse tropeço,
   **com o número**, em 07/09: *«eu contei `grep -c '<a [^>]*download'`,
   recebi 2 e escrevi que havia dois links. Eram os comentários de novo. Um
   casador de texto não sabe a diferença entre fazer e falar sobre fazer.»*
   Eu recebi os mesmos **2** e escrevi a mesma coisa, onze dias depois, com a
   lei aberta na mesma pasta. **Lei escrita não impede o erro; ela só o nomeia
   depois** — e o valor dela apareceu na hora de conferir, não na de errar.

3. **E foi o erro que trouxe o único ganho.** Na `pedidos.html` não há
   comentário de fonte nenhum, e o aviso apareceu igual. O que o varredor
   casou foi o **texto do pedido 327**, que a página publica **como
   conteúdo** e que cita `createObjectURL` e `window.claude.downloads` ao
   explicar o problema.

## 3. O que a medição disse

| medida, nas cinco páginas de `docs/dossie/` | resultado |
|---|---:|
| `window.print()` de verdade | dossiê e pedidos |
| `<a download>` de verdade | **0** |
| `createObjectURL` de verdade | **0** |
| `window.claude` de verdade | **0** |
| ocorrências que são **prosa** sobre a armadilha | 2 no dossiê (comentário), 6 nos pedidos (conteúdo do 327) |
| páginas de testes, gráficos e status | **0** de qualquer padrão, nem como prosa |

## 4. A regra

**Comentário sobre a armadilha casa com a busca pela armadilha — e conteúdo
publicado sobre a armadilha também.** A primeira metade já era lei aqui; a
segunda é nova, e é pior de achar: uma página que **documenta** um defeito
passa a ser sinalizada como **tendo** o defeito, e o aviso nunca vai parar de
aparecer, porque a página existe justamente para falar dele.

E o corolário, que é o que me custou o pedido errado: **antes de abrir pedido
a partir de um aviso automático, procure se ele já tem número — e meça se o
que o varredor casou é código ou prosa.** Pedido mal fundado é pior que pedido
faltando: ele vira trabalho que alguém faz.

## 5. Como está guardado hoje

- O pedido **380 foi recolhido** com o motivo medido, apontando para o 327 —
  como o 362 foi em 18/09 de manhã. Recolher com número é o formato desta casa
  para achado que morre medido.
- O **alcance novo** está no `docs/dossie/LEIA-ME.md`, na seção do falso
  positivo, com a tabela das cinco páginas.
- **O que NÃO está guardado, e fica nomeado:** não há guarda que impeça a
  próxima repetição. Um conferidor que distinga «fazer» de «falar sobre fazer»
  é exatamente o casador genérico que esta casa já **recusou com número** — e
  a recusa continua valendo. O que existe é a lei escrita, que desta vez pegou
  o erro na conferência e não antes dela.
