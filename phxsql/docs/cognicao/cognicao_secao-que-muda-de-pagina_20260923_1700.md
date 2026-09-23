# Cognição: mudar uma seção de página é mudar três coisas junto — o marcador, a receita do número e o CSS dela

**Descoberto em 23/09/2026, entre 16h40 e 18h20**, ao partir o dossiê no pedido
411 (a §18 virou a oitava página).

## 1. O que aconteceu

O dossiê estava em **2.703.573 bytes** e o guarda da republicação exige reler a
versão publicada inteira antes de aceitar a nova — o mesmo teto de ~450 KiB
(**460.800 bytes**) que partiu a página dos pedidos em cinco no pedido 403. A
§18, as vinte capturas, sozinha pesava **2.106.613 bytes**: **77,9%** do
arquivo.

Mover uma seção parece um recortar-e-colar. São três coisas de uma vez, e as
três só apareceram ao fazer:

1. **Os marcadores mudam de casa, e cada um tem um dono que faz `sys.exit`.**
   `capturas:`, `bancada:`, `bancada:tabela:`, `bancada:diagnostico:`, `trio:`,
   `tetos:` e `cobertura:` viviam nas seções movidas. Apagar o marcador quebra
   **cinco** geradores de uma vez, e cada um deles para dizendo que a marca não
   está no dossiê — em vez de gravar em lugar nenhum.
2. **O CSS da seção vai junto.** `.telas`, `.tela`, `.qual` e `.larga` só
   serviam à galeria: **923 bytes** que ficariam no dossiê mordendo nada, e
   faltariam na página nova. É o corolário da receita do número, aplicado a
   folha de estilo.
3. **A legenda da figura carrega o número da casa antiga.** A prosa movida
   trazia `<b>Figura 27.</b>` e `<b>Figura 28.</b>` — os números que elas tinham
   no dossiê. Na página nova elas são a 1 e a 2.

## 2. O que eu concluí primeiro, e estava errado

**Errei duas vezes, e as duas eram «plausível não é medido».**

**A primeira:** escrevi a seção nova do gerador **parafraseando** a §32 (a
bancada), de cabeça, porque «é prosa, eu li». A §32 tem **13 subseções, três
figuras e seis notas em 53.157 bytes** — a paráfrase teria publicado umas dez.
O contrato dizia «nada se perde», e o jeito de cumprir não é escrever de novo:
é **transportar verbatim**, por script, e conferir o byte. Foi por isso que o
molde deixou de usar `str.format()` — 34 KiB de HTML movido têm chaves dentro,
e dobrar chave à mão é a forma clássica de perder uma linha numa mudança que
deveria ser um transporte.

**A segunda:** deixei a semente `Figura 27` no molde e contei com o
`numerar-figuras.py`, que roda por último, para acertar. O portão reprovou na
hora: o gerador da casca **recoloca 27 a cada corrida** e o numerador volta a
pôr 1 — **nunca há ponto fixo**, e o portão fica VERMELHO para sempre,
alternando com o numerador. Pôr «1» como semente também não resolve: morre na
primeira figura que alguém insira acima.

**E a terceira, que eu nem desconfiei:** achei que copiar
`body{overflow-x:hidden}` do dossiê bastava para a página não rolar de lado.

## 3. O que a medição disse

| o que | medido |
|---|---:|
| dossiê antes | 2.703.573 B |
| §18 sozinha | 2.106.613 B (77,9%) |
| as 21 imagens `data:` | 2.171.386 B — **4,71× o teto** |
| só as 20 capturas | 2.046 KiB — **4,55× o teto** |
| dossiê depois da mudança | **459.395 B**, folga de **1.405 B (0,30%)** |
| página nova | 2.258.328 B — **4,90× o teto** |

E o que a sonda disse sobre o estouro lateral, que é o número que eu não
esperava:

> `sonda-de-estouro.mjs console-em-imagens.html 390` → **ESTOURO: a página rola
> 246 px para o lado**, com `body{overflow-x:hidden}` no lugar.

Quem segurava no dossiê **não era o `overflow-x:hidden`**: era o
`.pagina{display:grid;grid-template-columns:minmax(0,1fr)}`. Eu tinha
simplificado a `.pagina` para um bloco com `padding`, e o `hidden` do corpo não
clampa o filho — clampa a pintura. O culpado concreto ficou nomeado: a tabela
dos quatro motores que o `trio:` entrega **sem um `.rolo` em volta**, com seis
cabeçalhos `nowrap` e **636 px de min-content**. No dossiê ela estava
**cortada** desde sempre — a última coluna existia e não dava para chegar nela.

## 4. A regra

**Seção que muda de página leva três coisas junto: o marcador com o dono que
faz `sys.exit`, o CSS que só servia a ela, e o número da legenda — e nenhuma
delas aparece lendo o diff.** Prosa movida se transporta por script e se
confere por byte; número de figura em prosa movida não se semeia, se **gera na
casca**, senão o portão nunca chega a ponto fixo. E antes de publicar, rode a
**sonda do estouro**: `body{overflow-x:hidden}` não segura filho largo — quem
segura é `minmax(0,1fr)`.

## 5. Como está guardado hoje

- **O transporte** está no `SECAO_2`/`SECAO_3` de
  `docs/dossie/pagina-do-console.py`, verbatim, com `trocar()` por `@@chave@@`
  em vez de `format` — e o motivo escrito no docstring da função.
- **A numeração** saiu da semente: `numerar_as_figuras()` do mesmo gerador
  renumera a casca pela ordem do documento, **importando a regra do
  `numerar-figuras.py`**, que continua sendo o dono dela e o conferidor final.
  O portão confirma o ponto fixo.
- **O CSS** foi junto para o gerador da página nova, e o do dossiê perdeu os
  923 bytes da galeria e os 469 da placa da marca.
- **O estouro** está consertado na `.pagina` e na `table.tab`, com o número de
  23/09 no comentário, e reconferido pela sonda: *sem rolagem lateral a 390 px*.
- **O buraco que fica**, e ele não é pequeno: a página nova tem **4,90× o
  teto**. Ela publica barato na primeira vez e o preço aparece na **segunda**,
  quando o guarda exigir reler a versão publicada. Está escrito no
  `docs/dossie/LEIA-ME.md`, na seção da oitava página, para ninguém descobrir
  na hora.
