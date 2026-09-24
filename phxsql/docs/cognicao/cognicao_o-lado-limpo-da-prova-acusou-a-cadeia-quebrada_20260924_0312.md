# O lado limpo da prova acusou nove cadeias quebradas

- **Quando:** 2026-09-24, 03:12
- **Onde:** `testes-web/phxzip/prova-das-guardas.mjs` e `exercitar.mjs --so`

## O que aconteceu

A prova das guardas repõe cada defeito numa cópia da tela e roda só a cadeia
de casos que a guarda precisa (`--so a,b,c`). Na primeira corrida, **9 das 12**
cadeias falharam também na cópia **limpa**, todas por `Timeout`: o caso que
abria a página (`page.goto`) não estava em nenhuma cadeia, e os casos rodavam
sobre `about:blank`.

## O que eu concluí primeiro, e estava errado

Que o lado «passa sem o defeito» era redundância — a regra da casa cobra o
«falha com o defeito», e eu o rodava por disciplina, não por esperar achar
algo. Sem ele, as nove teriam aparecido como guardas que **falham com o
defeito**: falhavam, só que por tempo esgotado.

## O que a medição disse

Primeira corrida: 3/12 provadas, 18 min (cada cadeia quebrada esperando 30 s
por passo). Com a página aberta na preparação, fora de qualquer caso, a mesma
cadeia de seis casos passou isolada. O conferir da **frase** esperada também
teria reprovado as nove (Timeout não casa com «PEDIDO_MALFORMADO») — são duas
travas independentes contra o mesmo engano.

## A regra

Numa prova por mutação, rode a mesma cadeia na cópia limpa e exija que passe; e
exija que a falha com o defeito traga a frase do defeito, não uma falha
qualquer. Caso que só roda depois de outro não pode depender de estado criado
fora da cadeia.

## Como está guardado hoje

O `prova-das-guardas.mjs` faz os dois lados e confere a frase (`espera`). O
`exercitar.mjs` abre a página na preparação. **Buraco:** nada impede um caso
novo de depender de um caso anterior fora da cadeia; quem acrescentar mutação
tem de rodar a prova, e é ela que acusa.
