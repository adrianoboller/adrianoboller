# Escrevi a lei às 03:10 e a quebrei às 03:25: o sintoma tem dono

**18/09/2026, 03:30** — hora da descoberta. Quinze minutos depois de comitar a
lei, e cinco depois de comitar a violação dela.

## 1. O que aconteceu

Às **03:10** escrevi a cognição *«o parecer acerta o achado e erra o rótulo»*,
cuja regra é:

> Confira também a frase que só emoldura o achado — e comece por ela quando ela
> fala de um documento. **O que não compila é o que passa.**

Às **03:05** — antes de escrever a lei, e é isso que agrava — o
`comunicacao.sh` imprimiu:

```
⚠️  catraca REPROVADA em mapa-da-trava.py:
    SUBIU                    alcancam-fsync    25 (teto 22)
```

Medi de quem era o salto, e medi bem: extraí `crates/*/src` de catorze
revisões com `git archive`, rodei **sempre o contador de hoje** nos dois lados,
e achei `20d2c59` (17/09 06:06). Registrei o pedido **362** com duas frentes:
(a) as catracas de `bancada/` entram num portão obrigatório, e (b) achar as
duas seções que passaram a alcançar `fsync` e tirá-las da trava.

Às **03:18** comitei isso, e a mensagem do commit dizia «sete commits e ~20
horas com a catraca vermelha, e cada rodada foi fechada anunciando portões
verdes». Relatei ao dono nos mesmos termos.

Às **03:25**, indo fazer a frente (b), abri o `mapa-da-trava.py` para achar as
duas seções — e o comentário dele, na linha 671, dizia: *«uma das três está
VERMELHA por decisão do dono (#252, metade 1)»*.

O pedido **252** estava no `PENDENCIAS.md`, **quarenta linhas acima** do 362 que
eu acabara de escrever, no arquivo que eu passei a noite editando.

## 2. O que eu concluí primeiro, e estava errado

Concluí que a catraca vermelha era um **achado**. Era um **sintoma**, e o
sintoma tinha dono.

E as duas frentes que eu propus estão as duas erradas, cada uma por um motivo
diferente — o 252 tinha medido as duas antes de mim:

**(b) não existe.** As duas seções são `empilhar` e
`empilhar_atualizar_com_cascata`, e **nenhuma das duas passou a alcançar
`fsync`**. O commit trocou `abrir_travada` + `ver_so_o_disco` pela porta
`abrir_travada_sem_sobrepor`, e as duas portas são envolucros de uma linha
sobre a **mesma** função, diferindo num booleano (`servidor.rs:10524-10561`; o
comentário entre elas diz «UMA implementacao»). O caminho de durabilidade é
idêntico. O que mudou foi o **corte da porta comum**: `abrir_travada` tem 34
chamadas e conta como HERDADO, fora de `proprias`, que é o conjunto que a
catraca soma; a porta nova tem 2 seções, fica abaixo do corte, e a mesma
durabilidade volta a contar como própria. **A catraca subiu porque um envolucro
compartilhado virou envolucro privado de duas seções** — e o commit que a
levantou deixou o motor **mais barato**, porque a porta nova existe para não
montar a sobreposição da transação sob a trava e apagá-la na linha seguinte
(`O(pendentes)` por operação, `O(n²)` por transação). Não há nada para desfazer.

**(a) já estava decidida, e melhor.** A metade (2) do 252 fechou em 16/09: a
`mapa-das-threads.py --catraca` está verde e virou teste da suíte; a
`mapa-da-trava.py --catraca` está vermelha, e **catraca vermelha parada com o
dono não pode virar portão** — deixaria a suíte de todas as frentes vermelha, e
a saída mais barata dessa pressão seria subir o teto de 22 para 23, que a
pétrea proíbe. Então ela entrou no `numeros-do-projeto.py` como relato com data.
Esse raciocínio é melhor que o meu, e eu teria apagado ele.

## 3. O que a medição disse

- Do meu registro, **a medição estava certa e era redundante**: mesmo commit
  (`20d2c59`), por método mais pobre — 14 pontos meus contra os 13 commits do
  252 com o denominador em 86 nos catorze pontos.
- **Das duas conclusões, zero sobreviveram.**
- O tempo entre escrever a lei e quebrá-la: **15 minutos**. O tempo entre
  quebrá-la e achá-lo: **5 minutos**, e só porque fui *fazer* a frente errada.
  Se a frente (b) fosse plausível o bastante para eu delegá-la sem abrir o
  arquivo, um agente teria passado uma hora procurando duas seções que não
  existem.
- A distância entre o meu erro e a resposta: **40 linhas** no mesmo arquivo.

## 4. A regra

> **Antes de registrar um achado a partir de um SINTOMA, procure o dono do
> sintoma.** Aviso, catraca vermelha, teste que falha e número fora do teto são
> sintomas — e neste repositório quase todo sintoma já tem um pedido que o
> explica. Procurar custa um `grep`; não procurar custa um pedido errado no
> documento e uma frente inventada.

E o corolário, que é o que a lei de 03:10 não alcançava: **eu a escrevi para a
moldura do parecer de OUTRO.** O buraco era a minha própria inferência — e
inferência própria não tem moldura, não tem autor a desconfiar, e por isso não
dispara nenhuma desconfiança. **A frase mais perigosa não é a que alguém me
manda: é a que eu mesmo deduzo de um número verdadeiro.**

## 5. Como está guardado hoje

- O pedido **362** foi **reescrito e recolhido** com o motivo medido, e marcado
  ☑️ — não apagado. Ele agora conta o erro, aponta o 252 como dono do sintoma, e
  registra as duas conclusões mortas. A contagem foi ao gerador: **362 pedidos,
  273 feitos**.
- O commit `bc86187` **continua dizendo a versão errada**, e eu não reescrevi a
  história: a linha está no `PENDENCIAS.md` e nesta cognição, que é onde esta
  casa guarda erro — histórico reescrito esconde, documento corrige. Quem ler o
  `bc86187` e estranhar cai aqui pelo número do pedido.

**Onde o buraco ficou:** não há guarda que impeça um pedido novo de duplicar um
existente. A busca é por número e por título, e o 252 não tem «fsync» no título
— tem «A catraca `alcancam-fsync`…», que eu **acharia** com um `grep fsync` de
três segundos. Não proponho conferidor: um casador de duplicatas em 362 pedidos
de prosa reprovaria os parecidos-e-distintos junto com os iguais, e é a mesma
recusa medida do conferidor de erro cru. O que segura esta classe é a regra da
§4, e o preço de esquecê-la está medido aqui: uma volta inteira de trabalho —
registro, commit, push, páginas e relatório ao dono — gasta num achado que já
tinha dono.
