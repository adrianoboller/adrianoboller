# A limitação registrada envelheceu — e o que envelheceu junto foi a guarda

*05/09/2026, 04:30 — a hora da descoberta, quando a primeira corrida da bancada
de contêiner voltou com `estagios com falha: ['a3-congelamento']`.*

## 1. O que aconteceu

O pedido 147 fechou o defeito «a trava de dados presa atrás de uma leitura de
rede» e registrou, com todas as letras, o que ficou por fazer: *«a bancada de
contêiner (`bancada/replicacao/docker/provar.py`) **não foi refeita** — o
daemon do Docker desta máquina estava fora do ar e não pôde ser levantado —,
então os `resultados.json` de lá continuam sendo o retrato do defeito»*.

O daemon voltou. A bancada rodou inteira em 8,1 min e **um estágio reprovou**:
justamente o `a3-congelamento`, o que tinha achado o defeito. E ele reprovou
porque o conserto **funcionou**.

A afirmação dele, em `bancada/replicacao/docker/provar.py`, era:

```python
ok = pior_varrer > 5_000 and pior_ping < 1_000
```

Ele nasceu para **achar** a trava presa, e só passava com ela presa. Com a
trava solta, o `varrer` da réplica respondeu em 7 ms e o veredito saiu `falha`.
A bancada irmã de loopback (`bancada/replicacao/trava.py`), escrita **depois**
do conserto, já afirmava o contrário — `pior_varrer < 2_000` — e nunca soube
que a gêmea dela apontava para o lado oposto.

## 2. O que eu concluí primeiro, e estava errado

Concluí que o achado da corrida seria a **confirmação dos dois números** — os
29.456 ms e os 33,3 s do contêiner virando 6 ms e 3,6 s. Era o que o pedido 147
pedia, e o que eu fui buscar.

Estava errado por ser pequeno demais. Confirmar o número já estava quase
garantido: o loopback tinha reproduzido o mesmo mecanismo com números do mesmo
tamanho (30.079 ms, 33,0 s), e essa semelhança é exatamente por que o pedido
147 disse confiar no resultado. O achado que **só a corrida** podia dar era
outro: **uma guarda apontando para trás, que ninguém podia ver porque a
limitação a impedia de rodar.**

E também concluí, ao ler «limitação registrada também envelhece», que o que
envelhecia era a **limitação** — «o Docker está fora do ar». Envelheceu, sim.
Mas o estrago maior não foi a frase ter caducado: foi a guarda que ela manteve
parada ter caducado **junto, e em silêncio**. Uma limitação que bloqueia um
papel bloqueia também tudo o que aquele papel conferiria. O 403 do `push`
custou três rodadas de backup à mão; este custou uma guarda que passou seis
dias afirmando o oposto do que o código faz.

## 3. O que a medição disse

Os dois lados do conserto, medidos na **mesma** bancada, com seis dias entre
eles:

| no contêiner | 30/08 (defeito) | 05/09 (conserto) |
|---|---:|---:|
| `a3` pior `varrer` na réplica | **29.456 ms** | **6 ms** |
| `a3` pior `ping` (não precisa da trava) | 6 ms | 9 ms |
| `b-abraco` escrita nos dois lados | **33,3 s** | **3,6 s** |
| `b-abraco` `EAGAIN` novos no diário | 1 e 1 | **0 e 0** |
| `b-particao` pior resposta da réplica | 29.456 ms (`varrer`) | 1.317 ms (`checksum`) |
| `b-cortes` retomada `DROP-20s` | **293,8 s** | **0,2 s** |

Os 293,8 s do `DROP-20s` não estavam em documento nenhum: a réplica que perdia
a rede com regra de descarte silencioso e prazo de 20 s levava quase cinco
minutos para voltar depois de a rede religar. As três regras de `REJECT` não
mudaram, e está certo — recusa dá erro na hora, e a réplica sempre soube tratar
erro na hora.

E o custo de descobrir isto, medido antes de começar porque o disco estava em
2,2 GB: binário `musl` **7 MiB**, imagem **34 MiB**, e a corrida inteira com
dez contêineres nunca desceu de **2.173 MiB livres**. O medo era maior que a
conta.

## 4. A regra

**Guarda que afirma o DEFEITO tem de passar a afirmar a GARANTIA no dia em que
o defeito for consertado — e o teto vem da bancada irmã, não de uma segunda
cópia do número.**

E o corolário sobre limitações, que é o alcance da lei que já existe:
**limitação que bloqueia um papel congela também as guardas daquele papel — ao
remedi-la, releia o que ela impediu de rodar antes de confiar no veredito.** A
primeira corrida depois de uma limitação levantada não é uma corrida de
rotina: é a primeira corrida do que envelheceu no escuro.

## 5. Como está guardado hoje

- O veredito do `a3` afirma a garantia, com o motivo e a data escritos acima da
  linha, em `bancada/replicacao/docker/provar.py`.
- Os tetos moram num lugar só: `TETO_VARRER_MS` e `TETO_PING_MS` nascem em
  `bancada/replicacao/trava.py` e o `docker/provar.py` os **importa**. Dois
  números iguais em dois arquivos são um número que envelhece de um lado só —
  que é o mesmo defeito, uma camada acima.
- `docs/REPLICACAO.md` §18 «A conta fechada no contêiner» e §17 aprendizado 7;
  `docs/DESEMPENHO.md` §4.13; `docs/PENDENCIAS.md` pedidos 147 e 193.

**A varredura foi feita, e o resultado importa mais que o achado.** Os **27**
vereditos das três bancadas (`replicacao/`, `cluster/`, `dblink/`) foram lidos
um a um atrás do mesmo naipe. Sobrou **um** com a forma «o defeito precisa
estar presente», e ele **não** é um caso do mesmo mal:

```python
ok = (roubado >= 200 and roubado3 <= 0 and ...)   # provar.py, estágio (e)
```

A fase 1 do estágio do firewall afirma que o intruso **rouba** 200 eventos com
tudo destrancado. É um **controle positivo**, não uma catraca: sem ele, «o
intruso levou 0» com as trancas ligadas não prova nada, porque zero também é o
que se mede quando o cenário não exercita o caminho. A própria bancada já pagou
para aprender isso — a lista negra sobreviveu a uma corrida e a fase *sem
tranca nenhuma* mediu zero roubo, com o número certo e a conclusão errada
(§17, aprendizado 5).

O que separa os dois, e é o que fica: **a guarda afirma o defeito no ponto que
o conserto deveria mudar; o controle positivo afirma o defeito num ponto que o
conserto deliberadamente NÃO muda.** O `a3` media a réplica consertada; a fase 1
mede um servidor com a proteção desligada de propósito. Um conferidor
automático por padrão de texto não separaria os dois — é a mesma razão pela
qual as oito interpolações de erro cru viraram recusa com número: *o que
distingue não está na forma da linha.*

**Onde o buraco ficou:** a varredura cobriu as três bancadas desta frente. As
outras — `acid/`, `concorrencia/`, `durabilidade/`, `guardas/`, `profiler/`,
`telemetria/` — não foram lidas, e a busca continua sendo por leitura, porque a
distinção acima não é casável por padrão. Nomeado no pedido 193.
