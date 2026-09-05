# Índice que superinforma mente para a chave estrangeira — e o pior leitor do índice é um escritor

**Descoberto em 05/09/2026, ~07:05**, lendo o desenho da Sombra (o MVCC desta
casa) para escrever `docs/SOMBRA.md` antes de qualquer código.

## 1. O que aconteceu

A `docs/PESQUISA-MVCC-E-FORMATO.md` §7.4 chama o adiamento da remoção no `.ndx`
de *«a divergência de que mais me orgulho»*. A ideia é boa e a restrição que a
causou é real: a folha do `.ndx` é `chave completa + rowid`, largura fixa, e um
bit de marca viraria um segundo formato a versionar — mas **remover não
rebalanceia** (`docs/FORMATO.md` §2), logo remover é **adiável**. Enquanto
houver visão aberta, a alteração de coluna indexada acrescenta a entrada nova e
não tira a velha.

O argumento que fecha a proposta é este, e é o que eu fui conferir:

> «O índice passa a **superinformar** — devolve rowids cuja chave corrente é
> outra —, e quem filtra é a verificação que o leitor já vai fazer de qualquer
> jeito.»

**No fonte, não existe essa verificação.** `Table::buscar`
(`crates/phxsql-store/src/table.rs`) devolve o que o `.ndx` respondeu **sem
reler a chave corrente da linha**; a única filtragem que ele faz é a das linhas
pendentes da própria transação, e o comentário ali explica que ela existe por
outro motivo (a linha pendente não está no índice).

E o consumidor que decide não é um leitor: `Table::conferir_fks` responde
*«existe este pai?»* chamando `mae.buscar(&indice, &chave)` e usando o resultado
como **prova de existência**. Com uma entrada velha adiada no índice da mãe, a
resposta seria **sim** para um pai cuja chave já mudou — e entraria uma órfã. Do
outro lado, a entrada velha no índice da filha faria o `excluir` da mãe recusar
uma exclusão legítima.

## 2. O que eu concluí primeiro, e estava errado

Que o adiamento era **de graça**, e que a única ressalva era a que o próprio
documento trazia — o índice **único**, onde a entrada velha ainda presente faz a
inserção da nova bater na unicidade.

Duas coisas erradas nessa leitura, e a segunda é a que dói:

* **li «o leitor filtra» como uma propriedade do motor, e era uma propriedade do
  leitor com visão aberta.** O leitor que não pediu MVCC nenhum passaria a
  receber linhas sob uma chave que elas não têm mais — e ele é o caso comum;
* **procurei os leitores do índice, e o consumidor que importa não lê para
  mostrar: lê para julgar.** A conferência de chave estrangeira não tem visão
  aberta com que filtrar, não é cliente de MVCC e não sabe que o índice está
  superinformando. Ela é um caminho de **escrita** consultando um índice de
  leitura.

## 3. O que a medição disse

Não é medição de tempo — é contagem no fonte, que é o que esta pergunta pedia:

| pergunta | resposta, no fonte |
|---|---|
| `Table::buscar` confere a chave corrente da linha? | **não**: devolve o que o `.ndx` deu; só filtra as pendentes da transação |
| `Table::varrer_indice` confere? | **não**: filtra só `Troca::Sumida` da sobreposição |
| quem usa `buscar` como prova de existência do pai? | `Table::conferir_fks`, no caminho de **gravação** da filha |
| quantos caminhos precisariam do filtro novo | `buscar`, `varrer_indice`, `intervalo`, a página ordenada e as **duas** pontas do `conferir_fks` |

E o efeito de cada ponta, que é o que transforma isto de detalhe em pétrea:
entrada velha no índice da **mãe** deixa entrar órfã; no índice da **filha**,
recusa exclusão legítima. A regra primordial da integridade não tem meio termo.

## 4. A regra

**Antes de aceitar que uma estrutura pode superinformar, conte no fonte QUEM a
lê — e conte também quem a lê para julgar, não só para mostrar. O pior leitor de
um índice costuma ser um escritor.**

E o corolário, que é a pétrea de sempre por outro caminho: *guarda nova entra
pedida, não imposta*. Adiamento é uma guarda **imposta** a quem nunca pediu
MVCC — ele muda a resposta do índice para todo mundo, e quem paga é a
conferência que não tem como filtrar.

## 5. Como está guardado hoje

* Em `docs/SOMBRA.md` §2.3, com o nome dos dois arquivos e das duas pontas.
* A decisão do dono de 04/09 (resposta 4: **recusar** a alteração de coluna
  indexada sob visão aberta, em vez de marcar ou adiar) já estava certa, e agora
  tem o motivo mais forte escrito ao lado do que ela tinha —
  `PESQUISA-MVCC-E-FORMATO.md` §8.0 dizia «é a única das três que preserva o
  zero-formato»; passa a ser também a única que não põe resposta errada no
  caminho da conferência de chave.
* **Onde o buraco ficou:** não há guarda que trave isto. Se o adiamento voltar,
  nada no `cargo test` reprova — o defeito só apareceria numa órfã em produção.
  O teste que o pega está escrito na §7 do `SOMBRA.md`, com o defeito reposto
  («adiada a remoção da entrada velha, o `conferir_fks` aceita uma órfã»), e ele
  **não existe** enquanto o adiamento não existir. Fica nomeado, que é diferente
  de esquecido.
