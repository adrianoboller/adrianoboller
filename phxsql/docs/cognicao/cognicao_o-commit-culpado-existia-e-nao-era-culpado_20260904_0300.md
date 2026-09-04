# O commit culpado existia, e não era culpado

**04/09/2026, 03:00** — frente C-cluster (papéis F e J), `bancada/cluster/`.

## 1. O que aconteceu

`python3 bancada/cluster/provar.py --so cluster` reprovou em **três**
asserções, duas corridas seguidas. A regra da casa diz que segunda falha é
real, e era: nada de *flake*.

As três, no passo (e) — matam-se `no1` e `no2`, sobra o `no3` sozinho (1 de 3):

```
FALHOU  no3 continua replica            -- veio: master
FALHOU  degradado diz que NAO promove   -- veio: [... 'sem maioria visivel (1 de 3): escrita recusada']
FALHOU  no2 volta master pela epoca persistida -- veio: replica
```

## 2. O que eu concluí primeiro, e estava errado

Não fui eu que concluí — o diagnóstico chegou pronto, e é por isso que ele é
bom de guardar: **eu teria concluído a mesma coisa**, e a cadeia de provas
parecia fechada.

A hipótese: a prova casa a frase literal `"NAO promovo"`, e essa mensagem foi
**reescrita** em `d4f8563` («toda recusa diz de qual sprint se trata»). Seria a
pétrea *«texto se resolve por chave, nunca por comparação da frase»* quebrando
dentro da prova que existe para protegê-la — bonito demais para estar errado.

E havia **evidência real** ao lado, o que é o que torna a armadilha eficaz: o
`resultados.json` versionado do cluster tinha `"REDIRECIONA 127.0.0.1:5310..."`
e a corrida de hoje traz `"[SP000028] REDIRECIONA ..."`. O `d4f8563` **está**
naquele arquivo. Só não está na falha.

Um comando desmontou tudo:

```
$ git log --oneline -S "NAO promovo" -- crates/phxsql-server/src/servidor.rs
adace51 Cluster com eleicao e promocao automatica, honesto sobre o que nao garante
```

**Um commit só, o que criou o cluster.** A frase nunca foi reescrita. E as
outras duas asserções nem comparam frase: comparam `papel`, que já é chave.
Das três falhas, **uma** casava frase, e a frase estava certa.

## 3. O que a medição disse

Rodei a bancada inteira sem tocar em nada: **passou, verde, zero falhas**. Uma
regressão que passa não é regressão — é corrida.

O que decide o passo (e) é a **ordem das mortes**. A eleição conta quem
*pulsou* dentro da janela (`EstadoCluster::vivos`) e o silêncio do master sai
do **mesmo relógio** (`master_visto_ms`): um par que morre **depois** do master
ainda está dentro da janela no instante em que o master é declarado calado.

Prova nos dois sentidos — mesmo binário, mesma configuração, mesmos três nós,
1,5 s entre as mortes (`bancada/cluster/fresta.py`):

| ordem | o nó que sobra | época | o que ele registrou |
|---|---|---|---|
| master primeiro | **master** | 0 → **1** | `PROMOVIDO ... eleito entre 2 vivos de 3 configurados` |
| par primeiro | replica | 0 → 0 | `sem maioria visivel (1 de 3): NAO promovo` |

O número que fecha a conta é o do próprio servidor: **2 vivos de 3**. Ele não
viu 1 de 3 e promoveu assim mesmo; ele viu maioria, e a maioria estava velha.

Reposta a ordem antiga na bancada, a corrida saiu **idêntica** à do relatório
que recebi, retrato `2d2f80604c4d158c` incluído. E a terceira falha é
**consequência da segunda**, medida: o isolado sobe a época, e o master que
volta com a época persistida menor se rebaixa —
`ha epoca 1 no ar e a minha e 0 -- REBAIXANDO a replica`.

O que **não** aconteceu em nenhuma das ordens: escrita aceita, dois masters,
divergência de retrato. O portão é `escrita_liberada`, recalculado a cada
500 ms, **independente do papel**.

## 4. A regra

**Antes de acusar um commit de ter mudado um texto, peça ao `git log -S` que
mostre quem o mudou.** Evidência verdadeira ao lado de uma conclusão falsa é
mais perigosa que evidência nenhuma — e uma asserção que reprova é a hora mais
barata de descobrir isso.

E a irmã, que é do papel F: **teste cuja pré-condição se estabelece por
corrida mede outra coisa metade das vezes** — e quando falha, falha com uma
cara que manda o diagnóstico para o lugar errado.

## 5. Como está guardado hoje

- O passo (e) mata o **par primeiro e o master 2 s depois** — a pré-condição
  «1 de 3» passa a ser verdade *por construção*, e não por sorte
  (`bancada/cluster/provar.py`).
- As asserções da frase viraram **chave**: `papel`, `epoca` (eleição *é* subir
  a época) e `escrita_liberada`. Prova real: reposta a ordem antiga,
  `epoca intacta` falha (`1 -> 2`); com a ordem certa, passa. Duas corridas
  verdes.
- A fresta tem roteiro próprio, `bancada/cluster/fresta.py`, que **afirma** só
  o que vale nas duas ordens e **mede** o resto: guarda que afirmasse o defeito
  viraria catraca contra o próprio conserto.
- Escrita em `docs/CLUSTER.md` §2.4, item 5.

**Onde o buraco ficou:** a fresta **não foi consertada** — o conserto é em
`cluster.rs`/`servidor.rs`, fora do território desta frente, e está em parecer
esperando decisão. Enquanto isso, um nó que esteve sozinho pode voltar
mandando num cluster que era maioria sem ele.
