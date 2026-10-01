# Listar `/proc/self/task` pula a thread viva quando a vizinha de antes morre: pedido 437

**Estado:** PENDENTE

Evidência candidata, para quem for promover:
`crates/phxsql-server/src/telemetria.rs::a_thread_viva_se_acha_pelo_tid_mesmo_com_a_vizinha_morrendo`
e a guarda `tarefa-pela-listagem-do-proc`, PROVADA em 01/10/2026 em 114 s.

## O que aconteceu

`telemetria::testes::as_threads_do_so_se_medem_e_nunca_sao_menos_que_as_registradas`
caiu uma vez em `cargo test --workspace`. No binário sozinho deu 0 em 45
corridas. O teste procurava a thread `presa-do-teste` listando
`/proc/self/task` e lendo o `comm` de cada entrada.

## O que eu concluí primeiro, e estava errado

A primeira montagem de medida usou vizinhas que nascem e morrem sem parar
(três laços de `spawn`/`join`) e 2.000 threads antigas morrendo aos poucos.
Deu **0 perdas em 20.000 leituras**, e quase fechei com «a listagem não
pula». O erro era de lugar. Quem morre precisa estar **imediatamente antes**
da presa na lista do núcleo, e as vizinhas recriadas nascem **depois** dela,
porque a lista segue a ordem de criação.

## O que a medição disse

Na montagem certa, a vizinha nasce logo antes da presa e morre durante as
leituras (`scratchpad`, binário `-O`, núcleo 6.18):

| Leitura | Leituras sem a presa |
|---|---|
| Listagem | **145** e **126** em 200.000, duas corridas |
| `/proc/self/task/<tid>/comm` | **0** em 200.000 |

Em binário de depuração foram 14 em 60.000. O mecanismo é o do
`proc_task_readdir`: quando a tarefa do cursor morre, o `next_tid` devolve
nada e o `getdents` para. O seguinte retoma pelo **índice**, que encolheu, e a
entrada logo depois da morta fica de fora. No `libtest`, a vizinha que morre
é a thread de outro teste, e por isso a queda só aparecia na suíte cheia.

## A regra

**Para saber se uma thread existe, leia pelo tid dela, nunca listando o
diretório.** A thread manda o próprio tid (`read_link("/proc/thread-self")`).
O mesmo vale para esperar uma tarefa sumir: o teste espera pelo caminho dela.

## Como está guardado hoje

`tarefa_chamada(tid, …)` e `tarefa_existe(tid)` são os ajudantes do teste.
`a_thread_viva_se_acha_pelo_tid_mesmo_com_a_vizinha_morrendo` roda a montagem
com 3.000 voltas e 40 leituras por volta. Com a listagem reposta, a guarda
cai; o número esperado de perdas é ~40, então passar por sorte é da ordem de
e^-40. **Ainda há um buraco:** fora deste teste, ninguém no código de produção
lista `/proc/self/task` (`threads_do_so` lê o `Threads:` do `status`), então o
alcance acaba aqui. Quem precisar de uma lista de tarefas em produção herda o
pulo.
