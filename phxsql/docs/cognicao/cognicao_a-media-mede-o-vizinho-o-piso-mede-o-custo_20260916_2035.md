# A média mede o vizinho; o piso mede o custo aditivo

**Descoberto em 16/09/2026, 20:35**, montando a bancada de custo da telemetria
(`bancada/telemetria/custo.py`, pedido 265).

## 1. O que aconteceu

A bancada nova mede quanto custa ligar a telemetria: mesma carga, mesmo
servidor, mesmo binário, e o interruptor (`telemetria_ligar` /
`telemetria_desligar`) virado em tempo de execução. A primeira corrida, com
nove pares de 2.000 `ping` cada, devolveu isto:

```
ping   desligada 101,54 µs   ligada 63,49 µs   −37,48%
```

A telemetria **acelerando o servidor em 37%** — o que é impossível: o código
ligado faz tudo o que o desligado faz, mais dois `Instant::now()`, um
`fetch_add` e um `lock`.

A máquina estava em carga 12 (outras frentes compilando ao lado), e as nove
amostras de um **mesmo lado** iam de 54 a 128 µs.

## 2. O que eu concluí primeiro, e estava errado

**«É ruído; basta medir mais.»** Subi de 9 para 400 pares — 160.000 `ping` por
lado, oito vezes mais trabalho. O número não melhorou: a mediana do lado
desligado passou a **161,29 µs** numa corrida e **59,61 µs** noutra, dois
minutos depois, sem uma linha de código mudar. Mais amostra da grandeza errada
não é mais informação: é mais medição do vizinho.

A segunda conclusão errada foi **pôr o veredito e a estimativa em listas
diferentes**. O veredito virou um teste de sinal sobre as *razões* por par e a
estimativa continuou sendo a diferença das *medianas* de cada lado. Uma corrida
saiu dizendo «resolvido, ligada custa» com **231 de 400** pares a favor e, na
célula ao lado, **−0,04%**. Veredito que aponta para um lado com o número
apontando para o outro é pior que veredito nenhum.

## 3. O que a medição disse

O que a telemetria acrescenta a um pedido é uma **constante**: num tempo
`t_i = base_i + c`, o `c` aparece inteiro em **todo** `t_i` — inclusive no
menor. E o menor é o único quase sem vizinho dentro. Trocada a estatística de
cada lado do par da mediana para o **p10** (o piso, com 10% de folga contra o
outlier para baixo), com o custo passando a ser a **mediana das diferenças por
par**:

| corrida | carga da máquina | piso desligado | custo do `ping` | pares a favor |
|---|---:|---:|---:|---:|
| 1ª | 13,8 | 22,413 µs | **+0,198 µs** | 374/400 |
| 2ª | 14,5 | 78,135 µs | **+0,274 µs** | 256/400 |
| 3ª | 6,5 | 65,428 µs | **+0,253 µs** | 269/400 |

O **piso** anda de 22 a 78 µs com a carga da máquina — ele é o vizinho. O
**custo** fica entre 0,198 e 0,274 µs nas três, com a carga variando 2,2×. A
grandeza que eu queria estava lá o tempo todo; a média é que não a mostrava.

E o `checksum` — o «pior caso» que o `docs/TELEMETRIA.md` §7 dava como
**+2,28%**, uma chamada a `siga()` por linha — ficou **dentro do ruído** nas
três corridas (41 a 46 de 80 pares). O `inserir`, que a mesma tabela dava como
+2,19%, também. O que dá para afirmar hoje é o `ping`, e é pouco: **da ordem de
0,25 µs por pedido**.

## 4. A regra

**Efeito aditivo se mede no piso, nunca na média — e o veredito tem de sair da
mesma lista que a estimativa.** A média de um lado mede a máquina; a diferença
emparelhada dos pisos mede o que se acrescentou. E quando o teste de decisão
conta uma lista e o número publicado sai de outra, os dois vão discordar — o
que decide e o que se lê são a mesma conta ou não são nada.

## 5. Como está guardado hoje

- `bancada/telemetria/custo.py` — o `piso()` e o `teste_de_sinal()`, cada um
  com o *porquê* no docstring, inclusive o número da corrida que os motivou.
- `custo.py --autoteste` — prova a porta **nos dois sentidos**, em segundos e
  sem subir servidor: efeito real resolve, ruído simétrico com cauda de 60×
  não resolve, a borda dos 3 σ confere (42 de 60 resolvem, 41 não) e o
  invariante «veredito e estimativa nunca discordam» é asserção.
- `bancada/telemetria/LEIA-ME.md` §*O preço da instrumentação* — as três
  correções, com o número de cada uma.
- `docs/TELEMETRIA.md` §7 — passou a dizer que as tabelas dele são a medição
  **ad-hoc de 09/2026** e que o número corrente sai da bancada.

**Onde o buraco ficou, dito em vez de escondido:** a regra do pedido 155 («o
vencedor só é contornado quando as faixas min–max não se cruzam») **não serve**
para efeito de ~1% em máquina compartilhada — as faixas sempre se cruzam, e ela
diria «não sei» até se a telemetria dobrasse o custo. Aqui entrou um teste de
sinal a 3 σ no lugar dela, e **só nesta bancada**: as barras dos gráficos
continuam sob a regra das faixas, que é a certa para efeito de *vezes*. Se uma
terceira bancada precisar da mesma troca, a decisão de qual régua vale onde
passa a ser do QA, e não de cada medidor.
