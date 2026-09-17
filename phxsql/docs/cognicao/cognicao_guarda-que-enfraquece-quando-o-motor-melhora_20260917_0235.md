# A guarda que enfraquece sozinha quando o motor melhora

*17/09/2026, 02:35 — a hora da descoberta, quando o `trava.py` voltou verde com
`4 corte(s)` onde a corrida anterior tinha feito 10.*

## 1. O que aconteceu

O estágio `queda` do `bancada/replicacao/trava.py` derruba a conexão entre a
réplica e o source **repetidas vezes no meio do alcance** e julga pela soma de
verificação dos dois lados. Ele passou hoje, com a mesma soma de sempre
(`1aa1e8124df2cba0`, 200.000/200.000 dos dois lados). E passou fazendo **4**
cortes, contra **10** na corrida de 05/09.

Ninguém mexeu no estágio. O que mudou foi o que ele mede: a réplica passou a
alcançar 200.000 eventos em **2,39 s** onde levava **4,54 s**. O laço corta a
cada 0,3 s *enquanto a réplica não alcançou*:

```python
while time.perf_counter() - t0 < 12 and (eventos_de(replica) or 0) < LINHAS:
    time.sleep(0.3)
    cortes += 1 if tubo.cortar() else 0
```

O número de cortes não é alvo: é **subproduto da lentidão do que se mede**.
Quanto melhor o motor fica, menos a guarda exercita — e o veredito só exige
`cortes > 0`, então ela continua verde enquanto enfraquece.

## 2. O que eu concluí primeiro, e estava errado

Concluí que o 4 era ruído da máquina — «uma corrida com menos carga, o corte
calhou de acontecer menos». Era plausível: a carga estava em 0,34 e nada mais
tinha mudado.

Estava errado, e o que derruba é aritmética, não estatística: 4,54 s ÷ 0,3 s ≈ 15
janelas de corte; 2,39 s ÷ 0,3 s ≈ 8. O número de cortes **acompanha o alcance**,
porque o alcance é o que define por quanto tempo o laço tem o direito de cortar.
Não é ruído: é uma função decrescente da velocidade do motor.

E concluí junto uma segunda coisa errada, mais confortável: «como passou, está
tudo bem». O `LEIA-ME.md` da bancada descreve o estágio como *«dez cortes de
conexão de verdade no meio do alcance»* — um número que ninguém digitou de novo
desde que virou 4, e que a página lê como descrição de desenho.

## 3. O que a medição disse

| corrida | alcance do estágio | eventos/s da réplica | cortes de conexão |
|---|---:|---:|---:|
| 05/09/2026 | 4,54 s | 44.062 | **10** |
| 17/09/2026 | 2,39 s | 83.567 | **4** |

O alcance ficou **1,90×** mais rápido e os cortes caíram **2,5×**. A soma de
verificação saiu idêntica nas duas corridas, o que é a boa notícia — a garantia
que o estágio existe para provar continua de pé.

A extrapolação, que é o que importa: abaixo de ~0,3 s de alcance, `cortes`
chega a **0** e `ok = … and cortes > 0` fica **falso**. Nesse dia a bancada
reprova **sem haver defeito nenhum** — e quem olhar vai procurar o defeito na
replicação, que é o lugar onde ele não está.

## 4. A regra

**Guarda cuja força é subproduto da lentidão do que ela mede enfraquece a cada
conserto — conte o exercício, não o relógio.** Se o estágio precisa de N cortes,
peça N cortes e alongue a carga até consegui-los; nunca deixe o número de
exercícios sair de uma divisão em que o denominador é o desempenho do motor.

E o corolário, que é irmão do da catraca: **número de exercício descrito em
documento é número que envelhece.** O `LEIA-ME` diz «dez» porque dez foi o que
saiu um dia — a descrição do desenho e a medida da corrida não podem ser o mesmo
texto.

## 5. Como está guardado hoje

**Não está.** O estágio continua como estava, de propósito: mudar o gatilho do
corte muda o desenho da medição, e isso é decisão do orquestrador, não do papel
que mede. As duas saídas têm custo diferente e nenhuma é neutra:

- cortar por **progresso aplicado** (a cada X eventos) em vez de por relógio —
  garante o número de cortes, mas amarra a bancada à telemetria da réplica;
- exigir um **mínimo de cortes** e alongar a carga até consegui-lo — mantém o
  gatilho simples, mas alonga o estágio junto com a melhora do motor, que é o
  contrário do que se quer.

Onde o buraco ficou: `bancada/replicacao/trava.py :: estagio_queda`, e a frase
«dez cortes» no `bancada/replicacao/LEIA-ME.md`. Registrado com número em
`docs/propostas/bateria-replicacao-2026-09-17.md` §3.3.

Este é o **terceiro** caso desta família nesta casa, e é o alcance que muda de
cada vez: o primeiro foi a guarda que só passava com o defeito de pé
(`a3-congelamento`, `cognicao_limitacao_que_envelheceu_20260905_0430.md`); o
segundo, a catraca cuja régua passou a medir mais (`TETO_TABELA_NA_MAO`); este é
a guarda cujo **tamanho da amostra** depende do defeito que ela não tem.
