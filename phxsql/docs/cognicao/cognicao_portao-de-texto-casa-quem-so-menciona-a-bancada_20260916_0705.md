# O portão de texto casa quem só MENCIONA a bancada — e duas bancadas educadas se esperam para sempre

Descoberto em 16/09/2026, 07:05 UTC, na corrida do CRUD da colmeia
(`bancada/colmeia/medir-crud.py`), com outra frente medindo na mesma máquina.

## 1. O que aconteceu

O `medir-crud.py` consultava o `bancada/esta-medindo.sh` antes de **cada**
seção cronometrada e esperava enquanto ele acusasse outra medição. O portão
acusou o PID 13755: um `bash -c` da outra frente cujo cmdline continha
`bancada/tomada/chutar-a-tomada.py` — não rodando a bancada, mas num laço de
`sleep 30` esperando o portão ficar livre de «colmeia». Ele esperava por mim;
eu esperava por ele. Dois cronômetros de 20 min, carga 0,3, ninguém medindo.

Às 07:47 o mesmo impasse voltou com **dois processos reais**: o
`chutar-a-tomada.py` em Python (que consulta o portão por dentro e espera até
20 min) e o meu `medir-crud.py`, cada um vendo o outro no portão.

## 2. O que eu concluí primeiro, e estava errado

Primeiro: «é um invólucro parado de outro agente, o portão está com falso
positivo, basta ignorar shells». Estreitei o crivo para só contar PID cujo
executável não é `bash`/`sh` — e isso resolveu o **primeiro** impasse, mas não
o segundo, porque no segundo os dois lados eram `python3` de verdade. O crivo
de texto não era a causa; era o sintoma. A causa era eu **voltar ao portão a
cada seção**, como se fosse recém-chegado a cada vez.

## 3. O que a medição disse

- `bancada/esta-medindo.sh` na máquina, às 07:05: 3 linhas — 13752 e 13755
  (`bash -c … chutar-a-tomada.py`), ambos `S (sleeping)`, filho único `sleep
  30`; a bancada em si não existia como processo.
- `chutar-a-tomada.py` l. 981–998: consulta o portão, espera bancada de tempo
  até 20 min com `sleep 30`.
- A corrida perdeu 8 × 30 s de espera antes de eu ver, e teria perdido a seção
  inteira (SQLite a N = 100.000) em «NÃO MEDIDA» por um limite que não era de
  máquina ocupada.
- Depois do conserto: `esperou_slot_s` no `resultados-crud.json` registra as
  esperas legítimas (120,9 s e 60,5 s por `cargo`/carga da outra frente); as
  24 combinações fecharam sem nenhuma NÃO MEDIDA.

## 4. A regra

**Quem chegou primeiro mede; quem chega depois espera no portão — e quem já
passou pelo portão não volta a ele no meio da própria corrida.** Consultar o
portão só na primeira seção; carga e compiladores, em todas. E um cmdline que
só *menciona* uma bancada não é uma bancada: só conta PID cujo executável não
é um shell.

## 5. Como está guardado hoje

Em `bancada/colmeia/medir-crud.py`: `esperar_slot(consultar_portao=…)` é
verdadeiro só na primeira seção da corrida; `medicao_alheia()` ignora PIDs cujo
`/proc/<pid>/exe` é `bash`/`sh`/`dash`/`zsh`; `--sem-portao` existe para a
continuação de uma corrida que já tinha passado; `--continuar` e
`--rust-pronto` fazem uma corrida interrompida não jogar fora o que mediu.

**Onde o buraco ficou:** o `esta-medindo.sh` da casa continua casando o
invólucro que só menciona a bancada (crivo 2, por caminho do script no
cmdline) — não o mudei, porque ele é provado no item 0b da bateria e é
território da outra frente. E o `chutar-a-tomada.py` volta ao portão por
dentro do processo, do mesmo jeito que eu voltava: duas bancadas com a mesma
educação se esperam até o limite de uma delas. Fica para o integrador decidir
se o protocolo «primeira seção só» entra no portão da casa ou em cada bancada.
