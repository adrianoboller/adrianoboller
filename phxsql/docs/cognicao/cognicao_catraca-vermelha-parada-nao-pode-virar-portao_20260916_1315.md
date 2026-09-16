# Cognição: catraca vermelha parada com o dono não pode virar portão — e o custo, que eu fui medir, não decidiu nada

- **Assunto:** onde amarrar as catracas dos dois mapas de concorrência (pendência #252, metade (2))
- **Descoberta:** 16/09/2026, 13:15 UTC
- **Arquivos:** `bancada/concorrencia/mapa-da-trava.py`, `bancada/concorrencia/mapa-das-threads.py`,
  `crates/phxsql-server/tests/catraca-do-mapa-das-threads.rs`,
  `docs/dossie/numeros-do-projeto.py`, `docs/dossie/portao-dos-geradores.py`

## 1. O que aconteceu

As catracas dos dois mapas só rodavam no item 0 da bateria, e a bateria ficou
dezoito dias sem rodar — oito deles com a `alcancam-fsync` furada e três
rodadas de portões verdes por cima. A tarefa mandava medir o custo dos dois
medidores e deixar **o número decidir** o lugar: barato vai para o `cargo
test`, caro vai para o `numeros-do-projeto.py`.

## 2. O que eu concluí primeiro, e estava errado

Que o custo decidiria. Medi os dois, achei os dois baratos, e o número **não
decidiu nada**: 3,1 s e 0,9 s somem igualmente dentro de um `cargo test
--workspace` que leva minutos, e pela régua do custo as duas iriam para a
suíte. Só que uma delas está **vermelha por decisão do dono** — e pô-la num
portão deixaria a suíte de todas as frentes vermelha até ele decidir. A saída
mais barata dessa pressão seria subir o teto de 22 para 23, que é exatamente o
que a pétrea proíbe. **O critério que eu fui medir era o certo para a metade
verde e cego para a vermelha.**

O segundo erro veio junto e morreu na mesma hora: pensei em pregar a vermelha
no número de hoje («23, parada, #252») para a suíte ficar verde. Isso é um
**teto sombra de 23** com outro nome — catraca frouxa com álibi. O próprio
`docs/CATRACAS.md` já tinha escrito a recusa para outro caso: um teto que
aceitasse «só três painéis velhos» seria a catraca frouxa que a casa proíbe.

## 3. O que a medição disse

- `mapa-das-threads.py --catraca`: **1,122 / 0,885 / 0,811 s** — verde.
- `mapa-da-trava.py --catraca`: **3,113 / 3,056 / 3,187 s** — vermelha (23, teto 22).
- Dentro da suíte, o teste novo custa **0,68 / 0,67 / 0,69 s**.
- E o motivo de a vermelha não virar portão do gerador também é medido, e não
  de gosto: `portao-dos-geradores.py:251` reprova o gerador que sai com código
  diferente de zero, com a mensagem «gerador que não emite quando a fonte
  sumiu é VERMELHO». Fazer o `numeros-do-projeto.py` sair não-zero por causa
  da catraca poria o portão vermelho **todo dia, dizendo a coisa errada** — e
  sinal falso é o que esta casa pune.

## 4. A regra

**Onde uma catraca mora decide-se pelo custo E pelo estado: vermelha parada
com o dono entra como RELATO com data, nunca como portão — e jamais pregada no
número de hoje, que é teto sombra.**

## 5. Como está guardado hoje

A verde virou teste da suíte (`catraca-do-mapa-das-threads.rs`, que roda o
medidor em vez de recontar). A vermelha entrou no `numeros-do-projeto.py`,
impressa antes dos minutos de `cargo test` e de novo na última linha, com a
data. As duas continuam no item 0 da bateria e no aviso de hora em hora.

**Onde o buraco ficou:** a `alcancam-fsync` continua **sem portão nenhum** —
três relatores dizem que ela está vermelha, e nenhum deles impede um quarto
`fsync` de entrar sob a trava. Só a decisão do dono na #252 (1) fecha isso, e
enquanto ela não vem o buraco é este, nomeado.
