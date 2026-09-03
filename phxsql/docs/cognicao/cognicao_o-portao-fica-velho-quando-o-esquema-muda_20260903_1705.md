# O portão fica velho quando o esquema muda — e a chave que nasce conferida não confere

*Descoberto às 17:05 de 03/09/2026, provando o cenário «declarar chave para
tabela que ainda não existe» do parecer do pedido 175.*

## 1. O que aconteceu

`Table` monta o portão `fks_conferidas` na abertura (`table.rs:454`): as
**posições** das chaves que pedem conferência, para que uma tabela sem chave
não pague nada no laço quente. Duas operações que mexem no esquema o refazem
depois — `acrescentar_coluna` (`:685`) e `remarcar_dado_pessoal` (`:4083`).

`redeclarar_chaves_estrangeiras` (`:562`) **não refaz**. É a única das três
cujo trabalho *é* mexer nas chaves estrangeiras.

Dois estragos, provados nos dois sentidos:

* **(A)** declarar uma chave conferida num handle aberto e gravar em seguida:
  o disco diz `verificar: true`, o motor **aceita a órfã**. Depois de reabrir,
  o mesmo `inserir` é recusado.
* **(B)** tirar uma chave de um handle que abriu com duas: o `inserir` seguinte
  entra em **pânico** — `table.rs:808`, *index out of bounds: the len is 1 but
  the index is 1*. A lista encolheu; a posição do portão não.

Se a lista tivesse sido **reordenada** em vez de encolhida, não haveria pânico:
haveria a conferência da chave **errada**, calada.

## 2. O que eu concluí primeiro, e estava errado

Eu tinha acabado de ler o comentário do `conferir_fks` — *«a mãe pode nem
existir, e desde que a chave nasce conferida isso deixou de ser teórico»* — e
escrevi no rascunho do parecer que o cenário 3 «declara e depois a gravação
recusa nomeando a tabela que falta», **citando o comentário como se fosse a
medição**. Rodei o caso só para ter a frase de erro exata para o documento.

O `inserir` **aceitou**. O comentário estava certo sobre o que a função faz — e
a função nunca foi chamada, porque o portão que decide se ela roda estava
vazio desde a abertura.

O erro tem nome nesta casa: eu tratei um comentário verdadeiro como prova de
comportamento. Um comentário descreve o corpo de uma função; ele não sabe dizer
se alguém a chama.

## 3. O que a medição disse

| | resultado |
|---|---|
| esquema em disco, depois de declarar | `verificar = true` |
| `inserir` com mãe inexistente, mesmo handle | **ACEITOU** |
| o mesmo `inserir`, depois de reabrir | recusou — «não existe clientes(id) com esse valor» |
| `inserir` depois de remover uma de duas chaves | **pânico**, `table.rs:808` |

E o alcance, que é a parte que muda o que fazer com isto: **hoje o defeito não
chega ao protocolo**. `abrir_travada` (`servidor.rs:6278`) abre uma `Table`
nova a cada pedido, e nem `op_declarar_fk` nem `op_excluir_fk` gravam linha no
mesmo handle depois de mexer no esquema. É **latente** — e o
`--example sonda-fk-buracos` já faz exatamente a sequência que o acorda.

## 4. A regra

**Quando uma operação mexer no esquema, procure todo estado que foi derivado do
esquema na abertura — e refaça todos, não o que a operação parece tocar.**

É a irmã de «quando o portão passar a olhar um campo novo, procure quem não tem
esse campo»: lá o furo era um campo que faltava no pedido; aqui é uma cópia que
envelheceu no handle. Nos dois casos o portão continua funcionando
perfeitamente — sobre a informação errada.

## 5. Como está guardado hoje

**Não está.** O buraco ficou aberto de propósito: esta frente era o **parecer**
do pedido 175, e o pedido não autoriza mexer no motor. Onde ele está registrado:

* `docs/PARECER-175-INDICE-NA-DECLARACAO.md` §6, com as duas provas reais, o
  alcance medido e a linha do conserto;
* e a precedência escrita ali: **isto vem antes do pedido 175**, porque a saída
  recomendada (criar o índice na declaração) acrescenta um **segundo** estado
  velho ao mesmo handle — o `self.ndx` —, e aí o portão desatualizado deixa de
  ser uma conferência que não roda e passa a ser uma árvore que não existe para
  quem a acabou de criar.

O conserto é uma linha, do mesmo formato das duas irmãs, e a prova real que ele
pede já está escrita: **(A) tem de recusar a órfã sem reabrir, e (B) tem de
deixar de entrar em pânico.** As duas falham hoje, e é isso que faz delas prova.
