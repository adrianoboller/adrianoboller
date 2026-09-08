# Cognição: o pulso e a replicação do cluster já andavam no MESMO transporte

Frente H1 (cluster cifrado), 08/09/2026. Descoberta ao medir onde o soquete do
cluster mora, antes de cifrar.

## 1. O que aconteceu

O item aberto (`docs/CIFRA-DO-FIO.md` §10) dizia: *«O cluster fala em claro. A
replicação do cluster passa pelo mesmo `rodada_da_replica`, mas o **pulso** da
eleição vai por outro caminho (`cluster.rs`).»* A frente H1 vinha com um portão
explícito: **se o pulso e a replicação usarem transportes incompatíveis, PARE e
relate uma proposta medida — não entregue metade do cluster cifrado.**

Medido no fonte, os dois estão no **mesmo** transporte: a
`crate::replica::Cliente`, que já sabia apertar a mão (`cifrar`). O pulso
(`Servidor::pulsar`, `servidor.rs`) conecta por `conectar_com_prazo` e manda
`cluster_pulso`; a replicação (`laco_da_replica_do_cluster` → `rodada_da_replica`
→ `replica::ligar`) monta uma `Origem` e **já cifrava** quando `origem.cifra`. O
servidor do outro lado é a própria porta de dados, que já atende `cifrar`. Cifrar
o cluster INTEIRO foi **1 chamada `cifrar` no pulso + 1 campo `cifra` na `Origem`
da replicação + config** — sem mudança de formato em disco.

## 2. O que eu concluí primeiro, e estava errado

Lendo a frase «o pulso vai por **outro caminho** (`cluster.rs`)», concluí que o
pulso viajava por um transporte separado do da replicação — talvez um soquete
cru dentro de `cluster.rs` — e que este era, com boa chance, o caso de
«transportes incompatíveis» em que o portão da frente mandava PARAR e propor
uma mudança de formato.

Errado. O `cluster.rs` **não tem soquete nenhum**: o próprio cabeçalho dele diz
*«Aqui mora o que se decide SEM soquete»* — papel, época, mapa dos nós e a função
pura `vencedor`. «Outro caminho» era **outra decisão** (o pulso não é
replicação de eventos), não **outro transporte**. O soquete do pulso mora em
`servidor.rs`, na mesma `replica::Cliente` da replicação. A palavra «caminho»
num documento é palpite sobre o transporte até alguém abrir o código.

## 3. O que a medição disse

- `cluster.rs`: **zero** chamadas de soquete — só estado e `vencedor`.
- `pulsar` e `laco_da_replica_do_cluster`: **os dois** em `servidor.rs`, **os
  dois** constroem `replica::Cliente`; a replicação já passava por
  `replica::ligar`, que faz `if origem.cifra { c.cifrar(origem.pino_do_fio()?)? }`.
- Custo real do INTEIRO: a linha `cliente.cifrar(no.pino_do_fio()?)?` no pulso, o
  campo `cifra: c.cifra` na `Origem` do cluster, o campo `cluster.cifra` e o
  `nos[].chave_do_fio` no `config.json`. **Zero** mudança de PSCH/`.reg`/`.ndx`;
  os campos novos do `config.json` são opcionais e retrocompatíveis (padrão
  desligado = cluster em claro, como antes).

E um segundo achado, do tipo que só aparece no encontro dos caminhos: o
`gravar_nos_do_cluster` reescreve `cluster.nos` **inteiro**. Acrescentar o pino
sem tocar nesse gravador faria todo `cluster_no_acrescentar`/`_remover` a quente
**apagar o pino de TODOS os nós** de uma vez — a cifra continuaria ligada,
rebaixada a escuta passiva em silêncio. Só se vê rastreando o gravador que
reescreve a lista, não lendo a struct.

## 4. A regra

**«Outro caminho» num documento é palpite sobre o transporte até alguém ler o
código: meça onde o soquete mora antes de declarar transportes incompatíveis e
parar.** E o corolário: **ao acrescentar um campo a um item que se reescreve
inteiro (a lista de nós), conserte o gravador que o reescreve — senão o campo
novo apaga o dos outros.**

## 5. Como está guardado hoje

- Código: `crates/phxsql-server/src/servidor.rs` (`pulsar`, `origem_do_master`,
  `subir_cluster`, `op_cluster_no_acrescentar`) e
  `crates/phxsql-server/src/config.rs` (`NoCluster.chave_do_fio` +
  `pino_do_fio`, `Cluster.cifra`, `Cluster::validar`, `gravar_nos_do_cluster`).
- Prova real por soquete: `crates/phxsql-server/tests/cluster-cifrado.rs` —
  passa em 0,13 s com a cifra; cai em 15 s (timeout do `cluster_estado`) com o
  defeito reposto.
- Guardas: `bancada/guardas/catalogo.py`, uma por metade —
  `pulso-do-cluster-em-claro` e `replicacao-do-cluster-em-claro` —, as duas
  PROVADAS pelo `provar-guardas.py`.
- Documentação: `docs/CIFRA-DO-FIO.md` §12, `docs/CLUSTER.md` §2.9,
  `MANUAL.txt`, `docs/PENDENCIAS.md`.
