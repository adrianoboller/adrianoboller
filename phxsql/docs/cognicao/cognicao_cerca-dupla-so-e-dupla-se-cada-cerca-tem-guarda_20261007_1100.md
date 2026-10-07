# Cerca dupla só é dupla se cada cerca tem a sua guarda

**Estado:** PENDENTE

## O que aconteceu

Pedido 655, 07/10/2026. O `limpar-frentes.sh` tem duas cercas contra apagar a
cópia suja de uma frente: a prova do `git status --porcelain` no próprio
script, e a recusa do `git worktree remove` sem `-f`. A guarda nova
(`bancada/guardas/frente-so-numa-pasta.py`) monta um repositório git de
verdade numa pasta temporária e roda o script real.

## O que eu concluí primeiro, e estava errado

Que a prova de comportamento bastava: repor o defeito (tirar a linha do
`porcelain`), rodar o script e ver a suja sumir. Ela **não sumiu**: o git
ainda recusou. A prova de comportamento passava com uma das duas cercas
derrubada — exatamente o «teste que passa por engano» —, e só reprovava com
as duas derrubadas ao mesmo tempo, que é o defeito de 01/10.

## O que a medição disse

Três corridas do script real na pasta temporária:

- sem a prova do `porcelain`: a suja **fica** (o git segura);
- só com o `-f`: a suja **fica** (o `porcelain` segura);
- sem a prova **e** com `-f`: a suja **some**.

Duas das três formas de afrouxar eram invisíveis ao comportamento. A guarda
passou a conferir cada cerca também pelo texto: `TETO_REMOVE_FORCADO` (o `-f`)
e as cinco provas que têm de aparecer antes do `worktree remove`.

E o mesmo desenho apareceu no 656: o teste da catraca e a tabela do
`QA-PDCA.md` eram duas cercas contra a catraca frouxa, e a tabela dizia «em
cima, sem folga» enquanto o teste tolerava 30 — a tabela compara a constante
com o medido, e não com o que o teste aceita.

## A regra

Quando houver duas defesas para o mesmo defeito, guarde cada uma pela
pergunta que só ela responde; o comportamento conjunto esconde a falta de
qualquer uma enquanto a outra estiver de pé.

## Como está guardado hoje

`bancada/guardas/frente-so-numa-pasta.py --autoteste` traz o caso «só a
prova do `porcelain` removida: o git ainda segura a suja… e por isso a régua
de texto a acusa sozinha». A folga do teste das catracas tem a
`bancada/guardas/folga-de-catraca.py`. O buraco que fica: defesas em dobro
fora destes dois lugares não têm crivo genérico, e eu não sei escrever um que
não seja padrão de texto.
