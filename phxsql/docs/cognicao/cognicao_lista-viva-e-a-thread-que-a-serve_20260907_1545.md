# A lista viva não basta: falta a thread que a serve — pedido 217

**07/09/2026, 15h45. Frente F2 (cluster).**

## 1. O que aconteceu

A `bancada/cluster/escalonar.py` (frente F3) mediu que escalonar o cluster a
quente **não funcionava**: um nó novo, com a lista completa na própria
configuração, ficava isolado a janela inteira. O pulso dele era recusado na
hora pelos antigos —

```
[SP000025] acesso negado: o no "no4" nao esta na lista de nos deste cluster
```

— e os antigos **nunca tentavam falar com ele**, porque ele não estava na lista
deles. O caminho que funcionava era editar `cluster.nos` dos três antigos e
reiniciá-los: **0,367 s de master fora do ar**.

## 2. O que eu concluí primeiro, e estava errado

**«O defeito é o crivo do pulso: `estado.config.no(&id)` lê o retrato do
arranque. Torne a lista viva e acabou.»**

A primeira metade estava certa e a segunda estava incompleta — de um jeito que
**os testes unitários não pegam**. Com a lista viva e a operação
`cluster_no_acrescentar`, o teste em processo passava: o pulso do `no3` era
recusado antes e aceito depois, e eu tinha a prova real nos dois sentidos.

Só que num cluster de verdade **ninguém pulsa o nó novo**. O `for no in
c.outros()` do arranque subia uma thread por nó do `config.json` e **nunca mais
olhava**: o nó acrescentado entrava na lista, era aceito se falasse, e não
tinha com quem falar — os antigos não tinham thread para ele. O teste unitário
não via porque nele o pulso chega como uma chamada de função, e não por um
soquete que alguém precisa abrir.

Foi a bancada contra o motor vivo que mostrou, e é exatamente a pétrea do
soquete por outro caminho: *o que depende do sistema operacional se prova
contra o sistema operacional*. Aqui o que dependia dele não era a queda de
conexão — era a **existência** dela.

E um terceiro engano, na mesma família: pus a conferência «este nó ainda está
na lista?» no topo do `laco_do_pulso` e me dei por satisfeito. O `pulsar` tem
um **laço de dentro** que dura enquanto a conexão durar, e ele podia não cair
nunca: um nó removido continuaria sendo pulsado. É o *«conserto entra no
caminho que o motivou, e o caminho IRMÃO fica»* — e irmão aqui não é o de nome
parecido, é o outro laço da mesma função.

## 3. O que a medição disse

`bancada/cluster/escalonar.py`, cinco nós em `127.0.0.1`, 07/09/2026 — as duas
metades na **mesma corrida**, para o número comparar máquina igual:

| | caminho antigo (editar e reiniciar) | a quente (pedido 217) |
|---|---|---|
| master fora do ar | **0,369 s** | **0 s** |
| escalonamento inteiro | 1,11 s (três reinícios) | **0,207 s** |
| a ordem em si | — | **5 ms** |
| retratos SHA-256 no fim | batem (4) | batem (5) |

O `0 s` é **medido, não deduzido**: uma batida de escrita de 10 em 10 ms roda
durante a ordem inteira e conta as recusas (**0 de 42**) e o **maior buraco
entre dois `ok`** (**11,5 ms** — o próprio intervalo da batida). Sem o buraco,
«não recusou» poderia ser «parou sem dar erro», que é pior.

E um defeito da própria bancada apareceu na primeira corrida: o passo (4)
esperava só o nó **novo** alcançar o master e tirava o retrato dos quatro — um
deles ainda estava em 320 contra 370. O retrato dizia «os quatro não batem»
onde a verdade era «um ainda não chegou». Medidor que espera o alvo errado
publica divergência que não existe.

## 4. A regra

> **Lista viva sem thread que a sirva é lista decorativa. Quando um recurso
> deixar de ser lido só no arranque, procure a THREAD que o arranque subiu por
> ele — e dê a ela um dono que reveja a lista.**

E o corolário do irmão: **quando puser uma conferência de saída num laço,
conte quantos laços a função tem.** O `laco_do_pulso` tem dois — o de fora, que
reconecta, e o de dentro, que dura o que a conexão durar.

## 5. Como está guardado hoje

- A lista viva e o registro de threads: `EstadoCluster::lista/total/no/outros/
  acrescentar/remover` e `marcar_pulso`/`desmarcar_pulso` —
  `crates/phxsql-server/src/cluster.rs`.
- O supervisor: `laco_do_supervisor_do_pulso` (`servidor.rs`), meio segundo de
  intervalo, uma thread de pulso por nó da lista viva. A thread morre sozinha
  quando o nó sai **ou muda de endereço** — e aí o supervisor sobe outra no
  endereço novo.
- As operações: `cluster_no_acrescentar` e `cluster_no_remover`, com portão
  próprio de `administrar`, gravação no `config.json` (desfeita se a validação
  recusar) e propagação com veredito **por nó**.
- As provas: quatro testes em processo (`no_acrescentado_a_quente_passa_a_ser_
  aceito_no_pulso`, `acrescentar_o_mesmo_no_duas_vezes_e_idempotente`,
  `remover_nao_alcanca_este_no_nem_o_master`, `operador_sem_administrar_nao_
  mexe_na_lista_de_nos`), a bancada de cinco nós, e a tela exercitada em
  `testes-web/capturas-cluster.mjs`.

**Onde o buraco ficou:** a propagação autentica com `cluster.usuario`, então
esse usuário precisa poder `administrar`. Sem isso a ordem local vale e a
propagação volta recusada **nomeando o nó** — é honesto, mas é um requisito
novo de configuração que só se descobre na primeira tentativa. Está escrito no
`docs/CLUSTER.md` §2.5 e no `MANUAL.txt`; não há guarda que avise no arranque.
