# `vizinhos` não basta para "quieta" — e o motivo plausível estava errado

- **Quando:** 2026-09-04, 15:30
- **Onde:** `bancada/concorrencia/quieta.py` (`Vigia`, `Amostra`),
  `bancada/concorrencia/ruido-do-controle.py` (novo)
- **Custo:** zero — a hipótese morreu numa medição de trinta segundos, antes
  de eu escrever qualquer explicação em documento permanente

## O que aconteceu

Medindo a premissa da S-B (quanto o `ping` varia hoje, em muitas corridas),
separei as corridas por `vizinhos ≤ 1` (o critério que o `quieta.Vigia` usa
para "sem outra frente do lado") esperando que esse subconjunto mostrasse
dispersão baixa — a base limpa para propor um teto mais apertado.

Não mostrou: nas 10 corridas com `vizinhos ≤ 1`, a ocupação da máquina ainda
variou de 50% a 83%, e o `ping` variou até 49,5% entre corridas — mais que o
triplo do teto de 15% já em vigor. `vizinhos ≤ 1` não estava filtrando as
corridas realmente quietas.

## O que eu concluí primeiro, e estava errado

Escrevi, no primeiro rascunho do `LEIA-ME.md`, que a causa era `steal` — o
hospedeiro tirando a vCPU de baixo do processo sem deixar rastro na fila de
`procs_running` deste contêiner. A explicação é elegante, explica o sintoma
exato (ocupação alta, vizinhos baixo) e eu quase a publiquei assim, como
fato, num documento que outras frentes leem.

Antes de publicar, medi. `/proc/stat` tem o campo `steal` separado dos outros
sete; bastava somar o delta dele numa janela de 3 s. Resultado: **0,0%**. A
explicação bonita não tinha base nenhuma — e é o mesmo erro, com outra
roupa, do «o mutex era o pior pedaço, porque serializa» que este projeto já
errou por 262.000×. *Diagnóstico plausível não é diagnóstico medido.*

## O que a medição disse

`bancada/concorrencia/ruido-do-controle.py`, 30 corridas de `ping`, 1 s cada,
04/09:

| grupo | n | CV | salto |
|---|---:|---:|---:|
| todas | 30 | 27,1% | 99,4% |
| `vizinhos ≤ 1` | 10 | 15,6% | 49,5% |
| `vizinhos > 1` | 20 | 27,5% | 92,9% |

E, testado à parte: `steal` no `/proc/stat`, no momento da escrita, **0,0%**.

A explicação que sobra, e que os números sustentam sem precisar de mais
nenhuma medição nova: o `quieta.Amostra` lê `procs_running` em só 3–5
instantes por janela (`passos`); um pico de tarefa rodável entre dois desses
instantes passa batido. A ocupação (`idle` acumulado pelo kernel ao longo da
janela inteira) não tem esse buraco — é por isso que ela viu 50–83% de
ocupação nas mesmas corridas em que `vizinhos` viu zero.

## A regra

**Um sinal por amostragem pontual (`vizinhos`) e um sinal por acumulação
contínua (`ocupada`) não são intercambiáveis, mesmo medindo "a mesma coisa"
— um perde bico curto, o outro não.** Isso não é pétrea nova: é o **alcance**
de uma que já existe — "o vigia nunca decide por um sinal só" — agora com o
caso concreto que prova por que a regra precisa dos três sinais e não só de
um. E a regra irmã, sobre método: **uma explicação que casa com o sintoma não
é prova até alguém medir a explicação em si**, não só o sintoma que ela
explica.

## Como está guardado hoje

`bancada/concorrencia/LEIA-ME.md` (item 4 da lista de aprendizados) e
`docs/CONCORRENCIA.md` §10.4 registram o achado **e** a hipótese morta lado a
lado — nenhum dos dois documentos afirma `steal` como causa. O que fica
**não medido**: qual mecanismo exato explica o buraco de amostragem
(agendamento do próprio kernel, granularidade do `time.sleep`, outra coisa) —
só que `vizinhos` sozinho tem o buraco, isso sim medido.
