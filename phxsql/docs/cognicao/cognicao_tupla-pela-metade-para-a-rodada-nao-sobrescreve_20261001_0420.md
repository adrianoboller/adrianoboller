# A tupla pela metade PARA a rodada, não sobrescreve — e a troca de chave só se vê do outro lado

**Estado:** PENDENTE

## O que aconteceu

Frente dos pedidos 329 e 331 e da troca de chave no bidirecional (01/10/2026),
feita duas vezes: a cópia de trabalho foi apagada antes da integração e o
trabalho foi refeito em cima do HEAD que já tinha 416/517/564 e 344. Medições
pelo soquete, em `crates/phxsql-server/tests/identidade-do-bidirecional.rs`:

- **Troca de chave.** Trocar o `id` 1 por 10 em A deixava B com `[1, 2, 10]` em
  vez de `[2, 10]`: a imagem do evento é o «depois», o aplicador procurava pela
  chave nova, não achava, inseria — e a antiga ficava.
- **Composta casando pela primeira coluna.** A decisão do pesquisador previa
  que «(1,2) sobrescreve (1,1)». Medido, não: a busca com a tupla pela metade
  não casa o índice composto, o aplicador devolve `Err`, e a **rodada para** —
  as linhas de A nunca chegam a B.
- **Exclusão pela porta no multi.** Na primeira passada (antes do 416 na
  árvore) ela parava a rodada com «evento de exclusao sem imagem». Na segunda,
  em cima do 416, a perna de exclusão da composta passou pelo soquete.

## O que eu concluí primeiro, e estava errado

1. Que o 517 estava na árvore porque a ordem dizia «já feito». Na primeira
   passada não estava; na segunda estava, e a exigência de obrigatória passou
   a valer para cada coluna da composta pelo mesmo predicado.
2. Que o defeito reposto da composta seria a sobrescrita prevista. Era parada
   de rodada.
3. Que o registro de números podia ir à memória e depois ao disco. O parecer
   do DBA mostrou o furo: com o disco recusando, a chamada seguinte achava o
   par «já conhecido» e o aceitava sem nunca ter gravado. Provado pela guarda
   `numero-aceito-antes-do-disco`.
4. Que «etiqueta desconhecida no rabo lê como sem rabo» era tolerância boa.
   Era o defeito de formato que o DBA nomeou: campo novo ignorado calado.

5. Que o teste do «disco antes de aceitar» provava a ordem. O provador o
   pegou **passando com o defeito reposto**: a pasta no lugar do arquivo
   existia já no arranque, o registro era lido como ilegível, e toda chamada
   recusava pelo motivo errado. Corrigido criando a pasta **depois** do
   arranque; aí a guarda caiu como devia.

## O que a medição disse

- Troca de chave, defeito reposto (`muda_chave_unica` → `false`):
  `left: [1, 2, 10] right: [2, 10]`.
- Composta pela metade: `Err` no aplicador; pelo soquete, o `esperar` de 20 s
  estoura.
- Colisão de número (329): com a conferência só do par, o caixa Y nunca recebe
  a linha do caixa X e **nada grita** em 20 s.

## A regra

Ao repor um defeito para provar uma guarda, escreva o sintoma **medido**, não
o previsto. E estado que vai a disco e decide aceite entra na memória **depois**
do `Ok` da gravação, nunca antes.

## Como está guardado hoje

Guardas `troca-de-chave-vira-linha-nova`, `fio-cifrado-perde-o-antes`,
`composta-casa-pela-primeira-coluna`, `numero-de-origem-conferido-so-no-par`,
`numero-de-origem-atribuido-ignorado`, `imagem-com-sobra-ignorada`,
`registro-de-numeros-ilegivel-vira-vazio` e `numero-aceito-antes-do-disco` em
`bancada/guardas/catalogo.py`. **O buraco:** o anúncio da capacidade do rabo
pelo `posicao` (parecer do DBA §2, item 2) não entrou.
