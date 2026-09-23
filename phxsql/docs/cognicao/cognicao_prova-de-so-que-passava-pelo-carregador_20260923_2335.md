# Prova contra o SO que passava por engano: o carregador dinâmico morria antes

## O que aconteceu

Achado C2 da revisão de segurança do phxvpn (23/09/2026): o
`senha::bytes_aleatorios` caía numa mistura previsível quando o `/dev/urandom`
não abria. O conserto passou a falhar fechado, e a prova sobe um processo
filho sem descritor livre, que tem de morrer em pânico sem imprimir bytes.

## O que eu concluí primeiro, e estava errado

Que `ulimit -n 3` (só stdin/out/err) no filho simulava a tabela esgotada. O
teste **passou**, e passaria do mesmo jeito com a mistura antiga de volta: o
filho morria com `libgcc_s.so.1: cannot open shared object file: Error 24`,
ou seja, o carregador dinâmico não conseguia abrir a biblioteca e o
executável nem chegava ao código. «Falhou» era verdade, mas não pelo motivo
que o teste afirmava.

## O que a medição disse

Com a asserção apertada para exigir a frase do pânico
(`sem fonte de entropia`) no stderr do filho, a versão `ulimit -n 3`
**reprovou**. Reescrita para o filho subir com `ulimit -n 64`, esgotar a
própria tabela abrindo `/dev/null` até dar erro e só então sortear, a prova
ficou nos dois sentidos: passa com o conserto e reprova com a mistura de
volta («o filho sem descritor devolveu bytes»).

## A regra

Prova que espera uma FALHA tem de conferir o MOTIVO da falha, não só que ela
aconteceu — senão qualquer outro defeito no caminho faz o teste passar.

## Como está guardado hoje

`senha::tests::sem_descritor_o_sorteio_falha_fechado` confere a frase do
pânico. O alcance não é geral: outros testes que esperam `is_err()` sem olhar
qual erro continuam podendo passar pelo motivo errado, e não há conferidor
para isso.
