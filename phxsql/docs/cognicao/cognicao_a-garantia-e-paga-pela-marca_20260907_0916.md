# A garantia é paga pela marca, e o preço aparece quando ela some

**07/09/2026, 09:16 UTC.**

## 1. O que aconteceu

O dono perguntou *«o transaction atomic está funcionando?»*. A resposta certa
não era citar o `docs/ACID.md`: era rodar o `bancada/transacoes/provar.py`, que
prova pelo **soquete** — e ele deu **36 conferências, 0 falhas**, incluindo a
que mais importa: `SIGKILL` no meio de um `COMMIT` de 3.000 linhas, banco
reaberto, e o desfecho sendo **1 ou 3001, nunca no meio**.

Duas faltas apareceram junto, e são o motivo de a pergunta ter sido feita:

1. o medidor **não gravava `resultados.json`**;
2. a bancada **não estava declarada** na tabela `BANCADAS` da
   `docs/dossie/pagina-dos-testes.py`.

Ou seja: a página que existe **justamente para dizer o que este banco prova**
não listava a prova das transações. Ela não estava «NÃO MEDIDA» — ela não
estava.

## 2. O que eu concluí primeiro, e estava errado

Que o «nunca metade» era propriedade do **caminho de escrita** — que o motor
simplesmente não deixava meia gravação existir no disco. Montei a injeção de
defeito `apaga-a-marca` esperando um resultado morno: sem a marca, o banco
voltaria com **nenhuma** das 3.000, um `ABORTED` legítimo, e só a conferência
«o relatório diz uma das duas» cairia.

Voltou com **43 de 3.000**. Pela metade.

E isso **não é defeito do motor — é a demonstração do contrário**, que é a
parte que eu não tinha entendido. O `SIGKILL` cai no meio da passada de commit,
então naquele instante há mesmo meia gravação no `.reg`: é o estado normal e
inevitável de um processo morto no meio do trabalho. A marca
`transacao_<id>.tx` é a **única** coisa que o resolve, reaplicando o que
faltava ao reabrir.

O «nunca metade» não é acidente do caminho de escrita. É **comprado** pela
marca — e o preço dela fica invisível enquanto ela está lá.

## 3. O que a medição disse

| corrida | linhas depois da queda | veredito |
|---|---:|---|
| limpa | **3001** de 3001 | 36 de 36 conferências, 0 falhas |
| com `apaga-a-marca` | **43** de 3001 | 4 falhas, entre elas «ficou pela METADE» |

O 43 muda a cada corrida: matar o processo no instante certo é uma corrida, e o
quanto a passada tinha escrito depende de onde o sinal caiu.

O portão fecha nos dois sentidos, e o controle positivo entra junto porque
recusa sem controle que passa já custou dois vereditos errados nesta casa:
`prova-dos-portoes.py` exige que o defeito **derrube** a conferência nomeada, e
que a corrida **sem** defeito passe limpa.

## 4. A regra

**Prova de garantia se escreve tirando o mecanismo que a garante, e não só
observando que ela vale.** Enquanto a marca está lá, o disco parece nunca ter
tido meia transação — e essa aparência é exatamente o que a marca produz.

E o corolário de bancada: **medidor que não grava resultado não aparece nem
como NÃO MEDIDA.** Ele some, e a pergunta que ele responde volta pela boca do
dono.

## 5. Como está guardado hoje

- `bancada/transacoes/provar.py` grava `resultados.json` — e **não grava** com
  defeito reposto, porque publicar o retrato de uma corrida sabotada é pior que
  não publicar nada.
- `bancada/transacoes/prova-dos-portoes.py` roda o medidor uma vez por defeito,
  exige a conferência certa entre as falhas, e fecha com o controle positivo.
- A bancada está **declarada** na `pagina-dos-testes.py` e aparece na página,
  com os quatro campos e a data da medição.
- **Onde o buraco fica:** há **um** defeito reposto, não uma família. Faltam
  pelo menos dois que eu sei nomear e não escrevi: marca **corrompida** (o CRC
  tem de recusá-la e o banco tem de dizer «descartada»), e `SIGKILL` **durante
  a própria recuperação** — que testa se a reaplicação é idempotente contra
  duas quedas, e não só contra uma.
