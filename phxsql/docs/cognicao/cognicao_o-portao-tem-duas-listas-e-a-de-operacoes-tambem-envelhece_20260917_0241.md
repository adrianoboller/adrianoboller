# O portão tem DUAS listas, e a de operações também envelhece

*Descoberto em 17/09/2026, 02:41 UTC, na revisão SEC adversária da replicação.*

Isto **não** é uma terceira cópia da pétrea *«portão de permissão é UM só — e o
campo que ele lê é o furo»*. É o **alcance** dela: descobri que a pergunta
obrigatória tem dois eixos, e a casa só tinha feito um.

## 1. O que aconteceu

O portão 2a-bis (`crates/phxsql-server/src/servidor.rs:9082-9110`) é o que
`replicacao.replicas_autorizadas` tranca. Quando ele nasceu, quem o escreveu
**fez a pergunta certa** e deixou a resposta no comentário, linhas 9096-9101:

> *«O campo que este portao passou a olhar e o IP DA SESSAO, entao a pergunta
> obrigatoria e quem NAO tem esse campo: job agendado, rotina interna e a
> replicacao chamada de dentro chegam com `ip` vazio»*

A pergunta foi feita pelo eixo **campo do pedido**, e a resposta está correta.
Mas o portão tem uma segunda lista, e essa ninguém interrogou —
`servidor.rs:309`:

```rust
pub(crate) const OPS_DE_REPLICACAO: &[&str] = &["posicao", "replicar", "aplicar"];
```

Fora dela está `cluster_pulso`, que:

- usa **a mesma credencial** (`usuarios.rs:388`: `"cluster_pulso" =>
  Atividade::Replicar`, e `docs/CLUSTER.md:408-410` diz *«o cluster autentica
  como a réplica»*);
- não lê dado nenhum, mas **decide quem manda**: um pulso de época maior
  rebaixa o master (`servidor.rs:3121-3131`) e o rebaixamento é **persistido**
  (`cluster.rs:500-505`).

Ou seja: a lista que existe para dizer *«de ONDE a replicação pode vir»* não
cobre a operação de replicação mais poderosa que existe.

## 2. O que eu concluí primeiro, e estava errado

Concluí que o buraco era **o IP não ser confiável atrás de proxy**, e fui atrás
de `X-Forwarded-For`. Escrevi meia página de achado antes de varrer: **não há
uma única leitura de `forwarded` ou `x-real-ip` em `crates/`**. O IP sempre sai
do par do soquete (`servidor.rs:8328-8331`, `7964-7969`). O portão não é
falsificável por pedido, e o diagnóstico plausível morreu na varredura.

O buraco não era o **campo** que o portão lê — esse está certo e já tinha sido
interrogado. Era a **lista de operações** a que ele se aplica. Procurei o furo
no eixo que a pétrea já nomeava, e por isso quase não o vi: a lei tinha me dado
uma lanterna, e eu apontei para onde ela apontava.

## 3. O que a medição disse

Contado no fonte, não de memória:

| medida | número |
|---|---|
| operações trancadas por `replicas_autorizadas` | **3** (`servidor.rs:309`) |
| operações que a lista **deveria** cobrir pelo critério «usa a credencial de replicação e vem de fora» | **4** — as três mais `cluster_pulso` |
| chamadas de `violacao_leve` no repositório | **9**, e **nenhuma** na recusa de id do `cluster_pulso` (`servidor.rs:3477-3481`) |
| pulsos forjados necessários para deixar o cluster sem master gravável, de forma persistente | **1** |

E o agravante que fecha a conta: `cluster_no_acrescentar` já exige
`administrar`, e o comentário que justifica isso (`usuarios.rs:390-396`) diz
com todas as letras *«com `Replicar` aqui, qualquer replica poderia inflar o
cluster com nos fantasmas e travar toda promocao»*. O raciocínio adversário
**já estava escrito**, aplicado à lista de nós, e não foi aplicado ao pulso —
que consegue o mesmo efeito sem tocar na lista.

## 4. A regra

**Todo portão tem duas listas: o CAMPO que ele lê e as OPERAÇÕES a que se
aplica. Interrogue as duas — pelo campo, procure quem não o tem; pela
operação, procure quem usa a mesma credencial e não está na lista.**

## 5. Como está guardado hoje

**Não está.** O achado é A1 e A11 de
`docs/propostas/revisao-sec-replicacao-2026-09-17.md`, com o cenário de
exploração e o teste adverso escritos, e o conserto é do papel B — o SEC não
conserta.

O que **está** guardado é o eixo do campo: o comentário do 9096-9101 é o
registro dele, e o `sem_regra_de_tabela_nada_muda` é a catraca do
comportamento velho. **O buraco deste arquivo é que não há catraca pelo eixo da
operação** — nenhum conferidor cruza «operações que exigem `Atividade::Replicar`»
com `OPS_DE_REPLICACAO`, e esse cruzamento é escrevível: as duas listas estão no
fonte, uma em `usuarios.rs:383-388` e a outra em `servidor.rs:309`. Quando a
lista vira código, a lei para de depender de alguém lembrar — é a mesma solução
que o `segredos::todo_parametro_com_cara_de_segredo_esta_na_lista` deu para a
lista de nomes do profiler.
