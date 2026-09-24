# Texto solto em flex come o espaço da frase — e só o alemão mostrou

- **Quando:** 2026-09-24, 02:47
- **Onde:** `crates/phxzip-web/ui/phxzip.js`, o rodapé da lista de compactar
  (`#resumoTeto`, dentro do `.teto-uso`, que é `inline-flex`)

## O que aconteceu

A frase «{usado} de {teto} que o servidor aceita» sai pela `frase()`, que corta
o texto nos marcadores e põe cada dado num `<span class="dado">` — do jeito
certo, para o dado nunca se estilizar. O fragmento foi posto **direto** num
container flex. Cada pedaço de texto virou um item flex anônimo: os espaços das
bordas sumiram e o `gap` de 6 px entrou no lugar deles.

## O que eu concluí primeiro, e estava errado

Olhando a captura em português, «11,5 KiB  de  16,0 MiB  que o servidor aceita»
pareceu **escolha de espaçamento**, largo mas legível — anotei como estética. Só
a captura em alemão, em 360 px, mostrou o defeito sem discussão: «11,0 KiB  von
16,0 MiB  **,** die der Server annimmt», com um espaço **antes da vírgula**. A
língua cuja pontuação encosta no dado é a que denuncia; em português o mesmo
defeito passava por estilo.

## O que a medição disse

A guarda nova (`textoSoltoEmFlex` no `testes-web/phxzip/exercitar.mjs`) procura
todo elemento `flex`/`grid` com texto próprio **e** filho elemento: achou
exatamente um, o `#resumoTeto`. Com a frase dentro de um `<span>`, zero — e a
prova das guardas repõe o defeito numa cópia da tela e confere que a guarda o
acusa pelo nome.

## A regra

Frase com dado no meio nunca é filha direta de flex ou grid: vai dentro de um
elemento inline. E a captura que conta é a da língua cuja pontuação encosta no
dado, não a do idioma de quem escreveu.

## Como está guardado hoje

`textoSoltoEmFlex` roda em três estados do exercício (lista, 360 px em alemão e
os sete estados do pseudoidioma), e está no `MUTACOES` do
`prova-das-guardas.mjs`. **Buraco:** a tela do PhxSql usa a mesma `marcado()`
com frase picada e não tem esta guarda; a `bateria.mjs` de lá não a roda.
